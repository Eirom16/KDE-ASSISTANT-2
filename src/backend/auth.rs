//! Auth local - Token bearer para el servidor HTTP 127.0.0.1
//!
//! El backend expone `/api/*` sin auth por defecto: cualquier proceso local
//! (incluida una web con DNS-rebinding) podría leer sesiones y ejecutar tools.
//! F0-3 lo cierra con un token aleatorio:
//! - `~/.config/kde-assistant/server.token` (0600, 64 hex chars)
//! - `Authorization: Bearer <token>` en todo `/api/*` excepto `/api/health`
//! - Check de `Host` (solo 127.0.0.1/localhost) y `Origin` (solo vacío/null/local)
//!
//! La UI QML recibe el token vía módulo generado `qml.auth`
//! (ver `write_qml_auth_module`); el env `KDE_ASSISTANT_TOKEN` queda como
//! respaldo para sesiones donde sí existe `Qt.platform.environment`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

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

/// Escribe el módulo QML `qml.auth` con el token, el cacheDir y el flag
/// de overlay del personaje.
///
/// Por qué: `Qt.platform.environment` no existe en muchas sesiones qml6
/// (devuelve null → cualquier env leída desde QML cae al fallback SIEMPRE).
/// Es la misma causa del personaje duplicado: el flag `KDE_ASSISTANT_AGENT_OVERLAY`
/// nunca llegaba al QML y la ventana standalone se mostraba pese al overlay.
/// En vez de leer env en QML, el backend genera este módulo en
/// `~/.cache/kde-assistant/qml/` y main.rs lo añade con `qml6 -I <dir>`
/// ANTES que `-I .`. El QML lo importa como `import qml.auth 1.0` →
/// `AuthToken.token`, `AuthToken.cacheDir`, `AuthToken.agentOverlay`.
/// En repo hay un fallback vacío (`qml/auth/`) para dev/valida_qml.
///
/// Retorna el directorio a pasar con `-I`.
pub fn write_qml_auth_module(token: &str, agent_overlay: bool) -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("sin cache_dir"))?
        .join("kde-assistant/qml-generated");
    write_qml_auth_module_to(token, agent_overlay, &dir)?;
    Ok(dir)
}

fn write_qml_auth_module_to(token: &str, agent_overlay: bool, dir: &Path) -> Result<()> {
    use std::fmt::Write as _;
    // Módulo `qml.auth`: <dir>/qml/auth/{qmldir,AuthToken.qml}.
    let mod_dir = dir.join("qml/auth");
    std::fs::create_dir_all(&mod_dir).context("creando dir qml auth")?;
    // Escapar el token para QML (solo hex esperado, pero sin sorpresas).
    let mut lit = String::with_capacity(token.len() + 2);
    lit.push('"');
    for c in token.chars() {
        match c {
            '"' => lit.push_str("\\\""),
            '\\' => lit.push_str("\\\\"),
            '\n' => lit.push_str("\\n"),
            c => lit.push(c),
        }
    }
    lit.push('"');
    let cache_dir = dir
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut cache_lit = String::from("\"file://");
    for c in cache_dir.chars() {
        match c {
            '"' => cache_lit.push_str("\\\""),
            '\\' => cache_lit.push_str("\\\\"),
            c => cache_lit.push(c),
        }
    }
    let _ = write!(cache_lit, "/\"");
    std::fs::write(
        mod_dir.join("qmldir"),
        "module qml.auth\nsingleton AuthToken 1.0 AuthToken.qml\n",
    )
    .context("qmldir auth")?;
    let qml = format!(
        "// Generado por kde-assistant al arrancar. No editar.\n\
         pragma Singleton\n\
         import QtQuick\n\
         QtObject {{\n\
         \x20   readonly property string token: {lit}\n\
         \x20   readonly property string cacheDir: {cache_lit}\n\
         \x20   readonly property bool agentOverlay: {agent_overlay}\n\
         }}\n"
    );
    std::fs::write(mod_dir.join("AuthToken.qml"), qml).context("AuthToken.qml")?;
    Ok(())
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

    #[test]
    fn qml_module_writes_token_and_cache() {
        let dir = std::env::temp_dir().join(format!("kda_qmlauth_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_qml_auth_module_to("abc123\"\\", true, &dir).unwrap();
        let mod_dir = dir.join("qml/auth");
        let qmldir = std::fs::read_to_string(mod_dir.join("qmldir")).unwrap();
        assert!(qmldir.contains("module qml.auth"));
        let qml = std::fs::read_to_string(mod_dir.join("AuthToken.qml")).unwrap();
        // Token escapado para QML y cacheDir con esquema file://.
        assert!(qml.contains("abc123\\\"\\\\"));
        assert!(qml.contains("pragma Singleton"));
        assert!(qml.contains("file://"));
        assert!(qml.contains("readonly property string token"));
        assert!(qml.contains("readonly property string cacheDir"));
        assert!(qml.contains("readonly property bool agentOverlay: true"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn qml_module_bakes_overlay_off() {
        let dir = std::env::temp_dir().join(format!("kda_qmlauth_off_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_qml_auth_module_to("t", false, &dir).unwrap();
        let qml = std::fs::read_to_string(dir.join("qml/auth/AuthToken.qml")).unwrap();
        assert!(qml.contains("readonly property bool agentOverlay: false"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
