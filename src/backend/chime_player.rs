//! Chime Player - rodio playback de WAVs embebidos
//!
//! Reproduce tonos sutiles para feedback acustico estilo Siri:
//! - activate: wake word detectado
//! - process: transcripcion completa, listo para responder
//! - deactivate: cancelacion manual
//!
//! Fase 1: stub funcional. Implementacion completa en Fase 5 con WAVs reales.

use anyhow::Result;

pub struct ChimePlayer {
    enabled: bool,
}

impl ChimePlayer {
    pub async fn new() -> Result<Self> {
        log::info!("ChimePlayer: inicializando (WAVs en Fase 5)");
        Ok(Self { enabled: true })
    }

    pub fn play_activate(&self) {
        if self.enabled {
            log::debug!("[Chime] activate");
            // TODO(Fase 5): reproducir assets/chimes/activate.wav con rodio
        }
    }

    pub fn play_process(&self) {
        if self.enabled {
            log::debug!("[Chime] process");
        }
    }

    pub fn play_deactivate(&self) {
        if self.enabled {
            log::debug!("[Chime] deactivate");
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
