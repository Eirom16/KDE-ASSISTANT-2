//! Backend de KDE Assistant v2
//!
//! Coordina todos los servicios: AI, sesiones, voz, hotword, KDE integration.

pub mod ai_service;
pub mod audio_capture;
pub mod chime_player;
pub mod hotword;
pub mod kde_integration;
pub mod session_manager;
pub mod speech_service;
pub mod tool_executor;
pub mod tool_registry;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::models::Config;

/// Estructura principal del backend que coordina todos los servicios.
pub struct Backend {
    pub config: Arc<RwLock<Config>>,
    pub ai: Arc<ai_service::AiService>,
    pub sessions: Arc<session_manager::SessionManager>,
    pub tools: Arc<tool_executor::ToolExecutor>,
    pub speech: Arc<speech_service::SpeechService>,
    pub audio: Arc<RwLock<Option<audio_capture::AudioCapture>>>,
    pub chimes: Arc<chime_player::ChimePlayer>,
    pub hotword: Arc<hotword::HotwordDetector>,
    pub kde: Arc<kde_integration::KdeIntegration>,
}

impl Backend {
    pub async fn new() -> Result<Self> {
        log::info!("Inicializando Backend...");

        let config = Config::load().await?;
        let config = Arc::new(RwLock::new(config));
        log::info!("Config cargada desde ~/.config/kde-assistant/config.json");

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

    /// Inicia captura de audio + deteccion de hotword en background.
    /// Retorna un receiver de eventos HotwordEvent.
    pub async fn start_voice_pipeline(&self) -> Result<mpsc::Receiver<hotword::HotwordEvent>> {
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(64);
        let (event_tx, event_rx) = mpsc::channel::<hotword::HotwordEvent>(8);

        // Crear y arrancar audio capture
        let mut cap = audio_capture::AudioCapture::new()?;
        let meter = cap.meter.clone();
        cap.start(move |frame: &[f32]| {
            // Reenviar al hotword detector
            let frame_vec = frame.to_vec();
            let tx = audio_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(frame_vec).await;
            });
        })?;
        *self.audio.write().await = Some(cap);

        log::info!("Voice pipeline started (meter: {})", meter.get());

        // Arrancar hotword
        self.hotword.start(audio_rx, event_tx).await?;

        Ok(event_rx)
    }

    /// Detiene captura de audio.
    pub async fn stop_voice_pipeline(&self) {
        if let Some(mut cap) = self.audio.write().await.take() {
            cap.stop();
        }
        self.hotword.stop();
    }
}
