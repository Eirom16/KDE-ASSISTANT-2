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
    extract::{Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
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
    pub chat_tasks: Arc<Mutex<std::collections::HashMap<String, tokio::task::AbortHandle>>>,
    /// Dictado al input (botón mic del chat): grabación sin pipeline de voz.
    pub dictation_active: Arc<std::sync::atomic::AtomicBool>,
    /// El sidecar de voz (Python/Pipecat) es dueño del mic. Los endpoints de
    /// dictado/voz/comando se reenvian al puerto 8766 cuando está activo.
    pub sidecar_active: Arc<std::sync::atomic::AtomicBool>,
}

/// Middleware F0-3: auth + host + origin para `/api/*`.
async fn auth_layer(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if !crate::backend::auth::path_requires_auth(&path) {
        return next.run(req).await;
    }
    if !crate::backend::auth::valid_host(req.headers()) {
        log::debug!("auth: host rechazado para {}", path);
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "host no permitido" })),
        )
            .into_response();
    }
    if !crate::backend::auth::valid_origin(req.headers()) {
        log::debug!("auth: origin rechazado para {}", path);
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "origin no permitido" })),
        )
            .into_response();
    }
    if !crate::backend::auth::valid_bearer(req.headers(), &state.local_token) {
        log::debug!("auth: bearer inválido para {}", path);
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized: falta Bearer local" })),
        )
            .into_response();
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
        .route("/api/chat", post(chat))
        .route("/api/chat/complete", post(chat_complete))
        .route("/api/chat/cancel", post(chat_cancel))
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
        .route("/api/voice/last", get(voice_last))
        .route("/api/voice/stream", get(voice_stream))
        .route("/api/agent/stream", get(agent_stream))
        .route("/api/agent/dashboard", get(agent_dashboard))
        .route("/api/voice/session", post(set_voice_session))
        .route(
            "/api/voice/calibration",
            get(calibration_status)
                .post(calibration_start)
                .delete(calibration_stop),
        )
        .route("/api/voice/log", post(voice_log))
        .route("/api/voice/barge_in", post(voice_barge_in))
        .route("/api/voice/sidecar-event", post(voice_sidecar_event))
        .route("/api/dictate/start", post(dictate_start))
        .route("/api/dictate/stop", post(dictate_stop))
        .route("/api/speak", post(speak_text))
        .route("/api/speak/stop", post(speak_stop))
        .route("/api/audio/devices", get(audio_devices))
        .route("/api/audio/device", post(set_audio_device))
        .route("/api/tools", get(list_tools))
        .route("/api/tools/schemas", get(list_tool_schemas))
        .route("/api/tools/execute", post(execute_tool_manual))
        .route("/api/tools/approve", post(approve_tool))
        .route("/api/tools/audit", get(list_tool_audit))
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
}

/// Cancela el agente en curso para una sesión (F0-7).
/// Aborta el task del agente; el stream SSE se cierra y no se persiste nada nuevo.
/// El QML además hace `xhr.abort()` en local.
async fn chat_cancel(
    State(state): State<AppState>,
    Json(body): Json<ChatCancelRequest>,
) -> impl IntoResponse {
    let key = body.session_id.unwrap_or_else(|| "default".to_string());
    let aborted = {
        let mut map = state.chat_tasks.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&key).map(|h| {
            h.abort();
            true
        })
    };
    // También silenciar TTS por si el turno ya estaba hablando.
    if aborted.unwrap_or(false) {
        state.speech.request_stop();
        log::info!("Chat cancelado para sesión {key}");
        (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "cancelled" })),
        )
    } else {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "nothing_to_cancel" })),
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatRegenerateRequest {
    pub session_id: String,
}

/// Regenera la última respuesta (F1-1): borra el trailing assistant/tools
/// posterior al último `user` y re-ejecuta el agente con ese historial.
/// SSE igual que `/api/chat` (token/tool_call/tool_result/done/error).
/// También sirve como "reintentar" tras un tool en error.
async fn chat_regenerate(
    State(state): State<AppState>,
    Json(body): Json<ChatRegenerateRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let sid = body.session_id.clone();
    log::info!("Regenerate request (session={sid})");

    // Validar y preparar historial. En error, el stream llevará un evento error.
    // (Un solo punto de retorno SSE para unificar el tipo opaque.)
    let prepared: Result<Vec<Message>, String> = {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if sessions.get_session(&sid).ok().flatten().is_none() {
            Err("Sesión no encontrada".to_string())
        } else {
            // Limpiar el intento anterior para no duplicar contexto.
            let _ = sessions.delete_trailing_after_last_user(&sid);
            let history = sessions.get_messages(&sid).unwrap_or_default();
            if !history.iter().any(|m| matches!(m, Message::User { .. })) {
                Err("No hay mensaje de usuario para regenerar".to_string())
            } else {
                Ok(history)
            }
        }
    };

    let (tx, rx) = mpsc::channel::<StreamEvent>(256);
    match prepared {
        Err(msg) => {
            let _ = tx.send(StreamEvent::Error { message: msg }).await;
            drop(tx);
        }
        Ok(history) => {
            let system_prompt = state.config.read().await.ai.system_prompt.clone();
            let mut messages = vec![Message::system(system_prompt)];
            // Ventana como en build_messages.
            const MAX_HISTORY: usize = 40;
            let start = history.len().saturating_sub(MAX_HISTORY);
            messages.extend(history.into_iter().skip(start));

            let ai = state.ai.clone();
            let tools_exec = state.tools.clone();
            let cfg_snapshot = state.config.read().await.clone();
            let tools = tool_registry::filtered_tools(&cfg_snapshot);
            let approvals = state.approvals.clone();
            let policy = agent_policy(&cfg_snapshot);
            let sid_for_task = sid.clone();
            let agent_task = tokio::spawn(async move {
                ai.run_agent(
                    messages,
                    tools,
                    tools_exec,
                    approvals,
                    policy,
                    Some(sid_for_task),
                    tx,
                )
                .await
            });
            let cancel_key = sid.clone();
            register_task(&state, &cancel_key, agent_task.abort_handle());

            let state2 = state.clone();
            let sid2 = sid.clone();
            let cancel_key2 = cancel_key.clone();
            tokio::spawn(async move {
                match agent_task.await {
                    Ok(Ok(outcome)) => {
                        unregister_if_finished(&state2, &cancel_key2);
                        let sessions = state2.sessions.lock().unwrap_or_else(|e| e.into_inner());
                        for m in &outcome.new_messages {
                            let _ = sessions.add_message(&sid2, m);
                        }
                        if outcome.new_messages.is_empty() && !outcome.response.trim().is_empty() {
                            let _ =
                                sessions.add_message(&sid2, &Message::assistant(outcome.response));
                        }
                        drop(sessions);
                        // F5: resumir el hilo en segundo plano si toca.
                        maybe_summarize(&state2, &sid2);
                    }
                    Ok(Err(e)) => {
                        unregister_if_finished(&state2, &cancel_key2);
                        log::warn!("Regenerate fallo: {e}");
                    }
                    Err(e) => {
                        if e.is_cancelled() {
                            log::info!("Regenerate cancelado para {cancel_key2}");
                        } else {
                            log::warn!("Task regenerate fallo: {e}");
                        }
                        unregister_if_finished(&state2, &cancel_key2);
                    }
                }
            });
        }
    }

    let sse_stream = stream_from_receiver(rx);
    let sse_stream = sse_stream.map(move |ev| {
        let (event_name, data) = sse_event(ev);
        Ok::<Event, Infallible>(Event::default().event(event_name).data(data))
    });

    Sse::new(sse_stream).keep_alive(KeepAlive::default())
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
    state.voice.set_active_session(valid.clone());
    Json(serde_json::json!({ "session_id": valid }))
}

#[derive(Debug, Serialize)]
struct CalibrationStatus {
    required: bool,
    count: u32,
    target: u32,
    active: bool,
}

async fn calibration_status(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await;
    Json(CalibrationStatus {
        required: cfg.speech.wake_word_enrollment_required,
        count: cfg.speech.wake_word_enrollment_count,
        target: 3,
        active: state.voice.calibration_active(),
    })
}

/// Inicia una muestra de "hey jarvis". La captura permanece local y usa el
/// mismo dispositivo ya abierto por AudioCapture.
async fn calibration_start(State(state): State<AppState>) -> impl IntoResponse {
    match state.voice.start_calibration() {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "recording" })),
        ),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// Guarda la muestra PCM resampleada a 16 kHz en data_local. Estas muestras
/// sirven para auditoría/calibración local; no se suben ni se usan como una
/// falsa re-entrenamiento del modelo ONNX.
async fn calibration_stop(State(state): State<AppState>) -> impl IntoResponse {
    if !state.voice.calibration_active() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "no hay calibración activa" })),
        );
    }
    let samples = state.voice.stop_calibration();
    if samples.len() < 12_800 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "muestra demasiado corta" })),
        );
    }
    let dir = match dirs::data_local_dir() {
        Some(d) => d.join("kde-assistant/wakeword/enrollment"),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "sin data_local_dir" })),
            )
        }
    };
    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }
    let count = state.config.read().await.speech.wake_word_enrollment_count + 1;
    let path = dir.join(format!("sample-{count}.wav"));
    let write = tokio::task::spawn_blocking(move || {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec)?;
        for sample in samples {
            writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        writer.finalize()?;
        Ok::<(), anyhow::Error>(())
    })
    .await;
    match write {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            );
        }
    }
    let mut cfg = state.config.write().await;
    cfg.speech.wake_word_enrollment_count = count.min(3);
    if cfg.speech.wake_word_enrollment_count >= 3 {
        cfg.speech.wake_word_enrollment_required = false;
    }
    let status = serde_json::json!({
        "required": cfg.speech.wake_word_enrollment_required,
        "count": cfg.speech.wake_word_enrollment_count,
        "target": 3,
        "active": false,
    });
    if let Err(e) = cfg.save().await {
        log::warn!("no se pudo guardar estado de calibración: {e}");
    }
    (StatusCode::OK, Json(status))
}

/// Payload empujado por el sidecar de voz (Python/Pipecat).
/// Eventos: {"type":"state","state":...} | {"type":"level","level":f32} |
///          {"type":"exchange","transcript":..,"response":..} .
/// Todos se reenvian al SSE /api/voice/stream (UI/personaje) sin cambios.
#[derive(Debug, serde::Deserialize)]
pub struct SidecarEvent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub level: Option<f32>,
    #[serde(default)]
    pub transcript: Option<String>,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

async fn voice_sidecar_event(
    State(state): State<AppState>,
    Json(ev): Json<SidecarEvent>,
) -> impl IntoResponse {
    match ev.kind.as_str() {
        "state" => {
            if let Some(s) = ev.state.as_deref() {
                // El sidecar manda estados ricos; la UI consume los 4 legacy.
                // bridge: los estados del sidecar van a emit_state y cambian
                // la VoiceState via emit_state. Mapeo directo:
                let sv = match s {
                    "wake_detected" => crate::backend::voice_state::VoiceState::WakeDetected,
                    "listening" => crate::backend::voice_state::VoiceState::Listening,
                    "thinking" => crate::backend::voice_state::VoiceState::Thinking,
                    "speaking" => crate::backend::voice_state::VoiceState::Responding,
                    "interrupted" => crate::backend::voice_state::VoiceState::Interrupted,
                    "tool_call" => crate::backend::voice_state::VoiceState::ToolExecuting,
                    "idle" => crate::backend::voice_state::VoiceState::Idle,
                    _ => crate::backend::voice_state::VoiceState::Idle,
                };
                state.voice.emit_state(sv);
            }
        }
        "level" => {
            if let Some(l) = ev.level {
                state.voice.emit_level_from_sidecar(l);
            }
        }
        "exchange" => {
            let transcript = ev.transcript.unwrap_or_default();
            let response = ev.response.unwrap_or_default();
            if !transcript.trim().is_empty() {
                state
                    .voice
                    .set_last_exchange_from_sidecar(transcript, response);
            }
        }
        _ => {}
    }
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
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
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    // Reusar la sesion indicada si existe; si no, la sesion unica de voz
    // (evita fragmentar el historial en N sesiones homonimas).
    let sid = match body.session_id.as_deref() {
        Some(id) if sessions.get_session(id).ok().flatten().is_some() => id.to_string(),
        _ => match sessions.get_or_create_voice_session() {
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
    drop(sessions);
    // F5: resumir el hilo en segundo plano si toca.
    maybe_summarize(&state, &sid);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "session_id": sid })),
    )
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

/// POST simple al sidecar de voz (127.0.0.1:8766), devolviendo su JSON.
async fn sidecar_post(path: &str) -> anyhow::Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:8766{path}"))
        .json(&serde_json::json!({}))
        .send()
        .await?;
    Ok(resp.json().await?)
}

/// Detiene la reproduccion TTS en curso (barge-in via hotkey/UI).
async fn speak_stop(State(state): State<AppState>) -> impl IntoResponse {
    state.speech.request_stop();
    // Si el VoicePipeline esta hablando, tambien hacer barge-in silencioso
    state.voice.barge_in_silent();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "stopped" })),
    )
}

/// Barge-in por voz: interrumpe TTS y reinicia escucha.
async fn voice_barge_in(State(state): State<AppState>) -> impl IntoResponse {
    state.voice.barge_in_silent();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "barge_in" })),
    )
}

/// Dictado al input del chat: empieza a grabar (solo STT, sin LLM ni TTS).
/// Cuando el sidecar Python posee el microfono (option por defecto), este
/// handler solo reenvia la peticion; el pipeline local queda como respaldo.
/// Rechaza si hay una captura activa (dictado o pipeline de voz).
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
    if tool_registry::permission(name) == tool_registry::Permission::Red {
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
    let session_id = body
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
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
            StatusCode::INTERNAL_SERVER_ERROR,
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
    if state.approvals.resolve(&body.tool_call_id, body.approved) {
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

/// Historial de herramientas ejecutadas (F4-3).
async fn list_tool_audit(
    State(state): State<AppState>,
    Query(q): Query<ToolAuditQuery>,
) -> impl IntoResponse {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.list_tool_audit(q.limit) {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Dispositivos de entrada + selección actual (F3-3).
async fn audio_devices(State(state): State<AppState>) -> impl IntoResponse {
    let devices =
        crate::backend::audio_capture::AudioCapture::list_input_devices().unwrap_or_default();
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
        let devices =
            crate::backend::audio_capture::AudioCapture::list_input_devices().unwrap_or_default();
        if !devices.iter().any(|d| d == &name) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "dispositivo no encontrado" })),
            );
        }
    }
    let mut cfg = state.config.write().await;
    let changed = cfg.speech.mic_device != name;
    cfg.speech.mic_device = name;
    match cfg.save().await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "restart_required": changed })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
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
        _ => cfg.ai.effective_base_url(),
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
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
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

/// Respuesta de `/api/chat/complete` (no streaming).
#[derive(Debug, Serialize)]
pub struct ChatCompleteResponse {
    pub transcript: String,
    pub response: String,
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<serde_json::Value>,
}

/// Mapea errores crudos del proveedor a mensajes comprensibles,
/// recortando HTML/JSON gigantes que el proveedor pueda devolver.
fn friendly_provider_error(raw: &str) -> String {
    let snippet: String = raw.chars().take(500).collect();
    if snippet.contains("401") || snippet.to_lowercase().contains("unauthorized") {
        return "No autorizado (401). Verifica tu API key en Configuración.".to_string();
    }
    if snippet.contains("429") || snippet.to_lowercase().contains("rate limit") {
        return "Límite de peticiones alcanzado (429). Espera unos segundos y reintenta."
            .to_string();
    }
    if snippet.contains("404")
        || snippet.to_lowercase().contains("model") && snippet.contains("not found")
    {
        return "Modelo no encontrado (404). Elige otro en Configuración → Modelo.".to_string();
    }
    if snippet.to_lowercase().contains("timeout") || snippet.to_lowercase().contains("timed out") {
        return "Tiempo de espera agotado. Revisa tu conexión e inténtalo de nuevo.".to_string();
    }
    if snippet.to_lowercase().contains("connection") || snippet.to_lowercase().contains("dns") {
        return format!("Sin conexión con el proveedor. Detalle: {snippet}");
    }
    format!("Error del asistente: {snippet}")
}

/// Clave para `chat_tasks`: session o "default".
fn task_key(session_id: &Option<String>) -> String {
    session_id.clone().unwrap_or_else(|| "default".to_string())
}

fn register_task(state: &AppState, key: &str, handle: tokio::task::AbortHandle) {
    let mut map = state.chat_tasks.lock().unwrap_or_else(|e| e.into_inner());
    map.insert(key.to_string(), handle);
}

fn unregister_if_finished(state: &AppState, key: &str) {
    let mut map = state.chat_tasks.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = map.get(key) {
        if h.is_finished() {
            map.remove(key);
        }
    }
}

fn agent_policy(cfg: &Config) -> crate::backend::approvals::ApprovalPolicy {
    crate::backend::approvals::ApprovalPolicy::from_config(cfg.tools.confirm_sensitive)
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

    // Ejecutar el agente (sin streaming al cliente). Respetar flags de config.
    let cfg_snapshot = state.config.read().await.clone();
    let tools = tool_registry::filtered_tools(&cfg_snapshot);
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(256);

    let ai = state.ai.clone();
    let tools_exec = state.tools.clone();
    let msgs = messages.clone();
    let approvals = state.approvals.clone();
    let policy = agent_policy(&cfg_snapshot);
    let session = req.session_id.clone();
    let agent_task = tokio::spawn(async move {
        ai.run_agent(msgs, tools, tools_exec, approvals, policy, session, tx)
            .await
    });
    // F0-7: registrar para /api/chat/cancel.
    let cancel_key = task_key(&req.session_id);
    register_task(&state, &cancel_key, agent_task.abort_handle());

    // Consumir eventos (para no bloquear el canal) y capturar la respuesta
    let mut full_response = String::new();
    // Capturar toolCalls para devolverlos al cliente (la UI los muestra).
    let mut tool_calls_out: Vec<serde_json::Value> = Vec::new();
    while let Some(ev) = rx.recv().await {
        match &ev {
            StreamEvent::Token { content } => full_response.push_str(content),
            StreamEvent::ToolCall { tool } => {
                tool_calls_out.push(serde_json::json!({
                    "id": tool.id, "name": tool.name, "arguments": tool.arguments
                }));
            }
            _ => {}
        }
    }

    let outcome = match agent_task.await {
        Ok(Ok(o)) => {
            unregister_if_finished(&state, &cancel_key);
            o
        }
        Ok(Err(e)) => {
            unregister_if_finished(&state, &cancel_key);
            log::warn!("Agente fallo: {e}");
            // Devolver el error como respuesta para que la UI lo muestre.
            // No persistir tools parciales en este caso.
            let body = ChatCompleteResponse {
                transcript: req.message.clone(),
                response: friendly_provider_error(&e.to_string()),
                session_id: req.session_id.clone(),
                tool_calls: tool_calls_out,
            };
            return (StatusCode::OK, Json(body));
        }
        Err(e) => {
            // F0-7: cancelado vía /api/chat/cancel → persistir solo usuario.
            if e.is_cancelled() {
                log::info!("Chat complete cancelado para {cancel_key}");
                if let Some(sid) = &req.session_id {
                    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = sessions.add_message(sid, &Message::user(req.message.clone()));
                }
                let body = ChatCompleteResponse {
                    transcript: req.message.clone(),
                    response: "Cancelado por el usuario.".to_string(),
                    session_id: req.session_id.clone(),
                    tool_calls: tool_calls_out,
                };
                return (StatusCode::OK, Json(body));
            }
            unregister_if_finished(&state, &cancel_key);
            log::warn!("Task agente fallo: {e}");
            let body = ChatCompleteResponse {
                transcript: req.message.clone(),
                response: format!("Error interno: {e}"),
                session_id: req.session_id.clone(),
                tool_calls: tool_calls_out,
            };
            return (StatusCode::OK, Json(body));
        }
    };

    // Si el agente retorno vacio (por ejemplo tool calls sin texto final),
    // usamos lo capturado en el stream.
    let mut response = if outcome.response.is_empty() {
        full_response
    } else {
        outcome.response.clone()
    };
    if response.trim().is_empty() && !outcome.new_messages.is_empty() {
        // Turno solo-tools sin texto: resumir para que el chat no quede vacío.
        response = format!(
            "He ejecutado {} herramienta(s). Revisa los detalles en el chat.",
            outcome.new_messages.len()
        );
    }

    // Persistir usuario + historial nuevo del turno (assistant+tools+final).
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let _ = sessions.add_message(sid, &Message::user(req.message.clone()));
        maybe_auto_title(&sessions, sid, &req.message);
        for m in &outcome.new_messages {
            let _ = sessions.add_message(sid, m);
        }
        // Si por alguna razón no hubo mensajes nuevos pero sí texto, guardar final.
        if outcome.new_messages.is_empty() && !response.trim().is_empty() {
            let _ = sessions.add_message(sid, &Message::assistant(response.clone()));
        }
        drop(sessions);
        // F5: resumir el hilo en segundo plano si toca.
        maybe_summarize(&state, sid);
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
        tool_calls: tool_calls_out,
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
    let _ = state.agent_events.send("processing".to_string());

    // Canal de stream events
    let (tx, rx) = mpsc::channel::<StreamEvent>(256);

    // Ejecutar el agente en un task (respetar flags de config).
    let ai = state.ai.clone();
    let tools_exec = state.tools.clone();
    let cfg_snapshot = state.config.read().await.clone();
    let tools = tool_registry::filtered_tools(&cfg_snapshot);
    let msgs = messages.clone();
    let approvals = state.approvals.clone();
    let policy = agent_policy(&cfg_snapshot);
    let session = req.session_id.clone();
    let agent_task = tokio::spawn(async move {
        ai.run_agent(msgs, tools, tools_exec, approvals, policy, session, tx)
            .await
    });
    // F0-7: registrar para /api/chat/cancel.
    let cancel_key = task_key(&req.session_id);
    register_task(&state, &cancel_key, agent_task.abort_handle());

    // Persistir el mensaje del usuario
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let _ = sessions.add_message(sid, &Message::user(req.message.clone()));
        maybe_auto_title(&sessions, sid, &req.message);
    }

    // Convertir el receiver a stream SSE
    let sse_stream = stream_from_receiver(rx);

    // Task que espera el resultado y persiste la respuesta
    let state2 = state.clone();
    let sid2 = req.session_id.clone();
    let cancel_key2 = cancel_key.clone();
    let sse_stream = sse_stream.map(move |ev| {
        let (event_name, data) = sse_event(ev);
        Ok::<Event, Infallible>(Event::default().event(event_name).data(data))
    });

    // Persistir el historial nuevo del turno (assistant+tools+final) cuando termina.
    tokio::spawn(async move {
        match agent_task.await {
            Ok(Ok(outcome)) => {
                unregister_if_finished(&state2, &cancel_key2);
                if let Some(sid) = &sid2 {
                    let sessions = state2.sessions.lock().unwrap_or_else(|e| e.into_inner());
                    for m in &outcome.new_messages {
                        let _ = sessions.add_message(sid, m);
                    }
                    if outcome.new_messages.is_empty() && !outcome.response.trim().is_empty() {
                        let _ = sessions.add_message(sid, &Message::assistant(outcome.response));
                    }
                    drop(sessions);
                    // F5: resumir el hilo en segundo plano si toca.
                    maybe_summarize(&state2, sid);
                }
            }
            Ok(Err(e)) => {
                unregister_if_finished(&state2, &cancel_key2);
                log::warn!("Agente SSE fallo: {e}");
            }
            Err(e) => {
                if e.is_cancelled() {
                    log::info!("Chat SSE cancelado para {cancel_key2}");
                } else {
                    log::warn!("Task agente SSE fallo: {e}");
                }
                // Limpiar solo si el handle almacenado ya terminó.
                unregister_if_finished(&state2, &cancel_key2);
            }
        }
        let _ = state2.agent_events.send("idle".to_string());
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
    let cfg = state.config.read().await.clone();
    let mut system_prompt = cfg.ai.system_prompt.clone();
    // F5: facts del usuario (solo si hay y está habilitado).
    if cfg.memory.enabled {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(facts) = sessions.list_facts() {
            if !facts.is_empty() {
                let mut block = String::from(
                    "\n\nDatos del usuario (recordados localmente, pueden estar desactualizados):",
                );
                for f in &facts {
                    block.push_str(&format!("\n- {}: {}", f.key, f.value));
                }
                system_prompt.push_str(&block);
            }
        }
    }
    let mut messages = Vec::new();
    messages.push(Message::system(system_prompt));

    // Cargar historial si hay session_id (ventana deslizante: últimos 40
    // para no saturar contexto ni superar max_tokens del proveedor).
    const MAX_HISTORY: usize = 40;
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        // F5: resumen del hilo largo (contexto más allá de la ventana).
        if let Ok((summary, _)) = sessions.get_summary(sid) {
            if !summary.trim().is_empty() {
                let short: String = summary.chars().take(1500).collect();
                messages.push(Message::system(format!(
                    "Resumen de la conversación anterior en esta sesión:\n{short}"
                )));
            }
        }
        if let Ok(history) = sessions.get_messages(sid) {
            // Inyectar historial (sin el system prompt duplicado)
            let start = history.len().saturating_sub(MAX_HISTORY);
            messages.extend(history.into_iter().skip(start));
        }
    }

    messages.push(Message::user(req.message.clone()));
    messages
}

/// Revisa si toca resumir el hilo y lo lanza en segundo plano (F5).
/// No bloquea la respuesta: el resumen estará en el siguiente turno.
fn maybe_summarize(state: &AppState, session_id: &str) {
    let enabled = state
        .config
        .try_read()
        .map(|c| c.memory.auto_summarize)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let (count, summarized) = {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let count = sessions.count_messages(session_id).unwrap_or(0);
        let summarized = sessions
            .get_summary(session_id)
            .map(|(_, c)| c)
            .unwrap_or(0);
        (count, summarized)
    };
    if !crate::backend::session_manager::should_summarize(count, summarized) {
        return;
    }
    let state2 = state.clone();
    let sid = session_id.to_string();
    tokio::spawn(async move {
        if let Err(e) = summarize_session(&state2, &sid).await {
            log::warn!("Resumen automático falló: {e}");
        }
    });
}

/// Resume el tramo no resumido (dejando los últimos 20 intactos) y lo guarda.
async fn summarize_session(state: &AppState, session_id: &str) -> anyhow::Result<()> {
    let (prev_summary, prev_count, chunk) = {
        let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let (prev, prev_count) = sessions.get_summary(session_id).unwrap_or_default();
        let all = sessions.get_messages(session_id).unwrap_or_default();
        // Dejar los últimos 20 mensajes fuera del resumen (contexto fresco).
        let end = all.len().saturating_sub(20);
        let start = (prev_count as usize).min(end);
        let mut text = String::new();
        for m in &all[start..end] {
            let (role, content) = match m {
                Message::System { content } => ("sistema", content.as_str()),
                Message::User { content } => ("usuario", content.as_str()),
                Message::Assistant { content, .. } => ("asistente", content.as_str()),
                Message::Tool { content, .. } => ("herramienta", content.as_str()),
            };
            // Envolver outputs (ya vienen envueltos) y recortar por mensaje.
            let short: String = content.chars().take(600).collect();
            text.push_str(&format!("[{role}] {short}\n"));
            if text.len() > 12000 {
                text.push_str("…[truncado]");
                break;
            }
        }
        (prev, prev_count, text)
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
            tool_call_id,
            content,
            image_url,
        } => (
            "tool_result",
            serde_json::json!({ "tool_call_id": tool_call_id, "content": content, "image_url": image_url }).to_string(),
        ),
        StreamEvent::ToolApprovalNeeded {
            tool_call_id,
            name,
            arguments,
            permission,
        } => (
            "approval_needed",
            serde_json::json!({ "tool_call_id": tool_call_id, "name": name, "arguments": arguments, "permission": permission }).to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::ai_service::AiService;
    use crate::backend::chime_player::ChimePlayer;
    use crate::backend::session_manager::SessionManager;
    use crate::backend::speech_service::SpeechService;
    use crate::backend::tool_executor::ToolExecutor;
    use crate::backend::voice_pipeline::VoicePipeline;

    async fn test_state() -> AppState {
        let cfg = Arc::new(RwLock::new(Config::default()));
        let sessions = Arc::new(Mutex::new(SessionManager::in_memory_for_tests().unwrap()));
        let ai = Arc::new(AiService::new(cfg.clone()).await.unwrap());
        let tools = Arc::new(ToolExecutor::new(cfg.clone()));
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
            ai,
            tools,
            approvals,
            config: cfg,
            sessions,
            speech,
            voice,
            agent_events: tokio::sync::broadcast::channel(8).0,
            local_token: "test-token".into(),
            chat_tasks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dictation_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            sidecar_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
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
    async fn dictate_start_conflicts_with_voice_pipeline() {
        let state = test_state().await;
        state.voice.start_recording(48000);
        let resp = dictate_start(State(state.clone())).await.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        state.voice.stop_recording();
    }

    /// Dictado: stop sin dictado activo -> 409 Conflict.
    #[tokio::test]
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
