//! AI Service - Cliente HTTP para OpenRouter API con streaming SSE
//!
//! Implementa el bucle de agente con tool calling.
//! Fase 1 (scaffold): estructura basica. Implementacion completa en Fase 2.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::Config;

pub struct AiService {
    pub config: Arc<RwLock<Config>>,
}

impl AiService {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        log::info!("AiService: inicializando cliente OpenRouter");
        // TODO(Fase 2): crear reqwest::Client con headers correctos
        // Authorization: Bearer <apiKey>
        // HTTP-Referer: kde-assistant (requerido por OpenRouter)
        // X-Title: KDE Assistant
        Ok(Self { config })
    }

    /// Envia un mensaje al LLM con streaming SSE.
    /// Retorna un receiver que emite tokens conforme llegan.
    pub async fn chat_stream(
        &self,
        _messages: Vec<crate::models::Message>,
        _tools: Vec<crate::models::Tool>,
    ) -> Result<tokio::sync::mpsc::Receiver<crate::models::StreamEvent>> {
        // TODO(Fase 2): implementar streaming SSE real
        // - POST a /chat/completions con stream: true
        // - Procesar cada chunk como JSON
        // - Detectar tool_call en delta
        // - Emitir eventos al canal
        anyhow::bail!("AiService::chat_stream no implementado aun (Fase 2)")
    }

    /// Bucle de agente: ejecuta tool calls hasta obtener respuesta final.
    pub async fn run_agent_loop(
        &self,
        _messages: Vec<crate::models::Message>,
        _tools: Vec<crate::models::Tool>,
        _on_event: impl Fn(crate::models::StreamEvent) + Send + 'static,
    ) -> Result<String> {
        // TODO(Fase 2): implementar bucle ReAct
        // 1. Llamar chat_stream
        // 2. Si recibe tool_call -> ejecutar via tool_executor -> reenviar resultado
        // 3. Si recibe texto final -> retornar
        // Max 8 iteraciones (prevenir loops infinitos)
        anyhow::bail!("AiService::run_agent_loop no implementado aun (Fase 2)")
    }
}
