//! Tool Executor - Ejecucion segura de herramientas del agente
//!
//! Reglas criticas (ver AGENTS.md):
//! - NUNCA usar sh -c con strings
//! - Siempre std::process::Command con array de argumentos
//! - Validar paths antes de escribir/leer
//! - Web search via DuckDuckGo Lite (sin API key)
//! - show_image descarga o copia a cache local

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;

use crate::models::{Config, ToolCall, ToolResult};

#[derive(Clone)]
pub struct ToolExecutor {
    config: std::sync::Arc<tokio::sync::RwLock<Config>>,
}

#[derive(Debug, Deserialize)]
struct EditArgs {
    path: String,
    mode: String,
    content: String,
}

impl ToolExecutor {
    pub fn new(config: std::sync::Arc<tokio::sync::RwLock<Config>>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult> {
        let id = tool_call.id.clone();
        let result = match tool_call.name.as_str() {
            "open_app" => self.open_app(tool_call).await,
            "create_file" => self.create_file(tool_call).await,
            "edit_file" => self.edit_file(tool_call).await,
            "read_file" => self.read_file(tool_call).await,
            "web_search" => self.web_search(tool_call).await,
            "show_image" => self.show_image(tool_call).await,
            other => Err(anyhow!("Herramienta desconocida: {other}")),
        };

        match result {
            Ok(mut r) => {
                r.tool_call_id = id;
                Ok(r)
            }
            Err(e) => Ok(ToolResult::error(id, e.to_string())),
        }
    }

    fn arg_string<'a>(args: &'a HashMap<String, serde_json::Value>, key: &str) -> Result<&'a str> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Argumento '{key}' requerido y debe ser string"))
    }

    fn get_config_snapshot(&self) -> Config {
        self.config
            .try_read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    /// Valida que un path este dentro de los paths permitidos.
    fn validate_path(&self, path: &str) -> Result<PathBuf> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.read_file && !cfg.tools.create_file && !cfg.tools.edit_file {
            bail!("Acceso al sistema de archivos deshabilitado en la configuracion");
        }

        let path = Path::new(path);
        let canonical = if path.exists() {
            std::fs::canonicalize(path).context("resolviendo path")?
        } else {
            // Para creacion, validar el directorio padre
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    std::fs::canonicalize(parent)?.join(path.file_name().unwrap_or_default())
                } else {
                    path.to_path_buf()
                }
            } else {
                path.to_path_buf()
            }
        };

        // Expandir ~ en allowed_paths
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        for allowed in &cfg.tools.allowed_paths {
            let allowed_expanded = if let Some(stripped) = allowed.strip_prefix("~/") {
                home.join(stripped)
            } else if allowed == "~" {
                home.clone()
            } else {
                PathBuf::from(allowed)
            };
            if canonical.starts_with(&allowed_expanded) {
                return Ok(canonical);
            }
        }

        bail!(format!(
            "Path no permitido: {} (debe estar dentro de allowed_paths)",
            canonical.display()
        ))
    }

    // === open_app ===
    async fn open_app(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.open_app {
            bail!("open_app deshabilitado en configuracion");
        }
        let name = Self::arg_string(&tc.arguments, "name")?;

        let resolved = resolve_app(name).ok_or_else(|| {
            anyhow!(
                "No se pudo resolver la aplicacion '{name}'. Prueba con: firefox, chrome, dolphin, konsole, code, spotify"
            )
        })?;

        // REGLA: nunca sh -c, siempre Command::new con args
        let (program, args) = resolved;
        log::info!("open_app: launching {} {:?}", program, args);
        Command::new(&program)
            .args(&args)
            .spawn()
            .with_context(|| format!("lanzando {program}"))?;

        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Aplicacion '{name}' abierta"),
        ))
    }

    // === create_file ===
    async fn create_file(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.create_file {
            bail!("create_file deshabilitado en configuracion");
        }
        let path = Self::arg_string(&tc.arguments, "path")?;
        let content = Self::arg_string(&tc.arguments, "content")?;

        let validated = self.validate_path(path)?;
        if validated.exists() {
            bail!(format!("El archivo ya existe: {}", validated.display()));
        }
        if let Some(parent) = validated.parent() {
            fs::create_dir_all(parent)
                .await
                .context("creando directorios padre")?;
        }
        fs::write(&validated, content)
            .await
            .context("escribiendo archivo")?;

        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Archivo creado: {}", validated.display()),
        ))
    }

    // === edit_file ===
    async fn edit_file(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.edit_file {
            bail!("edit_file deshabilitado en configuracion");
        }
        let raw = serde_json::to_string(&tc.arguments)?;
        let args: EditArgs = serde_json::from_str(&raw)?;

        let validated = self.validate_path(&args.path)?;
        if !validated.exists() {
            bail!(format!("El archivo no existe: {}", validated.display()));
        }

        let final_content = match args.mode.as_str() {
            "overwrite" => args.content,
            "append" => {
                let existing = fs::read_to_string(&validated).await.unwrap_or_default();
                format!("{existing}{}", args.content)
            }
            other => bail!(format!("Modo no soportado: {other}")),
        };

        fs::write(&validated, final_content).await?;
        Ok(ToolResult::success(
            tc.id.clone(),
            format!(
                "Archivo editado: {} (mode={})",
                validated.display(),
                args.mode
            ),
        ))
    }

    // === read_file ===
    async fn read_file(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.read_file {
            bail!("read_file deshabilitado en configuracion");
        }
        let path = Self::arg_string(&tc.arguments, "path")?;
        let validated = self.validate_path(path)?;
        let content = fs::read_to_string(&validated)
            .await
            .with_context(|| format!("leyendo {}", validated.display()))?;

        // Limitar a 32KB para no saturar el contexto
        let truncated = if content.len() > 32_000 {
            format!(
                "{}...\n[truncado, total {} bytes]",
                &content[..32_000],
                content.len()
            )
        } else {
            content
        };
        Ok(ToolResult::success(tc.id.clone(), truncated))
    }

    // === web_search (DuckDuckGo Lite) ===
    async fn web_search(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.web_search {
            bail!("web_search deshabilitado en configuracion");
        }
        let query = Self::arg_string(&tc.arguments, "query")?;

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 KDE-Assistant/2.0")
            .build()?;
        let url = format!(
            "https://lite.duckduckgo.com/lite/?q={}",
            urlencoding::encode(query)
        );

        let html = client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        // Extraccion best-effort de resultados (link-title-snippet triples)
        // DuckDuckGo muestra una pagina anti-bots si detecta uso automatizado.
        if is_ddg_challenge(&html) {
            return Ok(ToolResult::success(
                tc.id.clone(),
                format!(
                    "DuckDuckGo bloqueo la busqueda automatica (anti-bots) para: {query}. \
                     Intentalo de nuevo en unos minutos."
                ),
            ));
        }
        let results = parse_ddg_lite(&html, 5);

        if results.is_empty() {
            return Ok(ToolResult::success(
                tc.id.clone(),
                format!("Sin resultados para: {query}"),
            ));
        }

        let mut out = format!("Resultados para '{query}':\n\n");
        for (i, r) in results.iter().enumerate() {
            out.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                r.title,
                r.url,
                r.snippet
            ));
        }
        Ok(ToolResult::success(tc.id.clone(), out))
    }

    // === show_image ===
    async fn show_image(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.show_image {
            bail!("show_image deshabilitado en configuracion");
        }
        let source = Self::arg_string(&tc.arguments, "source")?;
        let caption = tc
            .arguments
            .get("caption")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let local_path = if source.starts_with("http://") || source.starts_with("https://") {
            // Descargar a cache
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow!("sin cache_dir"))?
                .join("kde-assistant/images");
            fs::create_dir_all(&cache_dir).await?;

            let ext = source
                .rsplit('.')
                .next()
                .and_then(|s| s.split('?').next())
                .unwrap_or("jpg");
            let ext = if ext.len() > 5 { "jpg" } else { ext };
            let file_name = format!("{}.{}", chrono::Utc::now().timestamp_millis(), ext);
            let dest = cache_dir.join(&file_name);

            let client = reqwest::Client::new();
            let bytes = client.get(source).send().await?.bytes().await?;
            fs::write(&dest, &bytes).await?;
            dest.to_string_lossy().to_string()
        } else if let Some(stripped) = source.strip_prefix("file://") {
            stripped.to_string()
        } else {
            // Path local
            let validated = self.validate_path(source)?;
            validated.to_string_lossy().to_string()
        };

        let msg = if caption.is_empty() {
            format!("Imagen: {local_path}")
        } else {
            format!("{caption} ({local_path})")
        };
        Ok(ToolResult::success(tc.id.clone(), msg).with_image(&local_path))
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigSnapshot;

// === Helpers ===

/// Resuelve un nombre de app a (programa, args[]) sin usar shell.
fn resolve_app(name: &str) -> Option<(String, Vec<String>)> {
    let name_lower = name.to_lowercase();

    // 1) Intentar como archivo .desktop por app id
    if let Some((cmd, args)) = try_desktop(&name_lower) {
        return Some((cmd, args));
    }

    // 2) Aliases comunes -> ejecutable directo
    let exe = match name_lower.as_str() {
        "firefox" | "navegador" | "browser" => "firefox",
        "chrome" | "chromium" | "google-chrome" => "google-chrome",
        "brave" => "brave",
        "dolphin" | "files" | "archivos" => "dolphin",
        "konsole" | "terminal" | "term" => "konsole",
        "kitty" => "kitty",
        "alacritty" => "alacritty",
        "code" | "vscode" | "vscodium" => "code",
        "spotify" => "spotify",
        "telegram" => "telegram-desktop",
        "discord" => "discord",
        "slack" => "slack",
        "obs" | "obs-studio" => "obs",
        "gimp" => "gimp",
        "inkscape" => "inkscape",
        "kdenlive" => "kdenlive",
        "steam" => "steam",
        "kate" | "editor" => "kate",
        "kcalc" | "calc" | "calculator" => "kcalc",
        "ark" => "ark",
        "gwenview" | "visor" => "gwenview",
        "okular" | "pdf" => "okular",
        "systemsettings" | "settings" | "config" | "configuracion" => "systemsettings",
        _ => return None,
    };

    // gtk-launch es seguro (no usa shell)
    Some(("gtk-launch".to_string(), vec![exe.to_string()]))
}

fn try_desktop(_app_id: &str) -> Option<(String, Vec<String>)> {
    // gtk-launch requiere que el .desktop exista en el sistema.
    // Sin chequeo del filesystem, devolvemos None para evitar falsos positivos.
    // En Fase 6 se comprobara via XDG_DATA_DIRS.
    None
}

struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn parse_ddg_lite(html: &str, max: usize) -> Vec<SearchResult> {
    // Parser tolerante al formato de DDG Lite (el orden de atributos y el
    // tipo de comillas han cambiado con el tiempo):
    //   <a rel="nofollow" href="...uddg=<url>&rut=..." class='result-link'>Titulo</a>
    //   ... <td class='result-snippet'>Extracto</td>
    let mut results = Vec::new();
    let mut pos = 0;
    while results.len() < max {
        let a_start = match html[pos..].find("<a ") {
            Some(p) => pos + p,
            None => break,
        };
        let tag_end = match html[a_start..].find('>') {
            Some(p) => a_start + p,
            None => break,
        };
        let tag = &html[a_start..=tag_end];
        if !tag.contains("result-link") {
            pos = tag_end + 1;
            continue;
        }
        let raw_url = extract_attr(tag, "href").unwrap_or_default();
        let url = decode_uddg(&raw_url);

        let after = tag_end + 1;
        let a_end = match html[after..].find("</a>") {
            Some(p) => after + p,
            None => break,
        };
        let title = strip_tags(&html[after..a_end])
            .trim()
            .trim_matches('"')
            .to_string();

        // Extracto: buscar result-snippet despues del enlace
        let snippet = match html[a_end..].find("result-snippet") {
            Some(p) => {
                let marker_end = a_end + p;
                match html[marker_end..].find('>') {
                    Some(q) => {
                        let start = marker_end + q + 1;
                        let end = html[start..]
                            .find("</td>")
                            .map(|r| start + r)
                            .unwrap_or(start);
                        strip_tags(&html[start..end]).trim().to_string()
                    }
                    None => String::new(),
                }
            }
            None => String::new(),
        };

        if !url.is_empty() && !title.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
        pos = a_end + 4;
    }
    results
}

/// Detecta la pagina anti-bots de DuckDuckGo.
fn is_ddg_challenge(html: &str) -> bool {
    html.contains("bots use DuckDuckGo") || html.contains("challenge to confirm")
}

/// Extrae el valor de un atributo HTML (acepta comillas simples o dobles).
fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let start = tag.find(&key)? + key.len();
    let rest = &tag[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end = rest[1..].find(quote)?;
    Some(rest[1..1 + end].to_string())
}

/// Si la URL es un redirect de DDG (/l/?uddg=<url>&...), devuelve la URL real.
fn decode_uddg(url: &str) -> String {
    let encoded = match url.find("uddg=") {
        Some(p) => {
            let rest = &url[p + 5..];
            match rest.find('&') {
                Some(q) => &rest[..q],
                None => rest,
            }
        }
        None => return url.to_string(),
    };
    percent_decode(encoded)
}

/// Decodifica secuencias %XX a nivel de bytes (preserva UTF-8).
/// Deja '+' intacto: en paths es literal.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // Las entidades HTML mas comunes en hrefs de DDG
    String::from_utf8_lossy(&out).replace("&amp;", "&")
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                other => format!("%{:02X}", other as u32),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Config;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn dummy_executor() -> ToolExecutor {
        let cfg = Arc::new(RwLock::new(Config::default()));
        ToolExecutor::new(cfg)
    }

    fn tc(name: &str, args: HashMap<String, serde_json::Value>) -> ToolCall {
        ToolCall {
            id: "t1".into(),
            name: name.into(),
            arguments: args,
        }
    }

    #[test]
    fn resolve_common_aliases() {
        assert!(resolve_app("firefox").is_some());
        assert!(resolve_app("dolphin").is_some());
        assert!(resolve_app("terminal").is_some());
        assert!(resolve_app("nonexistent_app_xyz").is_none());
    }

    #[test]
    fn parse_ddg_extracts_results() {
        let html = r#"
        <a rel="nofollow" class="result-link" href="https://example.com">Title 1</a>
        <td class="result-snippet">Snippet 1</td>
        <a rel="nofollow" class="result-link" href="https://example2.com">Title 2</a>
        <td class="result-snippet">Snippet 2</td>
        "#;
        let r = parse_ddg_lite(html, 5);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].url, "https://example.com");
        assert_eq!(r[0].title, "Title 1");
    }

    #[test]
    fn parse_ddg_new_format_single_quotes() {
        let html = r#"
        <tr><td><a rel="nofollow" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fes.wikipedia.org%2Fwiki%2FPar%C3%ADs&amp;rut=abc" class='result-link'>París - Wikipedia</a></td></tr>
        <tr><td class='result-snippet'>París es la capital de Francia</td></tr>
        "#;
        let r = parse_ddg_lite(html, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://es.wikipedia.org/wiki/París");
        assert_eq!(r[0].title, "París - Wikipedia");
        assert_eq!(r[0].snippet, "París es la capital de Francia");
    }

    #[test]
    fn detects_ddg_challenge() {
        assert!(is_ddg_challenge(
            "Unfortunately, bots use DuckDuckGo too. challenge to confirm"
        ));
        assert!(!is_ddg_challenge("<a class='result-link'>x</a>"));
    }

    #[test]
    fn decode_uddg_extracts_real_url() {
        let u = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fx&amp;rut=1";
        assert_eq!(decode_uddg(u), "https://example.com/x");
        assert_eq!(
            decode_uddg("https://example.com/plain"),
            "https://example.com/plain"
        );
    }

    #[test]
    fn strip_tags_removes_html() {
        assert_eq!(strip_tags("<b>hola</b> mundo"), "hola mundo");
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error_result() {
        let ex = dummy_executor();
        let res = ex.execute(&tc("no_existe", HashMap::new())).await.unwrap();
        assert!(!res.success);
        assert!(res.content.contains("desconocida"));
    }
}
