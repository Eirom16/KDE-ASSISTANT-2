//! Explicit opt-in offline workers: synthetic PCM and separately authorised human WAVs.
//! Never opens audio devices. Human assets are supplied explicitly and remain private.
use kde_assistant_lib::backend::{
    voice_coordinator::validate_model,
    voice_process::{command, run, Cancellation},
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

fn isolated(program: &str, profile: &Path) -> tokio::process::Command {
    let mut cmd = command(program);
    for (key, name) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_RUNTIME_DIR", "runtime"),
        ("HOME", "home"),
        ("TMPDIR", "tmp"),
    ] {
        cmd.env(key, profile.join(name));
    }
    cmd.env("PIPEWIRE_REMOTE", "f4-unreachable-audio");
    cmd.env(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", profile.join("absent-bus").display()),
    );
    cmd
}

fn cpu_ticks(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let fields: Vec<&str> = stat
        .rsplit_once(") ")
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap()
}

fn children() -> Vec<u32> {
    let mut pids = Vec::new();
    for entry in std::fs::read_dir("/proc/self/task").unwrap().flatten() {
        if let Ok(text) = std::fs::read_to_string(entry.path().join("children")) {
            pids.extend(
                text.split_whitespace()
                    .filter_map(|s| s.parse::<u32>().ok()),
            );
        }
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}
#[tokio::test]
#[ignore = "Requires explicitly authorised model via KDA_F4_LOCAL_MODEL; no microphone"]
async fn authorised_whisper_worker_cancels_and_times_out_without_orphans() {
    use std::os::unix::fs::PermissionsExt;
    let profile = std::env::temp_dir().join(format!("f4-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&profile).unwrap();
    std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o700)).unwrap();
    for name in ["config", "data", "cache", "runtime", "home", "tmp"] {
        let directory = profile.join(name);
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let model =
        PathBuf::from(std::env::var("KDA_F4_LOCAL_MODEL").expect("explicit model required"));
    validate_model(&model).unwrap();
    let baseline = children();
    let pcm: Vec<u8> = (0..48000)
        .flat_map(|i| ((f32::sin(i as f32 * 0.1728) * 8192.0) as i16).to_le_bytes())
        .collect();
    for cancel_after in [Some(30u64), Some(500), None] {
        let mut cmd = isolated(env!("CARGO_BIN_EXE_kde-assistant"), &profile);
        cmd.arg("--voice-stt-worker").arg(&model).arg("es");
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let timer = tokio::spawn(async move {
            if let Some(ms) = cancel_after {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                signal.cancel();
            }
        });
        let result = run(cmd, &pcm, 65536, Duration::from_millis(1000), &cancel).await;
        timer.await.unwrap();
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains(if cancel_after.is_some() {
                "cancelada"
            } else {
                "agotado"
            }),
            "{message}"
        );
        assert_eq!(
            children(),
            baseline,
            "worker must be collected before returning"
        );
    }
    // Unlike elapsed-time probes, this observes entry after load/state creation
    // and actual CPU progress while the shared decoder is inside whisper_full.
    let wav_path = profile.join("fixture.wav");
    let mut wav = hound::WavWriter::create(
        &wav_path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for sample in pcm.as_chunks::<2>().0 {
        wav.write_sample(i16::from_le_bytes(*sample)).unwrap();
    }
    wav.finalize().unwrap();
    let phase = profile.join("phase");
    let mut cmd = isolated(env!("CARGO_BIN_EXE_bench_voice"), &profile);
    cmd.arg(&model)
        .arg(wav_path)
        .arg("1")
        .arg(profile.join("unused.json"))
        .args(["3", "0", "0.2", "--phase-file"])
        .arg(&phase);
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let known = baseline.clone();
    let watcher = tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        while tokio::time::Instant::now() < deadline {
            if std::fs::read(&phase).ok().as_deref() == Some(b"inference") {
                let pid = children().into_iter().find(|p| !known.contains(p)).unwrap();
                let before = cpu_ticks(pid);
                while tokio::time::Instant::now() < deadline {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    let delta = cpu_ticks(pid).saturating_sub(before);
                    if delta >= 2 {
                        let issued = tokio::time::Instant::now();
                        signal.cancel();
                        return (pid, delta, issued);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        signal.cancel();
        panic!("No confirmed CPU progress after inference entry");
    });
    let result = run(cmd, &[], 65536, Duration::from_secs(10), &cancel).await;
    let finished = tokio::time::Instant::now();
    let (pid, delta, issued) = watcher.await.unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelada"));
    assert_eq!(children(), baseline);
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    eprintln!("F4_CONFIRMED_INFERENCE_CANCEL pid={pid} cpu_ticks_after_entry={delta} cancel_wait_ms={:.3} reaped=true", finished.saturating_duration_since(issued).as_secs_f64() * 1000.0);
    for name in ["config", "data", "cache", "runtime", "home", "tmp"] {
        assert_eq!(std::fs::read_dir(profile.join(name)).unwrap().count(), 0);
    }
    std::fs::remove_dir_all(profile).unwrap();
}

#[tokio::test]
#[ignore = "Requires explicit authorised Vosk library/model and human corpus; no microphone"]
async fn authorised_native_vosk_parity_cancel_timeout_and_reaping() {
    use std::os::unix::fs::PermissionsExt;
    let library =
        PathBuf::from(std::env::var("KDA_F4_VOSK_LIBRARY").expect("explicit runtime required"));
    let model = PathBuf::from(std::env::var("KDA_F4_VOSK_MODEL").expect("explicit model required"));
    let corpus = PathBuf::from(
        std::env::var("KDA_F4_VOSK_CORPUS").expect("explicit authorised corpus required"),
    );
    let profile = std::env::temp_dir().join(format!("f4-native-vosk-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&profile).unwrap();
    std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o700)).unwrap();
    for name in ["config", "data", "cache", "runtime", "home", "tmp"] {
        let directory = profile.join(name);
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let baseline = children();
    for id in ["short", "applications", "numbers", "long"] {
        let mut wav = hound::WavReader::open(corpus.join(format!("{id}.wav"))).unwrap();
        assert_eq!(wav.spec().channels, 1);
        assert_eq!(wav.spec().sample_rate, 16000);
        assert!(wav.duration() <= 30 * 16000);
        let pcm: Vec<u8> = wav
            .samples::<i16>()
            .flat_map(|s| s.unwrap().to_le_bytes())
            .collect();
        let folder = if corpus.join(format!("comparison-{id}")).is_dir() {
            corpus.join(format!("comparison-{id}"))
        } else {
            corpus.join("comparison")
        };
        let reference: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join(format!("{id}-vosk_small_es.json"))).unwrap(),
        )
        .unwrap();
        let expected = reference["warm_runs"][0]["measurement"]["text"]
            .as_str()
            .unwrap();
        let mut cmd = isolated(env!("CARGO_BIN_EXE_kde-assistant"), &profile);
        cmd.arg("--voice-vosk-worker")
            .arg(&library)
            .arg(&model)
            .env("OPENBLAS_NUM_THREADS", "1")
            .env("OMP_NUM_THREADS", "1");
        let start = tokio::time::Instant::now();
        let result = run(
            cmd,
            &pcm,
            65536,
            Duration::from_secs(30),
            &Cancellation::default(),
        )
        .await
        .unwrap();
        let actual: String = serde_json::from_slice(&result).unwrap();
        assert!(
            actual.split_whitespace().eq(expected.split_whitespace()),
            "native/Python text parity failed for {id}"
        );
        assert_eq!(children(), baseline);
        eprintln!(
            "F4_NATIVE_VOSK_PARITY sample={id} cold_worker_ms={:.3} text_matches=true reaped=true",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    let mut cmd = isolated(env!("CARGO_BIN_EXE_kde-assistant"), &profile);
    cmd.arg("--voice-vosk-worker").arg(&library).arg(&model);
    let result = run(
        cmd,
        &[0u8; 6400],
        65536,
        Duration::from_millis(1),
        &Cancellation::default(),
    )
    .await;
    assert!(result.unwrap_err().to_string().contains("agotado"));
    assert_eq!(children(), baseline);

    let bad = profile.join("bad-model/am");
    std::fs::create_dir_all(&bad).unwrap();
    for path in [profile.join("bad-model"), bad.clone()] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let corrupt = std::fs::File::create(bad.join("final.mdl")).unwrap();
    corrupt
        .set_len(std::fs::metadata(model.join("am/final.mdl")).unwrap().len())
        .unwrap();
    drop(corrupt);
    let mut check = isolated(env!("CARGO_BIN_EXE_kde-assistant"), &profile);
    check
        .arg("--voice-vosk-check")
        .arg(&library)
        .arg(profile.join("bad-model"));
    let result = run(
        check,
        &[],
        4096,
        Duration::from_secs(5),
        &Cancellation::default(),
    )
    .await
    .unwrap();
    let error: Option<String> = serde_json::from_slice(&result).unwrap();
    assert!(error.unwrap().contains("Integridad"));
    assert_eq!(children(), baseline);

    let phase = profile.join("phase");
    let mut cmd = isolated(env!("CARGO_BIN_EXE_bench_voice"), &profile);
    cmd.args(["--vosk"])
        .arg(&library)
        .arg(&model)
        .arg(corpus.join("long.wav"))
        .arg(profile.join("unused.json"))
        .arg("--phase-file")
        .arg(&phase)
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("OMP_NUM_THREADS", "1");
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let known = baseline.clone();
    let watcher = tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            if std::fs::read(&phase).ok().as_deref() == Some(b"inference") {
                let pid = children().into_iter().find(|p| !known.contains(p)).unwrap();
                let before = cpu_ticks(pid);
                while tokio::time::Instant::now() < deadline {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    let delta = cpu_ticks(pid).saturating_sub(before);
                    if delta >= 2 {
                        let issued = tokio::time::Instant::now();
                        signal.cancel();
                        return (pid, delta, issued);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        signal.cancel();
        panic!("No native Vosk inference progress confirmed");
    });
    let result = run(cmd, &[], 65536, Duration::from_secs(20), &cancel).await;
    let finished = tokio::time::Instant::now();
    let (pid, delta, issued) = watcher.await.unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelada"));
    assert_eq!(children(), baseline);
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    eprintln!("F4_NATIVE_VOSK_CANCEL pid={pid} cpu_ticks_after_entry={delta} cancel_wait_ms={:.3} reaped=true", finished.saturating_duration_since(issued).as_secs_f64()*1000.0);
    for name in ["config", "data", "cache", "runtime", "home", "tmp"] {
        assert_eq!(std::fs::read_dir(profile.join(name)).unwrap().count(), 0);
    }
    std::fs::remove_dir_all(profile).unwrap();
}
