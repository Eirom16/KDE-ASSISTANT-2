//! Invalid diagnostics must exit before touching even a synthetic profile.
use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn internal_and_unknown_arguments_never_start_backend_or_create_profile() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("f4-cli-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let missing = root.join("missing.bin");
    let missing = missing.to_str().unwrap();
    let cases = [
        (vec!["--voice-wake-stream"], false),
        (vec!["--voice-wake-stream", missing, missing, "NaN"], false),
        (vec!["--voice-wake-stream", missing, missing, "0.8"], false),
        (vec!["--voice-wake-worker"], false),
        (vec!["--voice-wake-worker", missing, missing, "0.8"], false),
        (vec!["--voice-wake-worker", missing, missing, "NaN"], false),
        (
            vec!["--voice-wake-worker", missing, missing, "0.8", "extra"],
            false,
        ),
        (
            vec!["--ui-off", "--voice-wake-worker", missing, missing, "0.8"],
            false,
        ),
        (vec!["--voice-model-check", missing], true),
        (vec!["--voice-model-check"], false),
        (vec!["--voice-model-check", missing, "extra"], false),
        (vec!["--voice-stt-worker"], false),
        (vec!["--voice-stt-worker", missing, "es"], false),
        (vec!["--voice-stt-worker", missing, "es", "extra"], false),
        (vec!["--voice-unknown"], false),
        (vec!["--voice-piper-check", missing, missing], true),
        (vec!["--voice-piper-check"], false),
        (
            vec!["--voice-piper-check", missing, missing, "extra"],
            false,
        ),
        (
            vec!["--voice-piper-worker", missing, missing, "1", "0"],
            false,
        ),
        (
            vec!["--voice-piper-worker", missing, missing, "NaN", "0"],
            false,
        ),
        (vec!["--voice-piper-worker"], false),
        (vec!["--voice-piper-stream"], false),
        (vec!["--voice-piper-managed"], false),
        (
            vec!["--voice-piper-managed", missing, missing, "1", "0"],
            false,
        ),
        (
            vec!["--voice-piper-managed", missing, missing, "NaN", "0"],
            false,
        ),
        (
            vec![
                "--ui-off",
                "--voice-piper-managed",
                missing,
                missing,
                "1",
                "0",
            ],
            false,
        ),
        (vec!["--voice-piper-warm-experiment"], false),
        (
            vec!["--voice-piper-warm-experiment", missing, missing, "1", "0"],
            false,
        ),
        (
            vec![
                "--ui-off",
                "--voice-piper-warm-experiment",
                missing,
                missing,
                "1",
                "0",
            ],
            false,
        ),
        (
            vec!["--voice-piper-stream", missing, missing, "1", "0", "extra"],
            false,
        ),
        (
            vec!["--voice-piper-stream", missing, missing, "NaN", "0"],
            false,
        ),
        (
            vec![
                "--ui-off",
                "--voice-piper-stream",
                missing,
                missing,
                "1",
                "0",
            ],
            false,
        ),
        (
            vec!["--ui-off", "--voice-piper-check", missing, missing],
            false,
        ),
        (vec!["--voice-vosk-check", missing, missing], true),
        (vec!["--voice-vosk-check"], false),
        (vec!["--voice-vosk-check", missing, missing, "extra"], false),
        (vec!["--voice-vosk-worker", missing, missing], false),
        (vec!["--voice-vosk-worker"], false),
        (
            vec!["--ui-off", "--voice-vosk-check", missing, missing],
            false,
        ),
        (vec!["--ui-off", "--voice-model-check", missing], false),
        (vec!["--voice-model-chek", missing], false),
        (vec!["--voic-model-check", missing], false),
        (vec!["--http-port", "0", "--unknown"], false),
        (vec!["--http-port", "0", "--http-port", "0"], false),
        (vec!["--http-port", "invalid"], false),
        (vec!["--send-msg", "hello"], false),
    ];
    for (args, success) in cases {
        let mut child = Command::new(env!("CARGO_BIN_EXE_kde-assistant"))
            .args(&args)
            .env_clear()
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_RUNTIME_DIR", root.join("runtime"))
            .current_dir(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Diagnostic did not exit: {args:?}");
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.success(), success, "{args:?}");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0, "{args:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("starting"));
    }
    fs::remove_dir(root).unwrap();
}
