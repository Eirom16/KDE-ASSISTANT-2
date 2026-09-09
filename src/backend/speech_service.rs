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
use tokio::sync::RwLock;

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

        // Inicializar whisper (STT)
        let cfg = config.read().await.clone();
        let whisper_path = whisper_model_path()?;
        let language_override = if cfg.speech.stt_language == "auto" {
            None
        } else {
            Some(cfg.speech.stt_language.clone())
        };
        let whisper = Arc::new(WhisperEngine::new(whisper_path, language_override)?);
        if whisper.model_exists() {
            log::info!("SpeechService: whisper con modelo ggml-base.bin listo");
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

/// Path del modelo whisper ggml-base.bin.
pub fn whisper_model_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("no se pudo obtener data_local_dir"))?
        .join("kde-assistant/models");
    Ok(base.join("ggml-base.bin"))
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
