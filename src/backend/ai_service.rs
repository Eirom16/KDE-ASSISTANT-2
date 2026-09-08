//! AI Service - Cliente HTTP OpenAI-compatible con streaming SSE
//!
//! Soporta varios proveedores (OpenRouter, Groq, OpenAI, personalizado)
//! segun `ai.provider` en la configuracion. Solo OpenRouter recibe los
//! headers extra HTTP-Referer/X-Title.
//!
//! Implementa:
//! - POST a /chat/completions con stream: true
//! - Procesamiento SSE token a token
//! - Deteccion de tool_calls en streaming (acumulacion fragmentada)
//! - Bucle de agente ReAct (max 8 iteraciones, configurable)

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::backend::tool_executor::ToolExecutor;
use crate::models::{Config, Message, StreamEvent, Tool, ToolCall};

const MAX_ITERATIONS_DEFAULT: u32 = 8;

pub struct AiService {
    config: Arc<RwLock<Config>>,
    http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAIMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<Tool>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage<'a> {
    role: &'a str,
    content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCallRef>>,
}

#[derive(Debug, Serialize)]
struct OpenAIToolCallRef {
    id: String,
    r#type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Default, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Delta,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<DeltaFunction>,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

impl AiService {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("KDE-Assistant/2.0")
            .build()?;
        let provider = config
            .try_read()
            .map(|c| crate::models::AiConfig::provider_name(c.ai.provider_id()).to_string())
            .unwrap_or_else(|_| "OpenRouter".to_string());
        log::info!("AiService: cliente {provider} listo");
        Ok(Self { config, http })
    }

    fn snapshot(&self) -> Config {
        self.config
            .try_read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    fn build_messages<'a>(&self, messages: &'a [Message]) -> Vec<OpenAIMessage<'a>> {
        messages
            .iter()
            .map(|m| match m {
                Message::System { content } => OpenAIMessage {
                    role: "system",
                    content: Some(content),
                    tool_call_id: None,
                    tool_calls: None,
                },
                Message::User { content } => OpenAIMessage {
                    role: "user",
                    content: Some(content),
                    tool_call_id: None,
                    tool_calls: None,
                },
                Message::Assistant {
                    content,
                    tool_calls,
                } => OpenAIMessage {
                    role: "assistant",
                    content: Some(content),
                    tool_call_id: None,
                    // Formato OpenAI: todo mensaje `tool` debe ir precedido de
                    // un `assistant` con sus `tool_calls` (Groq lo exige).
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(
                            tool_calls
                                .iter()
                                .map(|tc| OpenAIToolCallRef {
                                    id: tc.id.clone(),
                                    r#type: "function".to_string(),
                                    function: OpenAIFunctionCall {
                                        name: tc.name.clone(),
                                        arguments: serde_json::to_string(&tc.arguments)
                                            .unwrap_or_else(|_| "{}".to_string()),
                                    },
                                })
                                .collect(),
                        )
                    },
                },
                Message::Tool {
                    tool_call_id,
                    content,
                } => OpenAIMessage {
                    role: "tool",
                    content: Some(content),
                    tool_call_id: Some(tool_call_id),
                    tool_calls: None,
                },
            })
            .collect()
    }

    /// Una llamada al LLM. Por cada `StreamEvent` que queramos exponer al
    /// frontend se envia por `tx`. Internamente acumula `tool_calls` que
    /// llegan fragmentados en el stream. Al final (DONE) emite los
    /// `ToolCall` acumulados.
    async fn call_once(
        &self,
        messages: &[Message],
        tools: &[Tool],
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<String> {
        let cfg = self.snapshot();
        let provider_id = cfg.ai.provider_id().to_string();
        let provider_name = crate::models::AiConfig::provider_name(&provider_id);
        let api_key = cfg.ai.effective_api_key();
        if api_key.trim().is_empty() {
            let env_var = crate::models::AiConfig::provider_env_var(&provider_id);
            bail!("API key no configurada. Ponla en Configuracion o configura {env_var} (proveedor actual: {provider_name})");
        }

        let url = format!("{}/chat/completions", cfg.ai.base_url.trim_end_matches('/'));
        let req = ChatRequest {
            model: &cfg.ai.model,
            messages: self.build_messages(messages),
            temperature: cfg.ai.temperature,
            max_tokens: cfg.ai.max_tokens,
            stream: true,
            tools: tools.to_vec(),
        };

        let mut req_builder = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json");
        // OpenRouter pide identificacion extra; el resto de proveedores
        // OpenAI-compatibles (Groq, OpenAI, custom) no la necesitan.
        if provider_id == "openrouter" {
            req_builder = req_builder
                .header("HTTP-Referer", "https://kde-assistant.local")
                .header("X-Title", "KDE Assistant");
        }
        let response = req_builder
            .json(&req)
            .send()
            .await
            .with_context(|| format!("enviando request a {provider_name}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("{provider_name} {status}: {body}");
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut full_text = String::new();
        let mut tool_acc: HashMap<usize, (String, String, String)> = HashMap::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.context("leyendo chunk SSE")?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(nl) = buffer.find('\n') {
                let line = buffer[..nl].trim().to_string();
                buffer.drain(..=nl);

                if line.is_empty() || !line.starts_with("data:") {
                    continue;
                }
                let data = line.trim_start_matches("data:").trim();
                if data == "[DONE]" {
                    let calls = collect_tool_calls(&mut tool_acc);
                    for call in calls {
                        if tx.send(StreamEvent::ToolCall { tool: call }).await.is_err() {
                            return Ok(full_text);
                        }
                    }
                    return Ok(full_text);
                }
                if data.is_empty() {
                    continue;
                }

                let parsed: StreamChunk = match serde_json::from_str(data) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                if let Some(choice) = parsed.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            full_text.push_str(content);
                            let _ = tx
                                .send(StreamEvent::Token {
                                    content: content.clone(),
                                })
                                .await;
                        }
                    }
                    if let Some(tcs) = &choice.delta.tool_calls {
                        for tc in tcs {
                            let entry = tool_acc
                                .entry(tc.index)
                                .or_insert_with(|| (String::new(), String::new(), String::new()));
                            if let Some(id) = &tc.id {
                                entry.0 = id.clone();
                            }
                            if let Some(func) = &tc.function {
                                if let Some(name) = &func.name {
                                    entry.1.push_str(name);
                                }
                                if let Some(args) = &func.arguments {
                                    entry.2.push_str(args);
                                }
                            }
                        }
                    }
                }
            }
        }

        let calls = collect_tool_calls(&mut tool_acc);
        for call in calls {
            let _ = tx.send(StreamEvent::ToolCall { tool: call }).await;
        }
        Ok(full_text)
    }

    /// Bucle de agente ReAct. Mantiene `messages`, ejecuta tool_calls, los
    /// reenvia al LLM, repite hasta respuesta final o max iteraciones.
    /// Emite `StreamEvent`s por el canal: Token, ToolCall, ToolResult, Done/Error.
    pub async fn run_agent(
        &self,
        mut messages: Vec<Message>,
        tools: Vec<Tool>,
        tools_exec: Arc<ToolExecutor>,
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<String> {
        let cfg = self.snapshot();
        let max_iter = if cfg.ai.max_tool_iterations == 0 {
            MAX_ITERATIONS_DEFAULT
        } else {
            cfg.ai.max_tool_iterations
        };

        for iteration in 0..max_iter {
            log::info!("Agent iter {}/{}", iteration + 1, max_iter);

            // Lanzar llamada con un canal interno
            let (inner_tx, mut inner_rx) = mpsc::channel::<StreamEvent>(256);
            let svc_handle = self.clone_handle()?;
            let msgs = messages.clone();
            let tools_c = tools.clone();
            let call_task =
                tokio::spawn(async move { svc_handle.call_once(&msgs, &tools_c, &inner_tx).await });

            // Procesar eventos: recoger texto y tool_calls
            let mut full_text = String::new();
            let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
            while let Some(ev) = inner_rx.recv().await {
                match &ev {
                    StreamEvent::Token { content } => full_text.push_str(content),
                    StreamEvent::ToolCall { tool } => pending_tool_calls.push(tool.clone()),
                    _ => {}
                }
                // Forward al canal externo
                if tx.send(ev).await.is_err() {
                    // Frontend cerro, cancelamos
                    call_task.abort();
                    bail!("Frontend cerro el canal");
                }
            }

            // Esperar a que termine la llamada
            let call_res = call_task.await.context("task del LLM")?;
            if let Err(e) = call_res {
                let _ = tx
                    .send(StreamEvent::Error {
                        message: e.to_string(),
                    })
                    .await;
                bail!("LLM call error: {e}");
            }

            // Si hay tool_calls: ejecutar y continuar
            if !pending_tool_calls.is_empty() {
                // Primero el mensaje assistant con los tool_calls (formato
                // OpenAI/Groq: los resultados `tool` siempre van precedidos
                // de su llamada).
                messages.push(Message::assistant_with_tools(
                    full_text.clone(),
                    pending_tool_calls.clone(),
                ));
                for tc in &pending_tool_calls {
                    log::info!("Ejecutando tool: {} (id={})", tc.name, tc.id);
                    let result = tools_exec.execute(tc).await.unwrap_or_else(|e| {
                        crate::models::ToolResult::error(tc.id.clone(), e.to_string())
                    });
                    let _ = tx
                        .send(StreamEvent::ToolResult {
                            tool_call_id: result.tool_call_id.clone(),
                            content: result.content.clone(),
                        })
                        .await;
                    messages.push(Message::Tool {
                        tool_call_id: result.tool_call_id,
                        content: result.content,
                    });
                }
                continue; // siguiente iteracion
            }

            // Sin tool_calls: respuesta final
            let _ = tx
                .send(StreamEvent::Done {
                    full_content: full_text.clone(),
                })
                .await;
            return Ok(full_text);
        }

        let _ = tx
            .send(StreamEvent::Error {
                message: format!("Limite de iteraciones ({}) alcanzado", max_iter),
            })
            .await;
        bail!("Limite de iteraciones del agente alcanzado")
    }

    /// Construye un clon ligero del servicio (mismo Arc de config, mismo http client).
    fn clone_handle(&self) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            config: self.config.clone(),
            http: self.http.clone(),
        }))
    }
}

fn collect_tool_calls(acc: &mut HashMap<usize, (String, String, String)>) -> Vec<ToolCall> {
    let mut out: Vec<ToolCall> = acc
        .drain()
        .map(|(_, (id, name, args_str))| {
            let arguments: HashMap<String, serde_json::Value> =
                serde_json::from_str(&args_str).unwrap_or_default();
            ToolCall {
                id,
                name,
                arguments,
            }
        })
        .collect();
    for (i, t) in out.iter_mut().enumerate() {
        if t.id.is_empty() {
            t.id = format!("call_{i}");
        }
        if t.name.is_empty() {
            t.name = "unknown".to_string();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_tool_calls_parses_json_args() {
        let mut acc = HashMap::new();
        acc.insert(
            0,
            (
                "abc".to_string(),
                "open_app".to_string(),
                r#"{"name":"firefox"}"#.to_string(),
            ),
        );
        let calls = collect_tool_calls(&mut acc);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "open_app");
        assert_eq!(
            calls[0].arguments.get("name").unwrap().as_str(),
            Some("firefox")
        );
    }

    #[test]
    fn collect_tool_calls_handles_missing_fields() {
        let mut acc = HashMap::new();
        acc.insert(0, (String::new(), String::new(), "{}".to_string()));
        let calls = collect_tool_calls(&mut acc);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_0");
        assert_eq!(calls[0].name, "unknown");
    }
}
