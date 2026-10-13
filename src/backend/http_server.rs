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
        || path.starts_with("/api/speak")
        || path.starts_with("/api/audio/")
        || (path.starts_with("/api/voice/")
            && !matches!(
                path.as_str(),
                "/api/voice/stream" | "/api/voice/last" | "/api/voice/session"
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

/// Configuración sin secretos: la UI recibe presencia y estado, nunca claves guardadas.
async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    let mut public = serde_json::to_value(&cfg).unwrap_or_default();
    public["ai"]["api_key"] = serde_json::json!("");
    public["ai"]["provider_keys"] = serde_json::json!({});
    public["ai"]["has_api_key"] = serde_json::json!(!cfg.ai.effective_api_key().trim().is_empty());
    public["ai"]["configured_provider_keys"] =
        serde_json::json!(cfg.ai.provider_keys.keys().collect::<Vec<_>>());
    public["capabilities"] = serde_json::json!({
        "text": "ready", "voice": if state.voice_enabled.load(std::sync::atomic::Ordering::Relaxed) { "active" } else { "deferred" },
        "whisper_model_present": state.speech.stt_model_exists(),
        "piper_binary_present": crate::backend::tts::piper::PiperEngine::find_binary().is_ok(),
        "microphone": "not_initialized_by_text_startup",
        "onnx_runtime": "deferred",
        "wakeword": "deferred",
        "piper_initialized": state.speech.tts_initialized(),
        "downloads_on_startup": false,
        "detail": "Voz opcional diferida; no se carga Piper, ONNX ni micrófono al iniciar texto"
    });
    Json(public).into_response()
}

/// Campos de credenciales vacíos de la vista redactada conservan claves previas.
/// Sólo entradas no vacías sustituyen una clave; no hay borrado implícito.
async fn update_config(
    State(state): State<AppState>,
    Json(mut new_cfg): Json<Config>,
) -> impl IntoResponse {
    let mut cfg = state.config.write().await;
    let mut keys = cfg.ai.provider_keys.clone();
    if keys
        .get(cfg.ai.provider_id())
        .is_none_or(|key| key.trim().is_empty())
        && !cfg.ai.api_key.trim().is_empty()
    {
        keys.insert(cfg.ai.provider_id().to_owned(), cfg.ai.api_key.clone());
    }
    for (provider, key) in &new_cfg.ai.provider_keys {
        if !key.trim().is_empty() {
            keys.insert(provider.clone(), key.clone());
        }
    }
    if !new_cfg.ai.api_key.trim().is_empty() {
        keys.insert(
            new_cfg.ai.provider_id().to_owned(),
            new_cfg.ai.api_key.clone(),
        );
    }
    new_cfg.ai.provider_keys = keys;
    if new_cfg.ai.api_key.trim().is_empty() {
        // La vista redactada no autoriza borrar/reemplazar un valor legacy,
        // incluso si el mapa activo contiene otra clave o cambia el proveedor.
        new_cfg.ai.api_key = cfg.ai.api_key.clone();
    }
    if let Err(e) = new_cfg.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }
    match new_cfg.save().await {
        Ok(()) => {
            *cfg = new_cfg;
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
        }
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

    let mut selected_ai = cfg.ai.clone();
    if selected_ai.provider_id() != provider_id {
        selected_ai.api_key.clear();
        selected_ai.base_url.clear();
    }
    selected_ai.provider = provider_id.clone();
    let base_url = match body.base_url.as_deref().map(str::trim) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => selected_ai.effective_base_url(),
    };
    let api_key = match body.api_key.as_deref().map(str::trim) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => selected_ai.effective_api_key(),
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
            if cfg.speech.auto_speak
                && state
                    .voice_enabled
                    .load(std::sync::atomic::Ordering::Relaxed)
                && !outcome.response.trim().is_empty()
            {
                let speech = owned
                    .dispatch(state.speech.speak(&outcome.response))
                    .await?;
                if let Err(error) = speech {
                    log::warn!("TTS de respuesta escrita: {error}");
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
            Ok::<_, anyhow::Error>(outcome.response)
        };
        // A closed HTTP body cancels even before provider headers/first token.
        // Do not drop work here: a dispatched tool must finish before terminal.
        let disconnect = async {
            out_tx.closed().await;
            owned.cancel();
            std::future::pending::<()>().await
        };
        let result = tokio::select! {
            result = std::panic::AssertUnwindSafe(work).catch_unwind() => result.unwrap_or_else(|_|Err(anyhow::anyhow!("supervisor_panic"))),
            _ = disconnect => unreachable!(),
        };
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
