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

impl Default for Config {
    fn default() -> Self {
        Self::default_internal()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_true")]
    pub enable_tool_calling: bool,
    #[serde(default = "default_max_iterations")]
    pub max_tool_iterations: u32,
}

fn default_temperature() -> f32 {
    0.7
}
fn default_max_tokens() -> u32 {
    2048
}
fn default_system_prompt() -> String {
    "Eres KDE Assistant, un asistente de escritorio para KDE Plasma Linux. Responde de forma concisa y util en el idioma del usuario.".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_iterations() -> u32 {
    8
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechConfig {
    #[serde(default = "default_stt_model")]
    pub stt_model: String, // Modelo whisper: "base", "tiny", "small"
    #[serde(default = "default_stt_language")]
    pub stt_language: String, // Idioma STT: "auto", "es", "en", etc.
    #[serde(default = "default_tts_engine")]
    pub tts_engine: String, // "piper" (unico soportado actualmente)
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String, // Alias legado (mantener compat con configs viejas)
    #[serde(default = "default_tts_rate")]
    pub tts_rate: f32, // Alias legado
    #[serde(default = "default_piper_model")]
    pub piper_model: String, // ej: "es_ES-sharvard-medium"
    #[serde(default = "default_length_scale")]
    pub piper_length_scale: f32, // 1.0 = normal
    #[serde(default)]
    pub piper_speaker: u32, // id de speaker
    #[serde(default = "default_wake_word")]
    pub wake_word: String,
    #[serde(default = "default_threshold")]
    pub wake_word_threshold: f32,
    #[serde(default)]
    pub auto_speak: bool,
    #[serde(default = "default_true")]
    pub chimes_enabled: bool,
    #[serde(default = "default_wake_word_model_path")]
    pub wake_word_model_path: String,
}

fn default_stt_model() -> String {
    "base".to_string()
}
fn default_stt_language() -> String {
    "auto".to_string()
}
fn default_tts_engine() -> String {
    "piper".to_string()
}
fn default_tts_voice() -> String {
    "es".to_string()
}
fn default_tts_rate() -> f32 {
    1.0
}
fn default_piper_model() -> String {
    "es_ES-sharvard-medium".to_string()
}
fn default_length_scale() -> f32 {
    1.0
}
fn default_wake_word() -> String {
    "hey kde".to_string()
}
fn default_threshold() -> f32 {
    0.8
}
fn default_wake_word_model_path() -> String {
    "assets/models/wake_word.onnx".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_ui_theme")]
    pub theme: String,
    #[serde(default = "default_window_w")]
    pub window_width: u32,
    #[serde(default = "default_window_h")]
    pub window_height: u32,
    #[serde(default)]
    pub show_session_panel: bool,
    #[serde(default = "default_true")]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub translucent: bool,
}

fn default_ui_theme() -> String {
    "system".to_string()
}
fn default_window_w() -> u32 {
    420
}
fn default_window_h() -> u32 {
    680
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub open_app: bool,
    #[serde(default = "default_true")]
    pub create_file: bool,
    #[serde(default = "default_true")]
    pub edit_file: bool,
    #[serde(default = "default_true")]
    pub read_file: bool,
    #[serde(default = "default_true")]
    pub web_search: bool,
    #[serde(default = "default_true")]
    pub show_image: bool,
    #[serde(default = "default_allowed_paths")]
    pub allowed_paths: Vec<String>,
}

fn default_allowed_paths() -> Vec<String> {
    vec![
        "~/Documentos".to_string(),
        "~/Descargas".to_string(),
        "~/Escritorio".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    #[serde(default = "default_shortcut_toggle")]
    pub toggle: String,
    #[serde(default = "default_shortcut_ptt")]
    pub push_to_talk: String,
}

fn default_shortcut_toggle() -> String {
    "Super+Shift+A".to_string()
}
fn default_shortcut_ptt() -> String {
    "Super+Shift+V".to_string()
}

impl Config {
    /// Carga la configuracion desde ~/.config/kde-assistant/config.json
    /// Si no existe, crea una con valores por defecto.
    /// Tolerante a campos faltantes: rellena con defaults via serde.
    pub async fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            log::info!("No existe config.json, creando uno por defecto");
            let config = Self::default();
            config.save().await?;
            return Ok(config);
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let config: Config = serde_json::from_str(&content).unwrap_or_else(|e| {
            log::warn!("Error parseando config.json: {e}. Usando defaults.");
            Self::default()
        });
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
        Self::default_internal()
    }

    fn default_internal() -> Self {
        Self {
            ai: AiConfig {
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                model: "openrouter/z-ai/glm-5.2:free".to_string(),
                temperature: default_temperature(),
                max_tokens: default_max_tokens(),
                system_prompt: default_system_prompt(),
                enable_tool_calling: true,
                max_tool_iterations: default_max_iterations(),
            },
            speech: SpeechConfig {
                stt_model: default_stt_model(),
                stt_language: default_stt_language(),
                tts_engine: default_tts_engine(),
                tts_voice: default_tts_voice(),
                tts_rate: default_tts_rate(),
                piper_model: default_piper_model(),
                piper_length_scale: default_length_scale(),
                piper_speaker: 0,
                wake_word: default_wake_word(),
                wake_word_threshold: default_threshold(),
                auto_speak: false,
                chimes_enabled: true,
                wake_word_model_path: default_wake_word_model_path(),
            },
            ui: UiConfig {
                theme: default_ui_theme(),
                window_width: default_window_w(),
                window_height: default_window_h(),
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
                allowed_paths: default_allowed_paths(),
            },
            shortcuts: ShortcutsConfig {
                toggle: default_shortcut_toggle(),
                push_to_talk: default_shortcut_ptt(),
            },
        }
    }
}
