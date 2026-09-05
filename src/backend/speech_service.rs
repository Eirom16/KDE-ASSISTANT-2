//! Speech Service - Sintesis (TTS) con espeak-ng
//!
//! Para STT (whisper-rs), se hara en una fase posterior cuando se
//! descargue el modelo ggml-base.bin (~140MB).
//!
//! Por ahora provee:
//! - synthesize(text) -> WAV bytes via espeak-ng
//! - speak_blocking(text) -> reproduce el audio generado

use anyhow::{Context, Result};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::Config;

pub struct SpeechService {
    pub config: Arc<RwLock<Config>>,
}

impl SpeechService {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        log::info!("SpeechService: TTS via espeak-ng");
        Ok(Self { config })
    }

    /// Sintetiza texto a bytes WAV usando espeak-ng.
    /// Retorna los bytes del archivo WAV generado.
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let cfg = self.config.read().await.clone();

        // Sanear texto: espeak-ng no maneja bien algunos caracteres
        let safe = text
            .replace('&', "and")
            .replace('<', "less than")
            .replace('>', "greater than");

        // Path al WAV temporal
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("sin cache_dir"))?
            .join("kde-assistant/tts");
        tokio::fs::create_dir_all(&cache_dir).await?;
        let out_path = cache_dir.join(format!("tts_{}.wav", chrono::Utc::now().timestamp_millis()));

        // espeak-ng -v voice -s rate -w output.wav "texto"
        let mut cmd = Command::new("espeak-ng");
        cmd.arg("-v").arg(&cfg.speech.tts_voice);
        cmd.arg("-s").arg(format!("{}", (175.0 * cfg.speech.tts_rate) as i32)); // rate por defecto 175 ppm
        cmd.arg("-w").arg(&out_path);
        cmd.arg("--stdin");

        // Pasar texto por stdin para evitar problemas con argumentos largos
        use std::io::Write;
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("lanzando espeak-ng")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(safe.as_bytes())?;
        }

        let status = child.wait().context("esperando espeak-ng")?;
        if !status.success() {
            anyhow::bail!("espeak-ng fallo con codigo {:?}", status.code());
        }

        // Leer el WAV generado
        let bytes = tokio::fs::read(&out_path)
            .await
            .with_context(|| format!("leyendo {}", out_path.display()))?;

        // Limpiar (opcional)
        let _ = tokio::fs::remove_file(&out_path).await;

        log::info!("TTS: sintetizadas {} muestras ({} bytes)", safe.len(), bytes.len());
        Ok(bytes)
    }

    /// Reproduce un texto hablado (sintetiza y reproduce).
    pub async fn speak(&self, text: &str) -> Result<()> {
        let wav_bytes = self.synthesize(text).await?;
        play_wav_bytes(&wav_bytes)?;
        Ok(())
    }

    /// STT placeholder. En una fase posterior se integrara whisper-rs.
    pub async fn transcribe(&self, _audio: &[f32]) -> Result<String> {
        anyhow::bail!("STT no implementado aun (requiere whisper-rs + modelo ggml-base.bin)")
    }

    /// Verifica si espeak-ng esta disponible.
    pub async fn is_available(&self) -> bool {
        Command::new("espeak-ng")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Lista las voces disponibles de espeak-ng.
    pub async fn list_voices() -> Result<Vec<String>> {
        let output = Command::new("espeak-ng")
            .arg("--voices")
            .arg("variant")
            .output()
            .context("listando voces espeak-ng")?;
        if !output.status.success() {
            return Ok(vec!["es".to_string(), "en".to_string()]);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let voices: Vec<String> = stdout
            .lines()
            .skip(1) // header
            .filter_map(|line| line.split_whitespace().nth(1).map(|s| s.to_string()))
            .collect();
        Ok(if voices.is_empty() {
            vec!["es".to_string(), "en".to_string()]
        } else {
            voices
        })
    }
}

/// Reproduce bytes WAV via rodio (sincrono, bloqueante).
pub fn play_wav_bytes(wav_bytes: &[u8]) -> Result<()> {
    use std::io::Cursor;

    let (_stream, handle) = rodio::OutputStream::try_default()
        .map_err(|e| anyhow::anyhow!("abriendo output stream: {e}"))?;
    let cursor = Cursor::new(wav_bytes.to_vec());
    let sink = handle.play_once(cursor).map_err(|e| anyhow::anyhow!("play_once: {e}"))?;
    sink.set_volume(0.9);
    sink.sleep_until_end();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn espeak_ng_available() {
        let svc = SpeechService::new(Arc::new(RwLock::new(Config::default()))).await.unwrap();
        // En CI puede no estar; solo verificar que el metodo no panic
        let _ = svc.is_available().await;
    }

    #[tokio::test]
    async fn list_voices_returns_something() {
        let voices = SpeechService::list_voices().await.unwrap();
        assert!(!voices.is_empty());
    }
}
