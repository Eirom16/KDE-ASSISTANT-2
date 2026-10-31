//! Offline, two-request cost experiment. No Backend, audio device, socket or settings.
use anyhow::{Context, Result};
use kde_assistant_lib::backend::voice_process::command;
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::ChildStdout,
};

async fn json(input: &mut ChildStdout) -> Result<serde_json::Value> {
    let size = input.read_u32_le().await? as usize;
    anyhow::ensure!((1..=8192).contains(&size), "Packet limit");
    let mut bytes = vec![0; size];
    input.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
fn usage(pid: u32) -> Result<serde_json::Value> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields: Vec<&str> = stat
        .rsplit_once(") ")
        .context("stat")?
        .1
        .split_whitespace()
        .collect();
    let ticks = fields[11].parse::<u64>()? + fields[12].parse::<u64>()?;
    let memory = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup"))?;
    let value = |key: &str| -> u64 {
        memory
            .lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|v| v.split_whitespace().next())
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(0)
    };
    Ok(serde_json::json!({"cpu_ticks":ticks,"rss_kib":value("Rss:"),"pss_kib":value("Pss:")}))
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() == 4, "binary runtime model required");
    // This executable refuses an ambient/personal profile, even for offline inference.
    let root =
        std::env::var("KDA_WARM_EXPERIMENT_ROOT").context("Explicit private root required")?;
    let root = Path::new(&root).canonicalize()?;
    for key in [
        "HOME",
        "TMPDIR",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
    ] {
        use std::os::unix::fs::MetadataExt;
        let path = std::env::var(key)?;
        let path = Path::new(&path);
        anyhow::ensure!(!path.is_symlink(), "Private symlink rejected");
        let path = path.canonicalize()?;
        let meta = path.metadata()?;
        anyhow::ensure!(
            path.starts_with(&root)
                && path != root
                && meta.is_dir()
                && meta.uid() == rustix::process::getuid().as_raw()
                && meta.mode() & 0o077 == 0,
            "Unsafe private profile"
        );
    }
    anyhow::ensure!(
        std::env::var("PIPEWIRE_REMOTE")? == "f4-unreachable-audio",
        "Unexpected audio socket"
    );
    let mut cmd = command(&args[1]);
    cmd.args([
        "--voice-piper-warm-experiment",
        &args[2],
        &args[3],
        "1",
        "0",
    ]);
    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let pid = child.id().context("PID")?;
    let work = async {
        let mut input = child.stdin.take().context("stdin")?;
        let mut output = child.stdout.take().context("stdout")?;
        let mut rows = Vec::new();
        let mut idle = Vec::new();
        for index in 0..2 {
            let text = b"Hola. Soy KDE Assistant y esta es una prueba de voz en espa\xc3\xb1ol.";
            let request = Instant::now();
            input.write_u32_le(text.len() as u32).await?;
            input.write_all(text).await?;
            input.flush().await?;
            let mut magic = [0; 8];
            output.read_exact(&mut magic).await?;
            anyhow::ensure!(&magic == b"KDA4PCM1", "protocol");
            let header = json(&mut output).await?;
            anyhow::ensure!(
                header["sample_rate"] == 22050
                    && header["sample_width"] == 2
                    && header["channels"] == 1,
                "format"
            );
            let mut bytes = 0usize;
            let mut first = None;
            let mut buffer = [0; 32768];
            loop {
                let size = output.read_u32_le().await? as usize;
                if size == 0 {
                    break;
                }
                anyhow::ensure!(
                    size <= 32768 && size & 1 == 0 && bytes + size <= 16 * 1024 * 1024,
                    "PCM bound"
                );
                output.read_exact(&mut buffer[..size]).await?;
                first.get_or_insert(request.elapsed().as_secs_f64() * 1000.);
                bytes += size;
            }
            let worker = json(&mut output).await?;
            anyhow::ensure!(
                worker["pcm_bytes"].as_u64() == Some(bytes as u64),
                "PCM count"
            );
            rows.push(serde_json::json!({"index":index,"first_pcm_ms":first,
                "complete_ms":request.elapsed().as_secs_f64()*1000.,"worker":worker,"memory":usage(pid)?}));
            if index == 0 {
                let before = usage(pid)?;
                tokio::time::sleep(Duration::from_secs(1)).await;
                idle.push(serde_json::json!({"seconds":1,"before":before,"after":usage(pid)?}));
            }
        }
        let before = usage(pid)?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        idle.push(serde_json::json!({"seconds":2,"before":before,"after":usage(pid)?}));
        let tail = Instant::now();
        // Keep stdin open: exit must come from the worker's idle TTL, not EOF.
        let status = child.wait().await?;
        anyhow::ensure!(status.success(), "Prepared worker failure");
        let eof = output.read_u8().await;
        anyhow::ensure!(eof.is_err(), "Trailing PCM");
        Ok::<_, anyhow::Error>(serde_json::json!({"rows":rows,"idle":idle,
            "ttl_tail_wait_ms":tail.elapsed().as_secs_f64()*1000.,
            "whole_ms":start.elapsed().as_secs_f64()*1000.,"reaped":!Path::new(&format!("/proc/{pid}")).exists(),
            "resident_default":false,"idle_ttl_seconds":3,"max_requests":2}))
    };
    let result = tokio::select! { biased;
        _=tokio::signal::ctrl_c()=>Err(anyhow::anyhow!("cancelled")),
        value=tokio::time::timeout(Duration::from_secs(60),work)=>match value { Ok(result)=>result, Err(error)=>Err(anyhow::anyhow!(error).context("experiment timeout")) },
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    println!("{}", serde_json::to_string(&result?)?);
    Ok(())
}
