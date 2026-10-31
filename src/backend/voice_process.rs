//! Bounded, cancellable audio/inference children. No shell, credentials or detached waiters.
use anyhow::{bail, Context, Result};
use std::{
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    wake: tokio::sync::Notify,
}
#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancellationState>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::SeqCst);
        self.0.wake.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }
    pub async fn cancelled(&self) {
        loop {
            let notified = self.0.wake.notified();
            tokio::pin!(notified);
            // Register before checking the flag: cancellation between the check
            // and await must wake this waiter too. Late callers see the flag.
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.env_clear()
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Audio helpers need the session socket, never provider keys or bearer tokens.
    for key in ["XDG_RUNTIME_DIR", "PIPEWIRE_REMOTE"] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    crate::owned_process::terminate_with_parent(cmd.as_std_mut());
    cmd
}

#[derive(Debug, serde::Serialize)]
pub struct Output {
    #[serde(skip)]
    pub data: Vec<u8>,
    pub spawn_ms: f64,
    pub first_output_ms: Option<f64>,
    pub output_complete_ms: f64,
    pub process_complete_ms: f64,
}
pub async fn run(
    cmd: Command,
    input: &[u8],
    limit: usize,
    deadline: Duration,
    cancel: &Cancellation,
) -> Result<Vec<u8>> {
    Ok(run_observed(cmd, input, limit, deadline, cancel)
        .await?
        .data)
}
pub async fn run_observed(
    mut cmd: Command,
    input: &[u8],
    limit: usize,
    deadline: Duration,
    cancel: &Cancellation,
) -> Result<Output> {
    let started = Instant::now();
    anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
    let mut child = cmd.spawn().context("No se pudo iniciar el motor local")?;
    let spawn_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut stdin = child.stdin.take().context("Motor sin entrada")?;
    let stdout = child.stdout.take().context("Motor sin salida")?;
    let work = async {
        let write = async {
            stdin.write_all(input).await?;
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<_, anyhow::Error>(())
        };
        let read = async {
            let mut data = Vec::new();
            let mut stdout = stdout.take(limit as u64 + 1);
            let mut first_output_ms = None;
            let mut buffer = [0u8; 32768];
            loop {
                let size = stdout.read(&mut buffer).await?;
                if size == 0 {
                    break;
                }
                first_output_ms.get_or_insert(started.elapsed().as_secs_f64() * 1000.0);
                anyhow::ensure!(
                    data.len() + size <= limit,
                    "Salida de audio demasiado grande"
                );
                data.extend_from_slice(&buffer[..size]);
            }
            Ok::<_, anyhow::Error>((
                data,
                first_output_ms,
                started.elapsed().as_secs_f64() * 1000.0,
            ))
        };
        let ((), data, status) = tokio::try_join!(write, read, async {
            Ok::<_, anyhow::Error>(child.wait().await?)
        })?;
        anyhow::ensure!(status.success(), "Motor local detenido o modelo inválido");
        Ok(Output {
            data: data.0,
            spawn_ms,
            first_output_ms: data.1,
            output_complete_ms: data.2,
            process_complete_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    };
    let result = tokio::select! { biased;
        _ = cancel.cancelled() => { Err(anyhow::anyhow!("Operación cancelada")) },
        _ = tokio::time::sleep(deadline) => { Err(anyhow::anyhow!("Tiempo de espera del motor agotado")) },
        value = work => value,
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}

pub async fn devices() -> Result<Vec<String>> {
    devices_with_cancel(&Cancellation::default()).await
}

/// Shared PipeWire capture contract for explicit dictation and Hey Jarvis.
pub async fn capture_command(
    cfg: &crate::models::config::SpeechConfig,
    cancel: &Cancellation,
) -> Result<Command> {
    let devices = devices_with_cancel(cancel).await?;
    anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
    anyhow::ensure!(
        !devices.is_empty(),
        "No hay micrófono disponible en PipeWire"
    );
    let mut cmd = command("/usr/bin/pw-record");
    cmd.args([
        "--raw",
        "--format",
        "s16",
        "--rate",
        "16000",
        "--channels",
        "1",
        "--quality",
        "4",
    ]);
    if !cfg.mic_device.trim().is_empty() {
        anyhow::ensure!(
            devices.contains(&cfg.mic_device),
            "Micrófono seleccionado no disponible; vuelve a elegirlo"
        );
        cmd.arg("--target").arg(&cfg.mic_device);
    }
    cmd.arg("--properties")
        .arg(r#"{"node.dont-reconnect":true,"node.dont-fallback":true,"node.dont-move":true}"#)
        .arg("-");
    Ok(cmd)
}

pub async fn devices_with_cancel(cancel: &Cancellation) -> Result<Vec<String>> {
    let data = run(
        command("/usr/bin/pw-dump"),
        &[],
        2 * 1024 * 1024,
        Duration::from_secs(3),
        cancel,
    )
    .await?;
    let nodes: serde_json::Value = serde_json::from_slice(&data)?;
    let Some(nodes) = nodes.as_array() else {
        bail!("Inventario PipeWire inválido")
    };
    Ok(nodes
        .iter()
        .filter_map(|node| {
            let props = &node["info"]["props"];
            (props["media.class"].as_str() == Some("Audio/Source"))
                .then(|| props["node.name"].as_str().map(str::to_owned))
                .flatten()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(code: &str) -> Command {
        let mut cmd = command("/usr/bin/python3");
        cmd.args(["-c", code]);
        cmd
    }
    #[tokio::test]
    async fn timeout_and_output_limit_kill_and_reap_the_exact_child() {
        for (code, limit, timeout) in [
            (
                "import time; time.sleep(10)",
                128,
                Duration::from_millis(40),
            ),
            (
                "import os,time; os.write(1,b'x'*4096); time.sleep(10)",
                128,
                Duration::from_secs(2),
            ),
        ] {
            let result = run(fixture(code), &[], limit, timeout, &Cancellation::default()).await;
            assert!(result.is_err());
        }
        // All children of this test process have been waited, including errors.
        // Other parallel unit tests can own children, so do not inspect global PIDs.
    }
    #[tokio::test]
    async fn stdout_is_drained_while_writing_and_secrets_are_not_inherited() {
        let data = vec![1u8; 256 * 1024];
        let result=run(fixture("import os,sys; assert 'OPENROUTER_API_KEY' not in os.environ; assert 'GROQ_API_KEY' not in os.environ; assert 'KDE_ASSISTANT_TOKEN' not in os.environ; assert 'HOME' not in os.environ; b=sys.stdin.buffer.read(); sys.stdout.buffer.write(b)"),&data,data.len(),Duration::from_secs(3),&Cancellation::default()).await.unwrap();
        assert_eq!(data, result);
    }
    #[tokio::test]
    async fn cancellation_waits_for_child_reaping() {
        use std::os::unix::fs::PermissionsExt;
        let directory = std::env::temp_dir().join(format!("f4-process-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("pid");
        let mut cmd = fixture(
            "import os,sys,time,pathlib; p=pathlib.Path(sys.argv[1]); t=p.with_suffix('.tmp'); t.write_text(str(os.getpid())); t.rename(p); time.sleep(10)",
        );
        cmd.arg(&path);
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let task =
            tokio::spawn(async move { run(cmd, &[], 128, Duration::from_secs(15), &signal).await });
        for _ in 0..200 {
            if path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let pid = std::fs::read_to_string(&path).unwrap();
        assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
        cancel.cancel();
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("cancelada"));
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[tokio::test]
    async fn cancellation_wakes_all_waiters_and_never_loses_an_early_signal() {
        for iteration in 0..100 {
            let cancel = Cancellation::default();
            let mut tasks = Vec::new();
            if iteration % 3 == 0 {
                cancel.cancel();
            }
            for _ in 0..16 {
                let token = cancel.clone();
                tasks.push(tokio::spawn(async move {
                    token.cancelled().await;
                }));
            }
            if iteration % 3 == 1 {
                tokio::task::yield_now().await;
            }
            cancel.cancel();
            cancel.cancel();
            tokio::time::timeout(Duration::from_secs(2), async {
                for task in tasks {
                    task.await.unwrap();
                }
            })
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_millis(50), cancel.cancelled())
                .await
                .unwrap();
        }
    }
}
