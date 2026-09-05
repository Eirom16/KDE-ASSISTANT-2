//! Hotword Detector - Wake word ML con ONNX/openWakeWord
//!
//! Detecta "Hey KDE" en tiempo real desde el audio input.
//! Umbral por defecto: 0.8 (configurable).
//!
//! Fase 1: stub. Implementacion en Fase 5 (requiere modelo ONNX).

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::Config;

pub struct HotwordDetector {
    pub config: Arc<RwLock<Config>>,
    pub threshold: f32,
}

impl HotwordDetector {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        log::info!("HotwordDetector: stub (ONNX en Fase 5)");
        Ok(Self {
            config,
            threshold: 0.8,
        })
    }

    pub async fn start(&self) -> Result<()> {
        // TODO(Fase 5): cargar modelo ONNX, iniciar loop de inferencia
        log::info!("Hotword detector activo (umbral {})", self.threshold);
        Ok(())
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
    }
}
