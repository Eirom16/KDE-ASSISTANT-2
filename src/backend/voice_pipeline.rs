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
    amplitude: Arc<AtomicU32>, // f32 bits 0..1
    speaking: Arc<AtomicBool>,
    barge_counter: Arc<AtomicU32>,
    /// Momento (ms epoch) en que empezó el TTS actual. Sirve para gracia
    /// anti-auto-corte: ignorar barge los primeros 500ms (el mic capta el altavoz).
    speaking_since_ms: Arc<AtomicU32>,
    /// Bus push de estado/nivel para la UI (F3-1, SSE `/api/voice/stream`).
    signal_tx: broadcast::Sender<VoiceSignal>,
    last_state: Arc<Mutex<String>>,
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
            amplitude: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            speaking: Arc::new(AtomicBool::new(false)),
            barge_counter: Arc::new(AtomicU32::new(0)),
            speaking_since_ms: Arc::new(AtomicU32::new(0)),
            signal_tx: broadcast::channel(64).0,
            last_state: Arc::new(Mutex::new("idle".to_string())),
            last_level_ms: Arc::new(AtomicU32::new(0)),
            last_level_sent: Arc::new(AtomicU32::new(0.0f32.to_bits())),
        }
    }

    /// Suscribe un receptor de señales de voz (para `/api/voice/stream`).
    pub fn subscribe(&self) -> broadcast::Receiver<VoiceSignal> {
        self.signal_tx.subscribe()
    }

    /// Último estado emitido ("idle" si aún ninguno).
    pub fn current_state(&self) -> String {
        self.last_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn amplitude(&self) -> f32 {
        f32::from_bits(self.amplitude.load(Ordering::Relaxed))
    }

    pub fn is_speaking(&self) -> bool {
        self.speaking.load(Ordering::Relaxed)
    }

    /// Barge-in silencioso: interrumpe TTS y reinicia escucha sin chime.
    /// Usado tanto por deteccion de voz (amplitud) como por hotkey/HTTP.
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
        self.emit_state("listening");
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

        // Barge-in: si esta hablando y hay voz fuerte, interrumpir.
        // Umbral 0.28 + 3 frames consecutivos (~60-90 ms) para evitar falsos.
        // Gracia 500ms tras iniciar TTS: el mic capta el altavoz y si no,
        // toda respuesta larga se auto-corta.
        if self.speaking.load(Ordering::Relaxed) {
            let since = self.speaking_since_ms.load(Ordering::Relaxed);
            let now = now_ms() as u32;
            let in_grace = now.saturating_sub(since) < 500;
            if !in_grace {
                if let Some(lvl) = lvl_opt {
                    if lvl > 0.28 {
                        let cnt = self.barge_counter.fetch_add(1, Ordering::Relaxed) + 1;
                        if cnt >= 3 {
                            self.barge_in_silent();
                        }
                    } else if lvl < 0.18 {
                        // Silencio sostenido resetea contador
                        self.barge_counter.store(0, Ordering::Relaxed);
                    }
                }
            }
            // No guardar en buffer mientras habla (salvo que barge-in ya reinicio escucha)
            // Si barge_in_silent se activo, recording ya es true, asi que el push de abajo lo captura.
        }

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

    /// Inicia la grabacion (chime + buffer + recording=true).
    pub fn start_listening(&self) {
        self.chimes.play_activate();
        let sr = self
            .buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sample_rate
            .load(Ordering::SeqCst);
        self.start_recording(sr.max(1));
        self.emit_state("listening");
        log::info!("Escuchando... (grabacion iniciada)");
    }

    /// Emite el estado de voz a la UI (SSE + archivo compat).
    /// Estados: idle | listening | processing | speaking
    /// El archivo se conserva para debug/fallback; la UI usa el SSE.
    pub fn emit_state(&self, state: &str) {
        *self.last_state.lock().unwrap_or_else(|e| e.into_inner()) = state.to_string();
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
        let content = format!("{state}|{timestamp}");
        let _ = std::fs::write(&path, content);
        let _ = self.signal_tx.send(VoiceSignal::State {
            state: state.to_string(),
        });
    }

    /// Compat: emite estado sin instancia (solo archivo, sin SSE).
    /// Preferir `emit_state` cuando haya `&self`.
    pub fn write_state(state: &str) {
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
        let content = format!("{state}|{timestamp}");
        let _ = std::fs::write(&path, content);
    }

    /// Detiene la grabacion y procesa el utterance (STT -> LLM -> TTS).
    /// Retorna (transcript, response).
    pub async fn stop_and_process(&self) -> Result<(String, String)> {
        self.chimes.play_deactivate();
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

    /// Graba hasta ~1.2s de silencio sostenido (tras 1.5s mínimos) o `max_secs`.
    /// Asume `start_listening()` ya llamado. Detiene la grabación y devuelve
    /// el audio a 16kHz. (Antes vivía inline en main.rs; F3-2 lo reutiliza.)
    pub async fn record_until_silence(&self, max_secs: u64) -> Vec<f32> {
        const MIN_SECS: f32 = 1.5;
        const SILENCE_RMS: f32 = 0.02;
        const SILENCE_POLLS: u32 = 6;
        const POLL_MS: u64 = 200;
        let mut silent_polls = 0u32;
        let mut elapsed_ms = 0u64;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
            elapsed_ms += POLL_MS;
            let sr = self.sample_rate().max(1) as f32;
            let recorded_secs = self.recording_len() as f32 / sr;
            let rms = self.recent_rms((sr * 0.4) as usize);
            if recorded_secs >= MIN_SECS && rms < SILENCE_RMS {
                silent_polls += 1;
            } else {
                silent_polls = 0;
            }
            if silent_polls >= SILENCE_POLLS || elapsed_ms >= max_secs * 1000 {
                break;
            }
        }
        self.stop_recording()
    }

    /// Espera voz real hasta `window_secs` (para conversación continua F3-2).
    /// Detecta nivel alto 3 polls seguidos (~300ms) con variación (evita
    /// disparar con un nivel congelado si no hay frames del mic).
    pub async fn wait_for_speech(&self, window_secs: u64) -> bool {
        const POLL_MS: u64 = 100;
        const NEEDED: u32 = 3;
        let start_lvl = self.amplitude();
        let mut hot = 0u32;
        let mut varied = false;
        let polls = window_secs.saturating_mul(1000) / POLL_MS;
        for _ in 0..polls {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
            let lvl = self.amplitude();
            if (lvl - start_lvl).abs() > 0.02 {
                varied = true;
            }
            if lvl > 0.35 {
                hot += 1;
                if hot >= NEEDED && varied {
                    return true;
                }
            } else if lvl < 0.2 {
                hot = 0;
            }
        }
        false
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
        let mut turns = Vec::new();
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
            // No pisar una escucha ya reiniciada por barge-in.
            if self.is_recording() {
                break;
            }
            log::info!("Escucha continua: esperando voz ({window}s)...");
            if !self.wait_for_speech(window).await {
                break;
            }
            self.start_listening();
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
            self.emit_state("idle");
        }
        r
    }

    async fn process_utterance_inner(
        &self,
        audio: &[f32],
        tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<(String, String)> {
        // 1. STT
        self.emit_state("processing");
        self.chimes.play_process();
        let language_override: Option<String> = {
            let cfg = self.config.read().await;
            if cfg.speech.stt_language == "auto" {
                None
            } else {
                Some(cfg.speech.stt_language.clone())
            }
        };
        let transcript = self
            .speech
            .transcribe(audio, language_override.as_deref())
            .await?;
        // F0-6 (B2): antes se retornaba ("","") en silencio y la UI no mostraba
        // nada ("¿qué pasó?"). Ahora aviso visible + hablado.
        if transcript.trim().is_empty() {
            log::info!("VoicePipeline STT vacío: aviso al usuario");
            let aviso = "No te escuché, ¿puedes repetirlo?".to_string();
            self.emit_state("speaking");
            self.speaking.store(true, Ordering::Relaxed);
            self.speaking_since_ms
                .store(now_ms() as u32, Ordering::Relaxed);
            self.barge_counter.store(0, Ordering::Relaxed);
            let speak_res = self.speech.speak(&aviso).await;
            self.speaking.store(false, Ordering::Relaxed);
            if let Err(e) = speak_res {
                log::warn!("TTS aviso fallo: {e}");
            }
            return Ok((
                "(inaudible)".to_string(),
                "No te escuché, ¿puedes repetirlo?".to_string(),
            ));
        }
        log::info!("VoicePipeline STT: '{}'", transcript);

        // 2. LLM (agente con tool calling)
        let response = self.respond(&transcript, tx).await?;

        // 3. TTS: en el pipeline de voz SIEMPRE se habla la respuesta
        // (regla: voz pregunta -> voz responde; el chat por texto nunca
        // habla salvo que auto_speak este activo, ver chat_complete).
        if !response.is_empty() {
            self.emit_state("speaking");
            self.speaking.store(true, Ordering::Relaxed);
            self.speaking_since_ms
                .store(now_ms() as u32, Ordering::Relaxed);
            self.barge_counter.store(0, Ordering::Relaxed);
            let speak_res = self.speech.speak(&response).await;
            self.speaking.store(false, Ordering::Relaxed);
            self.barge_counter.store(0, Ordering::Relaxed);
            // Si hubo barge-in, no sobreescribir estado listening
            if self
                .buffer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .recording
                .load(Ordering::Relaxed)
            {
                log::info!("TTS interrumpido por barge-in, manteniendo listening");
            } else if let Err(e) = speak_res {
                log::warn!("TTS fallo: {e}");
            }
        }

        Ok((transcript, response))
    }

    /// Envia un texto al LLM (agente) y retorna la respuesta final.
    /// Opcionalmente reenvia los eventos del stream al canal `tx` (para la UI).
    async fn respond(
        &self,
        text: &str,
        tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
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
        let messages = vec![
            Message::system(cfg.ai.system_prompt.clone()),
            Message::user(text.to_string()),
        ];
        // NOTA F0-1: la voz aún no inyecta historial de sesión (siguiente paso).
        // Al menos respetar los flags de tools para no llamar tools deshabilitadas.
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
                None,
                stream_tx,
            )
            .await
        });

        // Reenviar eventos al canal externo (si hay consumidor)
        let forward_task = tokio::spawn(async move {
            while let Some(ev) = stream_rx.recv().await {
                if let Some(ext) = &tx {
                    if ext.send(ev).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Esperar a que el agente termine
        let outcome = run_task.await.context("run_agent task")??;
        drop(forward_task);
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
        vp.emit_state("listening");
        assert_eq!(vp.current_state(), "listening");
        match rx.recv().await.unwrap() {
            VoiceSignal::State { state } => assert_eq!(state, "listening"),
            other => panic!("esperaba State, llegó {other:?}"),
        }
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
    async fn wait_for_speech_times_out_quiet() {
        let vp = test_pipeline().await;
        // Sin frames del mic, la amplitud es 0: debe agotar 1s sin detectar.
        let t0 = std::time::Instant::now();
        assert!(!vp.wait_for_speech(1).await);
        assert!(t0.elapsed() >= std::time::Duration::from_millis(900));
    }
}
