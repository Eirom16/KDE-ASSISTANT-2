//! Audio Capture - cpal input + VAD + nivel de amplitud
//!
//! Fase 1: stub. Implementacion en Fase 5.

use anyhow::Result;

pub struct AudioCapture;

impl AudioCapture {
    pub fn new() -> Result<Self> {
        log::info!("AudioCapture: stub (cpal en Fase 5)");
        Ok(Self)
    }

    pub fn start(&mut self) -> Result<()> {
        // TODO(Fase 5): cpal input stream
        Ok(())
    }

    pub fn amplitude(&self) -> f32 {
        // TODO(Fase 5): retornar RMS del ultimo frame
        0.0
    }
}
