//! Tool Executor - Ejecucion segura de herramientas del agente
//!
//! Herramientas expuestas al LLM:
//! - open_app(name): lanza apps del sistema
//! - create_file(path, content): crea archivos
//! - edit_file(path, mode, content): edita archivos
//! - read_file(path): lee archivos
//! - web_search(query): busca en web
//! - show_image(source, caption?): inyecta imagen en chat
//!
//! Fase 1: estructura. Implementacion completa en Fase 2.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::{Config, ToolCall, ToolResult};

pub struct ToolExecutor {
    pub config: Arc<RwLock<Config>>,
}

impl ToolExecutor {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self { config }
    }

    /// Ejecuta una herramienta y retorna el resultado.
    pub async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult> {
        log::info!("Ejecutando herramienta: {}", tool_call.name);

        let result = match tool_call.name.as_str() {
            "open_app" => self.open_app(tool_call).await,
            "create_file" => self.create_file(tool_call).await,
            "edit_file" => self.edit_file(tool_call).await,
            "read_file" => self.read_file(tool_call).await,
            "web_search" => self.web_search(tool_call).await,
            "show_image" => self.show_image(tool_call).await,
            _ => anyhow::bail!("Herramienta desconocida: {}", tool_call.name),
        };

        result
    }

    async fn open_app(&self, _tc: &ToolCall) -> Result<ToolResult> {
        // TODO(Fase 2): lanzar app con gtk-launch/xdg-open/Command::new
        // NUNCA usar sh -c (ver regla #1 en AGENTS.md)
        anyhow::bail!("open_app no implementado (Fase 2)")
    }

    async fn create_file(&self, _tc: &ToolCall) -> Result<ToolResult> {
        // TODO(Fase 2): validar path, escribir archivo
        anyhow::bail!("create_file no implementado (Fase 2)")
    }

    async fn edit_file(&self, _tc: &ToolCall) -> Result<ToolResult> {
        // TODO(Fase 2): overwrite/append atomico
        anyhow::bail!("edit_file no implementado (Fase 2)")
    }

    async fn read_file(&self, _tc: &ToolCall) -> Result<ToolResult> {
        // TODO(Fase 2): leer y devolver contenido
        anyhow::bail!("read_file no implementado (Fase 2)")
    }

    async fn web_search(&self, _tc: &ToolCall) -> Result<ToolResult> {
        // TODO(Fase 2): DuckDuckGo Lite / SearXNG con reqwest
        anyhow::bail!("web_search no implementado (Fase 2)")
    }

    async fn show_image(&self, _tc: &ToolCall) -> Result<ToolResult> {
        // TODO(Fase 2): descargar o copiar imagen, devolver URL local
        anyhow::bail!("show_image no implementado (Fase 2)")
    }
}
