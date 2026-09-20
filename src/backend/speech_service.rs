//! Speech Service - Sintesis (TTS) con piper-tts + Reconocimiento (STT) con whisper-rs
//!
//! TTS: motor neural local (ONNX), alta calidad y casi humana en espanol.
//! STT: whisper-rs con modelo ggml-base.bin (local, auto-descarga).
//!
//! El binario `piper-tts` se invoca como subproceso (wrapper en
//! `crate::backend::tts::piper::PiperEngine`).

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::backend::stt::WhisperEngine;
use crate::backend::stt_api::SttApiClient;
use crate::backend::tts::piper::PiperEngine;
use crate::models::Config;

pub struct SpeechService {
    pub config: Arc<RwLock<Config>>,
    pub piper: PiperEngine,
    pub whisper: Arc<WhisperEngine>,
    stop_requested: Arc<AtomicBool>,
    /// Cliente STT por API (Groq). Cache: se reconstruye si cambian
    /// base_url/key/modelo (clave guardada junto al cliente).
    stt_api: std::sync::Mutex<Option<(SttApiKey, Arc<SttApiClient>)>>,
    /// Nivel del audio TTS reproduciéndose AHORA (f32 bits, 0..1 escalado
    /// como el nivel del mic). El pipeline lo usa como referencia de eco:
    /// solo hay barge-in si el mic excede claramente lo que estamos tocando.
    tts_now: Arc<AtomicU32>,
}

/// Clave de cacheo del cliente STT API: cambia con la config.
#[derive(Debug, PartialEq, Eq)]
struct SttApiKey {
    base_url: String,
    api_key: String,
    model: String,
}

/// Backend STT resuelto para el turno actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SttBackend {
    Local,
    Api,
}

impl SpeechService {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        let piper = PiperEngine::new().context("inicializando Piper TTS")?;
        let models = piper.list_models();
        let available = piper.has_binary();

        if !available {
            log::warn!("piper-tts no encontrado. TTS no funcionara hasta instalar.");
        } else if models.is_empty() {
            log::warn!(
                "piper-tts disponible pero sin modelos en {}. Descarga uno desde https://huggingface.co/rhasspy/piper-voices",
                piper.models_dir.display()
            );
        } else {
            log::info!(
                "SpeechService: piper-tts con {} modelo(s): {:?}",
                models.len(),
                models.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
        }

        // Inicializar whisper (STT) con el modelo elegido (tiny/base).
        let cfg = config.read().await.clone();
        let whisper_path = whisper_model_path_for(&cfg.speech.stt_model)?;
        let language_override = if cfg.speech.stt_language == "auto" {
            None
        } else {
            Some(cfg.speech.stt_language.clone())
        };
        let whisper = Arc::new(WhisperEngine::new(whisper_path, language_override)?);
        if whisper.model_exists() {
            log::info!(
                "SpeechService: whisper con modelo {} listo",
                cfg.speech.stt_model
            );
        } else {
            log::warn!(
                "whisper sin modelo en '{}'. Se descargara automaticamente al iniciar.",
                whisper_path_display()
            );
        }

        Ok(Self {
            config,
            piper,
            whisper,
            stop_requested: Arc::new(AtomicBool::new(false)),
            stt_api: std::sync::Mutex::new(None),
            tts_now: Arc::new(AtomicU32::new(0.0f32.to_bits())),
        })
    }

    /// Nivel del TTS reproduciéndose ahora (0..1, misma escala que el mic).
    /// 0 cuando no suena nada. Referencia de eco para el barge-in.
    pub fn tts_playback_level(&self) -> f32 {
        f32::from_bits(self.tts_now.load(Ordering::Relaxed))
    }

    /// Sintetiza texto a bytes WAV.
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let cfg = self.config.read().await.clone();
        self.piper
            .synthesize(
                text,
                Some(&cfg.speech.piper_model),
                Some(cfg.speech.piper_length_scale),
            )
            .await
    }

    /// Reproduce el texto sintetizado inmediatamente (interrumpible via `request_stop`).
    pub async fn speak(&self, text: &str) -> Result<()> {
        // Reset cancel antes de cada locucion
        self.stop_requested.store(false, Ordering::Relaxed);
        let cfg = self.config.read().await.clone();
        let wav = self
            .piper
            .synthesize(
                text,
                Some(&cfg.speech.piper_model),
                Some(cfg.speech.piper_length_scale),
            )
            .await?;
        if self.stop_requested.load(Ordering::Relaxed) {
            log::info!("TTS cancelado durante sintesis");
            return Ok(());
        }
        let flag = self.stop_requested.clone();
        let level_cell = Some(self.tts_now.clone());
        tokio::task::spawn_blocking(move || play_wav_bytes_with_cancel(&wav, flag, level_cell))
            .await
            .context("tts playback task")??;
        Ok(())
    }

    /// Solicita interrumpir la reproduccion en curso (barge-in).
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Relaxed);
        log::info!("TTS stop solicitado (barge-in)");
    }

    /// Reproduce un stream de oraciones (plan §11): sintetiza la siguiente
    /// mientras suena la actual (síntesis anticipada, baja latencia).
    ///
    /// El canal se cierra cuando el LLM termina de emitir. `request_stop()`
    /// (barge-in) corta la reproducción actual y descarta lo pendiente:
    /// nunca se reproduce audio de una operación ya cancelada (plan §12/§13).
    ///
    /// Errores de síntesis de una oración se loguean y se sigue con la
    /// siguiente; un fallo de reproducción (dispositivo) aborta el stream.
    pub async fn speak_streaming(&self, mut sentences: mpsc::Receiver<String>) -> Result<()> {
        self.stop_requested.store(false, Ordering::Relaxed);
        let (wav_model, wav_scale) = {
            let cfg = self.config.read().await;
            (
                cfg.speech.piper_model.clone(),
                cfg.speech.piper_length_scale,
            )
        };
        let (wav_tx, mut wav_rx) = mpsc::channel::<Vec<u8>>(2);
        let producer_stop = self.stop_requested.clone();
        let piper = self.piper.clone();
        let producer = tokio::spawn(async move {
            while let Some(sentence) = sentences.recv().await {
                if producer_stop.load(Ordering::Relaxed) {
                    break;
                }
                match piper
                    .synthesize(&sentence, Some(&wav_model), Some(wav_scale))
                    .await
                {
                    Ok(wav) => {
                        if wav_tx.send(wav).await.is_err() {
                            break; // consumidor cancelado (barge-in)
                        }
                    }
                    Err(e) => {
                        let head: String = sentence.chars().take(40).collect();
                        log::warn!("TTS streaming: oración '{head}…' falló: {e}");
                    }
                }
            }
        });

        let mut playback_error: Option<anyhow::Error> = None;
        while let Some(wav) = wav_rx.recv().await {
            if self.stop_requested.load(Ordering::Relaxed) {
                log::info!("TTS streaming: cola descartada por barge-in");
                break;
            }
            let flag = self.stop_requested.clone();
            let level_cell = Some(self.tts_now.clone());
            match tokio::task::spawn_blocking(move || {
                play_wav_bytes_with_cancel(&wav, flag, level_cell)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    log::warn!("TTS streaming: reproducción falló: {e}");
                    playback_error = Some(e);
                    break;
                }
                Err(e) => {
                    log::warn!("TTS streaming: task de reproducción falló: {e}");
                    playback_error = Some(e.into());
                    break;
                }
            }
        }
        // Cierre: si el productor sigue vivo (canal de oraciones abierto),
        // cortarlo; si ya terminó, esto es no-op.
        producer.abort();
        let _ = producer.await;
        if let Some(e) = playback_error {
            return Err(e);
        }
        Ok(())
    }

    /// Verifica si hay un stop pendiente.
    pub fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }

    /// STT: transcribe audio (mono f32, 16kHz) a texto.
    /// Routing según `speech.stt_backend`:
    /// - "api"/"auto" con proveedor Groq y key → API (whisper-large-v3-turbo,
    ///   ~1s por turno). Fallback a local si la API falla (red/caída).
    /// - "local" o sin key → whisper-rs local (offline, ~30s por turno en CPU).
    ///
    /// Soporta auto-deteccion de idioma y override explicito.
    pub async fn transcribe(&self, audio: &[f32], language: Option<&str>) -> Result<String> {
        let backend = {
            let cfg = self.config.read().await;
            self.resolve_stt_backend(&cfg)
        };
        if backend == SttBackend::Api {
            let t0 = std::time::Instant::now();
            match self.transcribe_via_api(audio, language).await {
                Ok(text) => {
                    let head: String = text.chars().take(60).collect();
                    log::info!(
                        "[STT] api: '{head}' ({}ms, {} samples)",
                        t0.elapsed().as_millis(),
                        audio.len()
                    );
                    return Ok(text);
                }
                Err(e) => {
                    log::warn!("[STT] API falló, cayendo a whisper local: {e}");
                }
            }
        }
        let t0 = std::time::Instant::now();
        // whisper es CPU-intensivo; ejecutar en spawn_blocking para no bloquear el runtime
        let whisper = self.whisper.clone();
        let audio = audio.to_vec();
        let language = language.map(|s| s.to_string());
        let out =
            tokio::task::spawn_blocking(move || whisper.transcribe(&audio, language.as_deref()))
                .await
                .context("whisper transcribe task")?;
        log::info!("[STT] local: {}ms", t0.elapsed().as_millis());
        out
    }

    /// Decide el backend STT según config y disponibilidad de key.
    fn resolve_stt_backend(&self, cfg: &Config) -> SttBackend {
        let wants_api = !cfg.speech.stt_backend.trim().eq_ignore_ascii_case("local");
        if !wants_api {
            return SttBackend::Local;
        }
        let is_groq = cfg.ai.provider_id() == "groq";
        let has_key = !cfg.ai.effective_api_key().is_empty();
        if is_groq && has_key {
            SttBackend::Api
        } else {
            if cfg.speech.stt_backend.trim().eq_ignore_ascii_case("api") {
                log::warn!(
                    "stt_backend=api pero el proveedor no es Groq o falta key; usando local"
                );
            }
            SttBackend::Local
        }
    }

    /// Transcribe vía API Groq, con el cliente cacheado por (url, key, modelo).
    async fn transcribe_via_api(&self, audio: &[f32], language: Option<&str>) -> Result<String> {
        let cfg = self.config.read().await.clone();
        let base_url = cfg.ai.effective_base_url();
        let api_key = cfg.ai.effective_api_key();
        let model = cfg.speech.stt_api_model.clone();
        let wanted = SttApiKey {
            base_url,
            api_key,
            model,
        };
        let client = {
            let mut cache = self.stt_api.lock().unwrap_or_else(|e| e.into_inner());
            match cache.as_ref() {
                Some((k, c)) if *k == wanted => c.clone(),
                _ => {
                    let client = Arc::new(
                        SttApiClient::new(
                            wanted.base_url.clone(),
                            wanted.api_key.clone(),
                            wanted.model.clone(),
                        )
                        .context("construyendo cliente STT API")?,
                    );
                    log::info!("STT API: modelo '{}' via {}", wanted.model, wanted.base_url);
                    *cache = Some((wanted, client.clone()));
                    client
                }
            }
        };
        client.transcribe(audio, language).await
    }

    /// Verifica si piper esta disponible (binario + al menos un modelo).
    pub async fn is_available(&self) -> bool {
        self.piper.is_available()
    }

    /// Lista los modelos de voz piper disponibles.
    pub async fn list_models(&self) -> Vec<String> {
        self.piper.list_models().into_iter().map(|m| m.id).collect()
    }

    /// Verifica si el modelo STT (whisper) esta disponible.
    pub fn stt_model_exists(&self) -> bool {
        self.whisper.model_exists()
    }

    /// Devuelve un mensaje de ayuda si piper no esta configurado.
    pub fn help_message(&self) -> String {
        PiperEngine::help_message()
    }
}

/// Path del modelo whisper según `stt_model` ("tiny" → ggml-tiny.bin).
pub fn whisper_model_path() -> Result<PathBuf> {
    // Compat: sin config a mano, base. El servicio usa `whisper_model_path_for`.
    whisper_model_path_for("base")
}

/// Path del modelo whisper para un `stt_model` dado.
pub fn whisper_model_path_for(stt_model: &str) -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("no se pudo obtener data_local_dir"))?
        .join("kde-assistant/models");
    let file = if stt_model.trim().eq_ignore_ascii_case("tiny") {
        "ggml-tiny.bin"
    } else {
        "ggml-base.bin"
    };
    Ok(base.join(file))
}

fn whisper_path_display() -> String {
    whisper_model_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.local/share/kde-assistant/models/ggml-base.bin".to_string())
}

/// Reproduce bytes WAV pero permite cancelar via `cancel` (barge-in).
/// Si se pasa `level_cell`, publica el nivel de lo reproducido (RMS de los
/// últimos ~300ms, escala 0..1 tipo mic): referencia de eco para que el
/// barge-in exija voz DEL USUARIO por encima de nuestro propio audio.
pub fn play_wav_bytes_with_cancel(
    wav_bytes: &[u8],
    cancel: Arc<AtomicBool>,
    level_cell: Option<Arc<AtomicU32>>,
) -> Result<()> {
    play_wav_bytes_with_cancel_impl(wav_bytes, cancel, level_cell)
}

/// Compat sin referencia de eco.
pub fn play_wav_bytes(wav_bytes: &[u8]) -> Result<()> {
    play_wav_bytes_with_cancel(wav_bytes, Arc::new(AtomicBool::new(false)), None)
}

fn play_wav_bytes_with_cancel_impl(
    wav_bytes: &[u8],
    cancel: Arc<AtomicBool>,
    level_cell: Option<Arc<AtomicU32>>,
) -> Result<()> {
    use std::io::Cursor;
    use std::time::{Duration, Instant};

    // Decodificar una vez (para el sobre de nivel). Si falla, reproducimos
    // igual: sin referencia de eco el barge usa solo el umbral mínimo.
    let decoded: Option<(Vec<f32>, u32)> = (|| {
        let mut r = hound::WavReader::new(Cursor::new(wav_bytes.to_vec())).ok()?;
        let spec = r.spec();
        let rate = spec.sample_rate;
        let ch = spec.channels.max(1) as usize;
        let vals: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => r
                .samples::<i16>()
                .map(|s| s.unwrap_or(0) as f32 / 32768.0)
                .collect(),
            hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        };
        if ch == 1 {
            Some((vals, rate))
        } else {
            Some((
                vals.chunks(ch)
                    .map(|c| c.iter().sum::<f32>() / ch as f32)
                    .collect(),
                rate,
            ))
        }
    })();

    let (_stream, handle) = rodio::OutputStream::try_default()
        .map_err(|e| anyhow::anyhow!("abriendo output stream: {e}"))?;
    let cursor = Cursor::new(wav_bytes.to_vec());
    let sink = handle
        .play_once(cursor)
        .map_err(|e| anyhow::anyhow!("play_once: {e}"))?;
    sink.set_volume(0.9);

    // Reportar nivel de reproducción: RMS de los últimos ~300ms, mismo
    // escalado que el mic (rms*6 clamp 1.0) para comparación directa.
    let t0 = Instant::now();
    while !sink.empty() {
        if cancel.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }
        if let (Some(cell), Some((samples, rate))) = (&level_cell, &decoded) {
            let now = t0.elapsed().as_secs_f32();
            let end = (now * *rate as f32) as usize;
            let start = end.saturating_sub((*rate as f32 * 0.3) as usize);
            if end > start && end <= samples.len() {
                let slice = &samples[start..end.min(samples.len())];
                let rms = (slice.iter().map(|s| s * s).sum::<f32>() / slice.len() as f32).sqrt();
                // El sink reproduce a volumen 0.9: el sobre debe incluir esa
                // atenuación para ser comparable con el nivel del mic.
                cell.store((rms * 0.9 * 6.0).min(1.0).to_bits(), Ordering::Relaxed);
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    // Al terminar (o ser cortado), la referencia vuelve a cero.
    if let Some(cell) = &level_cell {
        cell.store(0.0f32.to_bits(), Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn piper_engine_initializes() {
        // Solo verifica que no panic; puede no tener piper instalado
        let cfg = Arc::new(RwLock::new(Config::default()));
        let svc = SpeechService::new(cfg).await;
        // No fallamos si piper no esta; el servicio debe inicializarse igual
        assert!(svc.is_ok());
    }
}
