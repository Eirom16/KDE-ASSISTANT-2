//! Test de STT (whisper-rs) + round-trip TTS -> STT
//! Uso: cargo run --bin test_stt
//!
//! 1. Verifica que el modelo whisper ggml-base.bin existe
//! 2. Sintetiza una frase con piper-tts (22kHz)
//! 3. La resamplea a 16kHz mono
//! 4. La transcribe con whisper y muestra el resultado

use anyhow::{Context, Result};
use kde_assistant_lib::backend::speech_service::SpeechService;
use kde_assistant_lib::backend::stt::WhisperEngine;
use std::sync::Arc;
use tokio::sync::RwLock;

use kde_assistant_lib::models::Config;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 1. Verificar modelo
    let model_path = kde_assistant_lib::backend::speech_service::whisper_model_path()?;
    log::info!("Modelo whisper en {}", model_path.display());
    if !model_path.exists() {
        anyhow::bail!(
            "Modelo whisper no encontrado en {}. Descargalo con el model_downloader.",
            model_path.display()
        );
    }

    // 2. Sintetizar una frase con piper-tts
    let cfg = Arc::new(RwLock::new(Config::default()));
    let speech = SpeechService::new(cfg.clone()).await?;
    let text = "Hola, soy KDE Assistant y esta es una prueba de reconocimiento de voz.";
    log::info!("Sintetizando con TTS: {text}");
    let wav_bytes = speech.synthesize(text).await?;
    log::info!("TTS genero {} bytes WAV", wav_bytes.len());

    // 3. Decodificar WAV y convertir a 16kHz mono f32
    let samples_16k = wav_to_16k_mono_f32(&wav_bytes)?;
    log::info!(
        "Audio convertido a 16kHz: {} samples ({:.2}s)",
        samples_16k.len(),
        samples_16k.len() as f32 / 16000.0
    );

    // 4. Transcribir con whisper
    let engine = WhisperEngine::new(model_path.clone(), None)?;
    log::info!("Transcribiendo con whisper (carga el modelo, puede tardar)...");
    let start = std::time::Instant::now();
    let transcript = engine.transcribe(&samples_16k, Some("es"))?;
    let elapsed = start.elapsed();

    log::info!(
        "Transcripcion ({}ms): {:?}",
        elapsed.as_millis(),
        transcript
    );
    println!("\n=== RESULTADO STT ===");
    println!("Original TTS: {text}");
    println!("Transcripcion: {transcript}");
    println!("=====================");

    Ok(())
}

/// Convierte bytes WAV a samples mono f32 16kHz (resample por decimacion lineal).
fn wav_to_16k_mono_f32(wav_bytes: &[u8]) -> Result<Vec<f32>> {
    use std::io::Cursor;

    let cursor = Cursor::new(wav_bytes.to_vec());
    let mut reader = hound::WavReader::new(cursor).context("leyendo WAV")?;
    let spec = reader.spec();
    let src_rate = spec.sample_rate;
    let channels = spec.channels as usize;
    log::info!(
        "WAV: {} Hz, {} canales, {} bits",
        src_rate,
        channels,
        spec.bits_per_sample
    );

    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap_or(0)).collect();

    // Convertir a mono f32
    let mono: Vec<f32> = if channels == 1 {
        samples.iter().map(|&s| s as f32 / 32768.0).collect()
    } else {
        let mut out = Vec::with_capacity(samples.len() / channels);
        for chunk in samples.chunks(channels) {
            let avg = chunk.iter().map(|&s| s as f32).sum::<f32>() / channels as f32;
            out.push(avg / 32768.0);
        }
        out
    };

    // Resample lineal a 16kHz
    if src_rate == 16000 {
        return Ok(mono);
    }
    let ratio = src_rate as f64 / 16000.0;
    let out_len = (mono.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(mono.len() - 1);
        let frac = (src_idx - idx0 as f64) as f32;
        let s0 = mono[idx0];
        let s1 = mono[idx1];
        out.push(s0 + (s1 - s0) * frac);
    }
    Ok(out)
}
