//! Simulación de la cadena real del wake word:
//! WAV (piper) → upsample lineal a 44100 (como si fuera el micro BT)
//! → `resample_to_16k` (la cadena del app) → OwwDetector.
//!
//! Aísla la pregunta: ¿el resample del app destruye la detección?
//!
//! Hay que generar primero /tmp/hey_jarvis_test.wav con:
//!   ./target/debug/test_wakeword
//!
//! ```bash
//! cargo test --test wake_resample_sim -- --ignored --nocapture
//! ```

use kde_assistant_lib::backend::audio_capture::resample_to_16k;
use kde_assistant_lib::backend::wakeword_ml::OwwDetector;
use std::path::PathBuf;

fn read_wav_mono_f32(path: &str) -> (Vec<f32>, u32) {
    let bytes = std::fs::read(path).expect("leyendo WAV de prueba");
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
    let spec = reader.spec();
    let ch = spec.channels as usize;
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.unwrap_or(0) as f32 / 32768.0)
            .collect(),
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
    };
    let mono: Vec<f32> = if ch == 1 {
        samples
    } else {
        samples
            .chunks(ch)
            .map(|c| c.iter().sum::<f32>() / ch as f32)
            .collect()
    };
    (mono, spec.sample_rate)
}

/// Resample lineal genérico (como el del app pero a cualquier rate).
fn resample_linear(samples: &[f32], src: u32, dst: u32) -> Vec<f32> {
    if src == dst {
        return samples.to_vec();
    }
    let ratio = src as f64 / dst as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let idx = i as f64 * ratio;
        let i0 = idx.floor() as usize;
        let i1 = (i0 + 1).min(samples.len() - 1);
        let f = (idx - i0 as f64) as f32;
        out.push(samples[i0] + (samples[i1] - samples[i0]) * f);
    }
    out
}

fn max_score(det: &mut OwwDetector, samples: &[f32]) -> f32 {
    let mut max = 0.0f32;
    for chunk in samples.chunks(1280) {
        let mut block = chunk.to_vec();
        block.resize(1280, 0.0);
        if let Ok(Some(s)) = det.feed(&block) {
            max = max.max(s);
        }
    }
    max
}

#[test]
#[ignore]
fn resample_44100_vs_directo() {
    let models_dir = PathBuf::from(std::env::var("HOME").unwrap())
        .join(".local/share/kde-assistant/models/wakeword");
    if !models_dir.join("hey_jarvis_v0.1.onnx").exists() {
        eprintln!("modelos wakeword no presentes; skip");
        return;
    }

    let (wav, src_rate) = read_wav_mono_f32("/tmp/hey_jarvis_test.wav");
    println!("WAV: {} samples @ {}Hz", wav.len(), src_rate);

    // 1. Directo a 16k (referencia: esto ya detectaba 0.9978)
    let at16 = resample_linear(&wav, src_rate, 16000);
    let mut det_a = OwwDetector::new(&models_dir, 0.4).unwrap();
    let score_direct = max_score(&mut det_a, &at16);
    println!("score directo 16k:  {score_direct:.4}");

    // 2. El camino del app: subir a 44100 (micro BT) y volver a bajar con
    //    resample_to_16k del app.
    let at44100 = resample_linear(&wav, src_rate, 44100);
    let back_16k = resample_to_16k(&at44100, 44100);
    let mut det_b = OwwDetector::new(&models_dir, 0.4).unwrap();
    let score_sim = max_score(&mut det_b, &back_16k);
    println!("score simulado 44.1k->16k (cadena app): {score_sim:.4}");

    // El umbral real del usuario es 0.4; si el score simulado cae mucho,
    // la cadena de resample del app es la culpable.
    println!(
        "veredicto: {}",
        if score_sim >= 0.4 {
            "CADENA OK (mirar micro BT)"
        } else {
            "RESAMPLE MATA LA DETECCION (bug en cadena)"
        }
    );
}
