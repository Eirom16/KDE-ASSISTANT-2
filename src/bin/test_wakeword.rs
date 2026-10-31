//! Offline diagnostic: MODELS_DIR ONNX_RUNTIME WAV [THRESHOLD]. Never captures or plays audio.
use anyhow::{Context, Result};
use kde_assistant_lib::backend::{audio_capture::resample_to_16k, wakeword_ml::OwwDetector};
use std::{path::Path, time::Instant};
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(
        (4..=5).contains(&args.len()),
        "Uso: test_wakeword MODELS_DIR ONNX_RUNTIME WAV [THRESHOLD]"
    );
    let threshold = args
        .get(4)
        .map(|s| s.parse::<f32>())
        .transpose()?
        .unwrap_or(0.8);
    anyhow::ensure!(
        threshold.is_finite() && (0.1..=1.0).contains(&threshold),
        "Umbral inválido"
    );
    let mut wav = hound::WavReader::open(&args[3])?;
    let spec = wav.spec();
    anyhow::ensure!(
        spec.channels == 1
            && spec.bits_per_sample == 16
            && spec.sample_format == hound::SampleFormat::Int,
        "Se requiere WAV PCM16 mono"
    );
    anyhow::ensure!(
        wav.duration() <= spec.sample_rate * 120,
        "WAV demasiado largo"
    );
    let samples: Vec<f32> = wav
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0))
        .collect::<std::result::Result<_, _>>()?;
    let audio = resample_to_16k(&samples, spec.sample_rate);
    std::env::set_var("ORT_DYLIB_PATH", &args[2]);
    let start = Instant::now();
    let mut detector = OwwDetector::new(Path::new(&args[1]), threshold)?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let mut max_score = 0.0f32;
    let mut scores = Vec::new();
    for chunk in audio.chunks(1280) {
        let mut frame = chunk.to_vec();
        frame.resize(1280, 0.0);
        let tick = Instant::now();
        let score = detector.feed(&frame)?.context("Sin score")?;
        max_score = max_score.max(score);
        scores.push((score, tick.elapsed().as_secs_f64() * 1000.0));
    }
    let feed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut times: Vec<f64> = scores.iter().map(|s| s.1).collect();
    times.sort_by(f64::total_cmp);
    let p95_ms = times
        .get(times.len() * 95 / 100)
        .copied()
        .unwrap_or_default();
    let rss = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let rss = rss
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .unwrap_or("unknown");
    println!(
        "{}",
        serde_json::json!({"max_score":max_score,"load_ms":load_ms,"feed_ms":feed_ms,"audio_seconds":audio.len() as f64 / 16000.0,"frame_p95_ms":p95_ms,"peak_rss":rss,"scores":scores})
    );
    Ok(())
}
