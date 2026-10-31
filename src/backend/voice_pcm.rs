//! Framed Piper -> bounded pipe -> pw-play. One owner waits both children on every path.
use super::voice_process::Cancellation;
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

const LIMIT: usize = 16 * 1024 * 1024;
const FRAME: usize = 32768;

#[derive(Debug, Serialize)]
pub struct Trace {
    pub spawn_ms: f64,
    pub first_pcm_received_ms: f64,
    pub player_spawn_ms: f64,
    pub pcm_complete_received_ms: f64,
    pub playback_complete_ms: f64,
    pub pcm_bytes: usize,
    pub worker: serde_json::Value,
}
async fn packet(input: &mut (impl AsyncRead + Unpin)) -> Result<serde_json::Value> {
    let size = input.read_u32_le().await? as usize;
    anyhow::ensure!((1..=8192).contains(&size), "Protocolo Piper inválido");
    let mut bytes = vec![0; size];
    input.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes).context("Estado Piper inválido")
}
async fn reap(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}
/// The callback means a valid PCM frame and an owned player, never acoustic proof.
pub async fn play(
    mut producer_cmd: Command,
    mut player_cmd: Command,
    text: &[u8],
    cancel: &Cancellation,
    on_player: impl FnOnce(),
) -> Result<Trace> {
    play_with_deadlines(
        producer_cmd.stdout(Stdio::piped()),
        &mut player_cmd,
        text,
        cancel,
        on_player,
        Duration::from_secs(90),
        Duration::from_secs(180),
    )
    .await
}
async fn play_with_deadlines(
    producer_cmd: &mut Command,
    player_cmd: &mut Command,
    text: &[u8],
    cancel: &Cancellation,
    on_player: impl FnOnce(),
    generation_deadline: Duration,
    playback_deadline: Duration,
) -> Result<Trace> {
    anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
    anyhow::ensure!(
        !text.is_empty() && text.len() <= 16000,
        "Texto TTS fuera de límite"
    );
    let start = Instant::now();
    let mut producer = producer_cmd.spawn().context("No se pudo iniciar Piper")?;
    let mut player: Option<Child> = None;
    let work = async {
        let mut output = producer.stdout.take().context("Piper sin salida")?;
        let mut input = producer.stdin.take().context("Piper sin entrada")?;
        let spawn_ms = start.elapsed().as_secs_f64() * 1000.;
        let mut on_player = Some(on_player);
        let mut trace = Trace {
            spawn_ms,
            first_pcm_received_ms: 0.,
            player_spawn_ms: 0.,
            pcm_complete_received_ms: 0.,
            playback_complete_ms: 0.,
            pcm_bytes: 0,
            worker: serde_json::Value::Null,
        };
        let stream = async {
            input.write_all(text).await?;
            input.shutdown().await?;
            drop(input);
            let mut magic = [0; 8];
            output.read_exact(&mut magic).await?;
            anyhow::ensure!(&magic == b"KDA4PCM1", "Protocolo Piper no compatible");
            let header = packet(&mut output).await?;
            if let Some(error) = header.get("error").and_then(|v| v.as_str()) {
                anyhow::bail!("{error}");
            }
            anyhow::ensure!(
                header["sample_rate"] == 22050
                    && header["channels"] == 1
                    && header["sample_width"] == 2,
                "Formato PCM Piper inválido"
            );
            let mut buffer = [0; FRAME];
            let mut sink = None;
            loop {
                let size = output.read_u32_le().await? as usize;
                if size == 0 {
                    break;
                }
                anyhow::ensure!(
                    size <= FRAME && size & 1 == 0 && trace.pcm_bytes + size <= LIMIT,
                    "Límite/formato PCM Piper inválido"
                );
                output.read_exact(&mut buffer[..size]).await?;
                anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
                if player.is_none() {
                    trace.first_pcm_received_ms = start.elapsed().as_secs_f64() * 1000.;
                    player_cmd.stdout(Stdio::null());
                    player = Some(
                        player_cmd
                            .spawn()
                            .context("No se pudo iniciar reproducción")?,
                    );
                    trace.player_spawn_ms = start.elapsed().as_secs_f64() * 1000.;
                    sink = player.as_mut().and_then(|p| p.stdin.take());
                    if let Some(callback) = on_player.take() {
                        callback();
                    }
                }
                sink.as_mut()
                    .context("Reproductor sin entrada")?
                    .write_all(&buffer[..size])
                    .await?;
                trace.pcm_bytes += size;
            }
            anyhow::ensure!(trace.pcm_bytes > 0, "Piper no generó PCM");
            trace.worker = packet(&mut output).await?;
            anyhow::ensure!(
                trace.worker["pcm_bytes"].as_u64() == Some(trace.pcm_bytes as u64),
                "Audio Piper incompleto"
            );
            let mut extra = [0; 1];
            anyhow::ensure!(
                output.read(&mut extra).await? == 0,
                "Datos Piper fuera del protocolo"
            );
            anyhow::ensure!(producer.wait().await?.success(), "Motor Piper detenido");
            trace.pcm_complete_received_ms = start.elapsed().as_secs_f64() * 1000.;
            if let Some(mut sink) = sink {
                sink.shutdown().await?;
            }
            Ok::<_, anyhow::Error>(())
        };
        tokio::time::timeout(generation_deadline, stream)
            .await
            .context("Tiempo de Piper agotado")??;
        let status = tokio::time::timeout(
            playback_deadline,
            player.as_mut().context("Sin reproductor")?.wait(),
        )
        .await
        .context("Tiempo de reproducción agotado")??;
        anyhow::ensure!(status.success(), "Reproducción local detenida");
        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        trace.playback_complete_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(trace)
    };
    let result = tokio::select! { biased;
        _ = cancel.cancelled() => Err(anyhow::anyhow!("Operación cancelada")),
        result = work => result,
    };
    if result.is_err() {
        // Reap the playback first: no queued PCM survives effective cancellation.
        if let Some(ref mut child) = player {
            reap(child).await;
        }
        reap(&mut producer).await;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::voice_process::command;
    use super::*;
    fn fixture(code: &str) -> Command {
        let mut cmd = command("/usr/bin/python3");
        cmd.args(["-I", "-B", "-S", "-c", code]);
        cmd
    }
    const HEADER: &str = "import sys,struct,json,os,time; o=sys.stdout.buffer; packet=lambda v:o.write(struct.pack('<I',len(json.dumps(v).encode()))+json.dumps(v).encode()); o.write(b'KDA4PCM1'); packet(dict(sample_rate=22050,channels=1,sample_width=2)); ";
    #[tokio::test]
    async fn streaming_preserves_order_and_finishes_both_children() {
        let dir = std::env::temp_dir().join(format!("f4-pcm-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("pcm");
        let mut sink = fixture(
            "import sys,pathlib; pathlib.Path(sys.argv[1]).write_bytes(sys.stdin.buffer.read())",
        );
        sink.arg(&file);
        let code = format!("{HEADER}o.write(struct.pack('<I',4)+b'abcd'); o.flush(); time.sleep(.05); o.write(struct.pack('<I',2)+b'ef'+struct.pack('<I',0)); packet(dict(pcm_bytes=6)); o.flush()");
        let trace = play(
            fixture(&code),
            sink,
            b"hello",
            &Cancellation::default(),
            || (),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"abcdef");
        assert!(trace.player_spawn_ms < trace.pcm_complete_received_ms);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn malformed_or_unverified_pcm_never_starts_player() {
        for code in ["import sys; sys.stdout.buffer.write(b'bad')".to_owned(),
            format!("{HEADER}o.write(struct.pack('<I',3)+b'bad')"),
            format!("{HEADER}o.write(struct.pack('<I',32770))"),
            format!("{HEADER}o.write(struct.pack('<I',4)+b'ab')"),
            format!("{HEADER}o.write(struct.pack('<I',0)); packet(dict(pcm_bytes=0))"),
            "import sys,struct; s=b'{\"error\":\"fixture missing model\"}'; sys.stdout.buffer.write(b'KDA4PCM1'+struct.pack('<I',len(s))+s)".into()] {
            let result = play(fixture(&code), fixture("raise RuntimeError('must never spawn')"), b"hello", &Cancellation::default(), || panic!("Invalid PCM reached player")).await;
            assert!(result.is_err());
        }
    }
    #[tokio::test]
    async fn producer_failure_after_pcm_and_total_limit_stop_the_player() {
        for tail in [
            "o.flush(); sys.exit(7)",
            "o.write(struct.pack('<I',0)); packet(dict(pcm_bytes=999)); o.flush()",
            "[o.write(struct.pack('<I',32768)+b'x'*32768) for _ in range(513)]; o.flush()",
        ] {
            let code = format!("{HEADER}o.write(struct.pack('<I',2)+b'ab'); {tail}");
            let result = play(
                fixture(&code),
                fixture("import sys; sys.stdin.buffer.read()"),
                b"hello",
                &Cancellation::default(),
                || (),
            )
            .await;
            assert!(result.is_err());
        }
    }
    #[tokio::test]
    async fn cancellation_and_timeout_reap_producer_and_backpressured_player() {
        for timeout in [false, true] {
            let dir = std::env::temp_dir().join(format!("f4-pcm-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&dir).unwrap();
            let prod_pid = dir.join("producer");
            let play_pid = dir.join("player");
            let code = format!("{HEADER}open(sys.argv[1],'w').write(str(os.getpid())); o.write(struct.pack('<I',32768)+b'x'*32768); o.flush(); time.sleep(10)");
            let mut prod = fixture(&code);
            prod.arg(&prod_pid);
            let mut sink = fixture(
                "import os,sys,time; open(sys.argv[1],'w').write(str(os.getpid())); time.sleep(10)",
            );
            sink.arg(&play_pid);
            let cancel = Cancellation::default();
            let signal = cancel.clone();
            let task = tokio::spawn(async move {
                play_with_deadlines(
                    &mut prod,
                    &mut sink,
                    b"hello",
                    &signal,
                    || (),
                    if timeout {
                        Duration::from_millis(300)
                    } else {
                        Duration::from_secs(3)
                    },
                    Duration::from_secs(3),
                )
                .await
            });
            for _ in 0..100 {
                if play_pid.exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            if !timeout {
                cancel.cancel();
            }
            assert!(task.await.unwrap().is_err());
            for file in [&prod_pid, &play_pid] {
                let pid = std::fs::read_to_string(file).unwrap();
                assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
            }
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
