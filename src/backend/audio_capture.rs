//! Audio Capture - Captura audio del microfono con cpal
//!
//! Provee:
//! - Stream de samples f32 (mono, 16kHz ideal para STT)
//! - RMS (amplitud) por ventana para visualizacion (VoiceOrb)
//! - VAD (Voice Activity Detection) basico por umbral de energia
//!
//! Uso:
//!   let mut cap = AudioCapture::new()?;
//!   cap.start(|frame| { /* procesar samples */ })?;
//!   let amp = cap.amplitude();  // 0.0 a 1.0
//!   cap.stop();

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Amplitud normalizada (0.0 a 1.0) calculada por RMS.
/// atomic para lectura lock-free desde el thread de UI.
pub struct AmplitudeMeter {
    rms: AtomicU32, // f32 bits
    pub window_size: usize,
}

impl AmplitudeMeter {
    pub fn new(window_size: usize) -> Self {
        Self {
            rms: AtomicU32::new(0.0f32.to_bits()),
            window_size,
        }
    }

    pub fn update(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();
        // Normalizar: f32 max ~1.0, voz normal 0.01-0.3
        // Aplicamos curva para que voz se sienta entre 0.3-0.9
        let normalized = (rms * 6.0).min(1.0);
        self.rms.store(normalized.to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.rms.load(Ordering::Relaxed))
    }
}

pub struct AudioCapture {
    stream: Option<Stream>,
    is_running: Arc<AtomicBool>,
    pub meter: Arc<AmplitudeMeter>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioCapture {
    pub fn new() -> Result<Self> {
        Ok(Self {
            stream: None,
            is_running: Arc::new(AtomicBool::new(false)),
            meter: Arc::new(AmplitudeMeter::new(1600)), // 100ms a 16kHz
            sample_rate: 0,
            channels: 0,
        })
    }

    /// Lista los dispositivos de entrada disponibles.
    pub fn list_input_devices() -> Result<Vec<String>> {
        let host = cpal::default_host();
        let devices = host.input_devices().context("listando input devices")?;
        Ok(devices.filter_map(|d| d.name().ok()).collect())
    }

    /// Inicia la captura. `callback` se invoca con cada frame de samples (mono, f32).
    /// Inicia la captura. Si `device_name` coincide con un dispositivo de
    /// entrada, lo usa; si no, el por defecto (con aviso).
    pub fn start<F>(&mut self, device_name: Option<&str>, mut callback: F) -> Result<()>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = match device_name.map(str::trim).filter(|s| !s.is_empty()) {
            Some(wanted) => match host
                .input_devices()
                .context("listando input devices")?
                .find(|d| d.name().as_deref().unwrap_or("") == wanted)
            {
                Some(d) => d,
                None => {
                    log::warn!("Micrófono '{wanted}' no encontrado, usando el por defecto");
                    host.default_input_device()
                        .context("no hay dispositivo de entrada por defecto")?
                }
            },
            None => host
                .default_input_device()
                .context("no hay dispositivo de entrada por defecto")?,
        };

        let device_name = device.name().unwrap_or_else(|_| "?".to_string());
        log::info!("AudioCapture: usando dispositivo '{device_name}'");

        let config = device
            .default_input_config()
            .context("obteniendo config de input")?;

        self.sample_rate = config.sample_rate().0;
        self.channels = config.channels();

        log::info!(
            "AudioCapture: sample_rate={} channels={} format={:?}",
            self.sample_rate,
            self.channels,
            config.sample_format()
        );

        let meter = self.meter.clone();
        let is_running = self.is_running.clone();

        let stream = match config.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), move |data| {
                callback(data);
                meter.update(data);
            })?,
            SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), move |data| {
                callback(data);
                meter.update(data);
            })?,
            SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), move |data| {
                callback(data);
                meter.update(data);
            })?,
            _ => anyhow::bail!("Formato de sample no soportado"),
        };

        stream.play().context("iniciando stream")?;
        self.stream = Some(stream);
        is_running.store(true, Ordering::Relaxed);

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(s) = self.stream.take() {
            drop(s);
        }
        self.is_running.store(false, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Amplitud RMS normalizada (0.0 a 1.0) para visualizacion.
    pub fn amplitude(&self) -> f32 {
        self.meter.get()
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut on_frame: impl FnMut(&[f32]) + Send + 'static,
) -> Result<Stream>
where
    T: Sample + cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let err_fn = |err| log::error!("cpal stream error: {err}");

    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Convertir a mono f32
            let frame_size = 512;
            let mut mono: Vec<f32> = Vec::with_capacity(data.len() / channels + 1);
            for chunk in data.chunks(channels * frame_size) {
                mono.clear();
                for sample in chunk.chunks(channels) {
                    // Promedio de canales -> mono
                    let sum: f32 = sample.iter().map(|s| f32::from_sample(*s)).sum();
                    mono.push(sum / channels as f32);
                }
                on_frame(&mono);
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}

/// VAD basico: detecta si hay voz en un frame segun umbral de RMS.
pub struct SimpleVad {
    pub threshold: f32,
    pub min_speech_frames: u32,
    pub silence_frames: u32,
    counter: u32,
    is_speaking: bool,
}

impl SimpleVad {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            min_speech_frames: 3,
            silence_frames: 8,
            counter: 0,
            is_speaking: false,
        }
    }

    /// Procesa un frame. Retorna true si se detecta habla.
    pub fn process(&mut self, frame: &[f32]) -> bool {
        if frame.is_empty() {
            return false;
        }
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
        let has_voice = rms > self.threshold;

        if has_voice {
            self.counter = self.counter.saturating_add(1);
            if self.counter >= self.min_speech_frames {
                self.is_speaking = true;
            }
        } else {
            self.counter = 0;
            if self.is_speaking {
                self.silence_frames = self.silence_frames.saturating_sub(1);
                if self.silence_frames == 0 {
                    self.is_speaking = false;
                    self.silence_frames = 8;
                }
            }
        }
        self.is_speaking
    }

    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    pub fn reset(&mut self) {
        self.counter = 0;
        self.is_speaking = false;
        self.silence_frames = 8;
    }
}

/// Resamplea audio mono f32 desde `src_rate` a 16kHz (requerido por whisper).
/// Usa interpolacion lineal. Retorna un buffer con la duracion preservada.
pub fn resample_to_16k(samples: &[f32], src_rate: u32) -> Vec<f32> {
    if src_rate == 16000 {
        return samples.to_vec();
    }
    if samples.len() < 2 {
        return samples.to_vec();
    }
    let ratio = src_rate as f64 / 16000.0;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = samples.len() - 1;
    for i in 0..out_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(last);
        let frac = (src_idx - idx0 as f64) as f32;
        let s0 = samples[idx0];
        let s1 = samples[idx1];
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}

/// Convierte samples interleaved multi-canal a mono f32.
pub fn convert_to_mono<T>(interleaved: &[T], channels: usize) -> Vec<f32>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let mut mono = Vec::with_capacity(interleaved.len() / channels);
    for sample in interleaved.chunks(channels) {
        let sum: f32 = sample.iter().map(|s| f32::from_sample(*s)).sum();
        mono.push(sum / channels as f32);
    }
    mono
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplitude_meter_normalizes() {
        let m = AmplitudeMeter::new(8);
        m.update(&[0.0; 8]);
        assert!(m.get() < 0.01);
        m.update(&[0.5; 8]);
        let v = m.get();
        assert!(v > 0.5 && v <= 1.0);
    }

    #[test]
    fn vad_detects_voice() {
        let mut vad = SimpleVad::new(0.05);
        // Silencio
        for _ in 0..5 {
            vad.process(&[0.001; 160]);
        }
        assert!(!vad.is_speaking());

        // Voz
        for _ in 0..10 {
            vad.process(&[0.3; 160]);
        }
        assert!(vad.is_speaking());
    }
}
