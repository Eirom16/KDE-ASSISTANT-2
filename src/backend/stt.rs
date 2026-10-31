//! STT (Speech-to-Text) con whisper-rs
//!
//! Wrapper alrededor de `whisper-rs` que:
//! - Carga el modelo ggml-base.bin de forma perezosa (una sola vez)
//! - Transcribe audio mono f32 a 16kHz
//! - Soporta auto-deteccion de idioma y override explicito
//!
//! F4 carga bajo demanda dentro de un worker terminable; nunca descarga modelos.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperEngine {
    model_path: PathBuf,
    context: Mutex<Option<Arc<WhisperContext>>>,
    language_override: Option<String>,
}

/// Explicit experimental settings. Product defaults retain full acoustic context.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct DecodeOptions {
    pub threads: i32,
    pub audio_ctx: i32,
    pub temperature_inc: f32,
}
impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            threads: std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(1)
                .clamp(1, 3),
            audio_ctx: 0,
            temperature_inc: 0.2,
        }
    }
}
#[derive(Debug, serde::Serialize)]
pub struct Measurement {
    pub state_ms: f64,
    pub inference_ms: f64,
    pub text_ms: f64,
    pub segments: i32,
    pub tokens: i32,
    pub text: String,
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
        let mut guard = self.context.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Carga el modelo en memoria (para pre-warm en background al arrancar).
    /// Es idempotente: si ya esta cargado no hace nada.
    pub fn ensure_loaded(&self) -> Result<()> {
        self.load_context()?;
        Ok(())
    }

    /// Transcribe audio (mono f32, 16kHz) a texto.
    ///
    /// Si `language` es None, whisper auto-detecta el idioma.
    pub fn transcribe(&self, audio: &[f32], language: Option<&str>) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        Ok(self
            .measure(audio, language, DecodeOptions::default())?
            .text)
    }

    /// Offline diagnostics use the same decoder, without changing product settings.
    pub fn measure(
        &self,
        audio: &[f32],
        language: Option<&str>,
        options: DecodeOptions,
    ) -> Result<Measurement> {
        self.measure_with_observer(audio, language, options, || Ok(()))
    }

    /// A benchmark can announce entry to inference after load/state allocation.
    /// Production uses the no-op observer, without files or diagnostic logging.
    pub fn measure_with_observer(
        &self,
        audio: &[f32],
        language: Option<&str>,
        options: DecodeOptions,
        before_inference: impl FnOnce() -> Result<()>,
    ) -> Result<Measurement> {
        anyhow::ensure!((1..=4).contains(&options.threads), "Hilos STT inválidos");
        anyhow::ensure!(
            (0..=1500).contains(&options.audio_ctx),
            "Contexto STT inválido"
        );
        anyhow::ensure!(
            options.temperature_inc.is_finite() && (0.0..=1.0).contains(&options.temperature_inc),
            "Temperatura STT inválida"
        );
        anyhow::ensure!(
            !audio.is_empty() && audio.len() <= 120 * 16000 && audio.iter().all(|s| s.is_finite()),
            "Audio STT inválido"
        );
        let ctx = self.load_context()?;
        let start = Instant::now();
        let mut state = ctx.create_state().context("creando whisper state")?;
        let state_ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut result = transcribe_with_lang(
            &mut state,
            audio,
            options,
            language.or(self.language_override.as_deref()),
            before_inference,
        )?;
        result.state_ms = state_ms;
        Ok(result)
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
    options: DecodeOptions,
    language: Option<&str>,
    before_inference: impl FnOnce() -> Result<()>,
) -> Result<Measurement> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });

    params.set_n_threads(options.threads);
    params.set_audio_ctx(options.audio_ctx);
    params.set_temperature_inc(options.temperature_inc);
    params.set_translate(false);
    params.set_no_context(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(false);

    params.set_language(language);

    before_inference()?;
    let start = Instant::now();
    state
        .full(params, audio)
        .context("ejecutando whisper (full)")?;
    let inference_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let mut text = String::new();
    let mut tokens = 0;
    for seg in state.as_iter() {
        tokens += seg.n_tokens();
        text.push_str(seg.to_str().unwrap_or(""));
    }
    Ok(Measurement {
        state_ms: 0.0,
        inference_ms,
        text_ms: start.elapsed().as_secs_f64() * 1000.0,
        segments: state.full_n_segments(),
        tokens,
        text: text.trim().to_string(),
    })
}
