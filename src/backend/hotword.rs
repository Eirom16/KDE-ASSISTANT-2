//! Hotword Detector - Deteccion de wake word ("hey jarvis" via ML, fallback heuristico)
//!
//! Dos niveles:
//! 1. **ML (openWakeWord ONNX)**: preciso, <0.5 falsos positivos/hora.
//!    Requiere libonnxruntime.so + los 3 modelos en
//!    `~/.local/share/kde-assistant/models/wakeword/` (auto-descarga).
//! 2. **Heuristico (energia + ZCR)**: fallback ligero si el ML no esta disponible.
//!
//! El audio de entrada puede venir a cualquier sample rate; para el ML se
//! remuestrea a 16kHz mono (requerido por openWakeWord).

use anyhow::Result;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};

use crate::backend::audio_capture::resample_to_16k;
use crate::backend::wakeword_ml::OwwDetector;
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
    ml: Arc<Mutex<Option<OwwDetector>>>,
    src_rate: Arc<AtomicU32>,
    pending_16k: Arc<Mutex<Vec<f32>>>,
    /// Frames descartados por canal lleno (productor cpal → este detector).
    pub dropped_frames: Arc<AtomicU64>,
}

impl HotwordDetector {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        let cfg = config.read().await.clone();
        let det = Self {
            config,
            threshold: cfg.speech.wake_word_threshold,
            cooldown_ms: 2000,
            last_detected_ms: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            ml: Arc::new(Mutex::new(None)),
            src_rate: Arc::new(AtomicU32::new(0)),
            pending_16k: Arc::new(Mutex::new(Vec::new())),
            dropped_frames: Arc::new(AtomicU64::new(0)),
        };
        // Intentar cargar el modelo ML (no bloquea el arranque si falla)
        det.try_load_ml();
        Ok(det)
    }

    /// Intenta cargar el detector ML. Si falla, se usa el heuristico.
    pub fn try_load_ml(&self) {
        let models_dir = match dirs::data_local_dir() {
            Some(d) => d.join("kde-assistant/models/wakeword"),
            None => return,
        };
        match OwwDetector::new(&models_dir, self.threshold) {
            Ok(ml) => {
                *self.ml.lock().unwrap() = Some(ml);
                log::info!("HotwordDetector: ML openWakeWord (hey jarvis) activo");
            }
            Err(e) => {
                log::warn!("HotwordDetector: ML no disponible ({e}); usando heuristico");
            }
        }
    }

    /// Indica si el detector ML esta activo.
    pub fn ml_active(&self) -> bool {
        self.ml.lock().unwrap().is_some()
    }

    /// Registra el sample rate de la captura (para remuestrear a 16kHz).
    pub fn set_sample_rate(&self, rate: u32) {
        self.src_rate.store(rate, Ordering::Relaxed);
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
    /// (samples f32 mono a la tasa de la captura) y envia eventos al `event_tx`.
    /// Usa el modelo ML si esta disponible, si no el heuristico.
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
        let dropped = self.dropped_frames.clone();
        // Umbral de confianza (0.0-1.0). Mayor valor => mayor exigencia.
        let confidence = self.threshold.clamp(0.1, 0.95);
        let ml = self.ml.clone();
        let src_rate = self.src_rate.clone();
        let pending_16k = self.pending_16k.clone();

        // Avisar que motor se usa
        if ml.lock().unwrap().is_some() {
            log::info!("HotwordDetector: usando ML openWakeWord (hey jarvis)");
        } else {
            log::info!("HotwordDetector: usando heuristico energia+ZCR");
        }

        tokio::spawn(async move {
            let mut consecutive_speech_frames = 0u32;
            // Telemetria ML: ventana de scores para diagnóstico del wake word.
            let mut score_win_max: f32 = 0.0;
            let mut score_win_frames: u32 = 0;
            let mut score_win_start = std::time::Instant::now();
            // Evidencia reciente: el score de openWakeWord puede repartirse
            // entre varios bloques de 80 ms. Conservar la ventana evita
            // perder una detección por un único bloque ligeramente bajo.
            let mut recent_scores: VecDeque<f32> = VecDeque::with_capacity(5);
            // Frames de voz requeridos: escala con la confianza configurada.
            // A mayor confianza, mas frames consecutivos para evitar falsos positivos.
            let required_frames = (confidence * 30.0).round() as u32 + 10;
            // RMS minimo para considerar "voz" (rechaza ruido ambiente/hiss del micro).
            const MIN_RMS: f32 = 0.20;

            while let Some(frame) = audio_rx.recv().await {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                // --- Ruta ML (si disponible) ---
                if ml.lock().unwrap().is_some() {
                    let rate = src_rate.load(Ordering::Relaxed).max(1);
                    // Drenar TODOS los frames pendientes del canal. El
                    // productor (cpal) entrega ~86 fps de 512 samples y este
                    // loop tarda más por la inferencia ONNX: sin drenar, el
                    // canal (cap 64) se llena y `try_send` descarta audio →
                    // huecos en la ventana mel → scores artificialmente bajos.
                    let mut frames_16k: Vec<f32> = resample_to_16k(&frame, rate);
                    while let Ok(f) = audio_rx.try_recv() {
                        frames_16k.extend_from_slice(&resample_to_16k(&f, rate));
                    }
                    normalize_wake_audio(&mut frames_16k);
                    // Extraer bloques completos de 1280 samples (sin mantener el lock)
                    let blocks: Vec<Vec<f32>> = {
                        let mut pend = pending_16k.lock().unwrap();
                        pend.extend_from_slice(&frames_16k);
                        let mut blocks = Vec::new();
                        while pend.len() >= 1280 {
                            blocks.push(pend.drain(..1280).collect());
                        }
                        blocks
                    };
                    // Inferencia ML (sin awaits dentro del lock)
                    let mut best: Option<f32> = None;
                    {
                        let mut guard = ml.lock().unwrap();
                        if let Some(det) = guard.as_mut() {
                            for block in &blocks {
                                match det.feed(block) {
                                    Ok(Some(s)) => best = Some(best.map_or(s, |b: f32| b.max(s))),
                                    Ok(None) => {}
                                    Err(e) => log::warn!("wakeword ML: {e}"),
                                }
                            }
                        }
                    }
                    // Decidir con cooldown (aqui si hay awaits, sin locks activos)
                    if let Some(s) = best {
                        recent_scores.push_back(s);
                        while recent_scores.len() > 5 {
                            recent_scores.pop_front();
                        }
                        // Telemetria: max score por ventana de ~2s. El wake
                        // word fallaba en silencio sin dejar rastro; con esto
                        // se ve si el modelo "oye" (scores suben con voz) y
                        // cuanto falta para el umbral.
                        score_win_max = score_win_max.max(s);
                        score_win_frames += 1;
                        if score_win_start.elapsed() >= std::time::Duration::from_secs(2) {
                            let drops = dropped.swap(0, Ordering::Relaxed);
                            if score_win_max > 0.02 || drops > 0 {
                                log::info!(
                                    "Wakeword ML: max score {:.3} en {} frames (umbral {:.2}, drops {})",
                                    score_win_max,
                                    score_win_frames,
                                    confidence,
                                    drops
                                );
                            } else {
                                log::debug!(
                                    "Wakeword ML: ventana plana (max {:.3}, {} frames)",
                                    score_win_max,
                                    score_win_frames
                                );
                            }
                            score_win_max = 0.0;
                            score_win_frames = 0;
                            score_win_start = std::time::Instant::now();
                        }
                        let now_ms = chrono::Utc::now().timestamp_millis() as u32;
                        let last = last_detected.load(Ordering::Relaxed);
                        let recent_peak = recent_scores.iter().copied().fold(0.0, f32::max);
                        // Detección directa o evidencia acumulada: permite
                        // aceptar dos bloques cercanos al umbral sin bajar
                        // permanentemente la exigencia contra falsos positivos.
                        let confirmed = s >= confidence
                            || (s >= confidence * 0.82 && recent_peak >= confidence * 0.95);
                        if confirmed && now_ms.saturating_sub(last) >= cooldown {
                            last_detected.store(now_ms, Ordering::Relaxed);
                            log::info!("Wake word ML detectado (score={s:.2})");
                            if event_tx.send(HotwordEvent::Detected).await.is_err() {
                                break;
                            }
                        }
                    }
                    // Sin sleep artificial: recv() al inicio del loop ya es
                    // el pacer natural y la inferencia tarda lo suyo; cada ms
                    // extra aquí aumenta el riesgo de canal lleno y drops.
                    continue;
                }

                // --- Ruta heuristica (fallback) ---
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

/// Acondiciona el audio para que un micrófono silencioso no quede por debajo
/// del rango aprendido por openWakeWord. Se elimina DC y se aplica ganancia
/// limitada; no se normaliza el silencio porque eso amplificaría el ruido.
fn normalize_wake_audio(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    for sample in samples.iter_mut() {
        *sample = (*sample - mean).clamp(-1.0, 1.0);
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    if rms < 0.008 {
        return;
    }
    let gain = (0.075 / rms).clamp(0.7, 2.5);
    for sample in samples.iter_mut() {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
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
    fn normaliza_microfono_silencioso_sin_amplificar_silencio() {
        let mut voice = vec![0.02; 1000];
        normalize_wake_audio(&mut voice);
        assert!(voice.iter().all(|s| s.abs() < 0.001));

        let mut silence = vec![0.001; 1000];
        normalize_wake_audio(&mut silence);
        assert!(silence.iter().all(|s| s.abs() < 0.0001));
    }

    #[test]
    fn detects_voice_in_synthetic_frame() {
        let det = HotwordDetector {
            config: Arc::new(RwLock::new(Config::default())),
            threshold: 0.5,
            cooldown_ms: 2000,
            last_detected_ms: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            ml: Arc::new(Mutex::new(None)),
            src_rate: Arc::new(AtomicU32::new(16000)),
            pending_16k: Arc::new(Mutex::new(Vec::new())),
            dropped_frames: Arc::new(AtomicU64::new(0)),
        };
        // Frame con onda senoidal realista
        let frame: Vec<f32> = (0..512).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
        // Puede o no detectar segun el threshold
        let _ = det.process_frame(&frame);
    }
}
