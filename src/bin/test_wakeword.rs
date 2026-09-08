//! Test del wake word ML (openWakeWord hey_jarvis)
//! Uso: cargo run --bin test_wakeword
//!
//! 1. Sintetiza "hey jarvis" con piper (voz inglesa)
//! 2. Lo convierte a 16kHz mono
//! 3. Lo pasa por OwwDetector en chunks como streaming real
//! 4. Muestra los scores y si detecta

use anyhow::{Context, Result};
use kde_assistant_lib::backend::wakeword_ml::OwwDetector;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let models_dir = PathBuf::from("/home/eirom/.local/share/kde-assistant/models/wakeword");
    let mut det = OwwDetector::new(&models_dir, 0.5)?;
    log::info!("Detector ML cargado");

    // Sintetizar "hey jarvis" con piper ingles
    let piper_bin = "/usr/bin/piper-tts";
    let model = "/home/eirom/.local/share/kde-assistant/models/piper/en_US-amy-medium.onnx";
    let config = "/home/eirom/.local/share/kde-assistant/models/piper/en_US-amy-medium.onnx.json";
    let out_wav = "/tmp/hey_jarvis_test.wav";

    let text = "Hey Jarvis.";
    log::info!("Sintetizando: {text}");
    let mut child = tokio::process::Command::new(piper_bin)
        .arg("-m")
        .arg(model)
        .arg("-c")
        .arg(config)
        .arg("-f")
        .arg(out_wav)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("lanzando piper-tts")?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(text.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        anyhow::bail!("piper fallo: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Convertir WAV a 16kHz mono f32
    let samples = wav_to_16k_mono(out_wav)?;
    log::info!(
        "Audio: {} samples 16kHz ({:.2}s)",
        samples.len(),
        samples.len() as f32 / 16000.0
    );

    // Pasar por el detector en chunks de 1280 (como streaming)
    let mut max_score: f32 = 0.0;
    let mut n_frames = 0;
    for chunk in samples.chunks(1280) {
        // Pad ultimo chunk si es corto
        let mut block = chunk.to_vec();
        while block.len() < 1280 {
            block.push(0.0);
        }
        if let Some(score) = det.feed(&block)? {
            n_frames += 1;
            if score > max_score {
                max_score = score;
            }
            if score > 0.01 {
                log::info!("frame {n_frames}: score={score:.4}");
            }
        }
    }

    println!("\n=== RESULTADO WAKE WORD ===");
    println!("Frames procesados: {n_frames}");
    println!("Score maximo: {max_score:.4} (threshold 0.5)");
    println!(
        "Deteccion: {}",
        if max_score >= 0.5 {
            "SI - 'hey jarvis' detectado"
        } else {
            "NO - no supero el umbral"
        }
    );
    println!("============================");

    // Tambien probar con silencio (debe dar ~0)
    let silence = vec![0.0f32; 1280 * 20];
    let mut det2 = OwwDetector::new(&models_dir, 0.5)?;
    let mut max_silence: f32 = 0.0;
    for chunk in silence.chunks(1280) {
        if let Some(s) = det2.feed(chunk)? {
            max_silence = max_silence.max(s);
        }
    }
    println!("Score maximo en silencio: {max_silence:.4} (debe ser ~0)");

    Ok(())
}

fn wav_to_16k_mono(path: &str) -> Result<Vec<f32>> {
    use std::io::Cursor;
    let bytes = std::fs::read(path)?;
    let cursor = Cursor::new(bytes);
    let mut reader = hound::WavReader::new(cursor).context("leyendo WAV")?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let src_rate = spec.sample_rate;
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap_or(0)).collect();
    let mono: Vec<f32> = if channels == 1 {
        samples.iter().map(|&s| s as f32 / 32768.0).collect()
    } else {
        samples
            .chunks(channels)
            .map(|c| c.iter().map(|&s| s as f32).sum::<f32>() / channels as f32 / 32768.0)
            .collect()
    };
    if src_rate == 16000 {
        return Ok(mono);
    }
    let ratio = src_rate as f64 / 16000.0;
    let out_len = (mono.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let idx = i as f64 * ratio;
        let i0 = idx.floor() as usize;
        let i1 = (i0 + 1).min(mono.len() - 1);
        let f = (idx - i0 as f64) as f32;
        out.push(mono[i0] + (mono[i1] - mono[i0]) * f);
    }
    Ok(out)
}
