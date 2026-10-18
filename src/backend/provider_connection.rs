//! Provider diagnostics never echo remote bodies, credentials or request URLs.
use crate::models::AiConfig;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct ProviderError {
    pub category: &'static str,
    pub error: &'static str,
    pub provider_status: Option<u16>,
}
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(status) = self.provider_status {
            write!(
                f,
                "provider_http: {status} {}: {}",
                self.category, self.error
            )
        } else {
            write!(f, "{}: {}", self.category, self.error)
        }
    }
}
impl std::error::Error for ProviderError {}

pub fn error(category: &'static str, message: &'static str) -> ProviderError {
    ProviderError {
        category,
        error: message,
        provider_status: None,
    }
}
pub fn transport(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        error(
            "timeout",
            "El proveedor agotó el tiempo de espera; vuelve a intentarlo.",
        )
    } else {
        error(
            "network",
            "No se pudo conectar con el proveedor. Revisa red, DNS, TLS y URL.",
        )
    }
}
pub fn client(timeout: Duration) -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout.min(Duration::from_secs(8)))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .user_agent("KDE-Assistant/2.0")
        .build()
        .map_err(|_| error("backend", "No se pudo inicializar el cliente del backend."))
}
pub fn authorize(request: reqwest::RequestBuilder, ai: &AiConfig) -> reqwest::RequestBuilder {
    let key = ai.effective_api_key();
    let request = if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    };
    if ai.provider_id() == "openrouter" {
        request
            .header("HTTP-Referer", "https://kde-assistant.local")
            .header("X-Title", "KDE Assistant")
    } else {
        request
    }
}
pub fn validate(ai: &AiConfig) -> Result<(), ProviderError> {
    ai.validate_provider()
        .map_err(|_| error("validation", "Proveedor inválido."))?;
    let url = reqwest::Url::parse(&ai.effective_base_url())
        .map_err(|_| error("validation", "URL base inválida."))?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(error(
            "validation",
            "La URL debe ser HTTP(S), sin credenciales, query ni fragmento.",
        ));
    }
    if !ai.endpoint_is_approved() {
        return Err(error("untrusted_endpoint", "Destino distinto del proveedor. Confirma explícitamente esta URL alternativa en Configuración antes de enviar credenciales o mensajes."));
    }
    if ai.effective_api_key().is_empty() && !matches!(ai.provider_id(), "ollama" | "lmstudio") {
        return Err(error(
            "missing_key",
            "API key no configurada. Añade una clave para este proveedor.",
        ));
    }
    Ok(())
}
pub(crate) async fn bytes(mut response: reqwest::Response) -> Result<Vec<u8>, ProviderError> {
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if data.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(error(
                "protocol",
                "Respuesta del proveedor demasiado grande.",
            ));
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
pub async fn http_error(response: reqwest::Response) -> ProviderError {
    let status = response.status().as_u16();
    // Only known machine codes influence classification. Remote text is untrusted.
    let code = bytes(response)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| {
            v["error"]["code"]
                .as_str()
                .or(v["error"]["type"].as_str())
                .map(str::to_owned)
        });
    let mut e = match (status, code.as_deref()) {
        (401, _) => error("invalid_key", "Clave API inválida o revocada. Reemplázala y prueba de nuevo."),
        (402, _) | (_, Some("insufficient_quota" | "billing_hard_limit_reached" | "quota_exceeded")) => error("quota", "Cuota o saldo insuficiente. Revisa la facturación del proveedor."),
        (403, _) => error("forbidden", "El proveedor denegó acceso. Revisa permisos de cuenta y modelo."),
        (_, Some("model_not_found" | "model_decommissioned")) => error("model_not_found", "Modelo inexistente, retirado o no disponible para esta cuenta. Consulta modelos."),
        (404, _) => error("not_found", "Recurso no encontrado. Revisa la URL base y el ID del modelo."),
        (429, _) => error("rate_limit", "Límite de solicitudes alcanzado. Espera y revisa los límites de tu cuenta."),
        (500..=599, _) => error("provider_unavailable", "El proveedor falló temporalmente. Inténtalo más tarde."),
        (300..=399, _) => error("redirect", "El proveedor redirige la solicitud. Usa la URL final de confianza."),
        _ => error("parameters", "El proveedor rechazó los parámetros. Revisa modelo, temperatura, límite de salida y compatibilidad de herramientas."),
    };
    e.provider_status = Some(status);
    e
}
pub fn chat_parameters(ai: &AiConfig, max_tokens: u32) -> Value {
    let mut value =
        json!({"model": ai.effective_model(), "temperature": ai.temperature, "stream": true});
    value[if ai.provider_id() == "groq" {
        "max_completion_tokens"
    } else {
        "max_tokens"
    }] = json!(max_tokens);
    value
}
fn non_chat(id: &str) -> bool {
    let lower = id.to_lowercase();
    [
        "whisper",
        "tts",
        "embed",
        "moderation",
        "guard",
        "dall-e",
        "dall·e",
    ]
    .iter()
    .any(|word| lower.contains(word))
}
pub async fn models(ai: &AiConfig, timeout: Duration) -> Result<Vec<String>, ProviderError> {
    validate(ai)?;
    let response = authorize(
        client(timeout)?.get(format!(
            "{}/models",
            ai.effective_base_url().trim_end_matches('/')
        )),
        ai,
    )
    .send()
    .await
    .map_err(transport)?;
    if !response.status().is_success() {
        return Err(http_error(response).await);
    }
    let value: Value = serde_json::from_slice(&bytes(response).await?)
        .map_err(|_| error("protocol", "El proveedor devolvió JSON inválido."))?;
    let data = value["data"]
        .as_array()
        .ok_or_else(|| error("protocol", "El catálogo del proveedor no contiene data[]."))?;
    let mut models: Vec<String> = data
        .iter()
        .filter(|v| v["active"] != false)
        .filter_map(|v| v["id"].as_str())
        .filter(|id| !non_chat(id) && id.len() <= 512 && !id.chars().any(char::is_control))
        .map(str::to_owned)
        .collect();
    models.sort();
    models.dedup();
    Ok(models)
}
#[derive(Debug)]
pub struct ProbeOutcome {
    pub limited: bool,
}

pub async fn probe(ai: &AiConfig, timeout: Duration) -> Result<ProbeOutcome, ProviderError> {
    let available = models(ai, timeout).await?;
    if !available.iter().any(|m| *m == ai.effective_model()) {
        return Err(error(
            "model_not_found",
            "El modelo no está en el catálogo accesible para esta cuenta. Consulta modelos.",
        ));
    }
    let mut body = chat_parameters(ai, 64);
    body["messages"] = json!([{"role":"user", "content":"Responde OK."}]);
    let response = authorize(
        client(timeout)?.post(format!(
            "{}/chat/completions",
            ai.effective_base_url().trim_end_matches('/')
        )),
        ai,
    )
    .json(&body)
    .send()
    .await
    .map_err(transport)?;
    if !response.status().is_success() {
        return Err(http_error(response).await);
    }
    let mut stream = response.bytes_stream();
    let mut parser = crate::backend::sse_parser::ChatStreamParser::diagnostic();
    let mut total = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(transport)?;
        total += chunk.len();
        if total > 2 * 1024 * 1024 {
            return Err(error("protocol", "Stream de prueba demasiado grande."));
        }
        parser
            .push(&chunk)
            .map_err(|_| error("protocol", "Streaming del proveedor inválido o incompleto."))?;
        if parser.is_done() {
            break;
        }
    }
    let limited = parser.is_limited();
    let (_, calls) = parser.finish().map_err(|_| {
        error(
            "protocol",
            "El proveedor interrumpió el stream antes de [DONE].",
        )
    })?;
    if !calls.is_empty() {
        return Err(error(
            "protocol",
            "La prueba recibió herramientas no solicitadas.",
        ));
    }
    Ok(ProbeOutcome { limited })
}
