//! Offline voice benchmark. Never opens an audio device or contacts a provider.
use anyhow::{Context, Result};
use kde_assistant_lib::backend::{
    audio_capture::resample_to_16k,
    stt::{DecodeOptions, WhisperEngine},
    voice_coordinator::validate_model,
};
use std::{path::PathBuf, time::Instant};
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--vosk") {
        return native_vosk(&args);
    }
    anyhow::ensure!(
        matches!(args.len(), 5 | 8 | 10),
        "Uso: bench_voice MODELO WAV REPETICIONES SALIDA_JSON [HILOS AUDIO_CTX TEMP_INC [--phase-file RUTA]] (offline)"
    );
    let model = PathBuf::from(&args[1]);
    validate_model(&model)?;
    let repeats: usize = args[3].parse()?;
    let options = if args.len() >= 8 {
        DecodeOptions {
            threads: args[5].parse()?,
            audio_ctx: args[6].parse()?,
            temperature_inc: args[7].parse()?,
        }
    } else {
        DecodeOptions::default()
    };
    let phase = if args.len() == 10 {
        anyhow::ensure!(args[8] == "--phase-file", "Opción de diagnóstico inválida");
        Some(PathBuf::from(&args[9]))
    } else {
        None
    };
    anyhow::ensure!(
        phase.is_none() || repeats == 1,
        "El probe de cancelación requiere una repetición"
    );
    anyhow::ensure!((1..=100).contains(&repeats), "Repeticiones 1..100");
    let conversion = Instant::now();
    let mut wav = hound::WavReader::open(&args[2])?;
    let spec = wav.spec();
    anyhow::ensure!(
        spec.bits_per_sample == 16
            && spec.sample_format == hound::SampleFormat::Int
            && spec.channels > 0,
        "WAV PCM16 requerido"
    );
    anyhow::ensure!(
        wav.duration() <= spec.sample_rate * 120,
        "Audio demasiado largo"
    );
    let pcm: Vec<f32> = wav
        .samples::<i16>()
        .map(|s| s.map(|s| s as f32 / 32768.0))
        .collect::<std::result::Result<_, _>>()?;
    let mono: Vec<f32> = pcm
        .chunks(spec.channels as usize)
        .map(|c| c.iter().sum::<f32>() / c.len() as f32)
        .collect();
    let audio = resample_to_16k(&mono, spec.sample_rate);
    let conversion_ms = conversion.elapsed().as_secs_f64() * 1000.0;
    let engine = WhisperEngine::new(model, None)?;
    let cold = Instant::now();
    engine.ensure_loaded()?;
    let load_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let mut runs = Vec::new();
    for i in 0..repeats {
        let start = Instant::now();
        let cpu_before = cpu_ticks()?;
        let measured = engine.measure_with_observer(&audio, Some("es"), options, || {
            if let Some(path) = &phase {
                use std::io::Write;
                let pending = path.with_extension("partial");
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&pending)?;
                file.write_all(b"inference")?;
                drop(file);
                std::fs::hard_link(&pending, path)?;
                std::fs::remove_file(pending)?;
            }
            Ok(())
        })?;
        let available_ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "run {} inference_ms {:.3}",
            i,
            start.elapsed().as_secs_f64() * 1000.0
        );
        runs.push(serde_json::json!({"run":i,"inference_ms":measured.inference_ms,"text_available_ms":available_ms,"cpu_ticks":cpu_ticks()?.saturating_sub(cpu_before),"measurement":measured}));
    }
    let mut times: Vec<f64> = runs
        .iter()
        .filter_map(|r| r["inference_ms"].as_f64())
        .collect();
    times.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        times[((times.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(times.len() - 1)]
    };
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let smaps = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
    let result = serde_json::json!({"evidence":"AUTOMATED_AUDIO_FIXTURE","source_wav":args[2],"audio_seconds":audio.len() as f64/16000.0,"conversion_ms":conversion_ms,"cold_model_load_ms":load_ms,"warm_runs":runs,"warm_p50_ms":percentile(0.5),"warm_p95_ms":percentile(0.95),"warm_min_ms":times[0],"warm_max_ms":times[times.len()-1],"n":repeats,"decode_options":options,"whisper_cpp_version":whisper_rs::WHISPER_CPP_VERSION,"cpp_system_info":whisper_rs::print_system_info(),"rust_debug_assertions":cfg!(debug_assertions),"proc_status":status.lines().filter(|l|l.starts_with("VmRSS:")||l.starts_with("VmHWM:")).collect::<Vec<_>>(),"pss":smaps.lines().filter(|l|l.starts_with("Pss:")).collect::<Vec<_>>()});
    std::fs::write(&args[4], serde_json::to_vec_pretty(&result)?).context("guardando benchmark")?;
    Ok(())
}

fn native_vosk(args: &[String]) -> Result<()> {
    let conversion = Instant::now();
    anyhow::ensure!(
        matches!(args.len(), 6 | 8),
        "Uso: bench_voice --vosk LIB MODELO WAV JSON [--phase-file RUTA]"
    );
    let phase = if args.len() == 8 {
        anyhow::ensure!(args[6] == "--phase-file", "Probe Vosk inválido");
        Some(PathBuf::from(&args[7]))
    } else {
        None
    };
    let mut wav = hound::WavReader::open(&args[4])?;
    let spec = wav.spec();
    anyhow::ensure!(
        spec.channels == 1
            && spec.sample_rate == 16000
            && spec.bits_per_sample == 16
            && spec.sample_format == hound::SampleFormat::Int
            && wav.duration() <= 120 * 16000,
        "WAV PCM16 mono 16 kHz acotado requerido"
    );
    let pcm = wav
        .samples::<i16>()
        .map(|s| s.map(i16::to_le_bytes))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let started = Instant::now();
    let inference_started = std::cell::Cell::new(None);
    let text = kde_assistant_lib::backend::vosk::transcribe_with_observer(
        std::path::Path::new(&args[2]),
        std::path::Path::new(&args[3]),
        &pcm,
        || {
            inference_started.set(Some(Instant::now()));
            if let Some(path) = &phase {
                use std::io::Write;
                let pending = path.with_extension("partial");
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&pending)?;
                file.write_all(b"inference")?;
                drop(file);
                std::fs::hard_link(&pending, path)?;
                std::fs::remove_file(pending)?;
            }
            Ok(())
        },
    )?;
    let finished = Instant::now();
    let inference = inference_started
        .get()
        .context("Falta fase de inferencia Vosk")?;
    std::fs::write(
        &args[5],
        serde_json::to_vec_pretty(&serde_json::json!({
            "engine":"native_vosk_0.3.45", "worker_total_ms":finished.duration_since(started).as_secs_f64()*1000.0,
            "conversion_ms":started.duration_since(conversion).as_secs_f64()*1000.0,
            "verification_load_ms":inference.duration_since(started).as_secs_f64()*1000.0,
            "inference_release_ms":finished.duration_since(inference).as_secs_f64()*1000.0,
            "text":text, "live_ptt":"NOT_TESTED"
        }))?,
    )?;
    Ok(())
}

fn cpu_ticks() -> Result<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat")?;
    let (_, fields) = stat.rsplit_once(") ").context("proc stat inválido")?;
    let fields: Vec<&str> = fields.split_whitespace().collect();
    Ok(fields[11].parse::<u64>()? + fields[12].parse::<u64>()?)
}
