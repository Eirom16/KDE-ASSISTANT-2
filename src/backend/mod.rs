//! Backend de KDE Assistant v2
//!
//! Coordina todos los servicios: AI, sesiones, voz, hotword, KDE integration.

pub mod agent_activity;
pub mod ai_service;
pub mod approvals;
pub mod audio_capture;
pub mod auth;
pub mod chime_player;
pub mod conversation_turn;
pub mod document_suite;
pub mod global_shortcuts;
pub mod hotkey_listener;
pub mod hotword;
pub mod http_server;
pub mod kde_integration;
pub mod model_downloader;
pub mod operation_journal;
pub mod permission_engine;
mod piper_protocol;
mod piper_resource_watch;
pub(crate) mod private_files;
pub mod provider_connection;
mod safe_fs;
pub mod session_manager;
pub mod speech_service;
pub mod sse_parser;
pub mod stt;
pub mod stt_api;
pub mod tool_executor;
pub mod tool_registry;
pub mod tts;
pub mod tts_stream;
pub mod vad;
pub mod voice_coordinator;
pub mod voice_pcm;
pub mod voice_pipeline;
pub mod voice_process;
pub mod voice_state;
pub mod voice_stt;
pub mod voice_tts;
pub mod vosk;
pub mod wake_audio;
pub mod wakeword;
pub mod wakeword_ml;
pub mod warm_piper;

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::models::Config;

/// Estructura principal del backend que coordina todos los servicios.
pub struct Backend {
    pub desktop: Arc<crate::desktop::Desktop>,
    pub native_voice: Arc<voice_coordinator::VoiceCoordinator>,
    pub config: Arc<RwLock<Config>>,
    pub ai: Arc<ai_service::AiService>,
    pub sessions: Arc<Mutex<session_manager::SessionManager>>,
    pub tools: Arc<tool_executor::ToolExecutor>,
    pub approvals: Arc<approvals::ApprovalManager>,
    pub turns: Arc<conversation_turn::TurnRegistry>,
    pub speech: Arc<speech_service::SpeechService>,
    /// AudioCapture (cpal) no es Send/Sync en todas las plataformas. Este Arc
    /// nunca abandona el hilo del controlador: el servidor HTTP reenvia solo
    /// campos Send+Sync via `http_state()`.
    #[allow(clippy::arc_with_non_send_sync)]
    pub audio: Arc<RwLock<Option<audio_capture::AudioCapture>>>,
    pub chimes: Arc<chime_player::ChimePlayer>,
    pub hotword: Arc<hotword::HotwordDetector>,
    pub kde: Arc<kde_integration::KdeIntegration>,
    pub voice: Arc<voice_pipeline::VoicePipeline>,
    /// Actividad no vocal del asistente (chat escrito y herramientas) para
    /// que el personaje la pueda representar sin falsificar estados de voz.
    pub agent_events: tokio::sync::broadcast::Sender<String>,
    /// El sidecar de voz (Python+Pipecat) es dueño exclusivo del mic cuando
    /// está vivo. main.rs lo activa y lo pone en true; los handlers de voz
    /// forwardean al sidecar en vez de abrir el mic desde Rust.
    pub sidecar_active: Arc<std::sync::atomic::AtomicBool>,
}

impl Backend {
    // El campo `audio` (cpal) no es Send/Sync en todas las plataformas; ver el
    // comentario del campo. Solo el hilo controlador lo toca.
    #[allow(clippy::arc_with_non_send_sync)]
    pub async fn new() -> Result<Self> {
        log::info!("Inicializando Backend...");

        let config = Config::load().await?;
        let config = Arc::new(RwLock::new(config));
        log::info!("Config cargada desde ~/.config/kde-assistant/config.json");

        let ai = Arc::new(ai_service::AiService::new(config.clone()).await?);
        let manager = session_manager::SessionManager::new().await?;
        if let Err(error) = resource_cache::startup(&manager) {
            log::warn!("Owned cache maintenance unavailable: {error}");
        }
        let sessions = Arc::new(Mutex::new(manager));
        // F4-3: el executor audita en SQLite.
        let tools = Arc::new(tool_executor::ToolExecutor::with_audit(
            config.clone(),
            sessions.clone(),
        ));
        let approvals = Arc::new(approvals::ApprovalManager::new());
        let speech = Arc::new(speech_service::SpeechService::new(config.clone()).await?);
        let chimes = Arc::new(chime_player::ChimePlayer::new().await?);
        let hotword = Arc::new(hotword::HotwordDetector::new(config.clone()).await?);
        let kde = Arc::new(kde_integration::KdeIntegration::new());
        let voice = Arc::new(voice_pipeline::VoicePipeline::new(
            config.clone(),
            speech.clone(),
            ai.clone(),
            tools.clone(),
            approvals.clone(),
            sessions.clone(),
            chimes.clone(),
        ));
        let (agent_events, _) = tokio::sync::broadcast::channel(32);

        log::info!("Todos los subservicios inicializados");

        let native_voice = Arc::new(voice_coordinator::VoiceCoordinator::default());
        let speech_cfg = config.read().await.speech.clone();
        native_voice.configure(&speech_cfg).await;
        Ok(Self {
            desktop: Arc::new(crate::desktop::Desktop::default()),
            native_voice,
            config,
            ai,
            sessions,
            tools,
            approvals,
            turns: Arc::new(conversation_turn::TurnRegistry::default()),
            speech,
            audio: Arc::new(RwLock::new(None)),
            chimes,
            hotword,
            kde,
            voice,
            agent_events,
            sidecar_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Construye el estado compartido para el servidor HTTP local.
    pub fn http_state(&self) -> Result<http_server::AppState> {
        let local_token = crate::backend::auth::ensure_server_token()?;
        Ok(http_server::AppState {
            desktop: self.desktop.clone(),
            native_voice: self.native_voice.clone(),
            http_port: 8765,
            ai: self.ai.clone(),
            tools: self.tools.clone(),
            approvals: self.approvals.clone(),
            config: self.config.clone(),
            sessions: self.sessions.clone(),
            speech: self.speech.clone(),
            voice: self.voice.clone(),
            agent_events: self.agent_events.clone(),
            local_token,
            chat_tasks: self.turns.clone(),
            dictation_active: self.voice.dictation_flag(),
            sidecar_active: self.sidecar_active.clone(),
            voice_enabled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }
}

pub mod history_context;
pub mod resource_cache;
pub mod storage_retention;
