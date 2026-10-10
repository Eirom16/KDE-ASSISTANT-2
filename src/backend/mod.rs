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
pub mod hotkey_listener;
pub mod hotword;
pub mod http_server;
pub mod kde_integration;
pub mod model_downloader;
pub mod operation_journal;
pub(crate) mod private_files;
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
pub mod voice_pipeline;
pub mod voice_state;
pub mod wakeword_ml;

use anyhow::Result;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};

use crate::models::Config;

/// Estructura principal del backend que coordina todos los servicios.
pub struct Backend {
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
        let sessions = Arc::new(Mutex::new(session_manager::SessionManager::new().await?));
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

        Ok(Self {
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

    /// Procesa un utterance de audio grabado (STT -> LLM -> TTS).
    /// Retorna (transcript, response).
    pub async fn process_voice(&self, audio: &[f32]) -> Result<(String, String)> {
        self.voice.process_utterance(audio, None).await
    }
    /// Construye el estado compartido para el servidor HTTP local.
    pub fn http_state(&self) -> Result<http_server::AppState> {
        let local_token = crate::backend::auth::ensure_server_token()?;
        Ok(http_server::AppState {
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

    /// Inicia captura de audio + deteccion de hotword en background.
    /// La misma captura alimenta tanto el detector de wake word como
    /// el buffer de grabacion del VoicePipeline (para PTT/wake word).
    /// Retorna un receiver de eventos HotwordEvent.
    pub async fn start_voice_pipeline(&self) -> Result<mpsc::Receiver<hotword::HotwordEvent>> {
        anyhow::ensure!(
            self.speech.is_available().await && self.speech.stt_model_exists(),
            "Voz no disponible: faltan Piper o modelos locales; no se descargan automáticamente"
        );
        self.hotword.try_load_ml();
        anyhow::ensure!(
            self.hotword.ml_active(),
            "Wake word no disponible: faltan modelos/runtime ONNX; captura desactivada"
        );
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(64);
        let (event_tx, event_rx) = mpsc::channel::<hotword::HotwordEvent>(8);

        let voice = self.voice.clone();

        // Micrófono preferido (F3-3, "" = por defecto).
        let mic_device = self.config.read().await.speech.mic_device.clone();
        let mic_opt = if mic_device.trim().is_empty() {
            None
        } else {
            Some(mic_device.as_str())
        };
        // Crear y arrancar audio capture
        let mut cap = audio_capture::AudioCapture::new()?;
        let dropped = self.hotword.dropped_frames.clone();
        cap.start(mic_opt, move |frame: &[f32]| {
            // 1) Alimentar al detector de wake word (try_send: no bloquea el thread de audio)
            let frame_vec = frame.to_vec();
            if audio_tx.try_send(frame_vec).is_err() {
                // Canal lleno: se cuenta (el loop ML lo reporta en su telemetria)
                dropped.fetch_add(1, Ordering::Relaxed);
            }
            // 2) Alimentar al buffer de grabacion (solo si esta grabando)
            voice.push_audio(frame);
        })?;

        // Registrar sample rate de la captura en el buffer de grabacion
        // y en el detector de wake word (para remuestrear a 16kHz)
        let sr = cap.sample_rate;
        self.voice
            .buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sample_rate
            .store(sr, Ordering::SeqCst);
        self.hotword.set_sample_rate(sr);

        *self.audio.write().await = Some(cap);

        log::info!("Voice pipeline started (sample_rate={sr})");

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

    /// Inicia la grabacion de voz (para wake word o PTT).
    pub fn start_listening(&self) {
        // La sample rate ya esta registrada por start_voice_pipeline
        let sr = self
            .voice
            .buffer
            .lock()
            .unwrap()
            .sample_rate
            .load(Ordering::SeqCst);
        self.voice.start_recording(sr.max(1));
        log::info!("Escuchando... (grabacion iniciada)");
    }

    /// Detiene la grabacion y procesa el utterance (STT -> LLM -> TTS).
    /// Retorna (transcript, response).
    pub async fn stop_listening_and_process(&self) -> Result<(String, String)> {
        let audio = self.voice.stop_recording();
        if audio.is_empty() {
            log::info!("Grabacion vacia, omitiendo procesamiento");
            return Ok((String::new(), String::new()));
        }
        log::info!("Procesando grabacion de {} samples", audio.len());
        self.voice.process_utterance(&audio, None).await
    }
}
