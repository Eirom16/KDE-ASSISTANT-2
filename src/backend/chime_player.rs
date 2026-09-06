//! Chime Player - Reproduce chimes sutiles estilo Siri
//!
//! Si los archivos WAV no existen, los genera sinteticamente al primer uso
//! (tonos senoidales breves que suenan a feedback acustico).
//!
//! - activate: tono ascendente 200->600Hz, 150ms
//! - process:  pulso suave 440Hz, 100ms
//! - deactivate: tono descendente 600->200Hz, 200ms

use anyhow::{Context, Result};
use hound::{SampleFormat, WavSpec};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ChimePlayer {
    pub enabled: AtomicBool,
    pub chimes_dir: PathBuf,
}

impl ChimePlayer {
    pub async fn new() -> Result<Self> {
        let chimes_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("sin data_dir"))?
            .join("kde-assistant/chimes");
        tokio::fs::create_dir_all(&chimes_dir).await.ok();

        let player = Self {
            enabled: AtomicBool::new(true),
            chimes_dir,
        };

        // Generar WAVs si no existen
        if let Err(e) = player.ensure_chimes().await {
            log::warn!("No se pudieron generar chimes: {e}");
        }

        Ok(player)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    async fn ensure_chimes(&self) -> Result<()> {
        for (name, gen) in &[
            ("activate", Self::gen_activate as fn() -> Vec<f32>),
            ("process", Self::gen_process as fn() -> Vec<f32>),
            ("deactivate", Self::gen_deactivate as fn() -> Vec<f32>),
        ] {
            let path = self.chimes_dir.join(format!("{name}.wav"));
            if !path.exists() {
                let samples = gen();
                write_wav(&path, &samples)?;
                log::info!("Chime generado: {}", path.display());
            }
        }
        Ok(())
    }

    /// Reproduce un chime por nombre. No bloquea.
    pub fn play(&self, name: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let path = self.chimes_dir.join(format!("{name}.wav"));
        if !path.exists() {
            return;
        }
        let name_owned = name.to_string();
        // Spawn en thread aparte para no bloquear
        std::thread::spawn(move || {
            if let Err(e) = play_wav_file(&path) {
                log::warn!("Error reproduciendo chime {name_owned}: {e}");
            }
        });
    }

    pub fn play_activate(&self) {
        log::debug!("[chime] activate");
        self.play("activate");
    }

    pub fn play_process(&self) {
        log::debug!("[chime] process");
        self.play("process");
    }

    pub fn play_deactivate(&self) {
        log::debug!("[chime] deactivate");
        self.play("deactivate");
    }

    // === Generadores de tonos sinteticos ===

    /// Tono ascendente 200Hz -> 800Hz en 150ms
    fn gen_activate() -> Vec<f32> {
        let sr = 44100;
        let dur = 0.18;
        let n = (sr as f32 * dur) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let p = i as f32 / n as f32; // 0 a 1
                                         // Frecuencia sube de 220 a 880Hz
            let freq = 220.0 + (880.0 - 220.0) * p;
            // Envolvente: ataque 20ms, sustain, release 30ms
            let env = envelope(t, 0.02, dur - 0.03, 0.03);
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.3;
            out.push(sample);
        }
        out
    }

    /// Pulso breve 440Hz, 100ms
    fn gen_process() -> Vec<f32> {
        let sr = 44100;
        let dur = 0.10;
        let n = (sr as f32 * dur) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let env = envelope(t, 0.01, dur - 0.02, 0.02);
            // Frecuencia: dos tonos superpuestos para riqueza
            let s1 = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            let s2 = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.5;
            out.push((s1 + s2) * env * 0.25);
        }
        out
    }

    /// Tono descendente 800Hz -> 220Hz en 200ms
    fn gen_deactivate() -> Vec<f32> {
        let sr = 44100;
        let dur = 0.20;
        let n = (sr as f32 * dur) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let p = i as f32 / n as f32;
            let freq = 800.0 - (800.0 - 220.0) * p;
            let env = envelope(t, 0.02, dur - 0.04, 0.04);
            out.push((2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.3);
        }
        out
    }
}

/// Envolvente ADSR simplificada: attack, sustain, release
fn envelope(t: f32, attack: f32, sustain: f32, release: f32) -> f32 {
    if t < attack {
        t / attack
    } else if t < attack + sustain {
        1.0
    } else if t < attack + sustain + release {
        1.0 - (t - attack - sustain) / release
    } else {
        0.0
    }
}

fn write_wav(path: &PathBuf, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(v)?;
    }
    writer.finalize()?;
    Ok(())
}

fn play_wav_file(path: &PathBuf) -> Result<()> {
    let (_stream, handle) = rodio::OutputStream::try_default()
        .map_err(|e| anyhow::anyhow!("abriendo output stream: {e}"))?;
    let file = BufReader::new(File::open(path).context("abriendo wav")?);
    // play_once acepta un Read+Seek; el WAV se decodifica internamente
    let sink = handle
        .play_once(file)
        .map_err(|e| anyhow::anyhow!("play_once: {e}"))?;
    sink.set_volume(0.5);
    sink.sleep_until_end();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ensure_chimes_creates_files() {
        let p = ChimePlayer::new().await.unwrap();
        assert!(p.chimes_dir.join("activate.wav").exists());
        assert!(p.chimes_dir.join("process.wav").exists());
        assert!(p.chimes_dir.join("deactivate.wav").exists());
    }
}
