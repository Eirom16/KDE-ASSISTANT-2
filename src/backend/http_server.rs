//! HTTP Server local - IPC entre el backend Rust y la UI QML
//!
//! Expone una API REST en localhost que la UI QML consume via XMLHttpRequest
//! + Server-Sent Events (SSE) para streaming.
//!
//! Endpoints:
//! - GET  /api/health           -> {"status":"ok"}
//! - POST /api/chat             -> SSE stream del agente (token/tool/done/error)
//! - GET  /api/sessions         -> lista de sesiones
//! - POST /api/session          -> crea sesion (body: {"title": "..."})
//!
//! SSE events (data = JSON):
//! - event: token,      data: {"content":"..."}
//! - event: tool_call,  data: {"name":"...", "arguments":{...}}
//! - event: tool_result,data: {"tool_call_id":"...", "content":"..."}
//! - event: done,       data: {"full_content":"..."}
//! - event: error,      data: {"message":"..."}

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::backend::ai_service::AiService;
use crate::backend::session_manager::SessionManager;
use crate::backend::tool_executor::ToolExecutor;
use crate::backend::tool_registry;
use crate::models::{Config, Message, StreamEvent};
use tokio::sync::RwLock;

/// Estado compartido del servidor HTTP.
#[derive(Clone)]
pub struct AppState {
    pub ai: Arc<AiService>,
    pub tools: Arc<ToolExecutor>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<Mutex<SessionManager>>,
    pub speech: Arc<crate::backend::speech_service::SpeechService>,
    pub voice: Arc<crate::backend::voice_pipeline::VoicePipeline>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: i64,
}

/// Construye el router del servidor HTTP.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/chat", post(chat))
        .route("/api/chat/complete", post(chat_complete))
        .route("/api/sessions", get(list_sessions))
        .route("/api/session", post(create_session).delete(delete_session))
        .route("/api/messages", get(list_messages))
        .route("/api/config", get(get_config).post(update_config))
        .route("/api/ai-models", post(list_ai_models))
        .route("/api/voice/last", get(voice_last))
        .route("/api/voice/log", post(voice_log))
        .route("/api/speak", post(speak_text))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct VoiceLogRequest {
    pub transcript: String,
    #[serde(default)]
    pub response: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VoiceLogResponse {
    pub session_id: String,
}

/// Ultimo intercambio por voz (para que la UI lo muestre en el chat).
async fn voice_last(State(state): State<AppState>) -> impl IntoResponse {
    match state.voice.last_exchange() {
        Some(ex) => Json(serde_json::json!({
            "transcript": ex.transcript,
            "response": ex.response,
            "timestamp_ms": ex.timestamp_ms,
        }))
        .into_response(),
        None => Json(serde_json::json!({
            "transcript": "",
            "response": "",
            "timestamp_ms": 0,
        }))
        .into_response(),
    }
}

/// Persiste un intercambio por voz en una sesion (la crea si no hay).
/// Retorna el session_id para que la UI lo seleccione.
async fn voice_log(
    State(state): State<AppState>,
    Json(body): Json<VoiceLogRequest>,
) -> impl IntoResponse {
    if body.transcript.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "transcript vacio" })),
        );
    }
    let sessions = state.sessions.lock().unwrap();
    // Reusar la sesion indicada si existe; si no, crear "Conversacion por voz".
    let sid = match body.session_id.as_deref() {
        Some(id) if sessions.get_session(id).ok().flatten().is_some() => id.to_string(),
        _ => match sessions.create_session("Conversacion por voz") {
            Ok(s) => s.id,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        },
    };
    let _ = sessions.add_message(&sid, &Message::user(body.transcript.clone()));
    if !body.response.trim().is_empty() {
        let _ = sessions.add_message(&sid, &Message::assistant(body.response.clone()));
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "session_id": sid })),
    )
}

#[derive(Debug, Deserialize)]
pub struct SpeakRequest {
    pub text: String,
}

/// Sintetiza y reproduce un texto (fire-and-forget para el boton reproducir).
async fn speak_text(
    State(state): State<AppState>,
    Json(body): Json<SpeakRequest>,
) -> impl IntoResponse {
    if body.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "texto vacio" })),
        );
    }
    let speech = state.speech.clone();
    let text = body.text.clone();
    tokio::spawn(async move {
        if let Err(e) = speech.speak(&text).await {
            log::warn!("POST /api/speak fallo: {e}");
        }
    });
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "status": "ok" })),
    )
}

/// Devuelve la configuracion actual (JSON completo).
async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    Json(cfg).into_response()
}
/// Actualiza la configuracion y la persiste en config.json.
async fn update_config(
    State(state): State<AppState>,
    Json(new_cfg): Json<Config>,
) -> impl IntoResponse {
    // Guardar en memoria y persistir
    let mut cfg = state.config.write().await;
    *cfg = new_cfg.clone();
    match cfg.save().await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AiModelsRequest {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiModelsResponse {
    pub models: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Modelos que claramente no son de chat (audio, embeddings, imagen, moderacion).
fn is_non_chat_model(id: &str) -> bool {
    let lower = id.to_lowercase();
    [
        "whisper",
        "tts",
        "embed",
        "moderation",
        "guard",
        "dall-e",
        "dall·e",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

/// Lista los modelos que ofrece la API del proveedor (`GET {base}/models`).
/// Acepta overrides opcionales para probar credenciales sin guardar.
/// Siempre responde 200 con `{models, error?}` para que la UI lo parsee facil.
async fn list_ai_models(
    State(state): State<AppState>,
    Json(body): Json<AiModelsRequest>,
) -> impl IntoResponse {
    use crate::models::AiConfig;

    let cfg = state.config.read().await.clone();
    let provider_id: String = match body.provider.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => AiConfig::normalize_provider_id(s).to_string(),
        _ => cfg.ai.provider_id().to_string(),
    };
    let provider_name = AiConfig::provider_name(&provider_id);

    let base_url = match body.base_url.as_deref().map(str::trim) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => cfg.ai.base_url.clone(),
    };
    let api_key = match body.api_key.as_deref().map(str::trim) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => cfg.ai.effective_api_key(),
    };
    if api_key.trim().is_empty() {
        let env_var = AiConfig::provider_env_var(&provider_id);
        return Json(AiModelsResponse {
            models: vec![],
            error: Some(format!("Falta API key (campo o variable {env_var})")),
        });
    }

    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("KDE-Assistant/2.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Json(AiModelsResponse {
                models: vec![],
                error: Some(format!("No se pudo crear cliente HTTP: {e}")),
            })
        }
    };

    let response = match client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Json(AiModelsResponse {
                models: vec![],
                error: Some(format!("No se pudo contactar {provider_name}: {e}")),
            })
        }
    };
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Json(AiModelsResponse {
            models: vec![],
            error: Some(format!("{provider_name} {status}: {snippet}")),
        });
    }

    let json: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return Json(AiModelsResponse {
                models: vec![],
                error: Some(format!("Respuesta inesperada de {provider_name}: {e}")),
            })
        }
    };
    let mut models: Vec<String> = json
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(str::to_string))
                .filter(|id| !is_non_chat_model(id))
                .collect()
        })
        .unwrap_or_default();
    models.sort();
    models.dedup();
    Json(AiModelsResponse {
        models,
        error: None,
    })
}

#[derive(Debug, Deserialize)]
pub struct DeleteSessionQuery {
    pub session_id: String,
}

/// Elimina una sesion y sus mensajes.
async fn delete_session(
    State(state): State<AppState>,
    Query(q): Query<DeleteSessionQuery>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap();
    match sessions.delete_session(&q.session_id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct MessageInfo {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

/// Lista los mensajes de una sesion.
async fn list_messages(
    State(state): State<AppState>,
    Query(q): Query<MessagesQuery>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap();
    match sessions.get_messages(&q.session_id) {
        Ok(msgs) => {
            let list: Vec<MessageInfo> = msgs
                .into_iter()
                .filter_map(|m| match m {
                    Message::User { content } => Some(MessageInfo {
                        role: "user".to_string(),
                        content,
                        image_url: None,
                    }),
                    Message::Assistant { content, .. } => Some(MessageInfo {
                        role: "assistant".to_string(),
                        content,
                        image_url: None,
                    }),
                    // Solo los tool con imagen interesan a la UI (para reinyectarlas)
                    Message::Tool {
                        content,
                        image_url: Some(u),
                        ..
                    } => Some(MessageInfo {
                        role: "tool".to_string(),
                        content,
                        image_url: Some(u),
                    }),
                    _ => None,
                })
                .collect();
            Json(list).into_response()
        }
        Err(e) => {
            log::warn!("Error listando mensajes: {e}");
            Json(Vec::<MessageInfo>::new()).into_response()
        }
    }
}

/// Respuesta de `/api/chat/complete` (no streaming).
#[derive(Debug, Serialize)]
pub struct ChatCompleteResponse {
    pub transcript: String,
    pub response: String,
    pub session_id: Option<String>,
}

/// Chat sin streaming: espera la respuesta completa y la devuelve como JSON.
/// Usado por la UI QML (XMLHttpRequest no hace streaming SSE de forma fiable).
async fn chat_complete(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    log::info!(
        "Chat completo: '{}' (session={:?})",
        truncate(&req.message, 60),
        req.session_id
    );

    // Construir historial
    let messages = build_messages(&state, &req).await;

    // Ejecutar el agente (sin streaming al cliente)
    let tools = tool_registry::all_tools();
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(256);

    let ai = state.ai.clone();
    let tools_exec = state.tools.clone();
    let msgs = messages.clone();
    let agent_task = tokio::spawn(async move { ai.run_agent(msgs, tools, tools_exec, tx).await });

    // Consumir eventos (para no bloquear el canal) y capturar la respuesta
    let mut full_response = String::new();
    while let Some(ev) = rx.recv().await {
        if let StreamEvent::Token { content } = &ev {
            full_response.push_str(content);
        }
    }

    let response = match agent_task.await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            log::warn!("Agente fallo: {e}");
            e.to_string()
        }
        Err(e) => {
            log::warn!("Task agente fallo: {e}");
            format!("Error interno: {e}")
        }
    };

    // Si el agente retorno vacio (por ejemplo tool calls sin texto final),
    // usamos lo capturado en el stream.
    let response = if response.is_empty() {
        full_response
    } else {
        response
    };

    // Persistir usuario + respuesta
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.lock().unwrap();
        let _ = sessions.add_message(sid, &Message::user(req.message.clone()));
        maybe_auto_title(&sessions, sid, &req.message);
        let _ = sessions.add_message(sid, &Message::assistant(response.clone()));
    }

    // Regla voz/texto: el chat escrito solo habla si auto_speak esta activo
    // (las respuestas por voz siempre hablan, ver voice_pipeline).
    let auto_speak = state.config.read().await.speech.auto_speak;
    if auto_speak && !response.trim().is_empty() {
        let speech = state.speech.clone();
        let resp = response.clone();
        tokio::spawn(async move {
            if let Err(e) = speech.speak(&resp).await {
                log::warn!("TTS de respuesta escrita fallo: {e}");
            }
        });
    }

    let body = ChatCompleteResponse {
        transcript: req.message.clone(),
        response,
        session_id: req.session_id.clone(),
    };
    (StatusCode::OK, Json(body))
}

/// Arranca el servidor HTTP en `127.0.0.1:port`.
pub async fn serve(state: AppState, port: u16) -> Result<()> {
    let app = router(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("Servidor HTTP local en http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "app": "kde-assistant" }))
}

/// Lista las sesiones guardadas.
async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap();
    let raw = match sessions.list_sessions() {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Error listando sesiones: {e}");
            return Json(Vec::<SessionInfo>::new()).into_response();
        }
    };
    let list: Vec<SessionInfo> = raw
        .into_iter()
        .map(|s| {
            let message_count = sessions.count_messages(&s.id).unwrap_or_default();
            SessionInfo {
                id: s.id,
                title: s.title,
                updated_at: s.updated_at,
                message_count,
            }
        })
        .collect();
    Json(list).into_response()
}

/// Crea una nueva sesion.
async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap();
    let title = body
        .title
        .unwrap_or_else(|| "Nueva conversación".to_string());
    match sessions.create_session(title) {
        Ok(s) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": s.id, "title": s.title })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// Maneja el chat: envia el mensaje al agente y streamea la respuesta (SSE).
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    log::info!(
        "Chat request: '{}' (session={:?})",
        truncate(&req.message, 60),
        req.session_id
    );

    // Construir historial de mensajes
    let messages = build_messages(&state, &req).await;

    // Canal de stream events
    let (tx, rx) = mpsc::channel::<StreamEvent>(256);

    // Ejecutar el agente en un task
    let ai = state.ai.clone();
    let tools_exec = state.tools.clone();
    let tools = tool_registry::all_tools();
    let msgs = messages.clone();
    let agent_task = tokio::spawn(async move { ai.run_agent(msgs, tools, tools_exec, tx).await });

    // Persistir el mensaje del usuario
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.lock().unwrap();
        let _ = sessions.add_message(sid, &Message::user(req.message.clone()));
        maybe_auto_title(&sessions, sid, &req.message);
    }

    // Convertir el receiver a stream SSE
    let sse_stream = stream_from_receiver(rx);

    // Task que espera el resultado y persiste la respuesta
    let state2 = state.clone();
    let sid2 = req.session_id.clone();
    let sse_stream = sse_stream.map(move |ev| {
        let (event_name, data) = sse_event(ev);
        Ok::<Event, Infallible>(Event::default().event(event_name).data(data))
    });

    // Persistir la respuesta cuando termina el agente
    tokio::spawn(async move {
        if let Ok(Ok(response)) = agent_task.await {
            if let Some(sid) = &sid2 {
                let sessions = state2.sessions.lock().unwrap();
                let _ = sessions.add_message(sid, &Message::assistant(response));
            }
        }
    });

    Sse::new(sse_stream).keep_alive(KeepAlive::default())
}

/// Si la sesion aun tiene el titulo por defecto, lo reemplaza por el
/// inicio del primer mensaje del usuario (max ~8 palabras / 42 chars).
fn maybe_auto_title(
    sessions: &std::sync::MutexGuard<SessionManager>,
    session_id: &str,
    user_text: &str,
) {
    let is_default = sessions
        .get_session(session_id)
        .ok()
        .flatten()
        .map(|s| {
            let t = s.title.trim().to_string();
            t == "Nueva conversación" || t == "New conversation" || t.is_empty()
        })
        .unwrap_or(false);
    if !is_default {
        return;
    }
    let title: String = user_text
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    let title: String = title.chars().take(42).collect();
    if !title.trim().is_empty() {
        let _ = sessions.update_session_title(session_id, title.trim());
    }
}

async fn build_messages(state: &AppState, req: &ChatRequest) -> Vec<Message> {
    let system_prompt = state.config.read().await.ai.system_prompt.clone();
    let mut messages = Vec::new();
    messages.push(Message::system(system_prompt));

    // Cargar historial si hay session_id
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.lock().unwrap();
        if let Ok(history) = sessions.get_messages(sid) {
            // Inyectar historial (sin el system prompt duplicado)
            messages.extend(history);
        }
    }

    messages.push(Message::user(req.message.clone()));
    messages
}

/// Convierte un StreamEvent en (event_name, data_json).
fn sse_event(ev: StreamEvent) -> (&'static str, String) {
    match ev {
        StreamEvent::Token { content } => (
            "token",
            serde_json::json!({ "content": content }).to_string(),
        ),
        StreamEvent::ToolCall { tool } => (
            "tool_call",
            serde_json::to_string(&tool).unwrap_or_default(),
        ),
        StreamEvent::ToolResult {
            tool_call_id,
            content,
            image_url,
        } => (
            "tool_result",
            serde_json::json!({ "tool_call_id": tool_call_id, "content": content, "image_url": image_url }).to_string(),
        ),
        StreamEvent::Done { full_content } => (
            "done",
            serde_json::json!({ "full_content": full_content }).to_string(),
        ),
        StreamEvent::Error { message } => (
            "error",
            serde_json::json!({ "message": message }).to_string(),
        ),
    }
}

/// Convierte un mpsc::Receiver en un Stream.
fn stream_from_receiver(rx: mpsc::Receiver<StreamEvent>) -> impl Stream<Item = StreamEvent> {
    futures_util::stream::unfold(
        rx,
        |mut rx| async move { rx.recv().await.map(|ev| (ev, rx)) },
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push_str("...");
        t
    }
}
