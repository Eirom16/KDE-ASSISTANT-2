//! Voice Pipeline - Orquesta el flujo completo de voz
//!
//! Flujo: Grabacion -> STT (whisper) -> LLM (agente) -> TTS (piper)
//!
//! Maneja:
//! - Buffer de grabacion compartido (para PTT y wake word)
//! - Resample a 16kHz antes de STT
//! - Respuesta del agente con tool calling
//! - Sintesis de voz (si auto_speak esta habilitado)
//!
//! Este modulo es el pegamento que une los subservicios de voz.

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::backend::ai_service::AiService;
use crate::backend::audio_capture::resample_to_16k;
use crate::backend::chime_player::ChimePlayer;
use crate::backend::speech_service::SpeechService;
use crate::backend::tool_executor::ToolExecutor;
use crate::backend::tool_registry;
use crate::backend::tts_stream::{plain_text_for_tts, SentenceChunker};
use crate::backend::vad::{VadConfig, VadEvent, VadState};
use crate::backend::voice_state::{VoiceErrorKind, VoiceState, VoiceStateMachine};
use crate::models::{Config, Message, StreamEvent};
use tokio::sync::RwLock;

/// Señal de voz para la UI (F3-1: push SSE en vez de polling de archivos).
#[derive(Debug, Clone)]
pub enum VoiceSignal {
    State { state: String },
    Level { level: f32 },
}

/// Buffer de grabacion compartida entre el thread de audio y el orquestador.
pub struct RecordingBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: Arc<AtomicU32>,
    pub recording: Arc<AtomicBool>,
}

impl Default for RecordingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingBuffer {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            sample_rate: Arc::new(AtomicU32::new(0)),
            recording: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.samples.extend_from_slice(samples);
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

pub struct VoicePipeline {
    pub config: Arc<RwLock<Config>>,
    pub speech: Arc<SpeechService>,
    pub ai: Arc<AiService>,
    pub tools: Arc<ToolExecutor>,
    pub approvals: Arc<crate::backend::approvals::ApprovalManager>,
    pub sessions: Arc<Mutex<crate::backend::session_manager::SessionManager>>,
    pub chimes: Arc<ChimePlayer>,
    pub buffer: Arc<Mutex<RecordingBuffer>>,
    last_exchange: Arc<Mutex<Option<VoiceExchange>>>,
    /// Sesión seleccionada en el chat. Es la fuente de contexto para voz y
    /// texto; si no hay una, se usa la sesión de voz histórica.
    active_session: Arc<Mutex<Option<String>>>,
    amplitude: Arc<AtomicU32>, // f32 bits 0..1
    speaking: Arc<AtomicBool>,
    barge_counter: Arc<AtomicU32>,
    /// Momento (ms epoch) en que empezó el TTS actual. La gracia de 300ms
    /// cubre el inicio (rodio abre el sink y el rodio de nivel tarda un
    /// tick en publicarse como referencia de eco en tts_now).
    speaking_since_ms: Arc<AtomicU32>,
    /// Bus push de estado/nivel para la UI (F3-1, SSE `/api/voice/stream`).
    signal_tx: broadcast::Sender<VoiceSignal>,
    /// Máquina de estados formal (plan §3). La UI recibe solo el mapeo
    /// legado a idle/listening/processing/speaking.
    machine: Mutex<VoiceStateMachine>,
    /// Dictado al input del chat en curso (lo marca http_server en
    /// `/api/dictate/*`). Mientras está activo el pipeline NO puede
    /// arrancar escucha: dictado y conversación comparten mic y buffer,
    /// y si ambos corren se pisan (bug reportado: "se invocan los dos").
    dictation: Arc<AtomicBool>,
    /// Throttle del nivel: último envío (ms epoch + valor).
    last_level_ms: Arc<AtomicU32>,
    last_level_sent: Arc<AtomicU32>, // f32 bits
}

/// Ultimo intercambio por voz (para que la UI lo muestre en el chat).
#[derive(Debug, Clone, serde::Serialize)]
pub struct VoiceExchange {
    pub transcript: String,
    pub response: String,
    /// Millis epoch en que termino el procesamiento.
    pub timestamp_ms: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Ventana de historial para voz (turnos que van al LLM). Más chica que la
/// del chat de texto: la voz es ráfaga, y cada turno refactura el historial
/// completo (con 40 mensajes se agotaba el TPM de Groq → 429).
const MAX_VOICE_HISTORY: usize = 12;

/// Recorta el historial a los últimos `max` mensajes, sin dejar pares
/// tool rotos: nada de `tool` huérfano al inicio ni `assistant` con
/// tool_calls sin sus respuestas al final (Groq los rechaza con 400).
fn trim_voice_history(history: Vec<Message>, max: usize) -> Vec<Message> {
    let start = history.len().saturating_sub(max);
    let mut slice: Vec<Message> = history.into_iter().skip(start).collect();
    while matches!(slice.first(), Some(Message::Tool { .. })) {
        slice.remove(0);
    }
    while matches!(
        slice.last(),
        Some(Message::Assistant { tool_calls, .. }) if !tool_calls.is_empty()
    ) {
        slice.pop();
    }
    slice
}

impl VoicePipeline {
    pub fn new(
        config: Arc<RwLock<Config>>,
        speech: Arc<SpeechService>,
        ai: Arc<AiService>,
        tools: Arc<ToolExecutor>,
        approvals: Arc<crate::backend::approvals::ApprovalManager>,
        sessions: Arc<Mutex<crate::backend::session_manager::SessionManager>>,
        chimes: Arc<ChimePlayer>,
    ) -> Self {
        Self {
            config,
            speech,
            ai,
            tools,
            approvals,
            sessions,
            chimes,
            buffer: Arc::new(Mutex::new(RecordingBuffer::new())),
            last_exchange: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
            amplitude: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            speaking: Arc::new(AtomicBool::new(false)),
            barge_counter: Arc::new(AtomicU32::new(0)),
            speaking_since_ms: Arc::new(AtomicU32::new(0)),
            signal_tx: broadcast::channel(64).0,
            machine: Mutex::new(VoiceStateMachine::new()),
            dictation: Arc::new(AtomicBool::new(false)),
            last_level_ms: Arc::new(AtomicU32::new(0)),
            last_level_sent: Arc::new(AtomicU32::new(0.0f32.to_bits())),
        }
    }

    /// Nivel para la UI cuando el mic lo posee el sidecar Python.
    pub fn emit_level_from_sidecar(&self, level: f32) {
        let _ = self.signal_tx.send(VoiceSignal::Level {
            level: level.clamp(0.0, 1.0),
        });
    }

    /// Guarda el intercambio (transcript/response) que viene del sidecar,
    /// disponible vía /api/voice/last.
    pub fn set_last_exchange_from_sidecar(&self, transcript: String, response: String) {
        *self.last_exchange.lock().unwrap_or_else(|e| e.into_inner()) = Some(VoiceExchange {
            transcript,
            response,
            timestamp_ms: now_ms(),
        });
    }

    pub fn set_active_session(&self, session_id: Option<String>) {
        *self
            .active_session
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = session_id;
    }

    /// Suscribe un receptor de señales de voz (para `/api/voice/stream`).
    pub fn subscribe(&self) -> broadcast::Receiver<VoiceSignal> {
        self.signal_tx.subscribe()
    }

    /// Último estado emitido, mapeado al formato legado de la UI
    /// ("idle" si aún ninguno). La UI filtra estados desconocidos.
    pub fn current_state(&self) -> String {
        self.machine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state()
            .ui_label()
            .to_string()
    }

    /// Estado formal actual (para lógica interna; no para la UI).
    pub fn state(&self) -> VoiceState {
        self.machine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state()
    }

    /// Handle compartido del flag de dictado (para http_server).
    pub fn dictation_flag(&self) -> Arc<AtomicBool> {
        self.dictation.clone()
    }

    /// ¿Hay un dictado al input del chat en curso?
    pub fn is_dictating(&self) -> bool {
        self.dictation.load(Ordering::Relaxed)
    }

    /// ¿Está el pipeline en reposo (apto para que el wake word abra turno)?
    /// El detector ML llama a esto por frame: mientras escuchamos, pensamos
    /// o hablamos, el wake word se ignora (si no, un "hey jarvis" repetido
    /// pisa el turno en curso — el audio que seguía se perdía).
    pub fn wake_gate_open(&self) -> bool {
        self.state() == VoiceState::Idle
    }

    pub fn amplitude(&self) -> f32 {
        f32::from_bits(self.amplitude.load(Ordering::Relaxed))
    }

    pub fn is_speaking(&self) -> bool {
        self.speaking.load(Ordering::Relaxed)
    }

    /// Barge-in silencioso: interrumpe TTS y reinicia escucha sin chime.
    /// Usado tanto por deteccion de voz (amplitud) como por hotkey/HTTP.
    /// Transición formal: RESPONDING -> INTERRUPTED -> LISTENING (plan §12).
    pub fn barge_in_silent(&self) {
        if !self.speaking.load(Ordering::Relaxed) {
            return;
        }
        self.speech.request_stop();
        self.speaking.store(false, Ordering::Relaxed);
        self.barge_counter.store(0, Ordering::Relaxed);
        {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.clear();
            if buf.sample_rate.load(Ordering::Relaxed) == 0 {
                buf.sample_rate.store(48000, Ordering::Relaxed);
            }
            buf.recording.store(true, Ordering::Relaxed);
        }
        if self.state() == VoiceState::Responding {
            self.emit_state(VoiceState::Interrupted);
        }
        self.emit_state(VoiceState::Listening);
        log::info!("Barge-in: TTS interrumpido, escucha reiniciada");
    }

    /// Inicia una grabacion (limpia el buffer y marca recording=true).
    pub fn start_recording(&self, sample_rate: u32) {
        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.clear();
        buf.sample_rate.store(sample_rate, Ordering::Relaxed);
        buf.recording.store(true, Ordering::Relaxed);
    }

    /// Anade samples al buffer de grabacion y actualiza el nivel para el orbe.
    /// Si esta hablando (`speaking`), detecta barge-in por amplitud.
    pub fn push_audio(&self, samples: &[f32]) {
        // Nivel para el orbe (siempre, aunque no se este grabando, para preview)
        let mut lvl_opt: Option<f32> = None;
        if !samples.is_empty() {
            let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
            let lvl = (rms * 6.0).min(1.0);
            self.amplitude.store(lvl.to_bits(), Ordering::Relaxed);
            self.emit_level(lvl);
            lvl_opt = Some(lvl);
        }

        // No hay cancelación acústica de eco en esta captura. Inferir un
        // barge-in solo con RMS hacía que Piper se oyera a sí mismo y abriese
        // falsos turnos (por ejemplo, transcritos como "Gracias"). La
        // interrupción explícita por Escape/botón Stop se conserva en
        // `barge_in_silent`; el automático volverá cuando haya AEC/VAD real.
        let _ = lvl_opt;

        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        if buf.recording.load(Ordering::Relaxed) {
            buf.push(samples);
        }
    }

    /// Emite el nivel a la UI (SSE + archivo compat). Throttle: 100ms o salto >0.08.
    fn emit_level(&self, level: f32) {
        let level = level.clamp(0.0, 1.0);
        let now = now_ms() as u32;
        let last_ms = self.last_level_ms.load(Ordering::Relaxed);
        let prev = f32::from_bits(self.last_level_sent.load(Ordering::Relaxed));
        if now.saturating_sub(last_ms) < 100 && (level - prev).abs() <= 0.08 {
            return;
        }
        self.last_level_ms.store(now, Ordering::Relaxed);
        self.last_level_sent
            .store(level.to_bits(), Ordering::Relaxed);
        let cache_dir = match dirs::cache_dir() {
            Some(d) => d.join("kde-assistant"),
            None => return,
        };
        let _ = std::fs::create_dir_all(&cache_dir);
        let path = cache_dir.join("voice.level");
        let _ = std::fs::write(&path, format!("{level:.3}"));
        let _ = self.signal_tx.send(VoiceSignal::Level { level });
    }

    /// Detiene la grabacion y retorna los samples (a 16kHz mono).
    pub fn stop_recording(&self) -> Vec<f32> {
        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.recording.store(false, Ordering::Relaxed);
        let src_rate = buf.sample_rate.load(Ordering::Relaxed);
        let samples = std::mem::take(&mut buf.samples);
        // Resample a 16kHz para whisper
        resample_to_16k(&samples, src_rate.max(1))
    }

    /// Verifica si esta grabando.
    pub fn is_recording(&self) -> bool {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .recording
            .load(Ordering::Relaxed)
    }

    /// Longitud actual de la grabacion en samples.
    pub fn recording_len(&self) -> usize {
        self.buffer.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Sample rate de la captura actual (0 si no hay).
    pub fn sample_rate(&self) -> u32 {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sample_rate
            .load(Ordering::Relaxed)
    }

    /// RMS de los ultimos `n` samples grabados (0.0 si no hay).
    /// Sirve para detectar silencio y cortar la grabacion antes del maximo.
    pub fn recent_rms(&self, n: usize) -> f32 {
        let buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        if buf.samples.is_empty() {
            return 0.0;
        }
        let start = buf.samples.len().saturating_sub(n);
        let slice = &buf.samples[start..];
        (slice.iter().map(|s| s * s).sum::<f32>() / slice.len() as f32).sqrt()
    }

    /// Inicia la grabacion y marca el buffer como dueño del turno.
    /// No hace nada si hay un dictado al chat en curso (comparten micro).
    pub fn start_listening(&self) {
        if self.is_dictating() {
            log::info!("[VOICE] start_listening ignorado: dictado al chat en curso");
            return;
        }
        let sr = self
            .buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sample_rate
            .load(Ordering::SeqCst);
        self.start_recording(sr.max(1));
        self.emit_state(VoiceState::Listening);
        log::info!("Escuchando... (grabacion iniciada)");
    }

    /// Marca el inicio de un tramo de habla (estado + flags de barge-in).
    /// `speaking_since` arranca la gracia de 300ms del barge-in; el nivel
    /// de eco real se lee luego de `speech.tts_playback_level()`.
    fn begin_speaking(&self) {
        self.emit_state(VoiceState::Responding);
        self.speaking.store(true, Ordering::Relaxed);
        self.speaking_since_ms
            .store(now_ms() as u32, Ordering::Relaxed);
        self.barge_counter.store(0, Ordering::Relaxed);
    }

    /// Cierra el tramo de habla (el estado de salida lo decide el llamador).
    fn end_speaking(&self) {
        self.speaking.store(false, Ordering::Relaxed);
        self.barge_counter.store(0, Ordering::Relaxed);
    }

    /// Emite el estado de voz a la UI (SSE + archivo compat) a través de la
    /// máquina de estados formal. La UI recibe el mapeo legado:
    /// idle | listening | processing | speaking.
    /// El archivo se conserva para debug/fallback; la UI usa el SSE.
    pub fn emit_state(&self, state: VoiceState) {
        let ui = {
            let mut m = self.machine.lock().unwrap_or_else(|e| e.into_inner());
            m.transition(state).ui_label().to_string()
        };
        let cache_dir = match dirs::cache_dir() {
            Some(d) => d.join("kde-assistant"),
            None => return,
        };
        let _ = std::fs::create_dir_all(&cache_dir);
        let path = cache_dir.join("voice.state");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let content = format!("{ui}|{timestamp}");
        let _ = std::fs::write(&path, content);
        let _ = self.signal_tx.send(VoiceSignal::State { state: ui });
    }

    /// Detiene la grabacion y procesa el utterance (STT -> LLM -> TTS).
    /// Retorna (transcript, response).
    /// No interviene si hay un dictado al chat en curso: el audio del buffer
    /// pertenece al dictado (PTT-end no debe robarlo).
    pub async fn stop_and_process(&self) -> Result<(String, String)> {
        if self.is_dictating() {
            log::info!("[VOICE] stop_and_process ignorado: dictado al chat en curso");
            return Ok((String::new(), String::new()));
        }
        let audio = self.stop_recording();
        if audio.is_empty() {
            log::info!("Grabacion vacia, omitiendo procesamiento");
            return Ok((String::new(), String::new()));
        }
        log::info!("Procesando grabacion de {} samples", audio.len());
        self.process_utterance(&audio, None).await
    }

    /// Ultimo intercambio completado (para la UI via /api/voice/last).
    pub fn last_exchange(&self) -> Option<VoiceExchange> {
        self.last_exchange
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Graba hasta que el VAD detecta fin de turno o se agota `max_secs`.
    /// Asume `start_listening()` ya llamado. Detiene la grabación y devuelve
    /// el audio a 16kHz.
    ///
    /// VAD (plan §6): silencio sostenido configurable (`vad_silence_ms`),
    /// mínimo grabado (`vad_min_record_ms`) y piso de ruido adaptativo
    /// (`vad_adaptive`): ya no depende de un timeout fijo ni de un umbral
    /// rígido, tolera pausas naturales y no se cuelga en ambientes ruidosos.
    pub async fn record_until_silence(&self, max_secs: u64) -> Vec<f32> {
        const POLL_MS: u64 = 100;
        const RMS_WINDOW_SECS: f32 = 0.25;
        let vad_cfg = VadConfig::from(&self.config.read().await.speech);
        let mut vad = VadState::new(vad_cfg);
        let mut elapsed_ms = 0u64;
        // Telemetria: diagnostico del endpointing (por que no corto, etc.)
        let mut rms_log: Vec<f32> = Vec::new();
        let mut end_reason = "timeout";
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
            elapsed_ms += POLL_MS;
            let sr = self.sample_rate().max(1) as f32;
            let recorded_ms = (self.recording_len() as f32 / sr * 1000.0) as u64;
            let rms = self.recent_rms((sr * RMS_WINDOW_SECS) as usize);
            rms_log.push(rms);
            let reason = vad.push(rms, recorded_ms, POLL_MS);
            if reason == VadEvent::TurnEnded {
                end_reason = "silencio";
                log::info!(
                    "VAD: fin de turno ({}ms grabados, umbral {:.3})",
                    recorded_ms,
                    vad.silence_threshold()
                );
                break;
            }
            if elapsed_ms >= max_secs * 1000 {
                break;
            }
        }
        if !rms_log.is_empty() {
            let mut sorted = rms_log.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p = |q: f32| sorted[(sorted.len() - 1).min((sorted.len() as f32 * q) as usize)];
            log::info!(
                "VAD fin ({end_reason}): {}ms, rms p10={:.3} p50={:.3} p90={:.3}, umbral {:.3}{}",
                elapsed_ms,
                p(0.10),
                p(0.50),
                p(0.90),
                vad.silence_threshold(),
                if end_reason == "timeout" {
                    " (revisar si el piso de ruido supera al umbral)"
                } else {
                    ""
                }
            );
        }
        self.stop_recording()
    }

    /// Espera voz real hasta `window_secs` (para conversación continua F3-2).
    /// Detecta nivel alto 3 polls seguidos (~300ms) con variación (evita
    /// disparar con un nivel congelado si no hay frames del mic).
    /// Umbral RELATIVO al ambiente al entrar: con el absoluto 0.35 bastaba
    /// ruido de sala (lvl~0.54) para hackear turnos fantasma.
    /// Si el usuario empieza un dictado al chat a mitad de la espera,
    /// devuelve false de inmediato: el micro queda para el dictado.
    pub async fn wait_for_speech(&self, window_secs: u64) -> bool {
        const POLL_MS: u64 = 100;
        const NEEDED: u32 = 3;
        let start_lvl = self.amplitude();
        // Umbral: ambiente + colchón suave. Con +0.20 el usuario tenía que
        // gritar en cuartos ruidosos (ambiente ~0.53 → umbral ~0.73). Con
        // auriculares, el ambiente es bajo y el piso es 0.50.
        let threshold = (start_lvl + 0.12).clamp(0.50, 0.85);
        let mut hot = 0u32;
        let mut varied = false;
        let polls = window_secs.saturating_mul(1000) / POLL_MS;
        for _ in 0..polls {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
            if self.is_dictating() {
                return false;
            }
            let lvl = self.amplitude();
            if (lvl - start_lvl).abs() > 0.02 {
                varied = true;
            }
            if lvl > threshold {
                hot += 1;
                if hot >= NEEDED && varied {
                    return true;
                }
            } else if lvl < 0.35 {
                hot = 0;
            }
        }
        false
    }

    /// Cierra una conversación por completo: vuelve a reposo SIEMPRE.
    /// Si una escucha quedó abierta (barge-in cuyo turno no llegó),
    /// descarta ese audio. Sin esto, el wake gate quedaba cerrado para
    /// siempre tras ciertos barge-ins y el usuario oía
    /// "pipeline ocupado" en cada "hey jarvis".
    pub fn end_conversation(&self) {
        // Solo cortamos grabación propia de voz; la del dictado al chat es
        // de otro dueño y vive su propio ciclo (stops vía API).
        if self.is_recording() && !self.is_dictating() {
            let _ = self.stop_recording(); // descartar audio colgado
        }
        self.end_speaking();
        self.emit_state(VoiceState::Idle);
    }

    /// Turno completo de voz: grabar → procesar (STT→LLM→TTS).
    /// Si `auto_listen` está activo y el turno aportó transcript, encadena
    /// hasta `max_extra` turnos más esperando voz en la ventana configurada.
    /// Retorna todos los `(transcript, response)` del hilo.
    pub async fn converse_voice_driven(
        &self,
        max_secs: u64,
        max_extra: u32,
    ) -> Vec<(String, String)> {
        let turns = self.converse_voice_driven_inner(max_secs, max_extra).await;
        // Regla dura: al terminar la conversación, el pipeline vuelve a
        // reposo SIEMPRE (de lo contrario el wake word queda bloqueado).
        self.end_conversation();
        turns
    }

    async fn converse_voice_driven_inner(
        &self,
        max_secs: u64,
        max_extra: u32,
    ) -> Vec<(String, String)> {
        let mut turns = Vec::new();
        // El wake word puede saltar durante un dictado al chat: el micro
        // es del dictado; este turno de voz se descarta entero.
        if self.is_dictating() {
            log::info!("[VOICE] turno wake ignorado: dictado al chat en curso");
            return turns;
        }
        // Primer turno (el llamador ya hizo start_listening tras el saludo).
        let audio = self.record_until_silence(max_secs).await;
        if audio.is_empty() {
            log::info!("Grabacion vacia, omitiendo procesamiento");
            return turns;
        }
        match self.process_utterance(&audio, None).await {
            Ok((t, r)) => {
                if t.is_empty() || t == "(inaudible)" {
                    return turns;
                }
                turns.push((t, r));
            }
            Err(e) => {
                log::warn!("Procesamiento de voz fallo: {e}");
                return turns;
            }
        }
        // Turnos encadenados (F3-2).
        turns.extend(self.continue_conversation(max_secs, max_extra).await);
        turns
    }

    /// Encadena turnos manos-libres tras un primer turno (F3-2).
    /// Solo actúa si `auto_listen` está activo. Retorna los turnos extra.
    ///
    /// Barge-in (plan §12): si el usuario interrumpió el TTS, la escucha ya
    /// está activa (reiniciada por `barge_in_silent`); ese audio se procesa
    /// como turno nuevo en vez de descartarse.
    pub async fn continue_conversation(
        &self,
        max_secs: u64,
        max_extra: u32,
    ) -> Vec<(String, String)> {
        let max_secs = max_secs.clamp(1, 30);
        let mut extra = Vec::new();
        for _ in 0..max_extra {
            let (enabled, window) = {
                let cfg = self.config.read().await;
                (cfg.speech.auto_listen, cfg.speech.listen_window_secs.max(2))
            };
            if !enabled {
                break;
            }
            if self.is_dictating() {
                // El dictado al chat tiene prioridad exclusiva sobre el
                // micro: no confundir su grabación con un turno de voz.
                break;
            }
            if self.is_recording() {
                // Barge-in: la escucha ya está activa (la reinició
                // `barge_in_silent`); su audio es el turno nuevo.
                log::info!("[VOICE] turno por barge-in: procesando audio ya capturado");
            } else {
                log::info!("Escucha continua: esperando voz ({window}s)...");
                if !self.wait_for_speech(window).await {
                    break;
                }
                self.start_listening(); // no-op defensivo si ganó un dictado
            }
            let audio = self.record_until_silence(max_secs).await;
            if audio.is_empty() {
                break;
            }
            match self.process_utterance(&audio, None).await {
                Ok((t, r)) => {
                    if t.is_empty() || t == "(inaudible)" {
                        break;
                    }
                    extra.push((t, r));
                }
                Err(e) => {
                    log::warn!("Procesamiento de voz fallo: {e}");
                    break;
                }
            }
        }
        // La cola gestionada también tiene que terminar en reposo: algunos
        // caminos del loop (barge-in procesado, escucha que expiró) podían
        // dejar el estado en Listening y bloquear el wake word.
        self.end_conversation();
        extra
    }

    /// Procesa un utterance completo: STT -> LLM -> TTS.
    ///
    /// Retorna (transcript, response). Emite eventos por el canal `tx`.
    /// Si hubo barge-in (recording=true), mantiene `listening` en vez de `idle`.
    pub async fn process_utterance(
        &self,
        audio: &[f32],
        tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<(String, String)> {
        let op = self
            .machine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .begin_operation();
        log::info!("[VOICE] operación #{op}: procesando utterance");
        let r = self.process_utterance_inner(audio, tx).await;
        // Guardar ANTES de marcar idle: la UI lee el intercambio al ver idle.
        if let Ok((t, r)) = &r {
            if !t.is_empty() {
                *self.last_exchange.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(VoiceExchange {
                        transcript: t.clone(),
                        response: r.clone(),
                        timestamp_ms: now_ms(),
                    });
                self.persist_exchange(t, r);
            }
        }
        // Si hay barge-in activo (escucha reiniciada), no pisar listening
        let is_listening = self
            .buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .recording
            .load(Ordering::Relaxed);
        if !is_listening {
            self.emit_state(VoiceState::Idle);
        }
        r
    }

    /// Persiste cada turno antes de volver a escuchar. Así un follow-up por
    /// voz y un mensaje escrito ven exactamente el mismo historial, incluso
    /// si QML aún no ha recibido el evento `idle`.
    fn persist_exchange(&self, transcript: &str, response: &str) {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let selected = self
            .active_session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let sid = selected
            .filter(|id| sessions.get_session(id).ok().flatten().is_some())
            .or_else(|| sessions.get_or_create_voice_session().ok().map(|s| s.id));
        if let Some(sid) = sid {
            let _ = sessions.add_message(&sid, &Message::user(transcript));
            if !response.trim().is_empty() {
                let _ = sessions.add_message(&sid, &Message::assistant(response));
            }
        }
    }

    async fn process_utterance_inner(
        &self,
        audio: &[f32],
        tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<(String, String)> {
        // 1. STT
        self.emit_state(VoiceState::Thinking);
        let language_override: Option<String> = {
            let cfg = self.config.read().await;
            if cfg.speech.stt_language == "auto" {
                None
            } else {
                Some(cfg.speech.stt_language.clone())
            }
        };
        let transcript = match self
            .speech
            .transcribe(audio, language_override.as_deref())
            .await
        {
            Ok(t) => t,
            Err(e) => {
                log::warn!("[STT] error: {e}");
                self.emit_state(VoiceState::Error(VoiceErrorKind::Stt));
                return Err(e);
            }
        };
        // F0-6 (B2): antes se retornaba ("","") en silencio y la UI no mostraba
        // nada ("¿qué pasó?"). Ahora aviso visible + hablado.
        if transcript.trim().is_empty() {
            log::info!("VoicePipeline STT vacío: aviso al usuario");
            let aviso = "No te escuché, ¿puedes repetirlo?";
            self.begin_speaking();
            let speak_res = self.speech.speak(aviso).await;
            self.end_speaking();
            if let Err(e) = speak_res {
                log::warn!("TTS aviso fallo: {e}");
            }
            return Ok((
                "(inaudible)".to_string(),
                "No te escuché, ¿puedes repetirlo?".to_string(),
            ));
        }
        log::info!("[STT] Final transcript: '{transcript}'");

        // 2+3. LLM streaming + TTS por oración (plan §10/§11): las oraciones
        // se sintetizan y suenan mientras el resto de la respuesta sigue
        // llegando. `respond` alimenta el canal de oraciones y lo cierra al
        // terminar; el player drena la cola y sale solo.
        let (sentence_tx, sentence_rx) = tokio::sync::mpsc::channel::<String>(4);
        let speech = self.speech.clone();
        let player = tokio::spawn(async move { speech.speak_streaming(sentence_rx).await });
        let response = match self.respond(&transcript, tx, Some(sentence_tx)).await {
            Ok(r) => r,
            Err(e) => {
                // Sin respuesta del LLM no hay nada que hablar: cortar el
                // player (una oración suelta de un turno fallido sería peor).
                log::warn!("[LLM] error: {e}");
                self.speech.request_stop();
                let _ = player.await;
                self.end_speaking();
                self.emit_state(VoiceState::Error(VoiceErrorKind::Llm));
                return Err(e);
            }
        };
        // Esperar a que termine de sonar lo que queda (o al barge-in).
        if let Err(e) = player.await {
            log::warn!("TTS streaming player: {e}");
        }
        self.end_speaking();

        Ok((transcript, response))
    }

    /// Envia un texto al LLM (agente) y retorna la respuesta final.
    /// Opcionalmente reenvia los eventos del stream al canal `tx` (para la UI)
    /// y las oraciones completas a `tts_tx` (TTS streaming, plan §11).
    async fn respond(
        &self,
        text: &str,
        tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
        tts_tx: Option<tokio::sync::mpsc::Sender<String>>,
    ) -> Result<String> {
        let cfg = self.config.read().await.clone();
        // F5: facts del usuario también en voz (SQLite local).
        let mut system_prompt = cfg.ai.system_prompt.clone();
        if cfg.memory.enabled {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Ok(facts) = sessions.list_facts() {
                if !facts.is_empty() {
                    system_prompt.push_str(
                        "\n\nDatos del usuario (recordados localmente, pueden estar desactualizados):",
                    );
                    for f in &facts {
                        system_prompt.push_str(&format!("\n- {}: {}", f.key, f.value));
                    }
                }
            }
        }
        let mut messages = vec![Message::system(system_prompt)];
        // Contexto de voz: mismo hilo canonico "Conversacion por voz".
        // Ventana de 12 (era 40): cada turno de voz pega el historial entero
        // en el prompt, y con 40 mensajes reventaba el TPM de Groq (429s).
        let mut voice_session_id: Option<String> = None;
        {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            let selected = self
                .active_session
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let session = selected
                .filter(|id| sessions.get_session(id).ok().flatten().is_some())
                .and_then(|id| sessions.get_session(&id).ok().flatten())
                .or_else(|| sessions.get_or_create_voice_session().ok());
            if let Some(vs) = session {
                if let Ok(history) = sessions.get_messages(&vs.id) {
                    messages.extend(trim_voice_history(history, MAX_VOICE_HISTORY));
                }
                voice_session_id = Some(vs.id);
            }
        }
        messages.push(Message::user(text.to_string()));
        // NOTA F0-1: la voz ahora inyecta historial (hecho).
        // Respetar los flags de tools para no llamar tools deshabilitadas.
        let tools = tool_registry::filtered_tools(&cfg);

        let (stream_tx, mut stream_rx) = tokio::sync::mpsc::channel::<StreamEvent>(256);

        // Ejecutar el agente en un task; los eventos van por stream_tx
        let svc = self.ai.clone();
        let exec = self.tools.clone();
        let appr = self.approvals.clone();
        let msgs = messages.clone();
        let tools_c = tools.clone();
        let run_task = tokio::spawn(async move {
            svc.run_agent(
                msgs,
                tools_c,
                exec,
                appr,
                crate::backend::approvals::ApprovalPolicy::voice(),
                voice_session_id,
                stream_tx,
            )
            .await
        });

        // Consumir eventos: reenviar a la UI y trocear en oraciones para TTS.
        // Si la UI se desconecta, se sigue alimentando TTS (fallos aislados).
        let mut ext = tx;
        let mut tts_sink = tts_tx;
        let mut chunker = SentenceChunker::new();
        let mut speaking_announced = false;
        while let Some(ev) = stream_rx.recv().await {
            match &ev {
                StreamEvent::Token { content } => {
                    if let Some(sink) = &tts_sink {
                        for sentence in chunker.push(content) {
                            if !speaking_announced {
                                self.begin_speaking();
                                speaking_announced = true;
                            }
                            let spoken = plain_text_for_tts(&sentence);
                            if !spoken.is_empty() && sink.send(spoken).await.is_err() {
                                // Player cortado (barge-in): dejar de
                                // alimentarlo; el texto sigue a la UI.
                                tts_sink = None;
                                break;
                            }
                        }
                    }
                }
                StreamEvent::ToolCall { .. } => {
                    self.emit_state(VoiceState::ToolExecuting);
                }
                StreamEvent::ToolResult { .. } => {
                    self.emit_state(VoiceState::Thinking);
                }
                _ => {}
            }
            if let Some(e) = &ext {
                if e.send(ev).await.is_err() {
                    ext = None;
                }
            }
        }
        // Última oración (resto sin signo de cierre).
        if let Some(sink) = &tts_sink {
            if let Some(rest) = chunker.flush() {
                if !speaking_announced {
                    self.begin_speaking();
                }
                let spoken = plain_text_for_tts(&rest);
                if !spoken.is_empty() {
                    let _ = sink.send(spoken).await;
                }
            }
        }
        // Cerrar el canal de oraciones: el player drena la cola y termina.
        drop(tts_sink);

        // Esperar a que el agente termine
        let outcome = run_task.await.context("run_agent task")??;
        Ok(outcome.response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::ai_service::AiService;
    use crate::backend::chime_player::ChimePlayer;
    use crate::backend::speech_service::SpeechService;
    use crate::backend::tool_executor::ToolExecutor;

    async fn test_pipeline() -> VoicePipeline {
        let cfg = Arc::new(RwLock::new(Config::default()));
        let ai = Arc::new(AiService::new(cfg.clone()).await.unwrap());
        let tools = Arc::new(ToolExecutor::new(cfg.clone()));
        let approvals = Arc::new(crate::backend::approvals::ApprovalManager::new());
        let sessions = Arc::new(Mutex::new(
            crate::backend::session_manager::SessionManager::new()
                .await
                .unwrap(),
        ));
        let speech = Arc::new(SpeechService::new(cfg.clone()).await.unwrap());
        let chimes = Arc::new(ChimePlayer::new().await.unwrap());
        VoicePipeline::new(cfg, speech, ai, tools, approvals, sessions, chimes)
    }

    #[tokio::test]
    async fn signal_broadcast_state() {
        let vp = test_pipeline().await;
        let mut rx = vp.subscribe();
        assert_eq!(vp.current_state(), "idle");
        vp.emit_state(VoiceState::Listening);
        assert_eq!(vp.current_state(), "listening");
        match rx.recv().await.unwrap() {
            VoiceSignal::State { state } => assert_eq!(state, "listening"),
            other => panic!("esperaba State, llegó {other:?}"),
        }
    }

    #[tokio::test]
    async fn emit_state_mapea_a_strings_legados() {
        // La UI solo entiende idle/listening/processing/speaking.
        let vp = test_pipeline().await;
        vp.emit_state(VoiceState::Listening);
        assert_eq!(vp.current_state(), "listening");
        vp.emit_state(VoiceState::Thinking);
        assert_eq!(vp.current_state(), "processing");
        vp.emit_state(VoiceState::ToolExecuting);
        assert_eq!(vp.current_state(), "processing");
        vp.emit_state(VoiceState::Responding);
        assert_eq!(vp.current_state(), "speaking");
        vp.emit_state(VoiceState::Idle);
        assert_eq!(vp.current_state(), "idle");
    }

    #[tokio::test]
    async fn barge_in_transita_interrupted_a_listening() {
        // Simular un TTS en curso y un barge-in.
        let vp = test_pipeline().await;
        vp.emit_state(VoiceState::Listening);
        vp.emit_state(VoiceState::Thinking);
        vp.emit_state(VoiceState::Responding);
        vp.speaking.store(true, Ordering::Relaxed);
        vp.barge_in_silent();
        assert_eq!(vp.state(), VoiceState::Listening);
        assert_eq!(vp.current_state(), "listening");
        assert!(!vp.is_speaking());
        assert!(vp.is_recording());
    }

    #[tokio::test]
    async fn signal_level_throttled() {
        let vp = test_pipeline().await;
        let mut rx = vp.subscribe();
        vp.emit_level(0.5);
        match rx.recv().await.unwrap() {
            VoiceSignal::Level { level } => assert!((level - 0.5).abs() < 0.001),
            other => panic!("esperaba Level, llegó {other:?}"),
        }
        // Segundo envío inmediato con delta pequeño: throttled, no llega nada.
        vp.emit_level(0.52);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn dictado_activo_bloquea_start_listening() {
        // El dictado al chat y la conversación por voz comparten micro y
        // buffer: con dictado activo, el pipeline no puede arrancar escucha.
        let vp = test_pipeline().await;
        vp.dictation_flag().store(true, Ordering::Relaxed);
        vp.start_listening();
        assert!(!vp.is_recording());
        assert_ne!(vp.state(), VoiceState::Listening);
    }

    #[tokio::test]
    async fn dictado_activo_aborta_wait_for_speech_rapido() {
        // Un dictado que empieza durante la ventana de auto_listen debe
        // cortar la espera de inmediato (sin esperar los 10s de la ventana).
        let vp = test_pipeline().await;
        vp.dictation_flag().store(true, Ordering::Relaxed);
        let t0 = std::time::Instant::now();
        assert!(!vp.wait_for_speech(10).await);
        assert!(t0.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn trim_voice_history_limita_y_no_rompe_tools() {
        use crate::models::ToolCall;
        let mk_tool_pair = |i: usize| {
            let tc = ToolCall {
                id: format!("call_{i}"),
                name: "t".to_string(),
                arguments: Default::default(),
            };
            vec![
                Message::assistant_with_tools("déjame ver", vec![tc]),
                Message::tool(format!("call_{i}"), "ok"),
            ]
        };
        let mut hist = vec![Message::user("h0")];
        for i in 1..=20 {
            hist.extend(mk_tool_pair(i));
            hist.push(Message::user(format!("h{i}")));
            hist.push(Message::assistant(format!("r{i}")));
        }
        let out = trim_voice_history(hist, MAX_VOICE_HISTORY);
        assert!(out.len() <= MAX_VOICE_HISTORY);
        // Sin tool huérfano al inicio.
        assert!(!matches!(out.first(), Some(Message::Tool { .. })));
        // Sin assistant con tool_calls colgando al final.
        assert!(!matches!(
            out.last(),
            Some(Message::Assistant { tool_calls, .. }) if !tool_calls.is_empty()
        ));
    }

    #[test]
    fn trim_voice_history_corta_el_par_cuando_cae_al_borde() {
        use crate::models::ToolCall;
        // [user, assistant+tool, tool, user] con max=2 → quedaria [tool, user]:
        // el tool huérfano debe desaparecer.
        let hist = vec![
            Message::user("a"),
            Message::assistant_with_tools(
                "",
                vec![ToolCall {
                    id: "c1".to_string(),
                    name: "t".to_string(),
                    arguments: Default::default(),
                }],
            ),
            Message::tool("c1", "ok"),
            Message::user("b"),
        ];
        let out = trim_voice_history(hist, 2);
        assert_eq!(out.len(), 1);
        assert!(matches!(out.first(), Some(Message::User { .. })));
    }

    #[tokio::test]
    async fn wait_for_speech_times_out_quiet() {
        let vp = test_pipeline().await;
        // Sin frames del mic, la amplitud es 0: debe agotar 1s sin detectar.
        let t0 = std::time::Instant::now();
        assert!(!vp.wait_for_speech(1).await);
        assert!(t0.elapsed() >= std::time::Duration::from_millis(900));
    }
}
