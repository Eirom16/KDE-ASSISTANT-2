//! Configuracion persistida en ~/.config/kde-assistant/config.json

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai: AiConfig,
    pub speech: SpeechConfig,
    pub ui: UiConfig,
    pub tools: ToolsConfig,
    pub shortcuts: ShortcutsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub system_prompt: String,
    pub enable_tool_calling: bool,
    pub max_tool_iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechConfig {
    pub stt_model: String,
    pub tts_engine: String,
    pub tts_voice: String,
    pub tts_rate: f32,
    pub wake_word: String,
    pub wake_word_threshold: f32,
    pub auto_speak: bool,
    pub chimes_enabled: bool,
    pub wake_word_model_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
    pub show_session_panel: bool,
    pub always_on_top: bool,
    pub translucent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub open_app: bool,
    pub create_file: bool,
    pub edit_file: bool,
    pub read_file: bool,
    pub web_search: bool,
    pub show_image: bool,
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    pub toggle: String,
    pub push_to_talk: String,
}

impl Config {
    /// Carga la configuracion desde ~/.config/kde-assistant/config.json
    /// Si no existe, crea una con valores por defecto.
    pub async fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            log::info!("No existe config.json, creando uno por defecto");
            let config = Self::default();
            config.save().await?;
            return Ok(config);
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let config: Config = serde_json::from_str(&content)?;
        log::info!("Config cargada desde {}", path.display());
        Ok(config)
    }

    pub async fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("No se pudo obtener config_dir"))?
            .join("kde-assistant");
        Ok(config_dir.join("config.json"))
    }

    pub fn default() -> Self {
        Self {
            ai: AiConfig {
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                model: "openrouter/z-ai/glm-5.2:free".to_string(),
                temperature: 0.7,
                max_tokens: 2048,
                system_prompt: "Eres KDE Assistant, un asistente de escritorio para KDE Plasma Linux. Responde de forma concisa y util en el idioma del usuario.".to_string(),
                enable_tool_calling: true,
                max_tool_iterations: 8,
            },
            speech: SpeechConfig {
                stt_model: "base".to_string(),
                tts_engine: "espeak-ng".to_string(),
                tts_voice: "es".to_string(),
                tts_rate: 1.0,
                wake_word: "hey kde".to_string(),
                wake_word_threshold: 0.8,
                auto_speak: false,
                chimes_enabled: true,
                wake_word_model_path: "assets/models/wake_word.onnx".to_string(),
            },
            ui: UiConfig {
                theme: "system".to_string(),
                window_width: 420,
                window_height: 680,
                show_session_panel: false,
                always_on_top: true,
                translucent: true,
            },
            tools: ToolsConfig {
                open_app: true,
                create_file: true,
                edit_file: true,
                read_file: true,
                web_search: true,
                show_image: true,
                allowed_paths: vec![
                    "~/Documentos".to_string(),
                    "~/Descargas".to_string(),
                    "~/Escritorio".to_string(),
                ],
            },
            shortcuts: ShortcutsConfig {
                toggle: "Super+Shift+A".to_string(),
                push_to_talk: "Super+Shift+V".to_string(),
            },
        }
    }
}
