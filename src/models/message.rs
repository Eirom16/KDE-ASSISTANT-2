//! Mensaje del chat

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::ToolCall;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    #[serde(rename = "system")]
    System { content: String },

    #[serde(rename = "user")]
    User { content: String },

    #[serde(rename = "assistant")]
    Assistant {
        content: String,
        /// Tool calls emitidos junto a esta respuesta (formato OpenAI:
        /// todo mensaje `tool` debe ir precedido de un `assistant` con sus
        /// `tool_calls`; sin esto Groq rechaza la conversacion con 400).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },

    #[serde(rename = "tool")]
    Tool {
        tool_call_id: String,
        content: String,
        /// URL/ruta local de imagen (solo show_image). Display-only:
        /// no se envia a la API, solo a la UI y a SQLite.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant {
            content: content.into(),
            tool_calls: Vec::new(),
        }
    }

    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self::Assistant {
            content: content.into(),
            tool_calls,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Tool {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            image_url: None,
        }
    }

    pub fn tool_with_image(
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
        image_url: Option<String>,
    ) -> Self {
        Self::Tool {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            image_url,
        }
    }
}

/// Evento del stream de AI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamEvent {
    /// Token de texto del asistente
    Token { content: String },

    /// Llamada a herramienta
    ToolCall { tool: ToolCall },

    /// Resultado de herramienta
    ToolResult {
        tool_call_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
    },

    /// Petición de confirmación para una tool 🟡/🔴 (F4-1).
    /// El agente se queda esperando `POST /api/tools/approve`.
    ToolApprovalNeeded {
        tool_call_id: String,
        name: String,
        arguments: HashMap<String, serde_json::Value>,
        permission: String,
    },

    /// Stream completo
    Done { full_content: String },

    /// Error
    Error { message: String },
}
