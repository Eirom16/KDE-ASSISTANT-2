//! STT por API (Groq `whisper-large-v3-turbo` / `whisper-large-v3`).
//!
//! Sustituto rápido del whisper local: el audio 16kHz mono se codifica a WAV
//! PCM16 en memoria y se sube vía multipart a
//! `{base_url}/audio/transcriptions` (endpoint OpenAI-compatible de Groq).
//!
//! Latencia típica: <1s para turnos de ~8s (vs ~30s del whisper local en
//! CPU). Mínimo facturado 10s por request; coste irrelevante por turno.

use anyhow::{Context, Result};

const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Recorte de errores del proveedor (regla F0-4: nunca volcar HTML al log chat).
const MAX_ERR_BODY: usize = 500;

pub struct SttApiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl SttApiClient {
    pub fn new(base_url: String, api_key: String, model: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("KDE-Assistant/2.0")
            .build()
            .context("construyendo cliente HTTP para STT API")?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Transcribe audio mono f32 a 16kHz. `language`: ISO-639-1 ("es"),
    /// None = auto-detección del proveedor.
    pub async fn transcribe(&self, samples: &[f32], language: Option<&str>) -> Result<String> {
        let wav = encode_wav_16k(samples);
        self.transcribe_with_retry(&wav, language).await
    }

    async fn transcribe_with_retry(&self, wav: &[u8], language: Option<&str>) -> Result<String> {
        let mut last_err = anyhow::anyhow!("sin intentos");
        for attempt in 1..=2 {
            match self.transcribe_once(wav, language).await {
                Ok(text) => return Ok(text),
                Err(e) => {
                    let retryable = e
                        .downcast_ref::<SttApiError>()
                        .is_some_and(|se| se.retryable);
                    log::warn!("STT API intento {attempt}/2 falló: {e}");
                    if !retryable {
                        return Err(e);
                    }
                    last_err = e;
                    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                }
            }
        }
        Err(last_err)
    }

    async fn transcribe_once(&self, wav: &[u8], language: Option<&str>) -> Result<String> {
        let (body, content_type) = build_transcription_multipart(wav, &self.model, language);
        let url = format!("{}/audio/transcriptions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await
            .context("enviando audio a la API STT")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let mut snippet: String = body.chars().take(MAX_ERR_BODY).collect();
            snippet = snippet.replace('\n', " ").trim().to_string();
            return Err(SttApiError {
                status: status.as_u16(),
                retryable: status.as_u16() == 429 || status.is_server_error(),
                message: format!("STT API {status}: {snippet}"),
            }
            .into());
        }

        #[derive(serde::Deserialize)]
        struct TranscriptionResponse {
            text: String,
        }
        let parsed: TranscriptionResponse = resp
            .json()
            .await
            .context("parseando respuesta STT (JSON)")?;
        Ok(parsed.text.trim().to_string())
    }
}

/// Error HTTP de la API de transcripción.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct SttApiError {
    #[allow(dead_code)]
    status: u16,
    retryable: bool,
    message: String,
}

/// Codifica samples f32 mono a WAV PCM16 16kHz (header RIFF clásico).
pub fn encode_wav_16k(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // tamaño chunk fmt
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&16000u32.to_le_bytes()); // sample rate
    out.extend_from_slice(&32000u32.to_le_bytes()); // byte rate (16k * 2)
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits por sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Construye el body multipart/form-data del endpoint de transcripciones.
/// Manual para no arrastrar features extra de reqwest.
fn build_transcription_multipart(
    wav: &[u8],
    model: &str,
    language: Option<&str>,
) -> (Vec<u8>, String) {
    let boundary = format!("----kde-assistant-{}", uuid::Uuid::new_v4().simple());
    let mut body: Vec<u8> = Vec::with_capacity(wav.len() + 512);
    let field = |body: &mut Vec<u8>, name: &str, value: &str| {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    };
    field(&mut body, "model", model);
    if let Some(lang) = language {
        field(&mut body, "language", lang);
    }
    field(&mut body, "response_format", "json");
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let content_type = format!("multipart/form-data; boundary={boundary}");
    (body, content_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_encode_header_valido() {
        let wav = encode_wav_16k(&[0.0, 0.5, -0.5]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        // data chunk size = 3 samples * 2 bytes
        let data_size = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data_size, 6);
        assert_eq!(wav.len(), 44 + 6);
    }

    #[test]
    fn wav_encode_roundtrip_con_hound() {
        let src: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let wav = encode_wav_16k(&src);
        let mut reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(spec.bits_per_sample, 16);
        let decoded: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect();
        assert_eq!(decoded.len(), src.len());
        let max_err = src
            .iter()
            .zip(decoded.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(max_err < 0.001, "max_err={max_err}");
    }

    #[test]
    fn multipart_contiene_campos_y_cierre() {
        let (body, ctype) =
            build_transcription_multipart(b"WAVDATA", "whisper-large-v3-turbo", Some("es"));
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(ctype.contains("multipart/form-data; boundary="));
        assert!(text.contains("name=\"model\""));
        assert!(text.contains("whisper-large-v3-turbo"));
        assert!(text.contains("name=\"language\""));
        assert!(text.contains("\r\nes\r\n"));
        assert!(text.contains("name=\"file\"; filename=\"audio.wav\""));
        assert!(text.contains("Content-Type: audio/wav"));
        assert!(text.contains("WAVDATA"));
        // Cierre del boundary al final
        assert!(body.ends_with(b"--\r\n"));
    }

    #[test]
    fn multipart_omite_language_en_auto() {
        let (body, _) = build_transcription_multipart(b"W", "m", None);
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(!text.contains("name=\"language\""));
    }
}
