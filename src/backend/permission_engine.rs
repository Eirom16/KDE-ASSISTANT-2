//! F2 authority: model output and caller-supplied consent strings are never grants.
use anyhow::{bail, ensure, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::models::{Config, ToolCall};

use super::safe_fs::SafePath;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    Green,
    Yellow,
    Red,
}

impl Risk {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Red => "red",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub risk: Risk,
    pub effect: String,
    pub resource: String,
    pub detail: String,
    pub identity: String,
    pub fingerprint: String,
}

fn string_arg<'a>(call: &'a ToolCall, key: &str) -> Result<&'a str> {
    let value = call.arguments.get(key).and_then(Value::as_str);
    let Some(value) = value else {
        bail!("invalid_tool_argument:{key}")
    };
    ensure!(
        !value.is_empty() && value.len() <= 4096,
        "invalid_tool_argument:{key}"
    );
    Ok(value)
}

fn content_arg(call: &ToolCall) -> Result<&str> {
    let content = call
        .arguments
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("invalid_tool_argument:content"))?;
    ensure!(content.len() <= 1024 * 1024, "consent_write_limit");
    Ok(content)
}

fn sensitive_path(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            ".ssh" | ".gnupg" | ".aws" | ".config" | ".local" | ".mozilla" | ".password-store"
        )
    }) || path.ends_with(".pem")
        || path.ends_with(".key")
}

pub fn plan(call: &ToolCall, cfg: &Config) -> Result<Plan> {
    ensure!(call.arguments.len() <= 16, "tool_argument_limit");
    let required: &[&str] = match call.name.as_str() {
        "system_info" => &[],
        "read_file" => &["path"],
        "create_file" => &["path", "content"],
        "edit_file" => &["path", "mode", "content"],
        _ => &[],
    };
    if matches!(
        call.name.as_str(),
        "system_info" | "read_file" | "create_file" | "edit_file"
    ) {
        ensure!(
            call.arguments.len() == required.len()
                && required.iter().all(|key| call.arguments.contains_key(*key)),
            "unexpected_tool_arguments"
        );
    }
    let ordered: BTreeMap<_, _> = call.arguments.iter().collect();
    let encoded = serde_json::to_vec(&ordered)?;
    ensure!(encoded.len() <= 256 * 1024, "tool_argument_limit");
    let (risk, effect, resource, identity) = match call.name.as_str() {
        "system_info" => {
            ensure!(cfg.tools.system_info, "tool_disabled: system_info flag");
            (
                Risk::Green,
                "Consultar información general del sistema",
                "equipo".to_owned(),
                String::new(),
            )
        }
        "read_file" | "create_file" | "edit_file" | "preview_document" => {
            if call.name == "preview_document" {
                bail!("tool_disabled: document renderer lacks bounded child lifetime")
            }
            ensure!(
                match call.name.as_str() {
                    "read_file" => cfg.tools.read_file,
                    "create_file" => cfg.tools.create_file,
                    "edit_file" => cfg.tools.edit_file,
                    _ => false,
                },
                "tool_disabled: individual flag"
            );
            let path = string_arg(call, "path")?;
            ensure!(!sensitive_path(path), "tool_disabled: secret location");
            let resolved = SafePath::resolve(cfg, path)?;
            let identity = resolved.consent_identity(call.name == "create_file")?;
            let risk = match call.name.as_str() {
                "edit_file" => Risk::Red,
                _ => Risk::Yellow,
            };
            (
                risk,
                match call.name.as_str() {
                    "read_file" => "Leer archivo y enviar su texto al proveedor de IA",
                    "create_file" => "Crear archivo",
                    _ => match string_arg(call, "mode")? {
                        "overwrite" => "Sobrescribir archivo y crear copia de seguridad .bak",
                        "append" => "Añadir texto al archivo y crear copia de seguridad .bak",
                        _ => bail!("invalid_edit_mode"),
                    },
                },
                resolved.display.to_string_lossy().into_owned(),
                identity,
            )
        }
        "find_file" | "find_document" | "show_image" | "copy_file" => {
            bail!("tool_disabled: adapter lacks consent-bound descriptors")
        }
        "web_search" | "open_url" | "open_app" | "open_file" | "focus_app" | "close_app"
        | "kdeconnect" | "list_open_apps" | "network_status" | "notify" | "remind_in"
        | "volume" | "brightness" | "media" => {
            bail!("tool_disabled: destination or process ownership not verified")
        }
        _ => bail!("tool_not_catalogued_or_disabled"),
    };
    let mut hash = Sha256::new();
    hash.update(call.name.as_bytes());
    hash.update([0]);
    hash.update(encoded);
    hash.update([0]);
    hash.update(identity.as_bytes());
    let detail = if matches!(call.name.as_str(), "create_file" | "edit_file") {
        let content = content_arg(call)?;
        let digest = Sha256::digest(content.as_bytes());
        format!("Contenido: {} bytes; SHA-256 {:x}", content.len(), digest)
    } else {
        String::new()
    };
    Ok(Plan {
        risk,
        effect: effect.to_owned(),
        resource,
        detail,
        identity,
        fingerprint: format!("{:x}", hash.finalize()),
    })
}
