//! Speech Service - STT (whisper-rs) + TTS (espeak-ng/piper)
//!
//! Fase 1: stub. Implementacion en Fase 5.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::Config;

pub struct SpeechService {
    pub config: Arc<RwLock<Config>>,
}

impl SpeechService {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        log::info!("SpeechService: inicializando (stub - whisper-rs en Fase 5)");
        Ok(Self { config })
    }

    pub async fn transcribe(&self, _audio: &[f32]) -> Result<String> {
        // TODO(Fase 5): whisper-rs transcribe
        anyhow::bail!("STT no implementado (Fase 5)")
    }

    pub async fn synthesize(&self, _text: &str) -> Result<Vec<u8>> {
        // TODO(Fase 5): espeak-ng o piper-tts
        anyhow::bail!("TTS no implementado (Fase 5)")
    }
}
