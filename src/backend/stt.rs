//! STT (Speech-to-Text) con whisper-rs
//!
//! Wrapper alrededor de `whisper-rs` que:
//! - Carga el modelo ggml-base.bin de forma perezosa (una sola vez)
//! - Transcribe audio mono f32 a 16kHz
//! - Soporta auto-deteccion de idioma y override explicito
//!
//! El modelo se espera en `~/.local/share/kde-assistant/models/ggml-base.bin`
//! (se descarga automaticamente via `model_downloader` si no existe).

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperEngine {
    model_path: PathBuf,
    context: Mutex<Option<Arc<WhisperContext>>>,
    language_override: Option<String>,
}

impl WhisperEngine {
    pub fn new(model_path: PathBuf, language_override: Option<String>) -> Result<Self> {
        Ok(Self {
            model_path,
            context: Mutex::new(None),
            language_override,
        })
    }

    /// Carga el contexto (perezoso, solo la primera vez).
    fn load_context(&self) -> Result<Arc<WhisperContext>> {
        let mut guard = self.context.lock().unwrap();
        if let Some(ctx) = guard.as_ref() {
            return Ok(ctx.clone());
        }
        log::info!(
            "Cargando modelo whisper desde {}...",
            self.model_path.display()
        );
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(&self.model_path, params).with_context(|| {
            format!(
                "cargando modelo whisper '{}'. Descargalo con el model_downloader.",
                self.model_path.display()
            )
        })?;
        log::info!("Modelo whisper cargado correctamente");
        let ctx = Arc::new(ctx);
        *guard = Some(ctx.clone());
        Ok(ctx)
    }

    /// Verifica si el modelo existe en disco.
    pub fn model_exists(&self) -> bool {
        self.model_path.exists()
    }

    /// Transcribe audio (mono f32, 16kHz) a texto.
    ///
    /// Si `language` es None, whisper auto-detecta el idioma.
    pub fn transcribe(&self, audio: &[f32], language: Option<&str>) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let ctx = self.load_context()?;
        let mut state = ctx.create_state().context("creando whisper state")?;

        // Numero de threads: usar los cores disponibles
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        let n_threads = n_threads.max(1);

        let lang = language.or(self.language_override.as_deref());
        transcribe_with_lang(&mut state, audio, n_threads, lang)
    }

    /// Retorna el lenguaje detectado en la ultima transcripcion.
    /// (Requiere guardar estado; por ahora retorna None)
    pub fn detected_language(&self) -> Option<String> {
        None
    }
}

fn transcribe_with_lang(
    state: &mut whisper_rs::WhisperState,
    audio: &[f32],
    n_threads: i32,
    language: Option<&str>,
) -> Result<String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });

    params.set_n_threads(n_threads);
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(false);

    if let Some(l) = language {
        params.set_language(Some(l));
    }

    state
        .full(params, audio)
        .context("ejecutando whisper (full)")?;

    let mut text = String::new();
    for seg in state.as_iter() {
        text.push_str(&seg.to_str().unwrap_or(""));
    }
    Ok(text.trim().to_string())
}
