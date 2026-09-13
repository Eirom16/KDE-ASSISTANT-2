//! Speech Service - Sintesis (TTS) con piper-tts + Reconocimiento (STT) con whisper-rs
//!
//! TTS: motor neural local (ONNX), alta calidad y casi humana en espanol.
//! STT: whisper-rs con modelo ggml-base.bin (local, auto-descarga).
//!
//! El binario `piper-tts` se invoca como subproceso (wrapper en
//! `crate::backend::tts::piper::PiperEngine`).

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::backend::stt::WhisperEngine;
use crate::backend::tts::piper::PiperEngine;
use crate::models::Config;

pub struct SpeechService {
    pub config: Arc<RwLock<Config>>,
    pub piper: PiperEngine,
    pub whisper: Arc<WhisperEngine>,
    stop_requested: Arc<AtomicBool>,
}

impl SpeechService {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        let piper = PiperEngine::new().context("inicializando Piper TTS")?;
        let models = piper.list_models();
        let available = piper.has_binary();

        if !available {
            log::warn!("piper-tts no encontrado. TTS no funcionara hasta instalar.");
        } else if models.is_empty() {
            log::warn!(
                "piper-tts disponible pero sin modelos en {}. Descarga uno desde https://huggingface.co/rhasspy/piper-voices",
                piper.models_dir.display()
            );
        } else {
            log::info!(
                "SpeechService: piper-tts con {} modelo(s): {:?}",
                models.len(),
                models.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
        }

        // Inicializar whisper (STT) con el modelo elegido (tiny/base).
        let cfg = config.read().await.clone();
        let whisper_path = whisper_model_path_for(&cfg.speech.stt_model)?;
        let language_override = if cfg.speech.stt_language == "auto" {
            None
        } else {
            Some(cfg.speech.stt_language.clone())
        };
        let whisper = Arc::new(WhisperEngine::new(whisper_path, language_override)?);
        if whisper.model_exists() {
            log::info!(
                "SpeechService: whisper con modelo {} listo",
                cfg.speech.stt_model
            );
        } else {
            log::warn!(
                "whisper sin modelo en '{}'. Se descargara automaticamente al iniciar.",
                whisper_path_display()
            );
        }

        Ok(Self {
            config,
            piper,
            whisper,
            stop_requested: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Sintetiza texto a bytes WAV.
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let cfg = self.config.read().await.clone();
        self.piper
            .synthesize(
                text,
                Some(&cfg.speech.piper_model),
                Some(cfg.speech.piper_length_scale),
            )
            .await
    }

    /// Reproduce el texto sintetizado inmediatamente (interrumpible via `request_stop`).
    pub async fn speak(&self, text: &str) -> Result<()> {
        // Reset cancel antes de cada locucion
        self.stop_requested.store(false, Ordering::Relaxed);
        let cfg = self.config.read().await.clone();
        let wav = self
            .piper
            .synthesize(
                text,
                Some(&cfg.speech.piper_model),
                Some(cfg.speech.piper_length_scale),
            )
            .await?;
        if self.stop_requested.load(Ordering::Relaxed) {
            log::info!("TTS cancelado durante sintesis");
            return Ok(());
        }
        let flag = self.stop_requested.clone();
        tokio::task::spawn_blocking(move || play_wav_bytes_with_cancel(&wav, flag))
            .await
            .context("tts playback task")??;
        Ok(())
    }

    /// Solicita interrumpir la reproduccion en curso (barge-in).
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Relaxed);
        log::info!("TTS stop solicitado (barge-in)");
    }

    /// Reproduce un stream de oraciones (plan §11): sintetiza la siguiente
    /// mientras suena la actual (síntesis anticipada, baja latencia).
    ///
    /// El canal se cierra cuando el LLM termina de emitir. `request_stop()`
    /// (barge-in) corta la reproducción actual y descarta lo pendiente:
    /// nunca se reproduce audio de una operación ya cancelada (plan §12/§13).
    ///
    /// Errores de síntesis de una oración se loguean y se sigue con la
    /// siguiente; un fallo de reproducción (dispositivo) aborta el stream.
    pub async fn speak_streaming(&self, mut sentences: mpsc::Receiver<String>) -> Result<()> {
        self.stop_requested.store(false, Ordering::Relaxed);
        let (wav_model, wav_scale) = {
            let cfg = self.config.read().await;
            (
                cfg.speech.piper_model.clone(),
                cfg.speech.piper_length_scale,
            )
        };
        let (wav_tx, mut wav_rx) = mpsc::channel::<Vec<u8>>(2);
        let producer_stop = self.stop_requested.clone();
        let piper = self.piper.clone();
        let producer = tokio::spawn(async move {
            while let Some(sentence) = sentences.recv().await {
                if producer_stop.load(Ordering::Relaxed) {
                    break;
                }
                match piper
                    .synthesize(&sentence, Some(&wav_model), Some(wav_scale))
                    .await
                {
                    Ok(wav) => {
                        if wav_tx.send(wav).await.is_err() {
                            break; // consumidor cancelado (barge-in)
                        }
                    }
                    Err(e) => {
                        let head: String = sentence.chars().take(40).collect();
                        log::warn!("TTS streaming: oración '{head}…' falló: {e}");
                    }
                }
            }
        });

        let mut playback_error: Option<anyhow::Error> = None;
        while let Some(wav) = wav_rx.recv().await {
            if self.stop_requested.load(Ordering::Relaxed) {
                log::info!("TTS streaming: cola descartada por barge-in");
                break;
            }
            let flag = self.stop_requested.clone();
            match tokio::task::spawn_blocking(move || play_wav_bytes_with_cancel(&wav, flag)).await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    log::warn!("TTS streaming: reproducción falló: {e}");
                    playback_error = Some(e);
                    break;
                }
                Err(e) => {
                    log::warn!("TTS streaming: task de reproducción falló: {e}");
                    playback_error = Some(e.into());
                    break;
                }
            }
        }
        // Cierre: si el productor sigue vivo (canal de oraciones abierto),
        // cortarlo; si ya terminó, esto es no-op.
        producer.abort();
        let _ = producer.await;
        if let Some(e) = playback_error {
            return Err(e);
        }
        Ok(())
    }

    /// Verifica si hay un stop pendiente.
    pub fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }

    /// STT: transcribe audio (mono f32, 16kHz) a texto.
    /// Soporta auto-deteccion de idioma y override explicito.
    pub async fn transcribe(&self, audio: &[f32], language: Option<&str>) -> Result<String> {
        // whisper es CPU-intensivo; ejecutar en spawn_blocking para no bloquear el runtime
        let whisper = self.whisper.clone();
        let audio = audio.to_vec();
        let language = language.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || whisper.transcribe(&audio, language.as_deref()))
            .await
            .context("whisper transcribe task")?
    }

    /// Verifica si piper esta disponible (binario + al menos un modelo).
    pub async fn is_available(&self) -> bool {
        self.piper.is_available()
    }

    /// Lista los modelos de voz piper disponibles.
    pub async fn list_models(&self) -> Vec<String> {
        self.piper.list_models().into_iter().map(|m| m.id).collect()
    }

    /// Verifica si el modelo STT (whisper) esta disponible.
    pub fn stt_model_exists(&self) -> bool {
        self.whisper.model_exists()
    }

    /// Devuelve un mensaje de ayuda si piper no esta configurado.
    pub fn help_message(&self) -> String {
        PiperEngine::help_message()
    }
}

/// Path del modelo whisper según `stt_model` ("tiny" → ggml-tiny.bin).
pub fn whisper_model_path() -> Result<PathBuf> {
    // Compat: sin config a mano, base. El servicio usa `whisper_model_path_for`.
    whisper_model_path_for("base")
}

/// Path del modelo whisper para un `stt_model` dado.
pub fn whisper_model_path_for(stt_model: &str) -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("no se pudo obtener data_local_dir"))?
        .join("kde-assistant/models");
    let file = if stt_model.trim().eq_ignore_ascii_case("tiny") {
        "ggml-tiny.bin"
    } else {
        "ggml-base.bin"
    };
    Ok(base.join(file))
}

fn whisper_path_display() -> String {
    whisper_model_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.local/share/kde-assistant/models/ggml-base.bin".to_string())
}

/// Reproduce bytes WAV via rodio (sincrono, bloqueante).
pub fn play_wav_bytes(wav_bytes: &[u8]) -> Result<()> {
    play_wav_bytes_with_cancel(wav_bytes, Arc::new(AtomicBool::new(false)))
}

/// Reproduce bytes WAV pero permite cancelar via `cancel` (barge-in).
pub fn play_wav_bytes_with_cancel(wav_bytes: &[u8], cancel: Arc<AtomicBool>) -> Result<()> {
    use std::io::Cursor;
    use std::time::Duration;

    let (_stream, handle) = rodio::OutputStream::try_default()
        .map_err(|e| anyhow::anyhow!("abriendo output stream: {e}"))?;
    let cursor = Cursor::new(wav_bytes.to_vec());
    let sink = handle
        .play_once(cursor)
        .map_err(|e| anyhow::anyhow!("play_once: {e}"))?;
    sink.set_volume(0.9);
    // Loop interrumpible: revisa cancel cada 30 ms en vez de sleep_until_end
    while !sink.empty() {
        if cancel.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn piper_engine_initializes() {
        // Solo verifica que no panic; puede no tener piper instalado
        let cfg = Arc::new(RwLock::new(Config::default()));
        let svc = SpeechService::new(cfg).await;
        // No fallamos si piper no esta; el servicio debe inicializarse igual
        assert!(svc.is_ok());
    }
}
