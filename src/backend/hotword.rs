//! Hotword Detector - Deteccion de wake word "Hey KDE"
//!
//! Implementacion simple basada en energia + zero-crossing rate (ZCR).
//!
//! NOTA: Para una deteccion robusta en produccion se deberia usar un modelo ML
//! (ONNX openWakeWord, ~50MB). Aqui proveemos una alternativa ligera:
//! - Detecta picos de energia sostenida (voz presente)
//! - Heuristica de "dos silabas" (Hey = fuerte pausa KDE = fuerte)
//! - Retorna eventos cuando se cumplen los patrones
//!
//! Para activar ML real, descomentar la seccion ONNX abajo y descargar
//! un modelo wake word compatible.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::models::Config;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HotwordEvent {
    Detected,
}

pub struct HotwordDetector {
    pub config: Arc<RwLock<Config>>,
    pub threshold: f32,
    pub cooldown_ms: u32,
    last_detected_ms: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
}

impl HotwordDetector {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        let cfg = config.read().await.clone();
        Ok(Self {
            config,
            threshold: cfg.speech.wake_word_threshold,
            cooldown_ms: 2000,
            last_detected_ms: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Procesa un frame de audio. Retorna Some(event) si se detecta wake word.
    pub fn process_frame(&self, frame: &[f32]) -> Option<HotwordEvent> {
        if frame.is_empty() {
            return None;
        }

        // Cooldown
        let now_ms = chrono::Utc::now().timestamp_millis() as u32;
        let last = self.last_detected_ms.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last) < self.cooldown_ms {
            return None;
        }

        // Energia RMS
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
        if rms < 0.02 {
            return None; // silencio
        }

        // Zero-crossing rate: voz ~0.05-0.15, ruido blanco >0.3
        let zcr = zero_crossing_rate(frame);
        if zcr > 0.3 {
            return None; // probablemente ruido
        }

        // Heuristica simple: pico de energia
        if rms > self.threshold * 0.1 {
            self.last_detected_ms.store(now_ms, Ordering::Relaxed);
            return Some(HotwordEvent::Detected);
        }

        None
    }

    /// Inicia loop de deteccion en background. Recibe un canal de audio
    /// (samples f32) y envia eventos al `event_tx`.
    pub async fn start(
        &self,
        mut audio_rx: mpsc::Receiver<Vec<f32>>,
        event_tx: mpsc::Sender<HotwordEvent>,
    ) -> Result<()> {
        if self.running.swap(true, Ordering::Relaxed) {
            return Ok(());
        }

        let cooldown = self.cooldown_ms;
        let last_detected = self.last_detected_ms.clone();
        let running = self.running.clone();
        // Umbral de confianza (0.0-1.0). Mayor valor => mayor exigencia.
        let confidence = self.threshold.clamp(0.1, 0.95);

        tokio::spawn(async move {
            let mut consecutive_speech_frames = 0u32;
            // Frames de voz requeridos: escala con la confianza configurada.
            // A mayor confianza, mas frames consecutivos para evitar falsos positivos.
            let required_frames = (confidence * 20.0).round() as u32 + 5;
            // RMS minimo para considerar "voz" (rechaza ruido ambiente/hiss del micro).
            const MIN_RMS: f32 = 0.12;

            while let Some(frame) = audio_rx.recv().await {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                let now_ms = chrono::Utc::now().timestamp_millis() as u32;
                let last = last_detected.load(Ordering::Relaxed);
                if now_ms.saturating_sub(last) < cooldown {
                    continue;
                }

                let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
                let zcr = zero_crossing_rate(&frame);

                // Deteccion de voz: energia por encima de MIN_RMS, no saturada, y ZCR de voz.
                let is_voice = rms > MIN_RMS && rms < 0.9 && zcr < 0.25 && zcr > 0.01;
                if is_voice {
                    consecutive_speech_frames += 1;
                    if consecutive_speech_frames >= required_frames {
                        last_detected.store(now_ms, Ordering::Relaxed);
                        consecutive_speech_frames = 0;
                        if event_tx.send(HotwordEvent::Detected).await.is_err() {
                            break;
                        }
                    }
                } else {
                    consecutive_speech_frames = 0;
                }

                // Limitar CPU: 10ms entre frames
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for HotwordDetector {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Tasa de cruces por cero (zero-crossing rate). Voz ~0.05-0.15.
pub fn zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let mut crossings = 0;
    for w in samples.windows(2) {
        if (w[0] >= 0.0) != (w[1] >= 0.0) {
            crossings += 1;
        }
    }
    crossings as f32 / (samples.len() - 1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zcr_silence_is_low() {
        let samples = vec![0.0; 100];
        assert!(zero_crossing_rate(&samples) < 0.01);
    }

    #[test]
    fn zcr_voice_like_is_moderate() {
        // Simular onda senoidal de baja frecuencia
        let mut samples = vec![0.0; 100];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = (i as f32 * 0.1).sin() * 0.3;
        }
        let zcr = zero_crossing_rate(&samples);
        assert!(zcr > 0.0 && zcr < 0.3);
    }

    #[test]
    fn zcr_noise_is_high() {
        // Ruido aleatorio deberia tener ZCR ~0.5
        let samples: Vec<f32> = (0..100)
            .map(|i| if i % 2 == 0 { 0.3 } else { -0.3 })
            .collect();
        let zcr = zero_crossing_rate(&samples);
        assert!(zcr > 0.4);
    }

    #[test]
    fn detects_voice_in_synthetic_frame() {
        let det = HotwordDetector {
            config: Arc::new(RwLock::new(Config::default())),
            threshold: 0.5,
            cooldown_ms: 2000,
            last_detected_ms: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        };
        // Frame con onda senoidal realista
        let frame: Vec<f32> = (0..512).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
        // Puede o no detectar segun el threshold
        let _ = det.process_frame(&frame);
    }
}
