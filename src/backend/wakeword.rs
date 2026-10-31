//! Local openWakeWord worker and F4 supervision. No downloader, AI or tool authority.
use super::voice_process::{self, Cancellation};
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub const FRAME_BYTES: usize = 2560; // PCM16LE, mono 16 kHz, 80 ms

pub fn models_dir(configured: &str) -> Result<PathBuf> {
    // Preserve compatibility with the old, unused single-model placeholder.
    if configured == "assets/models/wake_word.onnx" || configured.is_empty() {
        return Ok(dirs::data_local_dir()
            .context("Sin directorio de datos")?
            .join("kde-assistant/models/wakeword"));
    }
    let path = PathBuf::from(configured);
    anyhow::ensure!(
        path.is_absolute() && configured.len() <= 4096,
        "Configura el directorio absoluto de los tres modelos openWakeWord"
    );
    Ok(path)
}

pub fn validate_models(dir: &Path) -> Result<()> {
    for name in [
        "melspectrogram.onnx",
        "embedding_model.onnx",
        "hey_jarvis_v0.1.onnx",
    ] {
        let path = dir.join(name);
        let metadata = std::fs::metadata(&path).with_context(|| {
            format!(
                "Falta modelo local {} (sin descarga automática)",
                path.display()
            )
        })?;
        anyhow::ensure!(
            metadata.is_file() && (1024..=64 * 1024 * 1024).contains(&metadata.len()),
            "Modelo openWakeWord incompleto: {}",
            path.display()
        );
    }
    Ok(())
}

/// Use openWakeWord's per-frame threshold; consume exactly one activation per worker.
#[derive(Default)]
struct Trigger {
    fired: bool,
}
impl Trigger {
    fn accept(&mut self, score: f32, threshold: f32) -> bool {
        if self.fired {
            return false;
        }
        self.fired = score.is_finite() && (threshold..=1.0).contains(&score);
        self.fired
    }
}

/// Restricted subprocess: fixed PCM frames in; READY then one DETECTED byte out.
pub fn worker(dir: &Path, runtime: &Path, threshold: f32) -> Result<()> {
    run_worker(dir, runtime, threshold, false)
}
pub fn stream_worker(dir: &Path, runtime: &Path, threshold: f32) -> Result<()> {
    run_worker(dir, runtime, threshold, true)
}
fn run_worker(dir: &Path, runtime: &Path, threshold: f32, streaming: bool) -> Result<()> {
    use std::io::{Read, Write};
    anyhow::ensure!(
        threshold.is_finite() && (0.1..=1.0).contains(&threshold),
        "Umbral wake inválido"
    );
    validate_models(dir)?;
    anyhow::ensure!(
        runtime.is_absolute() && runtime.is_file(),
        "Runtime ONNX local ausente"
    );
    std::env::set_var("ORT_DYLIB_PATH", runtime);
    let mut detector = super::wakeword_ml::OwwDetector::new(dir, threshold)?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    output.write_all(b"R")?;
    output.flush()?;
    let mut frame = [0u8; FRAME_BYTES];
    let mut trigger = Trigger::default();
    loop {
        if streaming {
            let mut command = [0u8];
            if input.read(&mut command)? == 0 {
                return Ok(());
            }
            match command[0] {
                b'X' => {
                    detector.reset()?;
                    output.write_all(b"R")?;
                    output.flush()?;
                    continue;
                }
                b'F' => (),
                _ => anyhow::bail!("Protocolo wake inválido"),
            }
        }
        // A partial frame is a protocol error, clean EOF ends the worker.
        let n = input.read(&mut frame[..1])?;
        if n == 0 {
            return Ok(());
        }
        input.read_exact(&mut frame[1..])?;
        let audio: Vec<f32> = frame
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();
        let score = detector.feed(&audio)?.context("Frame wake sin score")?;
        if streaming {
            output.write_all(&score.to_le_bytes())?;
            output.flush()?;
        } else if trigger.accept(score, threshold) {
            output.write_all(b"D")?;
            output.flush()?;
            return Ok(());
        }
    }
}

pub fn detector_command(dir: &Path, runtime: &Path, threshold: f32) -> Result<Command> {
    let mut cmd = voice_process::command(std::env::current_exe()?);
    cmd.arg("--voice-wake-worker")
        .arg(dir)
        .arg(runtime)
        .arg(threshold.to_string());
    Ok(cmd)
}

/// One audio source and one inference child, bounded backpressure, no retained recording.
/// Capture is spawned only after a successful model/runtime warmup handshake.
pub async fn wait(
    mut detector: Command,
    capture: Command,
    cancel: &Cancellation,
    ready: impl FnOnce(),
) -> Result<()> {
    anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
    let mut child = detector
        .spawn()
        .context("No se pudo iniciar openWakeWord")?;
    let result = async {
        let mut output = child.stdout.take().context("Wake sin salida")?;
        let mut input = child.stdin.take().context("Wake sin entrada")?;
        let mut byte = [0u8];
        tokio::select! { biased;
            _ = cancel.cancelled() => anyhow::bail!("Operación cancelada"),
            result = tokio::time::timeout(Duration::from_secs(15), output.read_exact(&mut byte)) => {
                result.context("openWakeWord no está listo")??;
            }
        }
        anyhow::ensure!(byte == *b"R", "Respuesta de inicio wake inválida");
        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        let mut capture = capture;
        let mut source = capture.spawn().context("No se pudo iniciar captura PipeWire")?;
        let copied = async {
            let mut audio = source.stdout.take().context("Captura sin PCM")?;
            drop(source.stdin.take());
            ready();
            let copy = async {
                let mut frame = [0u8; FRAME_BYTES];
                loop {
                    tokio::time::timeout(Duration::from_secs(3), audio.read_exact(&mut frame)).await
                        .context("Micrófono sin datos")??;
                    tokio::time::timeout(Duration::from_secs(3), input.write_all(&frame)).await
                        .context("openWakeWord no procesa en tiempo real")??;
                }
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            };
            tokio::select! { biased;
                _ = cancel.cancelled() => anyhow::bail!("Operación cancelada"),
                // An unconsumed arm expires; it never survives indefinitely in the tray.
                _ = tokio::time::sleep(Duration::from_secs(120)) => anyhow::bail!("Hey Jarvis caducó; actívalo de nuevo"),
                result = output.read_exact(&mut byte) => {
                    result.context("openWakeWord detenido")?;
                    anyhow::ensure!(byte == *b"D", "Evento wake inválido");
                    Ok(())
                },
                result = copy => result,
            }
        }.await;
        let _ = source.kill().await;
        let _ = source.wait().await;
        copied
    }.await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(code: &str) -> Command {
        let mut cmd = voice_process::command("/usr/bin/python3");
        cmd.args(["-c", code]);
        cmd
    }
    #[test]
    fn subthreshold_nan_and_duplicates_are_rejected() {
        let mut t = Trigger::default();
        for s in [0.0, 0.1, f32::NAN, f32::INFINITY, 1.1, 0.79] {
            assert!(!t.accept(s, 0.8));
        }
        assert!(t.accept(0.8, 0.8));
        for _ in 0..100 {
            assert!(!t.accept(1.0, 0.8));
        }
    }
    #[tokio::test]
    async fn handshake_stream_and_one_detection() {
        let detector = fixture("import sys; sys.stdout.buffer.write(b'R'); sys.stdout.flush(); assert len(sys.stdin.buffer.read(5120))==5120; sys.stdout.buffer.write(b'D'); sys.stdout.flush()");
        let audio = fixture("import os,time; os.write(1,b'\\0'*5120); time.sleep(10)");
        wait(detector, audio, &Cancellation::default(), || {})
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn failure_before_ready_never_spawns_capture() {
        wait(
            fixture("pass"),
            fixture("raise Exception('must not run')"),
            &Cancellation::default(),
            || panic!("must not capture"),
        )
        .await
        .unwrap_err();
    }
    #[tokio::test]
    async fn eof_and_invalid_events_fail_closed() {
        for (detector, source) in [
            ("import sys; sys.stdout.buffer.write(b'R'); sys.stdout.flush(); sys.stdin.buffer.read()",
             "pass"),
            ("import sys,time; sys.stdout.buffer.write(b'RX'); sys.stdout.flush(); time.sleep(10)",
             "import time; time.sleep(10)"),
        ] {
            wait(fixture(detector), fixture(source), &Cancellation::default(), || {})
                .await.unwrap_err();
        }
    }
    #[tokio::test]
    async fn cancelled_before_handshake_never_starts_audio() {
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let task = tokio::spawn(async move {
            wait(
                fixture("import time; time.sleep(10)"),
                fixture("pass"),
                &signal,
                || panic!("Capture before readiness"),
            )
            .await
        });
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
    }
    #[tokio::test]
    async fn cancel_while_waiting_for_pcm_reaps_children() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("jarvis-reap-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let detector_pid = dir.join("detector");
        let source_pid = dir.join("source");
        let mut detector = fixture("import os,sys,time,pathlib; pathlib.Path(sys.argv[1]).write_text(str(os.getpid())); sys.stdout.buffer.write(b'R'); sys.stdout.flush(); time.sleep(10)");
        detector.arg(&detector_pid);
        let mut source = fixture("import os,sys,time,pathlib; pathlib.Path(sys.argv[1]).write_text(str(os.getpid())); time.sleep(10)");
        source.arg(&source_pid);
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let ready = std::sync::Arc::new(tokio::sync::Notify::new());
        let task_ready = ready.clone();
        let task = tokio::spawn(async move {
            wait(detector, source, &signal, || {
                task_ready.notify_one();
            })
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), ready.notified())
            .await
            .unwrap();
        for _ in 0..200 {
            if source_pid.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let pids: Vec<String> = [&detector_pid, &source_pid]
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect();
        assert!(pids
            .iter()
            .all(|p| Path::new(&format!("/proc/{p}")).exists()));
        let stopped = std::time::Instant::now();
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(pids
            .iter()
            .all(|p| !Path::new(&format!("/proc/{p}")).exists()));
        eprintln!(
            "wake_cancel_and_reap_ms={:.3}",
            stopped.elapsed().as_secs_f64() * 1000.0
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
