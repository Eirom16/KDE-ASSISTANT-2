//! Backend de KDE Assistant v2
//!
//! Coordina todos los servicios: AI, sesiones, voz, hotword, KDE integration.
//! En Fase 1 (scaffold) solo expone la estructura. En fases siguientes
//! cada subservicio se implementa progresivamente.

pub mod ai_service;
pub mod chime_player;
pub mod hotword;
pub mod kde_integration;
pub mod session_manager;
pub mod speech_service;
pub mod tool_executor;
pub mod tool_registry;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::Config;

/// Estructura principal del backend que coordina todos los servicios.
///
/// Cada subservicio se almacena detras de un `Arc<RwLock<...>>` para permitir
/// acceso concurrente desde multiples tasks de Tokio, mientras que el thread
/// de Qt accede a traves de signals/slots (no directamente).
pub struct Backend {
    pub config: Arc<RwLock<Config>>,
    pub ai: Arc<ai_service::AiService>,
    pub sessions: Arc<session_manager::SessionManager>,
    pub tools: Arc<tool_executor::ToolExecutor>,
    pub speech: Arc<speech_service::SpeechService>,
    pub audio: Arc<RwLock<Option<audio_capture_stub::AudioCaptureStub>>>,
    pub chimes: Arc<chime_player::ChimePlayer>,
    pub hotword: Arc<hotword::HotwordDetector>,
    pub kde: Arc<kde_integration::KdeIntegration>,
}

impl Backend {
    pub async fn new() -> Result<Self> {
        log::info!("Inicializando Backend...");

        // Cargar configuracion
        let config = Config::load().await?;
        let config = Arc::new(RwLock::new(config));

        log::info!("Config cargada desde ~/.config/kde-assistant/config.json");

        // Inicializar subservicios
        let ai = Arc::new(ai_service::AiService::new(config.clone()).await?);
        let sessions = Arc::new(session_manager::SessionManager::new().await?);
        let tools = Arc::new(tool_executor::ToolExecutor::new(config.clone()));
        let speech = Arc::new(speech_service::SpeechService::new(config.clone()).await?);
        let chimes = Arc::new(chime_player::ChimePlayer::new().await?);
        let hotword = Arc::new(hotword::HotwordDetector::new(config.clone()).await?);
        let kde = Arc::new(kde_integration::KdeIntegration::new());

        log::info!("Todos los subservicios inicializados");

        Ok(Self {
            config,
            ai,
            sessions,
            tools,
            speech,
            audio: Arc::new(RwLock::new(None)),
            chimes,
            hotword,
            kde,
        })
    }
}

/// Stub temporal para audio_capture mientras se integra cpal en Fase 5.
pub mod audio_capture_stub {
    use anyhow::Result;

    pub struct AudioCaptureStub;

    impl AudioCaptureStub {
        pub fn new() -> Result<Self> {
            Ok(Self)
        }
    }
}
