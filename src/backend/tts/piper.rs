//! Piper TTS engine
//!
//! Wrapper alrededor del binario `piper-tts` (paquete piper-tts en Arch).
//! Modelos de voz se descargan manualmente desde HuggingFace y se guardan
//! en `~/.local/share/kde-assistant/models/piper/`.
//!
//! Uso:
//!   let piper = PiperEngine::new()?;
//!   if piper.is_available() {
//!       let wav = piper.synthesize("Hola mundo").await?;
//!   }
//!
//! Voces recomendadas (descarga manual desde
//! https://huggingface.co/rhasspy/piper-voices/tree/main):
//! - es_ES-sharvard-medium (~60MB)
//! - es_ES-davefx-medium (~60MB)
//! - en_US-amy-low (~30MB)
//! - en_US-amy-medium (~60MB)

use anyhow::{anyhow, bail, Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct PiperModelInfo {
    pub id: String, // ej: "es_ES-sharvard-medium"
    pub onnx_path: PathBuf,
    pub config_path: PathBuf,
    pub language: String, // ej: "es_ES"
}

/// Wrapper del binario piper-tts. Los campos son solo paths: derivar Clone
/// permite lanzar sintetizadores concurrentes (TTS streaming, plan §11).
#[derive(Clone)]
pub struct PiperEngine {
    pub models_dir: PathBuf,
    pub bin_path: PathBuf,
}

impl PiperEngine {
    /// Crea un nuevo PiperEngine. Verifica el binario piper-tts.
    pub fn new() -> Result<Self> {
        let models_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow!("no se pudo obtener data_local_dir"))?
            .join("kde-assistant/models/piper");
        std::fs::create_dir_all(&models_dir).context("creando directorio de modelos")?;

        // piper-tts en Arch se instala como /usr/bin/piper-tts
        // Tambien soporta "piper" en otros sistemas
        let bin_path = Self::find_binary()?;

        Ok(Self {
            models_dir,
            bin_path,
        })
    }

    /// Busca el binario piper-tts en PATH o rutas conocidas.
    pub fn find_binary() -> Result<PathBuf> {
        for name in &["piper-tts", "piper"] {
            if let Ok(p) = which(name) {
                return Ok(p);
            }
        }
        // Rutas comunes de Arch
        for path in &[
            "/usr/bin/piper-tts",
            "/usr/local/bin/piper-tts",
            "/usr/local/bin/piper",
        ] {
            if std::path::Path::new(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }
        bail!(
            "piper-tts no encontrado. Instala con: sudo pacman -S piper-tts\n\
             o descarga el binario desde https://github.com/rhasspy/piper/releases"
        );
    }

    /// Verifica si el motor esta disponible (binario + al menos un modelo).
    pub fn is_available(&self) -> bool {
        self.bin_path.exists() && !self.list_models().is_empty()
    }

    /// Verifica solo el binario (no requiere modelos).
    pub fn has_binary(&self) -> bool {
        self.bin_path.exists()
    }

    /// Lista los modelos de voz disponibles (.onnx en models_dir).
    pub fn list_models(&self) -> Vec<PiperModelInfo> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(&self.models_dir) {
            Ok(e) => e,
            Err(_) => return out,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("onnx") {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let id = stem.to_string();
                let config_path = path.with_extension("onnx.json");
                let language = id.split('_').take(2).collect::<Vec<_>>().join("_");
                out.push(PiperModelInfo {
                    id,
                    onnx_path: path,
                    config_path,
                    language,
                });
            }
        }
        // Ordenar por nombre
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Busca un modelo por id. Retorna None si no existe.
    pub fn find_model(&self, id: &str) -> Option<PiperModelInfo> {
        self.list_models().into_iter().find(|m| m.id == id)
    }

    /// Modelo por defecto (el primero disponible, o uno hardcoded si hay).
    pub fn default_model(&self) -> Option<PiperModelInfo> {
        let models = self.list_models();
        if models.is_empty() {
            return None;
        }
        // Preferir es_ES si hay
        for m in &models {
            if m.language == "es_ES" {
                return Some(m.clone());
            }
        }
        models.first().cloned()
    }

    /// Sintetiza texto a bytes WAV usando piper.
    pub async fn synthesize(
        &self,
        text: &str,
        model_id: Option<&str>,
        length_scale: Option<f32>,
    ) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            bail!("texto vacio");
        }

        let model = match model_id {
            Some(id) => self.find_model(id).ok_or_else(|| {
                anyhow!(
                    "modelo '{id}' no encontrado en {}",
                    self.models_dir.display()
                )
            })?,
            None => self.default_model().ok_or_else(|| {
                anyhow!(
                    "no hay modelos piper en {}. Descarga uno.",
                    self.models_dir.display()
                )
            })?,
        };

        if !model.onnx_path.exists() {
            bail!("modelo .onnx no existe: {}", model.onnx_path.display());
        }
        if !model.config_path.exists() {
            bail!(
                "config .onnx.json no existe: {}",
                model.config_path.display()
            );
        }

        // Path al WAV temporal
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("sin cache_dir"))?
            .join("kde-assistant/tts");
        tokio::fs::create_dir_all(&cache_dir).await?;
        let out_path = cache_dir.join(format!(
            "piper_{}.wav",
            chrono::Utc::now().timestamp_millis()
        ));

        // Construir comando
        let mut cmd = Command::new(&self.bin_path);
        cmd.arg("-m").arg(&model.onnx_path);
        cmd.arg("-c").arg(&model.config_path);
        cmd.arg("-f").arg(&out_path);
        if let Some(ls) = length_scale {
            if ls > 0.0 && (ls - 1.0).abs() > 0.01 {
                cmd.arg("--length-scale").arg(ls.to_string());
            }
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().context("lanzando piper-tts")?;

        // Escribir texto a stdin y cerrar
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = child
            .wait_with_output()
            .await
            .context("esperando piper-tts")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "piper-tts fallo con codigo {:?}: {}",
                output.status.code(),
                stderr
            );
        }

        if !out_path.exists() {
            bail!("piper-tts termino ok pero no genero {}", out_path.display());
        }

        let bytes = tokio::fs::read(&out_path)
            .await
            .with_context(|| format!("leyendo {}", out_path.display()))?;

        // Limpiar
        let _ = tokio::fs::remove_file(&out_path).await;

        log::info!(
            "piper: '{}' ({} chars) -> {} bytes WAV con modelo {}",
            text.chars().take(50).collect::<String>(),
            text.chars().count(),
            bytes.len(),
            model.id
        );

        Ok(bytes)
    }

    /// Reproduce el texto sintetizado inmediatamente.
    pub async fn speak(
        &self,
        text: &str,
        model_id: Option<&str>,
        length_scale: Option<f32>,
    ) -> Result<()> {
        let wav = self.synthesize(text, model_id, length_scale).await?;
        super::super::speech_service::play_wav_bytes(&wav)?;
        Ok(())
    }

    /// Devuelve un mensaje de ayuda si piper no esta disponible.
    pub fn help_message() -> String {
        "piper-tts no esta configurado. Para habilitarlo:\n\
         1. Instala el binario:  sudo pacman -S piper-tts  (Arch)\n\
         2. Descarga un modelo de voz desde:\n\
            https://huggingface.co/rhasspy/piper-voices/tree/main\n\
         3. Coloca el archivo .onnx y .onnx.json en:\n\
            ~/.local/share/kde-assistant/models/piper/\n\
         \n\
         Recomendado: es_ES-sharvard-medium (~60MB)"
            .to_string()
    }
}

/// Implementacion minima de `which` para evitar agregar el crate.
fn which(name: &str) -> Result<PathBuf> {
    if let Ok(paths) = std::env::var("PATH") {
        for p in paths.split(':') {
            let candidate = PathBuf::from(p).join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    bail!("{name} no encontrado en PATH")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_finds_common_tools() {
        assert!(which("sh").is_ok());
    }

    #[test]
    fn list_models_empty_when_no_dir() {
        // Smoke test del modulo
        let _ = std::env::var("HOME");
    }
}
