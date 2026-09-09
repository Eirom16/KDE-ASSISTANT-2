//! Auth local - Token bearer para el servidor HTTP 127.0.0.1
//!
//! El backend expone `/api/*` sin auth por defecto: cualquier proceso local
//! (incluida una web con DNS-rebinding) podría leer sesiones y ejecutar tools.
//! F0-3 lo cierra con un token aleatorio:
//! - `~/.config/kde-assistant/server.token` (0600, 64 hex chars)
//! - `Authorization: Bearer <token>` en todo `/api/*` excepto `/api/health`
//! - Check de `Host` (solo 127.0.0.1/localhost) y `Origin` (solo vacío/null/local)
//!
//! La UI QML recibe el token por env `KDE_ASSISTANT_TOKEN` (inyectado por main.rs
//! al lanzar `qml6`) y lo envía en cada XHR.

use anyhow::{Context, Result};
use std::path::PathBuf;

const TOKEN_BYTES_HEX: usize = 64;

/// Path del token: `~/.config/kde-assistant/server.token`.
pub fn server_token_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("No se pudo obtener config_dir"))?
        .join("kde-assistant");
    Ok(dir.join("server.token"))
}

/// Lee el token si existe (trim). None si no hay o está vacío.
pub fn load_server_token() -> Option<String> {
    let path = server_token_path().ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let t = content.trim().to_string();
    if t.len() >= 32 {
        Some(t)
    } else {
        None
    }
}

/// Garantiza que existe un token, creándolo con 0600 si falta.
/// Retorna el token.
pub fn ensure_server_token() -> Result<String> {
    if let Some(t) = load_server_token() {
        return Ok(t);
    }
    let path = server_token_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creando dir config")?;
    }
    // uuid v4 usa getrandom: 2x uuid sin guiones = 64 hex chars.
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    // Asegurar 64 chars (simple() ya da 32 cada uno).
    let token: String = token.chars().take(TOKEN_BYTES_HEX).collect();
    std::fs::write(&path, format!("{token}\n")).context("escribiendo server.token")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    log::info!("Token local creado en {}", path.display());
    Ok(token)
}

/// ¿La ruta requiere auth? Todo `/api/*` salvo `/api/health`.
pub fn path_requires_auth(path: &str) -> bool {
    if path == "/api/health" {
        return false;
    }
    path.starts_with("/api/")
}

/// Valida `Authorization: Bearer <token>` con comparación constante (anti-timing).
pub fn valid_bearer(headers: &axum::http::HeaderMap, expected: &str) -> bool {
    let got = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let prefix = "Bearer ";
    if !got.starts_with(prefix) {
        return false;
    }
    let got_token = &got[prefix.len()..];
    constant_eq(got_token.as_bytes(), expected.as_bytes())
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Valida `Host`: solo loopback. `None` (tests sin host) se permite.
pub fn valid_host(headers: &axum::http::HeaderMap) -> bool {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host.is_empty() {
        return true;
    }
    let h = host.to_lowercase();
    // Aceptar con o sin puerto.
    h.starts_with("127.0.0.1") || h.starts_with("localhost") || h.starts_with("[::1]")
}

/// Valida `Origin`: QML envía vacío/`null`. Un navegador evil enviaría
/// `https://evil.com`: rechazar todo lo que no sea vacío/null/loopback/file.
pub fn valid_origin(headers: &axum::http::HeaderMap) -> bool {
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if origin.is_empty() || origin == "null" {
        return true;
    }
    let o = origin.to_lowercase();
    o.contains("127.0.0.1")
        || o.contains("localhost")
        || o.contains("[::1]")
        || o.starts_with("file://")
        || o.starts_with("qrc:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_no_auth_others_yes() {
        assert!(!path_requires_auth("/api/health"));
        assert!(path_requires_auth("/api/sessions"));
        assert!(path_requires_auth("/api/chat"));
        assert!(path_requires_auth("/api/config"));
        assert!(!path_requires_auth("/"));
    }

    #[test]
    fn bearer_validation() {
        use axum::http::{HeaderMap, HeaderValue};
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str("Bearer abc123").unwrap(),
        );
        assert!(valid_bearer(&h, "abc123"));
        assert!(!valid_bearer(&h, "wrong"));
        let empty = HeaderMap::new();
        assert!(!valid_bearer(&empty, "abc123"));
    }

    #[test]
    fn host_validation() {
        use axum::http::{HeaderMap, HeaderValue};
        let mut h = HeaderMap::new();
        assert!(valid_host(&h));
        h.insert(
            axum::http::header::HOST,
            HeaderValue::from_str("127.0.0.1:8765").unwrap(),
        );
        assert!(valid_host(&h));
        h.insert(
            axum::http::header::HOST,
            HeaderValue::from_str("evil.com").unwrap(),
        );
        assert!(!valid_host(&h));
    }

    #[test]
    fn origin_validation() {
        use axum::http::{HeaderMap, HeaderValue};
        let h = HeaderMap::new();
        assert!(valid_origin(&h));
        let mut evil = HeaderMap::new();
        evil.insert(
            axum::http::header::ORIGIN,
            HeaderValue::from_str("https://evil.com").unwrap(),
        );
        assert!(!valid_origin(&evil));
        let mut ok = HeaderMap::new();
        ok.insert(
            axum::http::header::ORIGIN,
            HeaderValue::from_str("null").unwrap(),
        );
        assert!(valid_origin(&ok));
    }
}
