//! Private managed Piper v1. PCM stays buffered until a matching terminal Done.
use anyhow::{Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{ChildStdin, ChildStdout},
};

pub const PCM_LIMIT: usize = 16 * 1024 * 1024;
const CONTROL_LIMIT: usize = 128 * 1024;
pub async fn send(out: &mut ChildStdin, value: Value) -> Result<()> {
    let encoded = serde_json::to_vec(&value)?;
    anyhow::ensure!(
        encoded.len() <= CONTROL_LIMIT,
        "Mensaje Piper demasiado grande"
    );
    out.write_u32_le(encoded.len() as u32).await?;
    out.write_all(&encoded).await?;
    out.flush().await?;
    Ok(())
}
pub async fn receive(input: &mut ChildStdout) -> Result<Value> {
    let size = input
        .read_u32_le()
        .await
        .context("EOF/IPC Piper interrumpido")? as usize;
    anyhow::ensure!(
        (1..=CONTROL_LIMIT).contains(&size),
        "Framing Piper inválido"
    );
    let mut data = vec![0; size];
    input
        .read_exact(&mut data)
        .await
        .context("Mensaje Piper incompleto")?;
    serde_json::from_slice(&data).context("Mensaje Piper corrupto")
}
pub fn identity(value: &Value, kind: &str, generation: &str, request: Option<&str>) -> Result<()> {
    anyhow::ensure!(
        value["protocol"] == 1 && value["kind"] == kind && value["generation"] == generation,
        "Generación o protocolo Piper incompatible"
    );
    if let Some(request) = request {
        anyhow::ensure!(value["request_id"] == request, "Resultado Piper obsoleto");
    }
    Ok(())
}
pub async fn ready(input: &mut ChildStdout, generation: &str) -> Result<f64> {
    let value = receive(input).await?;
    identity(&value, "ready", generation, None)?;
    anyhow::ensure!(
        value["sample_rate"] == 22050 && value["channels"] == 1 && value["sample_width"] == 2,
        "Formato Piper preparado incompatible"
    );
    Ok(value["ready_ms"].as_f64().unwrap_or(0.))
}
pub struct Audio {
    pub pcm: Vec<u8>,
    pub synthesis_ms: f64,
}
pub async fn reserve(
    input: &mut ChildStdout,
    out: &mut ChildStdin,
    generation: &str,
    request: &str,
) -> Result<()> {
    send(
        out,
        json!({"protocol":1,"kind":"reserve","generation":generation,"request_id":request}),
    )
    .await?;
    let value = receive(input).await?;
    identity(&value, "reserved", generation, Some(request))
}
pub async fn synthesize(
    input: &mut ChildStdout,
    out: &mut ChildStdin,
    generation: &str,
    request: &str,
    text: &str,
    on_inference: impl FnOnce(),
) -> Result<Audio> {
    anyhow::ensure!(
        !text.trim().is_empty() && text.len() <= 16000,
        "Texto TTS fuera de límite"
    );
    send(
        out,
        json!({"protocol":1,"kind":"synthesize","generation":generation,
        "request_id":request,"text":text}),
    )
    .await?;
    let begin = receive(input).await?;
    identity(&begin, "begin", generation, Some(request))?;
    on_inference();
    let mut pcm = Vec::new();
    let mut digest = Sha256::new();
    let mut buffer = [0; 32768];
    loop {
        let size = input.read_u32_le().await.context("PCM Piper incompleto")? as usize;
        if size == 0 {
            break;
        }
        if size == u32::MAX as usize {
            let terminal = receive(input).await?;
            identity(&terminal, "error", generation, Some(request))?;
            anyhow::bail!("Piper no pudo terminar la síntesis");
        }
        anyhow::ensure!(
            size <= buffer.len() && size & 1 == 0 && pcm.len() + size <= PCM_LIMIT,
            "PCM Piper excede sus límites o formato"
        );
        input
            .read_exact(&mut buffer[..size])
            .await
            .context("Frame PCM Piper incompleto")?;
        digest.update(&buffer[..size]);
        pcm.extend_from_slice(&buffer[..size]);
    }
    let done = receive(input).await?;
    identity(&done, "done", generation, Some(request))?;
    anyhow::ensure!(
        !pcm.is_empty()
            && done["pcm_bytes"].as_u64() == Some(pcm.len() as u64)
            && done["pcm_sha256"] == hex::encode(digest.finalize())
            && done["sample_rate"] == 22050
            && done["sample_width"] == 2
            && done["channels"] == 1,
        "Resultado PCM Piper inválido o truncado"
    );
    Ok(Audio {
        pcm,
        synthesis_ms: done["synthesis_ms"].as_f64().unwrap_or(0.),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::voice_process;
    use std::time::Duration;
    const FAKE: &str = r#"
import sys,struct,json,hashlib,time
mode=sys.argv[1]
def packet(v):
 b=json.dumps(v).encode();sys.stdout.buffer.write(struct.pack('<I',len(b))+b);sys.stdout.buffer.flush()
def read():
 n=struct.unpack('<I',sys.stdin.buffer.read(4))[0];return json.loads(sys.stdin.buffer.read(n))
v=read(); g=v['generation'];r=v['request_id']
if mode=='hang':time.sleep(5);sys.exit()
if mode=='eof':sys.exit()
if mode=='control_limit':sys.stdout.buffer.write(struct.pack('<I',131073));sys.stdout.buffer.flush();sys.exit()
if mode=='corrupt':sys.stdout.buffer.write(struct.pack('<I',1)+b'{');sys.stdout.buffer.flush();sys.exit()
packet(dict(protocol=2 if mode=='version' else 1,kind='begin',generation='stale' if mode=='generation' else g,request_id='stale' if mode=='request' else r))
if mode in ['odd','large','truncated']:
 n={'odd':3,'large':32770,'truncated':4}[mode];sys.stdout.buffer.write(struct.pack('<I',n)+b'\0');sys.stdout.buffer.flush();sys.exit()
if mode=='error':
 sys.stdout.buffer.write(struct.pack('<I',0xffffffff));packet(dict(protocol=1,kind='error',generation=g,request_id=r));sys.exit()
pcm=b'\0\0'*16
if mode=='total_limit':
 for i in range(513):sys.stdout.buffer.write(struct.pack('<I',32768)+b'\0'*32768);sys.stdout.buffer.flush()
 sys.exit()
sys.stdout.buffer.write(struct.pack('<I',len(pcm))+pcm+struct.pack('<I',0))
packet(dict(protocol=1,kind='done',generation=g,request_id=r,pcm_bytes=len(pcm)+int(mode=='count'),pcm_sha256='invalid' if mode=='hash' else hashlib.sha256(pcm).hexdigest(),sample_rate=16000 if mode=='rate' else 22050,channels=1,sample_width=2,synthesis_ms=1.))
"#;
    #[tokio::test]
    async fn bounded_protocol_rejects_stale_corrupt_truncated_and_stalled_workers() {
        for mode in [
            "valid",
            "version",
            "generation",
            "request",
            "odd",
            "large",
            "truncated",
            "error",
            "count",
            "hash",
            "rate",
            "total_limit",
            "control_limit",
            "corrupt",
            "eof",
            "hang",
        ] {
            let mut command = voice_process::command("/usr/bin/python3");
            command
                .env_clear()
                .args(["-I", "-B", "-S", "-c", FAKE, mode]);
            let mut child = command.spawn().unwrap();
            let pid = child.id().unwrap();
            let mut input = child.stdout.take().unwrap();
            let mut out = child.stdin.take().unwrap();
            let mut began = false;
            let result = tokio::time::timeout(
                Duration::from_millis(500),
                synthesize(
                    &mut input,
                    &mut out,
                    "generation",
                    "request",
                    "Texto sintético",
                    || began = true,
                ),
            )
            .await;
            if mode == "valid" {
                assert_eq!(result.unwrap().unwrap().pcm.len(), 32);
                assert!(began);
            } else {
                assert!(
                    result.is_err() || result.unwrap().is_err(),
                    "accepted {mode}"
                );
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
            assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        }
    }
    #[test]
    fn ready_identity_cannot_be_only_a_pid() {
        assert!(identity(
            &json!({"protocol":1,"pid":123}),
            "ready",
            "generation",
            None
        )
        .is_err());
    }
}
