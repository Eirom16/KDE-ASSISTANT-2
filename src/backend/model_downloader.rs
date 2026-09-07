//! Model Downloader - Descarga automatica de modelos ML
//!
//! Descarga los modelos necesarios (Whisper STT, Piper TTS, Wake Word ONNX)
//! a `~/.local/share/kde-assistant/models/` en el primer arranque si no existen.
//!
//! Caracteristicas:
//! - Streaming con barra de progreso (callback de progreso)
//! - Verificacion de integridad (sha256 opcional)
//! - Reintento automatico (hasta 3 veces)
//! - Reanudacion: no vuelve a descargar si el archivo ya existe
//!
//! Nota: siguiendo la regla "Sin Shell Escaping", usamos reqwest directamente
//! (streaming de bytes) sin invocar curl/wget.

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

pub const WHISPER_BASE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

pub const PIPER_ES_SHARVARD_MEDIUM_ONNX: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/sharvard_medium/es_ES-sharvard-medium.onnx";
pub const PIPER_ES_SHARVARD_MEDIUM_JSON: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/sharvard_medium/es_ES-sharvard-medium.onnx.json";

/// Especificacion de un modelo a descargar.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub name: &'static str,
    pub url: &'static str,
    /// Path relativo dentro de models_dir. Ej: "ggml-base.bin" o "piper/es_ES-sharvard-medium.onnx"
    pub rel_path: &'static str,
    /// sha256 esperado (opcional). Si None, se omite la verificacion.
    pub sha256: Option<&'static str>,
}

/// Callback de progreso: (name, downloaded_bytes, total_bytes)
pub type ProgressCallback = Arc<dyn Fn(&str, u64, Option<u64>) + Send + Sync>;

pub struct ModelDownloader {
    pub models_dir: PathBuf,
    pub cancel: Arc<AtomicBool>,
}

impl ModelDownloader {
    pub fn new() -> Result<Self> {
        let models_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("no se pudo obtener data_local_dir"))?
            .join("kde-assistant/models");
        std::fs::create_dir_all(&models_dir).context("creando directorio de modelos")?;
        Ok(Self {
            models_dir,
            cancel: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Retorna el path absoluto para un modelo dado.
    pub fn path_for(&self, spec: &ModelSpec) -> PathBuf {
        self.models_dir.join(spec.rel_path)
    }

    /// Verifica si un modelo ya esta descargado (y con hash valido si se especifico).
    pub fn is_downloaded(&self, spec: &ModelSpec) -> bool {
        let path = self.path_for(spec);
        path.exists()
            && spec.sha256.map_or(true, |expected| {
                verify_sha256(&path, expected).unwrap_or(false)
            })
    }

    /// Descarga un modelo. No hace nada si ya existe (y hash valido).
    pub async fn download(
        &self,
        spec: &ModelSpec,
        progress: Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        let dest = self.path_for(spec);

        // Ya descargado y valido
        if self.is_downloaded(spec) {
            log::info!("Modelo '{}' ya existe, omitiendo descarga", spec.name);
            return Ok(dest);
        }

        // Reintento hasta 3 veces
        let mut last_err = None;
        for attempt in 1..=3 {
            log::info!(
                "Descargando modelo '{}' (intento {}/3)...",
                spec.name,
                attempt
            );
            match self.download_once(spec, &dest, progress.clone()).await {
                Ok(p) => return Ok(p),
                Err(e) => {
                    log::warn!("Intento {attempt} fallo: {e}");
                    last_err = Some(e);
                    // Limpiar archivo parcial
                    let _ = tokio::fs::remove_file(&dest).await;
                    // Esperar un poco antes de reintentar
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }

        bail!(
            "No se pudo descargar '{}' despues de 3 intentos: {:?}",
            spec.name,
            last_err
        )
    }

    async fn download_once(
        &self,
        spec: &ModelSpec,
        dest: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Descargar a archivo temporal
        let tmp = dest.with_extension("part");
        let mut file = tokio::fs::File::create(&tmp).await?;

        let client = reqwest::Client::builder()
            .user_agent("KDE-Assistant/2.0")
            .build()?;

        let response = client.get(spec.url).send().await?;
        if !response.status().is_success() {
            bail!("HTTP {} al descargar {}", response.status(), spec.url);
        }

        let total = response.content_length();
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            if self.cancel.load(Ordering::Relaxed) {
                bail!("descarga cancelada");
            }
            let bytes = chunk?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &bytes).await?;
            downloaded += bytes.len() as u64;
            if let Some(cb) = &progress {
                cb(spec.name, downloaded, total);
            }
        }
        file.flush().await?;
        drop(file);

        // Verificar hash opcional
        if let Some(expected) = spec.sha256 {
            if !verify_sha256(&tmp, expected)? {
                bail!("checksum sha256 no coincide para '{}'", spec.name);
            }
        }

        // Renombrar a destino final
        tokio::fs::rename(&tmp, dest).await?;
        log::info!(
            "Modelo '{}' descargado ({:.1} MB) -> {}",
            spec.name,
            downloaded as f64 / 1024.0 / 1024.0,
            dest.display()
        );
        Ok(dest.to_path_buf())
    }
}

/// Calcula el sha256 de un archivo y lo compara con el esperado.
fn verify_sha256(path: &Path, expected: &str) -> Result<bool> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hex_digest = hex::encode(digest);
    Ok(hex_digest.eq_ignore_ascii_case(expected))
}

/// Los modelos necesarios para el funcionamiento completo.
pub fn required_models() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            name: "whisper-base",
            url: WHISPER_BASE_URL,
            rel_path: "ggml-base.bin",
            sha256: None,
        },
        ModelSpec {
            name: "piper-es-sharvard-medium",
            url: PIPER_ES_SHARVARD_MEDIUM_ONNX,
            rel_path: "piper/es_ES-sharvard-medium.onnx",
            sha256: None,
        },
        ModelSpec {
            name: "piper-es-sharvard-medium-json",
            url: PIPER_ES_SHARVARD_MEDIUM_JSON,
            rel_path: "piper/es_ES-sharvard-medium.onnx.json",
            sha256: None,
        },
    ]
}

/// Verifica si el hash de un archivo es valido (para tests).
pub fn checksum(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_of_known_content() {
        let dir = std::env::temp_dir().join("kda_test_checksum");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("test.txt");
        std::fs::write(&p, "hello world").unwrap();
        // sha256 de "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert_eq!(checksum(&p).unwrap(), expected);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn path_for_resolves_relative() {
        let dl = ModelDownloader {
            models_dir: PathBuf::from("/tmp/models"),
            cancel: Arc::new(AtomicBool::new(false)),
        };
        let spec = ModelSpec {
            name: "test",
            url: "http://x",
            rel_path: "whisper/ggml-base.bin",
            sha256: None,
        };
        assert_eq!(
            dl.path_for(&spec),
            PathBuf::from("/tmp/models/whisper/ggml-base.bin")
        );
    }
}
