//! Speech Service - Sintesis (TTS) con piper-tts
//!
//! Motor neural local (ONNX), alta calidad y casi humana en espanol.
//! El binario `piper-tts` se invoca como subproceso (wrapper en
//! `crate::backend::tts::piper::PiperEngine`).
//!
//! STT (whisper-rs) queda como placeholder para una fase futura.

use anyhow::{anyhow, bail, Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backend::tts::piper::PiperEngine;
use crate::models::Config;

pub struct SpeechService {
    pub config: Arc<RwLock<Config>>,
    pub piper: PiperEngine,
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

        Ok(Self { config, piper })
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

    /// Reproduce el texto sintetizado inmediatamente.
    pub async fn speak(&self, text: &str) -> Result<()> {
        let cfg = self.config.read().await.clone();
        self.piper
            .speak(
                text,
                Some(&cfg.speech.piper_model),
                Some(cfg.speech.piper_length_scale),
            )
            .await
    }

    /// STT placeholder. En una fase posterior se integrara whisper-rs.
    pub async fn transcribe(&self, _audio: &[f32]) -> Result<String> {
        bail!("STT no implementado aun (requiere whisper-rs + modelo ggml-base.bin)")
    }

    /// Verifica si piper esta disponible (binario + al menos un modelo).
    pub async fn is_available(&self) -> bool {
        self.piper.is_available()
    }

    /// Lista los modelos de voz piper disponibles.
    pub async fn list_models(&self) -> Vec<String> {
        self.piper.list_models().into_iter().map(|m| m.id).collect()
    }

    /// Devuelve un mensaje de ayuda si piper no esta configurado.
    pub fn help_message(&self) -> String {
        PiperEngine::help_message()
    }
}

/// Reproduce bytes WAV via rodio (sincrono, bloqueante).
pub fn play_wav_bytes(wav_bytes: &[u8]) -> Result<()> {
    use std::io::Cursor;

    let (_stream, handle) = rodio::OutputStream::try_default()
        .map_err(|e| anyhow::anyhow!("abriendo output stream: {e}"))?;
    let cursor = Cursor::new(wav_bytes.to_vec());
    let sink = handle
        .play_once(cursor)
        .map_err(|e| anyhow::anyhow!("play_once: {e}"))?;
    sink.set_volume(0.9);
    sink.sleep_until_end();
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
