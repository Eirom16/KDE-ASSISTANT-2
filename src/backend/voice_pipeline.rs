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

use crate::backend::ai_service::AiService;
use crate::backend::audio_capture::resample_to_16k;
use crate::backend::chime_player::ChimePlayer;
use crate::backend::speech_service::SpeechService;
use crate::backend::tool_executor::ToolExecutor;
use crate::backend::tool_registry;
use crate::models::{Config, Message, StreamEvent};
use tokio::sync::RwLock;

/// Buffer de grabacion compartida entre el thread de audio y el orquestador.
pub struct RecordingBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: Arc<AtomicU32>,
    pub recording: Arc<AtomicBool>,
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
    pub chimes: Arc<ChimePlayer>,
    pub buffer: Arc<Mutex<RecordingBuffer>>,
    last_exchange: Arc<Mutex<Option<VoiceExchange>>>,
    amplitude: Arc<AtomicU32>, // f32 bits 0..1
    speaking: Arc<AtomicBool>,
    barge_counter: Arc<AtomicU32>,
    /// Momento (ms epoch) en que empezó el TTS actual. Sirve para gracia
    /// anti-auto-corte: ignorar barge los primeros 500ms (el mic capta el altavoz).
    speaking_since_ms: Arc<AtomicU32>,
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
        chimes: Arc<ChimePlayer>,
    ) -> Self {
        Self {
            config,
            speech,
            ai,
            tools,
            chimes,
            buffer: Arc::new(Mutex::new(RecordingBuffer::new())),
            last_exchange: Arc::new(Mutex::new(None)),
            amplitude: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            speaking: Arc::new(AtomicBool::new(false)),
            barge_counter: Arc::new(AtomicU32::new(0)),
            speaking_since_ms: Arc::new(AtomicU32::new(0)),
        }
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
        Self::write_state("listening");
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
            Self::write_level(lvl);
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

    fn write_level(level: f32) {
        let cache_dir = match dirs::cache_dir() {
            Some(d) => d.join("kde-assistant"),
            None => return,
        };
        let _ = std::fs::create_dir_all(&cache_dir);
        let path = cache_dir.join("voice.level");
        let _ = std::fs::write(&path, format!("{:.3}", level.clamp(0.0, 1.0)));
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
        Self::write_state("listening");
        log::info!("Escuchando... (grabacion iniciada)");
    }

    /// Escribe el estado de voz para la UI (`~/.cache/kde-assistant/voice.state`).
    /// Estados: idle | listening | processing | speaking
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
        self.last_exchange.lock().unwrap().clone()
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
            Self::write_state("idle");
        }
        r
    }

    async fn process_utterance_inner(
        &self,
        audio: &[f32],
        tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<(String, String)> {
        // 1. STT
        Self::write_state("processing");
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
            Self::write_state("speaking");
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
            Self::write_state("speaking");
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
        let msgs = messages.clone();
        let tools_c = tools.clone();
        let run_task =
            tokio::spawn(async move { svc.run_agent(msgs, tools_c, exec, stream_tx).await });

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
        let _ = forward_task;
        Ok(outcome.response)
    }
}
