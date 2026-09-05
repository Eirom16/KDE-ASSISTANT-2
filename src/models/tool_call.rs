//! Tool Call - Estructura para llamadas a herramientas del LLM

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub success: bool,
    pub content: String,
    /// Para show_image: ruta local o URL de la imagen
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

impl ToolResult {
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            success: true,
            content: content.into(),
            image_url: None,
        }
    }

    pub fn error(tool_call_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            success: false,
            content: message.into(),
            image_url: None,
        }
    }

    pub fn with_image(mut self, url: impl Into<String>) -> Self {
        self.image_url = Some(url.into());
        self
    }
}

/// Definicion de una herramienta para enviar al LLM (formato OpenAI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"

    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameters {
    #[serde(rename = "type")]
    pub param_type: String, // "object"

    pub properties: HashMap<String, ToolProperty>,

    #[serde(default)]
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProperty {
    #[serde(rename = "type")]
    pub prop_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ToolProperty>>,
}
