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

use anyhow::{Context, Result};
use axum::{
    extract::{Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::Stream;
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::backend::ai_service::AiService;
use crate::backend::conversation_turn::{ClientLease, Turn, TurnRegistry, TurnState};
use crate::backend::session_manager::SessionManager;
use crate::backend::tool_executor::ToolExecutor;
use crate::backend::tool_registry;
use crate::models::{Config, Message, StreamEvent};
use tokio::sync::RwLock;

/// Estado compartido del servidor HTTP.
#[derive(Clone)]
pub struct AppState {
    pub desktop: Arc<crate::desktop::Desktop>,
    pub native_voice: Arc<crate::backend::voice_coordinator::VoiceCoordinator>,
    pub http_port: u16,
    pub voice_enabled: Arc<std::sync::atomic::AtomicBool>,
    pub ai: Arc<AiService>,
    pub tools: Arc<ToolExecutor>,
    pub approvals: Arc<crate::backend::approvals::ApprovalManager>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<Mutex<SessionManager>>,
    pub speech: Arc<crate::backend::speech_service::SpeechService>,
    pub voice: Arc<crate::backend::voice_pipeline::VoicePipeline>,
    /// Estados visuales del chat escrito para el avatar. No se mezclan con
    /// el stream de voz porque "thinking" no significa que el micrófono esté
    /// activo.
    pub agent_events: tokio::sync::broadcast::Sender<String>,
    /// Token bearer local (F0-3). Se exige en todo `/api/*` salvo `/health`.
    pub local_token: String,
    /// Handles de agentes en curso por sesión (F0-7 cancel).
    /// Clave: session_id o "default" si no hay.
    pub chat_tasks: Arc<TurnRegistry>,
    /// Dictado al input (botón mic del chat): grabación sin pipeline de voz.
    pub dictation_active: Arc<std::sync::atomic::AtomicBool>,
    /// El sidecar de voz (Python/Pipecat) es dueño del mic. Los endpoints de
    /// dictado/voz/comando se reenvian al puerto 8766 cuando está activo.
    pub sidecar_active: Arc<std::sync::atomic::AtomicBool>,
}

// Outbound-only UI channel. There is deliberately no action POST endpoint.
async fn desktop_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    let rx = state.desktop.subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        let action = match rx.recv().await {
            Ok(action) => action,
            Err(_) => return None,
        };
        let event = match action {
            crate::desktop::UiAction::Show => Event::default().event("desktop").data("show"),
            crate::desktop::UiAction::Toggle => Event::default().event("desktop").data("toggle"),
            crate::desktop::UiAction::Status => Event::default().event("desktop").data("status"),
            crate::desktop::UiAction::PortalToggle(token) => Event::default()
                .event("desktop_activation")
                .data(serde_json::json!({"action":"toggle", "token":token}).to_string()),
        };
        Some((Ok(event), rx))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
async fn desktop_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"shortcuts": state.desktop.shortcut_status(), "window": "qt_quick", "blur": "NOT_SUPPORTED"}),
    )
}

/// Middleware F0-3: auth + host + origin para `/api/*`.
async fn auth_layer(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if !crate::backend::auth::path_requires_auth(&path) {
        return next.run(req).await;
    }
    if !crate::backend::auth::valid_host_for_port(req.headers(), state.http_port) {
        log::debug!("auth: host rechazado para {}", path);
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "host no permitido" })),
        )
            .into_response();
    }
    if !crate::backend::auth::valid_origin_for_port(req.headers(), state.http_port) {
        log::debug!("auth: origin rechazado para {}", path);
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "origin no permitido" })),
        )
            .into_response();
    }
    if !crate::backend::auth::valid_server_token(&state.local_token)
        || !crate::backend::auth::valid_bearer(req.headers(), &state.local_token)
    {
        log::debug!("auth: bearer inválido para {}", path);
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized: falta Bearer local" })),
        )
            .into_response();
    }
    let optional_voice = path.starts_with("/api/dictate/")
        || (path.starts_with("/api/voice/")
            && !matches!(
                path.as_str(),
                "/api/voice/stream"
                    | "/api/voice/last"
                    | "/api/voice/session"
                    | "/api/voice/status"
                    | "/api/voice/start"
                    | "/api/voice/stop"
                    | "/api/voice/cancel"
                    | "/api/voice/background"
            ));
    if optional_voice
        && !state
            .voice_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error": "Voz diferida/desactivada; el chat escrito sigue disponible. No se descargan dependencias automáticamente."
        }))).into_response();
    }
    next.run(req).await
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
        .route("/api/desktop/events", get(desktop_events))
        .route("/api/desktop/status", get(desktop_status))
        .route("/api/chat", post(chat))
        .route("/api/chat/complete", post(chat_complete))
        .route("/api/chat/cancel", post(chat_cancel))
        .route("/api/chat/turn", get(current_turn))
        .route("/api/chat/regenerate", post(chat_regenerate))
        .route("/api/sessions", get(list_sessions))
        .route(
            "/api/session",
            post(create_session)
                .delete(delete_session)
                .patch(rename_session),
        )
        .route("/api/messages", get(list_messages))
        .route("/api/config", get(get_config).post(update_config))
        .route("/api/ai-models", post(list_ai_models))
        .route("/api/ai-test", post(test_ai_connection))
        .route("/api/voice/last", get(voice_last))
        .route("/api/voice/stream", get(voice_stream))
        .route("/api/agent/stream", get(agent_stream))
        .route("/api/agent/dashboard", get(agent_dashboard))
        .route("/api/voice/session", post(set_voice_session))
        .route("/api/voice/status", get(native_voice_status))
        .route("/api/voice/start", post(native_voice_start))
        .route("/api/voice/stop", post(native_voice_stop))
        .route("/api/voice/cancel", post(native_voice_cancel))
        .route("/api/voice/background", post(native_background))
        .route("/api/speak", post(speak_text))
        .route("/api/speak/stop", post(native_voice_cancel))
        .route("/api/audio/devices", get(audio_devices))
        .route("/api/audio/device", post(set_audio_device))
        .route("/api/tools", get(list_tools))
        .route("/api/tools/schemas", get(list_tool_schemas))
        .route("/api/tools/execute", post(execute_tool_manual))
        .route("/api/tools/approve", post(approve_tool))
        .route("/api/tools/audit", get(list_tool_audit))
        .route("/api/tools/operations", get(list_operations))
        .route("/api/storage", get(storage_diagnostics))
        .route(
            "/api/memory/facts",
            get(list_facts).post(upsert_fact).delete(delete_fact),
        )
        .layer(middleware::from_fn_with_state(state.clone(), auth_layer))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct ChatCancelRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    pub turn_id: String,
}

#[derive(Deserialize)]
struct CurrentTurnQuery {
    session_id: Option<String>,
}
async fn current_turn(
    State(state): State<AppState>,
    Query(query): Query<CurrentTurnQuery>,
) -> Response {
    match state.chat_tasks.current(&query.session_id) {
        Some(turn) => Json(serde_json::json!({"turn_id":turn.id,"session_id":turn.session_id,"state":turn.state()})).into_response(),
        None => (StatusCode::NOT_FOUND,Json(serde_json::json!({"code":"turn_not_found"}))).into_response(),
    }
}

async fn chat_cancel(
    State(state): State<AppState>,
    Json(body): Json<ChatCancelRequest>,
) -> Response {
    let Some(turn) = state.chat_tasks.find(&body.session_id, &body.turn_id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"code":"turn_not_found","error":"El turno no pertenece a esta sesión o ya fue recogido"}))).into_response();
    };
    let requested = turn.cancel();
    state
        .native_voice
        .cancel_turn(turn.session_id.as_deref(), &turn.id)
        .await;
    let effective = state.chat_tasks.cancel_and_wait(&turn).await;
    (
        if effective {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(serde_json::json!({
            "turn_id":turn.id,"session_id":turn.session_id,"cancellation_requested":requested,
            "cancellation_effective":turn.state() == TurnState::Cancelled,"state":turn.state()
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct ChatRegenerateRequest {
    pub session_id: String,
}
async fn chat_regenerate(
    State(state): State<AppState>,
    Json(body): Json<ChatRegenerateRequest>,
) -> Response {
    start_stream(
        state,
        ChatRequest {
            session_id: Some(body.session_id),
            message: String::new(),
        },
        true,
    )
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

/// Stream push de estado/nivel de voz (F3-1, SSE).
/// Eventos: `state` {state}, `level` {level}.
/// Reemplaza el polling de `voice.state`/`voice.level` por archivos.
async fn voice_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    use futures_util::stream::{self, StreamExt};

    let rx = state.voice.subscribe();
    let init_state = state.voice.current_state();
    let init = stream::once(async move {
        let data = serde_json::json!({ "state": init_state }).to_string();
        Ok::<Event, Infallible>(Event::default().event("state").data(data))
    });
    let live = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(sig) => {
                    let (name, data) = match sig {
                        crate::backend::voice_pipeline::VoiceSignal::State { state } => {
                            ("state", serde_json::json!({ "state": state }).to_string())
                        }
                        crate::backend::voice_pipeline::VoiceSignal::Level { level } => {
                            ("level", serde_json::json!({ "level": level }).to_string())
                        }
                        crate::backend::voice_pipeline::VoiceSignal::Transcript { content } => (
                            "transcript",
                            serde_json::json!({ "content": content }).to_string(),
                        ),
                        crate::backend::voice_pipeline::VoiceSignal::Token { content } => (
                            "token",
                            serde_json::json!({ "content": content }).to_string(),
                        ),
                        crate::backend::voice_pipeline::VoiceSignal::Exchange {
                            transcript,
                            response,
                            timestamp_ms,
                        } => (
                            "exchange",
                            serde_json::json!({
                                "transcript": transcript,
                                "response": response,
                                "timestamp_ms": timestamp_ms,
                            })
                            .to_string(),
                        ),
                    };
                    break Some((
                        Ok::<Event, Infallible>(Event::default().event(name).data(data)),
                        rx,
                    ));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break None,
            }
        }
    });
    Sse::new(init.chain(live)).keep_alive(KeepAlive::default())
}

/// Stream de actividad del chat escrito para el personaje. Mantenerlo
/// separado del stream de voz evita que una respuesta tecleada encienda el
/// orbe o el micrófono en la UI principal.
async fn agent_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    use futures_util::stream;

    let rx = state.ai.activity.events.subscribe();
    let legacy = state.agent_events.subscribe();
    let live = stream::unfold((rx, legacy), |(mut rx, mut legacy)| async move {
        loop {
            let next = tokio::select! {
                ev = rx.recv() => ev.map(|data| ("activity", data.to_string())),
                ev = legacy.recv() => ev.map(|state| ("state", serde_json::json!({"state":state}).to_string())),
            };
            match next {
                Ok((name, data)) => {
                    break Some((
                        Ok::<Event, Infallible>(Event::default().event(name).data(data)),
                        (rx, legacy),
                    ));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break None,
            }
        }
    });
    Sse::new(live).keep_alive(KeepAlive::default())
}

async fn agent_dashboard(State(state): State<AppState>) -> impl IntoResponse {
    let mut snapshot = state.ai.activity.snapshot();
    let sessions = state.sessions.clone();
    match tokio::task::spawn_blocking(move || {
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .tool_usage()
    })
    .await
    {
        Ok(Ok(usage)) => snapshot["tool_usage"] = usage,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"No se pudo consultar la actividad"})),
            )
        }
    }
    (StatusCode::OK, Json(snapshot))
}

// F4 native operations never accept audio, transcripts or tool results from clients.
async fn native_voice_status(State(state): State<AppState>) -> impl IntoResponse {
    let mut snapshot = serde_json::to_value(state.native_voice.snapshot()).unwrap_or_default();
    snapshot["fast"] = serde_json::to_value(state.native_voice.fast_snapshot()).unwrap_or_default();
    snapshot["background"] =
        serde_json::to_value(state.native_voice.background_snapshot()).unwrap_or_default();
    Json(snapshot)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeVoiceStart {
    session_id: String,
    scope: String,
    #[serde(default)]
    wake: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackgroundControl {
    action: String,
    generation: Option<String>,
}
async fn native_background(
    State(state): State<AppState>,
    Json(body): Json<BackgroundControl>,
) -> Response {
    if body.action == "ack" {
        let Some(id) = body.generation.filter(|g| g.len() == 36) else {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"Identidad inválida"})),
            )
                .into_response();
        };
        state.native_voice.acknowledge_background(&id);
        return Json(state.native_voice.background_snapshot()).into_response();
    }
    let cfg = state.config.read().await.speech.clone();
    match state.native_voice.set_background(&body.action, cfg).await {
        Ok(status) => Json(status).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}
async fn native_voice_start(
    State(state): State<AppState>,
    Json(body): Json<NativeVoiceStart>,
) -> Response {
    let sessions = state.sessions.clone();
    let sid = body.session_id.clone();
    let exists = tokio::task::spawn_blocking(move || {
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_session(&sid)
            .ok()
            .flatten()
            .is_some()
    })
    .await
    .unwrap_or(false);
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"Conversación no disponible"})),
        )
            .into_response();
    }
    let cfg = state.config.read().await.speech.clone();
    let result = if body.wake {
        state
            .native_voice
            .start_wake(body.session_id, body.scope, cfg)
            .await
    } else {
        state
            .native_voice
            .start(body.session_id, body.scope, cfg)
            .await
    };
    match result {
        Ok(snapshot) => (StatusCode::ACCEPTED, Json(snapshot)).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}
async fn native_voice_stop(
    State(state): State<AppState>,
    Json(id): Json<crate::backend::voice_coordinator::Identity>,
) -> Response {
    match state.native_voice.stop(&id) {
        Ok(()) => Json(state.native_voice.snapshot()).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}
async fn native_voice_cancel(
    State(state): State<AppState>,
    Json(id): Json<crate::backend::voice_coordinator::Identity>,
) -> Response {
    match state.native_voice.cancel(&id).await {
        Ok(()) => Json(state.native_voice.snapshot()).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct VoiceSessionRequest {
    #[serde(default)]
    session_id: Option<String>,
}

/// La conversación hablada usa la misma sesión que el chat visible. QML lo
/// actualiza al seleccionar/crear una sesión, antes del próximo turno de voz.
async fn set_voice_session(
    State(state): State<AppState>,
    Json(body): Json<VoiceSessionRequest>,
) -> impl IntoResponse {
    let valid = body.session_id.filter(|id| {
        state
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_session(id)
            .ok()
            .flatten()
            .is_some()
    });
    let scope = state.native_voice.select_session(valid.clone()).await;
    Json(serde_json::json!({ "session_id": valid, "scope": scope }))
}

#[derive(Debug, Deserialize)]
pub struct FactUpsertRequest {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct FactDeleteQuery {
    pub key: String,
}

/// Lista los facts del usuario (F5, memoria local).
async fn list_facts(State(state): State<AppState>) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.list_facts() {
        Ok(facts) => Json(facts).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Crea o actualiza un fact (F5).
async fn upsert_fact(
    State(state): State<AppState>,
    Json(body): Json<FactUpsertRequest>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.upsert_fact(&body.key, &body.value) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// Borra un fact (F5).
async fn delete_fact(
    State(state): State<AppState>,
    Query(q): Query<FactDeleteQuery>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.delete_fact(&q.key) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakRequest {
    pub session_id: String,
    pub scope: String,
    pub text: String,
}

/// Manual reading is scoped to the selected conversation; it never starts a turn.
async fn speak_text(State(state): State<AppState>, Json(body): Json<SpeakRequest>) -> Response {
    let cfg = state.config.read().await.speech.clone();
    match state
        .native_voice
        .speak(body.session_id, None, Some(body.scope), body.text, cfg)
        .await
    {
        Ok(snapshot) => (StatusCode::ACCEPTED, Json(snapshot)).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}

/// POST simple al sidecar de voz (127.0.0.1:8766), devolviendo su JSON.
#[cfg(test)]
async fn sidecar_post(path: &str) -> anyhow::Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:8766{path}"))
        .json(&serde_json::json!({}))
        .send()
        .await?;
    Ok(resp.json().await?)
}

/// Dictado al input del chat: empieza a grabar (solo STT, sin LLM ni TTS).
/// Cuando el sidecar Python posee el microfono (option por defecto), este
/// handler solo reenvia la peticion; el pipeline local queda como respaldo.
/// Rechaza si hay una captura activa (dictado o pipeline de voz).
#[cfg(test)]
async fn dictate_start(State(state): State<AppState>) -> impl IntoResponse {
    use std::sync::atomic::Ordering;
    if state.sidecar_active.load(Ordering::SeqCst) {
        return match sidecar_post("/dictate/start").await {
            Ok(v) => (StatusCode::OK, Json(v)),
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("sidecar: {e}") })),
            ),
        };
    }
    use crate::backend::voice_state::VoiceState;
    let voice_busy = state.voice.is_recording()
        || state.voice.is_speaking()
        || matches!(
            state.voice.state(),
            VoiceState::Listening | VoiceState::Thinking | VoiceState::Responding
        );
    if voice_busy || state.dictation_active.swap(true, Ordering::SeqCst) {
        state.dictation_active.store(false, Ordering::SeqCst);
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "ya hay una captura de voz en curso" })),
        );
    }
    let sr = state.voice.sample_rate();
    state.voice.start_recording(if sr > 0 { sr } else { 48000 });
    state
        .voice
        .emit_state(crate::backend::voice_state::VoiceState::Listening);
    log::info!("Dictado iniciado (solo STT)");
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "recording" })),
    )
}

/// Dictado al input del chat: para la grabación y transcribe con Whisper.
/// Retorna `{ transcript }` (puede estar vacío si no se escuchó nada).
#[cfg(test)]
async fn dictate_stop(State(state): State<AppState>) -> impl IntoResponse {
    use std::sync::atomic::Ordering;
    if state.sidecar_active.load(Ordering::SeqCst) {
        return match sidecar_post("/dictate/stop").await {
            Ok(v) => (StatusCode::OK, Json(v)),
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("sidecar: {e}") })),
            ),
        };
    }
    struct DictationGuard(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for DictationGuard {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    if !state.dictation_active.load(Ordering::SeqCst) {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "no hay dictado en curso" })),
        );
    }
    let _guard = DictationGuard(state.dictation_active.clone());
    state
        .voice
        .emit_state(crate::backend::voice_state::VoiceState::Thinking);
    let audio = state.voice.stop_recording();
    let resp = if audio.is_empty() {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "transcript": "" })),
        )
    } else {
        log::info!("Dictado: transcribiendo {} samples", audio.len());
        let language = {
            let cfg = state.config.read().await;
            if cfg.speech.stt_language == "auto" {
                None
            } else {
                Some(cfg.speech.stt_language.clone())
            }
        };
        match state.speech.transcribe(&audio, language.as_deref()).await {
            Ok(t) => {
                log::info!("Dictado STT: '{t}'");
                (StatusCode::OK, Json(serde_json::json!({ "transcript": t })))
            }
            Err(e) => {
                log::warn!("Dictado STT fallo: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
            }
        }
    };
    state
        .voice
        .emit_state(crate::backend::voice_state::VoiceState::Idle);
    resp
}

/// Catálogo de herramientas con nivel de permiso y flag (F4-1).
async fn list_tools(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    let enabled = tool_registry::filtered_tools(&cfg);
    let enabled_names: std::collections::HashSet<&str> =
        enabled.iter().map(|t| t.function.name.as_str()).collect();
    let list: Vec<serde_json::Value> = tool_registry::all_tools()
        .iter()
        .map(|t| {
            let name = t.function.name.as_str();
            serde_json::json!({
                "name": name,
                "description": t.function.description,
                "permission": tool_registry::permission(name).as_str(),
                "enabled": enabled_names.contains(name),
            })
        })
        .collect();
    Json(list).into_response()
}

/// Catálogo completo de herramientas CON schemas JSON (para el LLM del
/// sidecar Python, voz). Solo las habilitadas en config.
async fn list_tool_schemas(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    let tools = tool_registry::filtered_tools(&cfg);
    Json(serde_json::to_value(tools).unwrap_or_default()).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ApproveToolRequest {
    pub tool_call_id: String,
    pub consent_id: String,
    pub turn_id: String,
    pub session_id: Option<String>,
    pub approved: bool,
}

/// Resuelve una petición de confirmación pendiente (F4-1).
#[derive(Debug, Deserialize)]
pub struct ExecuteToolRequest {
    pub name: String,
    /// Objeto JSON o string JSON (el QML lo manda como string).
    #[serde(default)]
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// POST /api/tools/execute — ejecución manual desde la UI (p. ej. adjuntar
/// imagen desde el InputBar). El click explícito del usuario cuenta como
/// confirmación: se permiten 🟢/🟡; 🔴 exige el flujo de aprobación del LLM.
/// Persiste el mensaje tool en la sesión (con stub assistant cuando hay
/// imagen, para que el historial la muestre adjunta al asistente).
async fn execute_tool_manual(
    State(state): State<AppState>,
    Json(body): Json<ExecuteToolRequest>,
) -> impl IntoResponse {
    let name = body.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "nombre de herramienta vacío" })),
        );
    }
    if !tool_registry::all_tools()
        .iter()
        .any(|t| t.function.name == name)
    {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("herramienta desconocida: {name}") })),
        );
    }
    let session_id = body
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if tool_registry::permission(name) == tool_registry::Permission::Red {
        let denied = crate::models::ToolCall {
            id: format!("manual-{}", uuid::Uuid::new_v4()),
            name: name.to_string(),
            arguments: Default::default(),
        };
        if let Err(error) = state.tools.record_denial(&denied, session_id.clone()) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"ok":false,"error":error.to_string()})),
            );
        }
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "esta herramienta exige el flujo de aprobación del agente (nivel rojo)"
            })),
        );
    }

    let arguments: std::collections::HashMap<String, serde_json::Value> = match &body.arguments {
        serde_json::Value::Object(m) => m.clone().into_iter().collect(),
        serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
        _ => Default::default(),
    };
    let tool_call = crate::models::ToolCall {
        id: format!("manual-{}", uuid::Uuid::new_v4()),
        name: name.to_string(),
        arguments,
    };

    match state
        .tools
        .execute(
            &tool_call,
            &crate::backend::tool_executor::ToolCtx::auto(session_id.clone()),
        )
        .await
    {
        Ok(result) => {
            if let Some(sid) = session_id {
                let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
                if result.image_url.is_some() {
                    let _ = sessions
                        .add_message(&sid, &Message::assistant("Imagen adjuntada al chat."));
                }
                let _ = sessions.add_message(
                    &sid,
                    &Message::tool_with_image(
                        result.tool_call_id.clone(),
                        result.content.clone(),
                        result.image_url.clone(),
                    ),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "success": result.success,
                    "content": result.content,
                    "image_url": result.image_url,
                })),
            )
        }
        Err(e) => (
            if e.to_string().starts_with("consent_required")
                || e.to_string().starts_with("tool_disabled")
                || e.to_string().starts_with("tool_not_catalogued")
            {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

async fn approve_tool(
    State(state): State<AppState>,
    Json(body): Json<ApproveToolRequest>,
) -> impl IntoResponse {
    if body.tool_call_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "tool_call_id vacío" })),
        );
    }
    let Some(turn) = state.chat_tasks.find(&body.session_id, &body.turn_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"turn_not_found"})),
        );
    };
    if turn.state() != TurnState::WaitingForApproval {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":"turn_not_waiting_for_approval"})),
        );
    }
    let decision = state
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .answer_permission(
            &body.consent_id,
            &body.tool_call_id,
            &body.turn_id,
            body.session_id.as_deref(),
            body.approved,
        );
    if decision.is_ok() && state.approvals.resolve(&body.consent_id, body.approved) {
        (
            StatusCode::OK,
            Json(
                serde_json::json!({ "status": if body.approved { "approved" } else { "denied" } }),
            ),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "petición expirada o inexistente" })),
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct ToolAuditQuery {
    #[serde(default = "default_audit_limit")]
    pub limit: i64,
}

fn default_audit_limit() -> i64 {
    50
}

/// Metadata for recoverable operations; protected by the existing IPC middleware.
#[derive(Deserialize)]
struct OperationQuery {
    session_id: Option<String>,
    #[serde(default)]
    recovery_only: bool,
    before: Option<i64>,
    #[serde(default = "default_audit_limit")]
    limit: i64,
}
async fn list_operations(
    State(state): State<AppState>,
    Query(query): Query<OperationQuery>,
) -> Response {
    match state
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .operation_page(
            query.session_id.as_deref(),
            query.limit,
            query.recovery_only,
            query.before,
        ) {
        Ok(records) => Json(records).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":error.to_string()})),
        )
            .into_response(),
    }
}

async fn list_tool_audit(
    State(state): State<AppState>,
    Query(q): Query<ToolAuditQuery>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.list_tool_audit(q.limit) {
        Ok(mut rows) => {
            // Redaction at the API boundary does not rewrite historical SQLite evidence.
            for row in &mut rows {
                row.args = "[redacted]".into();
            }
            Json(rows).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Dispositivos de entrada + selección actual (F3-3).
async fn audio_devices(State(state): State<AppState>) -> impl IntoResponse {
    let devices = match crate::backend::voice_process::devices().await {
        Ok(devices) => devices,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"No se pudo consultar PipeWire"})),
            )
                .into_response()
        }
    };
    let current = state.config.read().await.speech.mic_device.clone();
    Json(serde_json::json!({ "devices": devices, "current": current })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct SetAudioDeviceRequest {
    #[serde(default)]
    pub name: String,
}

/// Guarda el micrófono preferido (F3-3). Se aplica al reiniciar la captura.
async fn set_audio_device(
    State(state): State<AppState>,
    Json(body): Json<SetAudioDeviceRequest>,
) -> impl IntoResponse {
    let name = body.name.trim().to_string();
    if !name.is_empty() {
        let devices = crate::backend::voice_process::devices()
            .await
            .unwrap_or_default();
        if !devices.iter().any(|d| d == &name) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "dispositivo no encontrado" })),
            );
        }
    }
    let mut cfg = state.config.write().await;
    let changed = cfg.speech.mic_device != name;
    let mut candidate = cfg.clone();
    candidate.speech.mic_device = name;
    match candidate.save().await {
        Ok(()) => {
            *cfg = candidate;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "ok", "restart_required": changed })),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({ "error": "No se pudo guardar el micrófono preferido; los cambios no se confirmaron." }),
            ),
        ),
    }
}

// An opaque process-local revision detects changes even to redacted secrets.
// JSON Value orders map keys; the private nonce prevents a public key-hash oracle.
fn config_revision(cfg: &Config) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    static NONCE: once_cell::sync::Lazy<String> =
        once_cell::sync::Lazy::new(|| uuid::Uuid::new_v4().to_string());
    let canonical = serde_json::to_vec(&serde_json::to_value(cfg)?)?;
    let mut digest = Sha256::new();
    digest.update(NONCE.as_bytes());
    digest.update(canonical);
    Ok(hex::encode(digest.finalize()))
}

/// Configuración sin secretos: la UI recibe presencia y estado, nunca claves guardadas.
async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    let (Ok(mut public), Ok(revision)) = (serde_json::to_value(&cfg), config_revision(&cfg)) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"No se pudo leer la configuración"})),
        )
            .into_response();
    };
    public["config_revision"] = serde_json::json!(revision);
    public["ai"]["api_key"] = serde_json::json!("");
    public["ai"]["provider_keys"] = serde_json::json!({});
    public["ai"]["has_unassigned_legacy_key"] =
        serde_json::json!(!cfg.ai.api_key.is_empty() && !cfg.ai.provider_keys.is_empty());
    public["ai"]["has_api_key"] = serde_json::json!(!cfg.ai.effective_api_key().trim().is_empty());
    let mut states = serde_json::Map::new();
    for id in [
        "openrouter",
        "groq",
        "openai",
        "deepseek",
        "mistral",
        "xai",
        "together",
        "fireworks",
        "cerebras",
        "sambanova",
        "lmstudio",
        "ollama",
        "custom",
    ] {
        let mut ai = cfg.ai.clone();
        if ai.provider_id() != id {
            ai.api_key.clear();
        }
        ai.provider = id.into();
        let source = if ai.provider_keys.get(id).is_some_and(|k| !k.is_empty())
            || (!ai.api_key.is_empty() && ai.provider_keys.is_empty())
        {
            "stored"
        } else if !ai.effective_api_key().is_empty() {
            "environment"
        } else {
            "none"
        };
        states.insert(id.into(), serde_json::json!(source));
    }
    public["ai"]["credential_states"] = states.into();
    public["ai"]["configured_provider_keys"] = serde_json::json!(cfg
        .ai
        .provider_keys
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, _)| k)
        .collect::<Vec<_>>());
    public["capabilities"] = serde_json::json!({
        "text": "ready", "voice": if state.voice_enabled.load(std::sync::atomic::Ordering::Relaxed) { "active" } else { "deferred" },
        "whisper_model_present": crate::backend::speech_service::whisper_model_path_for(&cfg.speech.stt_model).is_ok_and(|path| path.is_file()),
        "stt_engine": cfg.speech.stt_engine,
        "stt_model_present": crate::backend::voice_stt::Selection::from_config(&cfg.speech).is_ok_and(|selected| selected.paths_present()),
        "piper_binary_present": crate::backend::voice_tts::Selection::from_config(&cfg.speech).is_ok_and(|selected| selected.paths_present()),
        "piper_runtime": "explicit_private_python_314_piper_180",
        "piper_initialized": matches!(state.native_voice.fast_snapshot().state,
            crate::backend::warm_piper::State::Ready | crate::backend::warm_piper::State::Synthesizing),
        "piper_fast_supported": cfg!(all(target_os="linux",target_arch="x86_64")),
        "piper_lifecycle": if cfg.speech.piper_fast {"optional_managed_worker"} else {"disposable_worker_no_backend_model"},
        "microphone": "not_initialized_by_text_startup",
        "onnx_runtime": "deferred",
        "wakeword": "openwakeword_background_opt_in",
        "native_dictation": true,
        "wake_word": "optional_off_by_default",
        "remote_audio": "disabled",
        "downloads_on_startup": false,
        "detail": "Voz opcional diferida; no se carga Piper, ONNX ni micrófono al iniciar texto"
    });
    Json(public).into_response()
}

#[derive(Deserialize)]
struct ConfigUpdate {
    #[serde(flatten)]
    config: Config,
    #[serde(default)]
    delete_provider_keys: Vec<String>,
    #[serde(default)]
    delete_legacy_key: bool,
    #[serde(default)]
    config_revision: Option<String>,
}

async fn update_config(
    State(state): State<AppState>,
    Json(body): Json<ConfigUpdate>,
) -> impl IntoResponse {
    let mut cfg = state.config.write().await;
    let Some(expected) = body.config_revision else {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            Json(
                serde_json::json!({"error":"Vuelve a cargar la configuración antes de guardar; falta su revisión."}),
            ),
        );
    };
    let Ok(actual) = config_revision(&cfg) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"No se pudo comprobar la configuración actual."})),
        );
    };
    if expected != actual {
        return (
            StatusCode::CONFLICT,
            Json(
                serde_json::json!({"error":"La configuración cambió desde otra acción. No se guardó nada; vuelve a cargarla y revisa tus cambios."}),
            ),
        );
    }
    let mut new_cfg = body.config;
    let mut previous = cfg.ai.clone();
    previous.migrate_profiles();
    let legacy = if body.delete_legacy_key {
        String::new()
    } else {
        previous.api_key.clone()
    };
    // A provider-only switch cannot borrow the previous profile's flat mirrors.
    if new_cfg.ai.provider_id() != previous.provider_id() {
        let id = new_cfg.ai.provider_id().to_owned();
        if !new_cfg.ai.provider_base_urls.contains_key(&id)
            && new_cfg.ai.base_url == previous.base_url
        {
            new_cfg.ai.base_url = crate::models::AiConfig::provider_preset_url(&id).into();
        }
        if !new_cfg.ai.provider_models.contains_key(&id) && new_cfg.ai.model == previous.model {
            new_cfg.ai.model.clear();
        }
    }
    let mut keys = previous.provider_keys;
    for (provider, key) in &new_cfg.ai.provider_keys {
        if !key.trim().is_empty() {
            keys.insert(provider.clone(), key.trim().into());
        }
    }
    if !new_cfg.ai.api_key.trim().is_empty() {
        keys.insert(
            new_cfg.ai.provider_id().into(),
            new_cfg.ai.api_key.trim().into(),
        );
    }
    for provider in body.delete_provider_keys {
        if crate::models::AiConfig::normalize_provider_id(&provider) != provider {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"Proveedor de credencial inválido"})),
            );
        }
        keys.insert(provider, String::new());
    }
    new_cfg.ai.provider_keys = keys;
    new_cfg.ai.api_key = legacy;
    new_cfg.ai.migrate_profiles();
    // Keep legacy mirrors aligned with the selected scoped profile.
    new_cfg.ai.base_url = new_cfg.ai.effective_base_url();
    new_cfg.ai.model = new_cfg.ai.effective_model();
    if let Err(e) = new_cfg.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        );
    }
    match new_cfg.save().await {
        Ok(()) => {
            *cfg = new_cfg;
            state.native_voice.configure(&cfg.speech).await;
            (StatusCode::OK, Json(serde_json::json!({"status":"ok"})))
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({"error":"No se pudo escribir la configuración. Revisa permisos y espacio; los cambios no se confirmaron."}),
            ),
        ),
    }
}

#[derive(Deserialize)]
pub struct AiModelsRequest {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub remove_key: bool,
    pub approved_endpoint: Option<String>,
}
fn selected_provider(cfg: &Config, body: AiModelsRequest) -> crate::models::AiConfig {
    let mut ai = cfg.ai.clone();
    ai.migrate_profiles();
    if let Some(provider) = body.provider {
        if ai.provider_id() != provider {
            ai.base_url = crate::models::AiConfig::provider_preset_url(&provider).into();
            ai.model.clear();
        }
        ai.provider = provider;
    }
    let id = ai.provider_id().to_owned();
    if let Some(url) = body.base_url {
        ai.provider_base_urls.insert(id.clone(), url);
    }
    if let Some(model) = body.model {
        ai.provider_models.insert(id.clone(), model);
    }
    if let Some(approved) = body.approved_endpoint {
        ai.provider_endpoint_approvals.insert(id.clone(), approved);
    }
    if body.remove_key {
        ai.provider_keys.insert(id, String::new());
    } else if let Some(key) = body.api_key.filter(|k| !k.trim().is_empty()) {
        ai.provider_keys.insert(id, key.trim().into());
    }
    ai
}
async fn list_ai_models(
    State(state): State<AppState>,
    Json(body): Json<AiModelsRequest>,
) -> axum::response::Response {
    let ai = selected_provider(&*state.config.read().await, body);
    match crate::backend::provider_connection::models(&ai, std::time::Duration::from_secs(20)).await
    {
        Ok(models) => Json(serde_json::json!({"models": models})).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(e)).into_response(),
    }
}
async fn test_ai_connection(
    State(state): State<AppState>,
    Json(body): Json<AiModelsRequest>,
) -> axum::response::Response {
    let ai = selected_provider(&*state.config.read().await, body);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        crate::backend::provider_connection::probe(&ai, std::time::Duration::from_secs(12)),
    )
    .await;
    match result {
        Ok(Ok(outcome)) => Json(serde_json::json!({"status":"ok", "limited":outcome.limited, "message":if outcome.limited { "Conexión y streaming recibidos; la salida alcanzó el límite de prueba. Respuesta completa y herramientas no comprobadas." } else { "Conexión y streaming verificados para este modelo. Herramientas no comprobadas." }})).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_GATEWAY, Json(e)).into_response(),
        Err(_) => (StatusCode::BAD_GATEWAY, Json(crate::backend::provider_connection::error("timeout", "La prueba agotó el tiempo de espera."))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteSessionQuery {
    pub session_id: String,
    #[serde(default)]
    pub erase_uncertain_evidence: bool,
}

/// Renombra una sesion (F1-2). Body: {"id": "...", "title": "..."}.
#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    pub id: String,
    pub title: String,
}

async fn rename_session(
    State(state): State<AppState>,
    Json(body): Json<RenameSessionRequest>,
) -> impl IntoResponse {
    let title = body.title.trim().to_string();
    if title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "título vacío" })),
        );
    }
    let title: String = title.chars().take(80).collect();
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.update_session_title(&body.id, title.trim()) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// Elimina una sesion y sus mensajes.
async fn delete_session(
    State(state): State<AppState>,
    Query(q): Query<DeleteSessionQuery>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        state.chat_tasks.while_session_idle(&q.session_id, || {
            state
                .sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .delete_session_confirmed(&q.session_id, q.erase_uncertain_evidence)
        })
    })
    .await;
    match result {
        Ok(Ok(())) => (StatusCode::OK, Json(serde_json::json!({"status":"ok"}))),
        Ok(Err(e)) => {
            let error = e.to_string();
            let status =
                if error.contains("session_busy") || error.contains("confirmation_required") {
                    StatusCode::CONFLICT
                } else if error == "session_not_found" {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
            (status, Json(serde_json::json!({"error":error})))
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"storage_worker_failed"})),
        ),
    }
}
async fn storage_diagnostics(State(state): State<AppState>) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || {
        state
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .storage_diagnostics()
    })
    .await
    {
        Ok(Ok(data)) => (StatusCode::OK, Json(data)),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"storage_diagnostics_unavailable"})),
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
    /// Timestamp RFC3339 del mensaje (para hora real en la UI).
    #[serde(default)]
    pub timestamp: String,
}

/// Lista los mensajes de una sesion.
async fn list_messages(
    State(state): State<AppState>,
    Query(q): Query<MessagesQuery>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.get_messages_with_timestamps(&q.session_id) {
        Ok(msgs) => {
            let list: Vec<MessageInfo> = msgs
                .into_iter()
                .filter_map(|(m, ts)| match m {
                    Message::User { content } => Some(MessageInfo {
                        role: "user".to_string(),
                        content,
                        image_url: None,
                        timestamp: ts,
                    }),
                    Message::Assistant { content, .. } => Some(MessageInfo {
                        role: "assistant".to_string(),
                        content,
                        image_url: None,
                        timestamp: ts,
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
                        timestamp: ts,
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

fn agent_policy(cfg: &Config) -> crate::backend::approvals::ApprovalPolicy {
    crate::backend::approvals::ApprovalPolicy::from_config(cfg.tools.confirm_sensitive)
}

#[derive(Debug)]
struct TurnEvent {
    name: &'static str,
    data: serde_json::Value,
}

fn emit(turn: &Turn, name: &'static str, data: serde_json::Value) -> TurnEvent {
    TurnEvent {
        name,
        data: turn.envelope(data),
    }
}

/// All resources are owned by this one supervisor (retained JoinHandle in registry).
/// The client only holds a lease: disconnect requests cancellation, never aborts
/// a supervisor which may be draining an already dispatched blocking tool.
fn start_turn(
    state: AppState,
    mut req: ChatRequest,
    regenerate: bool,
) -> Result<(Arc<Turn>, mpsc::Receiver<TurnEvent>)> {
    if let Some(sid) = &req.session_id {
        anyhow::ensure!(
            state
                .sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_session(sid)?
                .is_some(),
            "session_not_found"
        );
    }
    let turn = state.chat_tasks.reserve(req.session_id.clone())?;
    let (out_tx, out_rx) = mpsc::channel::<TurnEvent>(256);
    let owned = turn.clone();
    let voice_scope = state.native_voice.scope_for(turn.session_id.as_deref());
    let registry = state.chat_tasks.clone();
    let task = tokio::spawn(async move {
        // Reserve delivery space for exactly one terminal event before data.
        let terminal_permit = out_tx.clone().reserve_owned().await.ok();
        let work = async {
            owned.transition(TurnState::Running)?;
            out_tx
                .send(emit(&owned, "turn", serde_json::json!({})))
                .await
                .map_err(|_| anyhow::anyhow!("client_disconnected"))?;
            {
                let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
                if regenerate {
                    anyhow::ensure!(sessions.regeneration_is_safe(req.session_id.as_deref().unwrap_or_default())?,"regeneration_requires_new_message: inspecciona efectos anteriores y envía un mensaje nuevo");
                    let cap = state
                        .config
                        .try_read()
                        .context("config unavailable")?
                        .ai
                        .max_context_bytes;
                    req.message =
                        sessions.retry_input(req.session_id.as_deref().unwrap_or_default(), cap)?;
                }
            }
            let messages = crate::backend::conversation_turn::cancellable(
                Some(&owned),
                build_messages(&state, &req, regenerate),
            )
            .await?;

            {
                let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
                let user = Message::user(&req.message);
                if regenerate {
                    sessions.begin_retry_turn(
                        &owned.id,
                        owned.session_id.as_deref().unwrap_or_default(),
                    )?;
                } else {
                    sessions.begin_turn(&owned.id, owned.session_id.as_deref(), Some(&user))?;
                }
                if !regenerate {
                    if let Some(sid) = &owned.session_id {
                        maybe_auto_title(&sessions, sid, &req.message);
                    }
                }
            }
            owned.check()?;
            let cfg = crate::backend::conversation_turn::cancellable(Some(&owned), async {
                Ok(state.config.read().await.clone())
            })
            .await?;
            if let (Some(session), Some(scope)) = (&owned.session_id, &voice_scope) {
                state
                    .native_voice
                    .prepare_for_turn(session, &owned.id, scope, &cfg.speech)
                    .await;
            }
            let (tx, mut rx) = mpsc::channel::<StreamEvent>(256);
            let agent = state.ai.run_agent_for_turn(
                messages,
                tool_registry::filtered_tools(&cfg),
                state.tools.clone(),
                state.approvals.clone(),
                agent_policy(&cfg),
                owned.session_id.clone(),
                tx,
                Some(owned.clone()),
            );
            let forward = async {
                while let Some(ev) = rx.recv().await {
                    if matches!(ev, StreamEvent::Done { .. } | StreamEvent::Error { .. }) {
                        continue;
                    }
                    if owned.state() == TurnState::Cancelling {
                        continue;
                    }
                    let (name, data) = sse_event(ev);
                    let data = serde_json::from_str(&data)?;
                    // Backpressure can't prevent cancellation. During tools keep draining.
                    tokio::select! {biased;
                        _ = owned.cancellation() => {},
                        result = out_tx.send(emit(&owned,name,data)) => {
                            if result.is_err() {owned.cancel();}
                        }
                    }
                }
                Ok::<_, anyhow::Error>(())
            };
            let (outcome, ()) = tokio::try_join!(agent, forward)?;
            // Optional post-processing stays inside this supervisor. Cancellation
            // drops summary HTTP; already started speech is drained like a tool.
            if cfg.memory.auto_summarize {
                let should = {
                    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(sid) = &owned.session_id {
                        let count = sessions.count_messages(sid)?;
                        let (_, summarized) = sessions.get_summary(sid)?;
                        crate::backend::session_manager::should_summarize(count, summarized)
                    } else {
                        false
                    }
                };
                if should {
                    if let Some(sid) = &owned.session_id {
                        // Maintenance failure doesn't falsify the successfully parsed chat.
                        let summary = crate::backend::conversation_turn::cancellable(
                            Some(&owned),
                            summarize_session(&state, sid),
                        )
                        .await;
                        owned.check()?;
                        if let Err(error) = summary {
                            log::warn!("Resumen automático: {error}");
                        }
                    }
                }
            }
            owned.complete(|| {
                state
                    .sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .publish_turn(
                        owned.session_id.as_deref(),
                        &owned.id,
                        &outcome.new_messages,
                    )
            })?;
            if cfg.speech.auto_speak && voice_scope.is_some() && !outcome.response.trim().is_empty()
            {
                if let Some(sid) = &owned.session_id {
                    let _ = state
                        .native_voice
                        .speak(
                            sid.clone(),
                            Some(owned.id.clone()),
                            voice_scope.clone(),
                            outcome.response.clone(),
                            cfg.speech.clone(),
                        )
                        .await;
                }
            }
            Ok::<_, anyhow::Error>(outcome.response)
        };
        // A closed HTTP body cancels even before provider headers/first token.
        // Do not drop work here: a dispatched tool must finish before terminal.
        let disconnect = async {
            out_tx.closed().await;
            owned.cancel();
            std::future::pending::<()>().await
        };
        let revoke_preparation = async {
            owned.cancellation().await;
            // The registry wakes on every terminal state, including success.
            // A completed turn is precisely the one allowed to read its final text.
            if owned.state() != TurnState::Completed {
                state
                    .native_voice
                    .cancel_turn(owned.session_id.as_deref(), &owned.id)
                    .await;
            }
            std::future::pending::<()>().await
        };
        let result = tokio::select! {
            _=revoke_preparation=>unreachable!(),
            result = std::panic::AssertUnwindSafe(work).catch_unwind() => result.unwrap_or_else(|_|Err(anyhow::anyhow!("supervisor_panic"))),
            _ = disconnect => unreachable!(),
        };
        if owned.state() != TurnState::Completed {
            state
                .native_voice
                .cancel_turn(owned.session_id.as_deref(), &owned.id)
                .await;
        }
        let terminal = if owned.state() == TurnState::Cancelling {
            let _ = owned.transition(TurnState::Cancelled);
            emit(
                &owned,
                "cancelled",
                serde_json::json!({"message":"Cancelado por el usuario; efectos iniciados pueden haber terminado"}),
            )
        } else {
            match result {
                Ok(response) => emit(&owned, "done", serde_json::json!({"full_content":response})),
                Err(error) => {
                    let _ = owned.transition(TurnState::Failed);
                    emit(
                        &owned,
                        "error",
                        serde_json::json!({"code":error_code(&error),"message":format!("{error:#}").chars().take(700).collect::<String>(),"retryable":false}),
                    )
                }
            }
        };
        if owned.state() != TurnState::Completed {
            let status = if owned.state() == TurnState::Cancelled {
                "cancelled"
            } else {
                "failed"
            };
            if let Err(error) = state
                .sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .settle_turn(&owned.id, status)
            {
                log::error!("turn persistence: {error}");
            }
        }
        // No terminal event is lost behind buffered tokens; no await retains resources.
        if let Some(permit) = terminal_permit {
            permit.send(terminal);
        }
    });
    registry.supervise(&turn, task);
    Ok((turn, out_rx))
}

fn error_code(error: &anyhow::Error) -> &'static str {
    if let Some(provider) =
        error.downcast_ref::<crate::backend::provider_connection::ProviderError>()
    {
        return match provider.category {
            "timeout" => "provider_timeout",
            "network" => "provider_transport",
            _ => "provider_error",
        };
    }
    let detail = format!("{error:#}");
    if detail.contains("context_invalid") {
        "context_invalid"
    } else if detail.contains("context_budget") {
        "context_budget_exceeded"
    } else if detail.contains("sse_") || detail.contains("malformed_tool") {
        "invalid_provider_stream"
    } else if detail.contains("timed out") || detail.contains("timeout") {
        "provider_timeout"
    } else if detail.contains("transport:") {
        "provider_transport"
    } else if detail.contains("provider_") {
        "provider_error"
    } else {
        "turn_failed"
    }
}
fn start_error(error: anyhow::Error) -> Response {
    let message = error.to_string();
    let status = if message.starts_with("session_busy") {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_REQUEST
    };
    (status,Json(serde_json::json!({"code":if status == StatusCode::CONFLICT {"session_busy"} else {"invalid_session"},"error":message}))).into_response()
}
async fn chat_complete(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> Response {
    let transcript = req.message.clone();
    let (turn, mut rx) = match start_turn(state, req, false) {
        Ok(v) => v,
        Err(e) => return start_error(e),
    };
    let _lease = ClientLease(turn.clone());
    let mut tools = Vec::new();
    while let Some(event) = rx.recv().await {
        match event.name {
            "tool_call" => tools.push(event.data),
            "done" => return Json(serde_json::json!({"transcript":transcript,"response":event.data["full_content"],"tool_calls":tools,"turn_id":turn.id,"session_id":turn.session_id,"state":turn.state()})).into_response(),
            "cancelled" => return (StatusCode::CONFLICT,Json(event.data)).into_response(),
            "error" => return (StatusCode::BAD_GATEWAY,Json(event.data)).into_response(),
            _ => {}
        }
    }
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"code":"supervisor_ended","turn_id":turn.id})),
    )
        .into_response()
}
fn start_stream(state: AppState, req: ChatRequest, regenerate: bool) -> Response {
    let (turn, rx) = match start_turn(state, req, regenerate) {
        Ok(v) => v,
        Err(e) => return start_error(e),
    };
    let stream =
        futures_util::stream::unfold((rx, ClientLease(turn)), |(mut rx, lease)| async move {
            rx.recv().await.map(|ev| {
                (
                    Ok::<Event, Infallible>(
                        Event::default()
                            .event(ev.name)
                            .id(ev.data["sequence"].to_string())
                            .data(ev.data.to_string()),
                    ),
                    (rx, lease),
                )
            })
        });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Arranca el servidor HTTP en `127.0.0.1:port`.
pub async fn serve(state: AppState, port: u16) -> Result<()> {
    anyhow::ensure!(
        crate::backend::auth::valid_server_token(&state.local_token),
        "Token inválido: API no publicada"
    );
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
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
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
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
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

async fn chat(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> Response {
    start_stream(state, req, false)
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

async fn build_messages(
    state: &AppState,
    req: &ChatRequest,
    regenerate: bool,
) -> Result<Vec<Message>> {
    let cfg = state.config.read().await.clone();
    let mut system_prompt = cfg.ai.system_prompt.clone();
    let sessions = state.sessions.clone();
    let sid = req.session_id.clone();
    let current = if regenerate {
        None
    } else {
        Some(Message::user(req.message.clone()))
    };
    tokio::task::spawn_blocking(move || {
        let sessions = sessions.lock().unwrap_or_else(|e| e.into_inner());
        if cfg.memory.enabled {
            let facts = sessions.context_facts(cfg.ai.max_context_bytes)?;
            if !facts.is_empty() {
                system_prompt.push_str(
                    "\n\nDatos del usuario (recordados localmente, pueden estar desactualizados):",
                );
                for fact in facts {
                    system_prompt.push_str(&format!("\n- {}: {}", fact.key, fact.value));
                }
            }
        }
        sessions.select_context(
            sid.as_deref(),
            vec![Message::system(system_prompt)],
            current,
            &cfg.ai,
            &tool_registry::filtered_tools(&cfg),
        )
    })
    .await?
}

/// Revisa si toca resumir el hilo y lo lanza en segundo plano (F5).
/// Artefacto legacy opcional; nunca sustituye el contexto conversacional.
async fn summarize_session(state: &AppState, session_id: &str) -> anyhow::Result<()> {
    let (prev_summary, prev_count, chunk) = {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.summary_excerpt(session_id)?
    };
    if chunk.trim().is_empty() {
        return Ok(());
    }
    let user = if prev_summary.trim().is_empty() {
        format!("Resume esta conversación en ≤250 palabras, hechos y decisiones:\n{chunk}")
    } else {
        format!(
            "Resumen previo:\n{prev_summary}\n\nContinúa la conversación:\n{chunk}\n\nActualiza el resumen (≤250 palabras)."
        )
    };
    let summary = state
        .ai
        .simple_completion(
            "Eres un resumidor conciso. Solo hechos, decisiones y contexto útil.",
            &user,
            512,
        )
        .await?;
    if summary.trim().is_empty() {
        anyhow::bail!("resumen vacío");
    }
    // Nuevo punto de corte = mensajes totales - 20.
    let new_count = {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let total = sessions.count_messages(session_id).unwrap_or(prev_count);
        total - 20.min(total)
    };
    {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.set_summary(session_id, summary.trim(), new_count)?;
    }
    log::info!("Resumen actualizado para sesión {session_id} (hasta {new_count})");
    Ok(())
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
            success,
            tool_call_id,
            content,
            image_url,
        } => (
            "tool_result",
            serde_json::json!({ "success": success, "tool_call_id": tool_call_id, "content": content, "image_url": image_url }).to_string(),
        ),
        StreamEvent::ToolApprovalNeeded {
            tool_call_id,
            consent_id,
            turn_id,
            session_id,
            name,
            effect,
            resource,
            detail,
            permission,
        } => (
            "approval_needed",
            serde_json::json!({ "tool_call_id": tool_call_id, "consent_id": consent_id, "turn_id":turn_id, "session_id":session_id, "name": name, "effect":effect, "resource":resource, "detail":detail, "permission": permission }).to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::ai_service::AiService;
    use crate::backend::chime_player::ChimePlayer;
    use crate::backend::conversation_turn::TurnRegistry;
    use crate::backend::session_manager::SessionManager;
    use crate::backend::speech_service::SpeechService;
    use crate::backend::tool_executor::ToolExecutor;
    use crate::backend::voice_pipeline::VoicePipeline;

    pub(super) async fn test_state() -> AppState {
        let cfg = Arc::new(RwLock::new(Config::default()));
        let sessions = Arc::new(Mutex::new(SessionManager::in_memory_for_tests().unwrap()));
        let ai = Arc::new(AiService::new(cfg.clone()).await.unwrap());
        let tools = Arc::new(ToolExecutor::legacy_fixture(cfg.clone()));
        let approvals = Arc::new(crate::backend::approvals::ApprovalManager::new());
        let speech = Arc::new(SpeechService::new(cfg.clone()).await.unwrap());
        let chimes = Arc::new(ChimePlayer::new().await.unwrap());
        let voice = Arc::new(VoicePipeline::new(
            cfg.clone(),
            speech.clone(),
            ai.clone(),
            tools.clone(),
            approvals.clone(),
            sessions.clone(),
            chimes,
        ));
        AppState {
            desktop: Arc::new(crate::desktop::Desktop::default()),
            native_voice: Arc::new(crate::backend::voice_coordinator::VoiceCoordinator::default()),
            http_port: 8765,
            voice_enabled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ai,
            tools,
            approvals,
            config: cfg,
            sessions,
            speech,
            voice,
            agent_events: tokio::sync::broadcast::channel(8).0,
            local_token: "test-token".into(),
            chat_tasks: Arc::new(TurnRegistry::default()),
            dictation_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            sidecar_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    #[tokio::test]
    async fn f3_http_desktop_sse_is_authenticated_bounded_and_never_dispatches() {
        use futures_util::StreamExt;
        let mut state = test_state().await;
        state.local_token = "b".repeat(64);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        state.http_port = address.port();
        let app = router(state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = format!("http://{address}/api/desktop/events");
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        assert_eq!(
            client.get(&url).send().await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        for host in [
            "localhost:8765".to_owned(),
            format!("localhost:{}@evil", address.port()),
        ] {
            assert_eq!(
                client
                    .get(&url)
                    .header("Host", host)
                    .bearer_auth(&state.local_token)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        for origin in [
            format!("http://localhost:{}/evil", address.port()),
            format!("http://localhost:{}.evil", address.port()),
        ] {
            assert_eq!(
                client
                    .get(&url)
                    .header("Origin", origin)
                    .bearer_auth(&state.local_token)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(&state.local_token)
                .json(&serde_json::json!({"action":"notify", "tool":"close_app"}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        let response = client
            .get(&url)
            .bearer_auth(&state.local_token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.bytes_stream();
        state
            .desktop
            .send(crate::desktop::UiAction::Toggle)
            .unwrap();
        let chunk = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&chunk).contains("data: toggle"));
        // On the current-thread test runtime this producer cannot yield to the
        // HTTP task: a stalled receiver must close on broadcast lag.
        for _ in 0..40 {
            state
                .desktop
                .send(crate::desktop::UiAction::Toggle)
                .unwrap();
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
                .await
                .unwrap()
                .is_none()
        );
        let fresh = client
            .get(&url)
            .bearer_auth(&state.local_token)
            .send()
            .await
            .unwrap();
        let mut fresh = fresh.bytes_stream();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), fresh.next())
                .await
                .is_err()
        );
        state.desktop.send(crate::desktop::UiAction::Show).unwrap();
        let chunk = tokio::time::timeout(std::time::Duration::from_secs(1), fresh.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&chunk).contains("data: show"));
        assert!(!String::from_utf8_lossy(&chunk).contains("toggle"));
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn f2_http_consent_requires_live_turn_and_exact_call_identity() {
        let mut state = test_state().await;
        state.local_token = "a".repeat(64);
        let root = std::env::temp_dir().join(format!("kda-f2-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("fixture.txt");
        std::fs::write(&file, "synthetic").unwrap();
        {
            let mut cfg = state.config.write().await;
            cfg.tools.allowed_paths = vec![root.to_string_lossy().into_owned()];
        }
        state.tools = Arc::new(ToolExecutor::with_audit(
            state.config.clone(),
            state.sessions.clone(),
        ));
        let sid = state
            .sessions
            .lock()
            .unwrap()
            .create_session("f2")
            .unwrap()
            .id;
        let turn = state.chat_tasks.reserve(Some(sid.clone())).unwrap();
        turn.transition(TurnState::Running).unwrap();
        state
            .sessions
            .lock()
            .unwrap()
            .begin_turn(&turn.id, Some(&sid), None)
            .unwrap();
        let call = crate::models::ToolCall {
            id: uuid::Uuid::new_v4().to_string(),
            name: "read_file".into(),
            arguments: serde_json::from_value(serde_json::json!({"path":file})).unwrap(),
        };
        let (_op, consent) = state
            .tools
            .prepare_for_turn_with_consent(&turn.id, &call, "chat")
            .unwrap();
        let consent = consent.unwrap();
        let _pending = state.approvals.register(&consent);
        turn.transition(TurnState::WaitingForApproval).unwrap();
        let wrong = approve_tool(
            State(state.clone()),
            Json(ApproveToolRequest {
                tool_call_id: "wrong".into(),
                consent_id: consent.clone(),
                turn_id: turn.id.clone(),
                session_id: Some(sid.clone()),
                approved: true,
            }),
        )
        .await
        .into_response();
        assert_eq!(wrong.status(), StatusCode::NOT_FOUND);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = router(state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = format!("http://{address}/api/tools/approve");
        let client = reqwest::Client::new();
        let forged_body = serde_json::json!({
            "tool_call_id":"wrong", "consent_id":consent,
            "turn_id":turn.id, "session_id":sid, "approved":true
        });
        let without_bearer = client
            .post(&url)
            .header("Host", "127.0.0.1:8765")
            .json(&forged_body)
            .send()
            .await
            .unwrap();
        assert_eq!(without_bearer.status(), StatusCode::UNAUTHORIZED);
        let forged_event = client
            .post(&url)
            .header("Host", "127.0.0.1:8765")
            .bearer_auth(&state.local_token)
            .json(&forged_body)
            .send()
            .await
            .unwrap();
        assert_eq!(forged_event.status(), StatusCode::NOT_FOUND);
        server.abort();
        let request = ApproveToolRequest {
            tool_call_id: call.id.clone(),
            consent_id: consent.clone(),
            turn_id: turn.id.clone(),
            session_id: Some(sid),
            approved: true,
        };
        assert_eq!(
            approve_tool(
                State(state.clone()),
                Json(ApproveToolRequest {
                    tool_call_id: request.tool_call_id.clone(),
                    consent_id: request.consent_id.clone(),
                    turn_id: request.turn_id.clone(),
                    session_id: request.session_id.clone(),
                    approved: true
                })
            )
            .await
            .into_response()
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            approve_tool(State(state.clone()), Json(request))
                .await
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn f1b2_cancel_dispatched_tool_drains_effect_and_blocks_next_tool() {
        use crate::backend::tool_executor::TestCreateGate;
        use std::time::Duration;
        let mut state = test_state().await;
        let root = std::env::temp_dir().join(format!("kda-tool-drain-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let first = root.join("first.txt");
        let second = root.join("second.txt");
        let sid = state
            .sessions
            .lock()
            .unwrap()
            .create_session("drain")
            .unwrap()
            .id;
        let response = format!(
            "data: {}\n\ndata: [DONE]\n\n",
            serde_json::json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":"provider-a","function":{"name":"create_file","arguments":serde_json::json!({"path":first,"content":"effect started"}).to_string()}},
            {"index":1,"id":"provider-b","function":{"name":"create_file","arguments":serde_json::json!({"path":second,"content":"must not execute"}).to_string()}}
        ]},"finish_reason":"tool_calls"}]})
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        {
            let mut cfg = state.config.write().await;
            cfg.ai.provider = "custom".into();
            cfg.ai.api_key = "test-fixture".into();
            cfg.ai.base_url = format!("http://{}", listener.local_addr().unwrap());
            cfg.ai.enable_tool_calling = true;
            cfg.tools.create_file = true;
            cfg.tools.confirm_sensitive = false;
            cfg.tools.allowed_paths = vec![root.to_string_lossy().into()];
        }
        let provider = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/chat/completions",
                    post(move || {
                        let response = response.clone();
                        async move { response }
                    }),
                ),
            )
            .await
            .unwrap();
        });
        let gate = Arc::new(TestCreateGate::default());
        let mut tools = ToolExecutor::with_audit(state.config.clone(), state.sessions.clone());
        tools.create_gate = Some(gate.clone());
        state.tools = Arc::new(tools);
        let (turn, mut rx) = start_turn(
            state.clone(),
            ChatRequest {
                session_id: Some(sid.clone()),
                message: "simulate".into(),
            },
            false,
        )
        .unwrap();
        let approval = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let event = rx.recv().await.expect("approval event");
                if event.name == "approval_needed" {
                    break event.data;
                }
            }
        })
        .await
        .unwrap();
        let decision = ApproveToolRequest {
            tool_call_id: approval["tool_call_id"].as_str().unwrap().into(),
            consent_id: approval["consent_id"].as_str().unwrap().into(),
            turn_id: turn.id.clone(),
            session_id: Some(sid.clone()),
            approved: true,
        };
        assert_eq!(
            approve_tool(State(state.clone()), Json(decision))
                .await
                .into_response()
                .status(),
            StatusCode::OK
        );
        tokio::time::timeout(Duration::from_secs(3), gate.started.notified())
            .await
            .unwrap();
        let response = chat_cancel(
            State(state.clone()),
            Json(ChatCancelRequest {
                session_id: Some(sid.clone()),
                turn_id: turn.id.clone(),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let ack: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(ack["cancellation_effective"], false);
        assert_eq!(turn.state(), TurnState::Cancelling);
        assert_eq!(state.chat_tasks.active_count(), 1);
        assert!(state.chat_tasks.reserve(Some(sid.clone())).is_err());
        gate.release.notify_one();
        tokio::time::timeout(Duration::from_secs(3), turn.wait_terminal())
            .await
            .unwrap();
        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev.name);
        }
        assert_eq!(turn.state(), TurnState::Cancelled);
        assert!(events.contains(&"cancelled"));
        assert!(!events.contains(&"done"));
        assert!(first.exists());
        assert!(!second.exists());
        let audit = state.sessions.lock().unwrap().list_tool_audit(10).unwrap();
        assert_eq!(audit.len(), 1);
        assert!(audit[0].success);
        let operations = state
            .sessions
            .lock()
            .unwrap()
            .list_operations(Some(&sid), 10)
            .unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].state, "succeeded");
        assert_eq!(operations[0].turn_id, turn.id);

        assert_eq!(
            state
                .sessions
                .lock()
                .unwrap()
                .get_messages(&sid)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(state.chat_tasks.active_count(), 0);
        provider.abort();
        assert!(provider.await.unwrap_err().is_cancelled());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// F0-3: solo el bearer exacto pasa el middleware de auth.
    #[test]
    fn bearer_auth_validation() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(!crate::backend::auth::valid_bearer(&headers, "expected"));
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer expected".parse().unwrap(),
        );
        assert!(crate::backend::auth::valid_bearer(&headers, "expected"));
        assert!(!crate::backend::auth::valid_bearer(&headers, "otro"));
    }

    /// Dictado: start cuando la voz del asistente ya graba -> 409 Conflict.
    #[tokio::test]
    #[cfg(test)]
    async fn dictate_start_conflicts_with_voice_pipeline() {
        let state = test_state().await;
        state.voice.start_recording(48000);
        let resp = dictate_start(State(state.clone())).await.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        state.voice.stop_recording();
    }

    /// Dictado: stop sin dictado activo -> 409 Conflict.
    #[tokio::test]
    #[cfg(test)]
    async fn dictate_stop_without_start_conflicts() {
        let state = test_state().await;
        let resp = dictate_stop(State(state)).await.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    /// Dictado: start+stop sin audio -> transcript vacio y la bandera queda
    /// liberada (puedes volver a dictar).
    #[tokio::test]
    async fn dictate_roundtrip_empty_transcript_releases_flag() {
        let state = test_state().await;
        let resp = dictate_start(State(state.clone())).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = dictate_stop(State(state.clone())).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 16 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["transcript"], "");
        assert!(!state
            .dictation_active
            .load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[cfg(test)]
mod f4_native_tests {
    use super::*;
    use crate::backend::voice_coordinator::{Identity, State as VoiceState};
    #[tokio::test]
    async fn background_control_fails_closed_before_capture_and_has_no_chat_authority() {
        let state = super::tests::test_state().await;
        let sid = state
            .sessions
            .lock()
            .unwrap()
            .create_session("background offline")
            .unwrap()
            .id;
        state.native_voice.select_session(Some(sid.clone())).await;
        state.config.write().await.speech.stt_model = "/nonexistent/background-stt.bin".into();
        assert!(!state.native_voice.background_snapshot().enabled);
        assert!(serde_json::from_value::<BackgroundControl>(
            serde_json::json!({"action":"enable","audio":"injected"})
        )
        .is_err());
        let response = native_background(
            State(state.clone()),
            Json(BackgroundControl {
                action: "enable".into(),
                generation: None,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !state.native_voice.background_snapshot().paused {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let status = state.native_voice.background_snapshot();
        assert!(status.enabled && status.paused && !status.microphone_active);
        assert!(status.capture_pid.is_none() && status.detector_pid.is_none());
        assert_eq!(status.state, "error");
        assert_eq!(
            state.sessions.lock().unwrap().count_messages(&sid).unwrap(),
            0
        );
        assert!(state
            .sessions
            .lock()
            .unwrap()
            .list_tool_audit(10)
            .unwrap()
            .is_empty());
        assert!(state.chat_tasks.current(&Some(sid)).is_none());
        native_background(
            State(state.clone()),
            Json(BackgroundControl {
                action: "disable".into(),
                generation: None,
            }),
        )
        .await;
        assert!(!state.native_voice.background_snapshot().enabled);
        state.native_voice.shutdown().await;
    }
    #[tokio::test]
    async fn wake_is_explicit_scoped_and_has_no_chat_or_tool_authority() {
        let state = super::tests::test_state().await;
        let sid = state
            .sessions
            .lock()
            .unwrap()
            .create_session("synthetic wake")
            .unwrap()
            .id;
        let scope = state.native_voice.select_session(Some(sid.clone())).await;
        state.config.write().await.speech.stt_model = "/nonexistent/wake-no-stt.bin".into();
        assert!(!state.config.read().await.speech.wake_word_enabled);
        assert!(
            serde_json::from_value::<NativeVoiceStart>(serde_json::json!({
                "session_id":sid, "scope":scope, "wake":true, "transcript":"Abre Dolphin"
            }))
            .is_err()
        );
        let response = native_voice_start(
            State(state.clone()),
            Json(NativeVoiceStart {
                session_id: sid.clone(),
                scope,
                wake: true,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        assert_eq!(state.native_voice.snapshot().state, VoiceState::Error);
        assert_eq!(
            state.sessions.lock().unwrap().count_messages(&sid).unwrap(),
            0
        );
        assert!(state.chat_tasks.current(&Some(sid)).is_none());
        assert!(state
            .sessions
            .lock()
            .unwrap()
            .list_tool_audit(10)
            .unwrap()
            .is_empty());
        state.native_voice.shutdown().await;
    }
    #[tokio::test]
    async fn dictation_requires_existing_selected_session_and_has_no_chat_effect() {
        let state = super::tests::test_state().await;
        let missing = native_voice_start(
            State(state.clone()),
            Json(NativeVoiceStart {
                wake: false,
                session_id: "missing".into(),
                scope: "missing".into(),
            }),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let sid = state
            .sessions
            .lock()
            .unwrap()
            .create_session("synthetic F4")
            .unwrap()
            .id;
        let wrong = native_voice_start(
            State(state.clone()),
            Json(NativeVoiceStart {
                wake: false,
                session_id: sid.clone(),
                scope: "wrong".into(),
            }),
        )
        .await;
        assert_eq!(wrong.status(), StatusCode::CONFLICT);
        let scope = state.native_voice.select_session(Some(sid.clone())).await;
        state.config.write().await.speech.stt_model = "/nonexistent/f4-no-model.bin".into();
        state.config.write().await.speech.stt_backend = "api".into();
        let started = native_voice_start(
            State(state.clone()),
            Json(NativeVoiceStart {
                wake: false,
                session_id: sid.clone(),
                scope,
            }),
        )
        .await;
        assert_eq!(started.status(), StatusCode::ACCEPTED);
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        assert_eq!(state.native_voice.snapshot().state, VoiceState::Error);
        assert_eq!(
            state.sessions.lock().unwrap().count_messages(&sid).unwrap(),
            0
        );
        assert!(state.chat_tasks.current(&Some(sid.clone())).is_none());
        state.native_voice.shutdown().await;
    }
    #[tokio::test]
    async fn wrong_identity_cannot_stop_or_cancel_voice_and_no_legacy_injection_routes() {
        let state = super::tests::test_state().await;
        let id = Identity {
            session_id: "wrong".into(),
            generation: "wrong".into(),
            turn_id: None,
        };
        assert_eq!(
            native_voice_stop(State(state.clone()), Json(id.clone()))
                .await
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            native_voice_cancel(State(state.clone()), Json(id))
                .await
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(state.native_voice.snapshot().state, VoiceState::Idle);
    }
}
