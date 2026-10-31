//! Bounded PCM history shared by wake detection and dictation; one PipeWire input.
use super::{
    voice_process::{self, Cancellation},
    wakeword::FRAME_BYTES,
};
use anyhow::{Context, Result};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
    task::JoinHandle,
};
const HISTORY_FRAMES: usize = 50; // Four seconds, 128000 bytes; never persisted.
#[derive(Default)]
struct Data {
    frames: VecDeque<Vec<u8>>,
    end: u64,
    closed: Option<String>,
}
#[derive(Default)]
pub struct Ring {
    data: Mutex<Data>,
    changed: tokio::sync::Notify,
}
impl Ring {
    pub(super) fn push(&self, pcm: Vec<u8>) {
        let mut data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        data.frames.push_back(pcm);
        data.end += 1;
        while data.frames.len() > HISTORY_FRAMES {
            data.frames.pop_front();
        }
        drop(data);
        self.changed.notify_waiters();
    }
    pub fn cursor(&self, pre_frames: u64) -> u64 {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        data.end
            .saturating_sub(pre_frames)
            .max(data.end - data.frames.len() as u64)
    }
    fn close(&self, reason: String) {
        self.data.lock().unwrap_or_else(|e| e.into_inner()).closed = Some(reason);
        self.changed.notify_waiters();
    }
    pub async fn read(&self, cursor: &mut u64, cancel: &Cancellation) -> Result<Vec<u8>> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
            {
                let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(reason) = &data.closed {
                    anyhow::bail!("{reason}");
                }
                let first = data.end - data.frames.len() as u64;
                anyhow::ensure!(
                    *cursor >= first,
                    "Audio atrasado: se excedió el buffer circular; pausa por seguridad"
                );
                if *cursor < data.end {
                    let frame = data.frames[(*cursor - first) as usize].clone();
                    *cursor += 1;
                    return Ok(frame);
                }
            }
            tokio::select! { biased;
                _ = cancel.cancelled() => anyhow::bail!("Operación cancelada"),
                _ = notified => (),
            }
        }
    }
}
pub struct Capture {
    pub ring: Arc<Ring>,
    pub pid: u32,
    cancel: Cancellation,
    task: Option<JoinHandle<()>>,
}
impl Capture {
    pub fn start(mut command: Command) -> Result<Self> {
        let mut child = command
            .spawn()
            .context("No se pudo abrir la captura PipeWire")?;
        let pid = child.id().context("Captura sin PID")?;
        let ring = Arc::new(Ring::default());
        let task_ring = ring.clone();
        let cancel = Cancellation::default();
        let task_cancel = cancel.clone();
        let task = tokio::spawn(async move {
            let result = async {
                drop(child.stdin.take());
                let mut output = child.stdout.take().context("Captura sin PCM")?;
                loop {
                    let mut frame = vec![0u8; FRAME_BYTES];
                    tokio::select! { biased;
                        _ = task_cancel.cancelled() => anyhow::bail!("Captura detenida"),
                        result = tokio::time::timeout(Duration::from_secs(3), output.read_exact(&mut frame)) => {
                            result.context("Micrófono sin datos; escucha pausada")?.context("Micrófono desconectado; escucha pausada")?;
                        }
                    }
                    task_ring.push(frame);
                }
                #[allow(unreachable_code)] Ok::<(), anyhow::Error>(())
            }.await;
            let _ = child.kill().await;
            let _ = child.wait().await;
            task_ring.close(
                result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Captura terminada".into()),
            );
        });
        Ok(Self {
            ring,
            pid,
            cancel,
            task: Some(task),
        })
    }
    pub async fn stop(mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
/// Persistent openWakeWord child; idle blocks on stdin rather than spinning.
pub struct Detector {
    child: Child,
    input: tokio::process::ChildStdin,
    output: tokio::process::ChildStdout,
}
impl Detector {
    pub async fn start(mut cmd: Command, cancel: &Cancellation) -> Result<Self> {
        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        let mut child = cmd.spawn().context("No se pudo iniciar openWakeWord")?;
        let work = async {
            let input = child.stdin.take().context("Wake sin entrada")?;
            let mut output = child.stdout.take().context("Wake sin salida")?;
            let mut ack = [0u8];
            output.read_exact(&mut ack).await?;
            anyhow::ensure!(ack == *b"R", "Inicio wake inválido");
            Ok::<_, anyhow::Error>((input, output))
        };
        let result = tokio::select! { biased;
            _ = cancel.cancelled() => Err(anyhow::anyhow!("Operación cancelada")),
            result = tokio::time::timeout(Duration::from_secs(15), work) => result.context("Timeout openWakeWord").and_then(|v| v),
        };
        match result {
            Ok((input, output)) => Ok(Self {
                child,
                input,
                output,
            }),
            Err(e) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                Err(e)
            }
        }
    }
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
    pub async fn score(&mut self, pcm: &[u8], cancel: &Cancellation) -> Result<f32> {
        anyhow::ensure!(pcm.len() == FRAME_BYTES, "Frame wake inválido");
        let work = async {
            self.input.write_all(b"F").await?;
            self.input.write_all(pcm).await?;
            let mut score = [0u8; 4];
            self.output.read_exact(&mut score).await?;
            let score = f32::from_le_bytes(score);
            anyhow::ensure!(
                score.is_finite() && (0.0..=1.0).contains(&score),
                "Score wake inválido"
            );
            Ok(score)
        };
        tokio::select! { biased;
            _ = cancel.cancelled() => anyhow::bail!("Operación cancelada"),
            value = tokio::time::timeout(Duration::from_secs(3), work) => value.context("Detector wake bloqueado")?,
        }
    }
    pub async fn reset(&mut self, cancel: &Cancellation) -> Result<()> {
        let work = async {
            self.input.write_all(b"X").await?;
            let mut ack = [0u8];
            self.output.read_exact(&mut ack).await?;
            anyhow::ensure!(ack == *b"R", "Reset wake inválido");
            Ok(())
        };
        tokio::select! { biased;
            _ = cancel.cancelled() => anyhow::bail!("Operación cancelada"),
            value = tokio::time::timeout(Duration::from_secs(3), work) => value.context("Reset wake bloqueado")?,
        }
    }
    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
pub fn detector_command(
    dir: &std::path::Path,
    runtime: &std::path::Path,
    threshold: f32,
) -> Result<Command> {
    let mut cmd = voice_process::command(std::env::current_exe()?);
    cmd.arg("--voice-wake-stream")
        .arg(dir)
        .arg(runtime)
        .arg(threshold.to_string());
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(n: i16) -> Vec<u8> {
        n.to_le_bytes().repeat(FRAME_BYTES / 2)
    }
    #[tokio::test]
    async fn ring_is_bounded_and_never_silently_loses_unconsumed_audio() {
        let ring = Ring::default();
        for n in 0..80 {
            ring.push(frame(n));
        }
        assert_eq!(ring.data.lock().unwrap().frames.len(), 50);
        let mut stale = 0;
        assert!(ring
            .read(&mut stale, &Cancellation::default())
            .await
            .is_err());
        let mut cursor = ring.cursor(25);
        assert_eq!(cursor, 55);
        for n in 55..80 {
            assert_eq!(
                ring.read(&mut cursor, &Cancellation::default())
                    .await
                    .unwrap(),
                frame(n)
            );
        }
    }
    #[tokio::test]
    async fn close_and_cancel_wake_a_blocked_reader() {
        let ring = Arc::new(Ring::default());
        let reader = ring.clone();
        let task = tokio::spawn(async move { reader.read(&mut 0, &Cancellation::default()).await });
        ring.close("disconnected".into());
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("disconnected"));
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(Ring::default().read(&mut 0, &cancel).await.is_err());
    }
}
