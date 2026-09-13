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

use crate::backend::session_manager::SessionManager;
use crate::models::{Config, ToolCall, ToolResult};

/// Contexto de una ejecución (F4-3): para la auditoría.
#[derive(Debug, Clone)]
pub struct ToolCtx {
    pub session_id: Option<String>,
    /// "auto" | "approved" | "denied".
    pub decided: String,
}

impl ToolCtx {
    pub fn auto(session_id: Option<String>) -> Self {
        Self {
            session_id,
            decided: "auto".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct ToolExecutor {
    config: std::sync::Arc<tokio::sync::RwLock<Config>>,
    sessions: Option<std::sync::Arc<std::sync::Mutex<SessionManager>>>,
}

#[derive(Debug, Deserialize)]
struct EditArgs {
    path: String,
    mode: String,
    content: String,
}

impl ToolExecutor {
    pub fn new(config: std::sync::Arc<tokio::sync::RwLock<Config>>) -> Self {
        Self {
            config,
            sessions: None,
        }
    }

    /// Constructor con auditoría persistente (F4-3).
    pub fn with_audit(
        config: std::sync::Arc<tokio::sync::RwLock<Config>>,
        sessions: std::sync::Arc<std::sync::Mutex<SessionManager>>,
    ) -> Self {
        Self {
            config,
            sessions: Some(sessions),
        }
    }

    pub async fn execute(&self, tool_call: &ToolCall, ctx: &ToolCtx) -> Result<ToolResult> {
        let id = tool_call.id.clone();
        let t0 = std::time::Instant::now();
        let result = match tool_call.name.as_str() {
            "open_app" => self.open_app(tool_call).await,
            "create_file" => self.create_file(tool_call).await,
            "edit_file" => self.edit_file(tool_call).await,
            "read_file" => self.read_file(tool_call).await,
            "web_search" => self.web_search(tool_call).await,
            "show_image" => self.show_image(tool_call).await,
            "find_file" => self.find_file(tool_call).await,
            "open_file" => self.open_file(tool_call).await,
            "open_url" => self.open_url(tool_call).await,
            "system_info" => self.system_info(tool_call).await,
            "notify" => self.notify(tool_call).await,
            "media" => self.media(tool_call).await,
            "volume" => self.volume(tool_call).await,
            "brightness" => self.brightness(tool_call).await,
            "network_status" => self.network_status(tool_call).await,
            "remind_in" => self.remind_in(tool_call).await,
            "kdeconnect" => self.kdeconnect(tool_call).await,
            other => Err(anyhow!("Herramienta desconocida: {other}")),
        };
        let ms = t0.elapsed().as_millis() as u64;

        let final_res = match result {
            Ok(mut r) => {
                r.tool_call_id = id;
                Ok(r)
            }
            Err(e) => Ok(ToolResult::error(id, e.to_string())),
        };

        // Auditoría best-effort (nunca rompe la ejecución).
        if let Ok(r) = &final_res {
            self.audit(tool_call, ctx, r.success, ms);
        }
        final_res
    }

    /// Registra una denegación del usuario (F4-1/F4-3): no se ejecutó nada.
    pub fn record_denial(&self, tool_call: &ToolCall, session_id: Option<String>) {
        let ctx = ToolCtx {
            session_id,
            decided: "denied".to_string(),
        };
        self.audit(tool_call, &ctx, false, 0);
    }

    fn audit(&self, tool_call: &ToolCall, ctx: &ToolCtx, success: bool, ms: u64) {
        let Some(sessions) = self.sessions.clone() else {
            return;
        };
        let args_json =
            serde_json::to_string(&tool_call.arguments).unwrap_or_else(|_| "{}".to_string());
        let permission = crate::backend::tool_registry::permission(&tool_call.name).as_str();
        let tool = tool_call.name.clone();
        let sid = ctx.session_id.clone();
        let decided = ctx.decided.clone();
        // Insert bloqueante breve (igual estilo que los handlers HTTP).
        let sessions = sessions.lock().unwrap_or_else(|e| e.into_inner());
        let _ = sessions.record_tool_audit(
            sid.as_deref(),
            &tool,
            &args_json,
            success,
            ms,
            permission,
            &decided,
        );
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
    /// Expande `~`, `$HOME`, canonicaliza y deniega symlinks que escapen.
    fn validate_path(&self, path: &str) -> Result<PathBuf> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.fs_enabled() {
            bail!("Acceso al sistema de archivos deshabilitado en la configuracion");
        }
        if path.trim().is_empty() {
            bail!("Path vacío no permitido");
        }

        // Expandir ~ y $HOME en el path pedido.
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let expanded_str = expand_home(path, &home);
        let path_buf = PathBuf::from(&expanded_str);

        // Rechazar NUL y paths absurdamente largos.
        if expanded_str.contains('\0') || expanded_str.len() > 4096 {
            bail!("Path inválido");
        }

        let canonical = if path_buf.exists() {
            std::fs::canonicalize(&path_buf).context("resolviendo path")?
        } else {
            // Para creación: canonicalizar el ancestro existente más cercano
            // y unir el resto sin permitir `..` que escape.
            let mut cur = path_buf.as_path();
            let mut suffix: Vec<std::ffi::OsString> = Vec::new();
            while !cur.exists() {
                match (cur.file_name(), cur.parent()) {
                    (Some(name), Some(parent)) => {
                        if name == ".." {
                            bail!("Path con '..' fuera de zona permitida");
                        }
                        suffix.push(name.to_os_string());
                        cur = parent;
                    }
                    _ => break,
                }
            }
            let base = if cur.as_os_str().is_empty() {
                PathBuf::from("/")
            } else if cur.exists() {
                std::fs::canonicalize(cur).context("resolviendo padre")?
            } else {
                PathBuf::from(cur)
            };
            let mut out = base;
            for comp in suffix.iter().rev() {
                out.push(comp);
            }
            out
        };

        // Expandir ~/$HOME en allowed_paths y canonicalizar lo que exista.
        for allowed in &cfg.tools.allowed_paths {
            let allowed_expanded_str = expand_home(allowed, &home);
            let allowed_path = PathBuf::from(&allowed_expanded_str);
            let allowed_canon = if allowed_path.exists() {
                std::fs::canonicalize(&allowed_path).unwrap_or(allowed_path)
            } else {
                allowed_path
            };
            if canonical.starts_with(&allowed_canon) {
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
                "No se pudo resolver la aplicacion '{name}'. Prueba con el nombre que aparece en el menú (ej: firefox, dolphin, konsole) o instala el .desktop en ~/.local/share/applications"
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

        // Backup antes de overwrite (permite deshacer desde el sistema).
        if args.mode == "overwrite" {
            let bak = validated.with_extension("bak");
            // No fallar si el backup falla; solo avisar.
            if let Err(e) = fs::copy(&validated, &bak).await {
                log::warn!("No se pudo crear backup de {}: {e}", validated.display());
            }
        }
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

        // Limitar a ~32k chars (no bytes) para no romper UTF-8 ni saturar contexto.
        const MAX_CHARS: usize = 32_000;
        let char_count = content.chars().count();
        let truncated = if char_count > MAX_CHARS {
            let head: String = content.chars().take(MAX_CHARS).collect();
            format!("{head}...\n[truncado, total {char_count} chars]")
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
            // Descargar a cache con límites (10MB, timeout 15s, solo imágenes).
            const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| anyhow!("sin cache_dir"))?
                .join("kde-assistant/images");
            fs::create_dir_all(&cache_dir).await?;

            let ext = sanitize_image_ext(source);
            let file_name = format!("{}.{}", chrono::Utc::now().timestamp_millis(), ext);
            let dest = cache_dir.join(&file_name);

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("KDE-Assistant/2.0")
                .build()?;
            let resp = client.get(source).send().await?.error_for_status()?;
            // Validar content-type si el servidor lo envía.
            if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
                let ct = ct.to_str().unwrap_or("").to_lowercase();
                if !ct.is_empty() && !ct.starts_with("image/") && !ct.contains("octet-stream") {
                    bail!("URL no es una imagen (content-type: {ct})");
                }
            }
            if let Some(len) = resp.content_length() {
                if len > MAX_IMAGE_BYTES as u64 {
                    bail!("Imagen demasiado grande ({} bytes, máx 10MB)", len);
                }
            }
            let bytes = resp.bytes().await?;
            if bytes.len() > MAX_IMAGE_BYTES {
                bail!("Imagen demasiado grande ({} bytes, máx 10MB)", bytes.len());
            }
            // Verificación mínima de firma (JPEG/PNG/GIF/WebP/BMP).
            if !looks_like_image(&bytes) {
                log::warn!("show_image: la descarga no parece imagen conocida, se guarda igual");
            }
            fs::write(&dest, &bytes).await?;
            dest.to_string_lossy().to_string()
        } else if let Some(stripped) = source.strip_prefix("file://") {
            // file:// también debe respetar allowed_paths (antes era bypass).
            if stripped.trim().is_empty() {
                bail!("file:// vacío no permitido");
            }
            let validated = self.validate_path(stripped)?;
            validated.to_string_lossy().to_string()
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

    // === find_file (F2-2) ===
    async fn find_file(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.find_file {
            bail!("find_file deshabilitado en configuracion");
        }
        let query = Self::arg_string(&tc.arguments, "query")?;
        if query.trim().is_empty() {
            bail!("Argumento 'query' vacío");
        }
        // Directorio base opcional (por defecto, todos los allowed_paths).
        let base: Option<PathBuf> = match tc.arguments.get("dir").and_then(|v| v.as_str()) {
            Some(d) if !d.trim().is_empty() => Some(self.validate_path(d)?),
            _ => None,
        };
        let roots: Vec<PathBuf> = match base {
            Some(b) => vec![b],
            None => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
                cfg.tools
                    .allowed_paths
                    .iter()
                    .map(|a| PathBuf::from(expand_home(a, &home)))
                    .filter(|p| p.exists())
                    .collect()
            }
        };
        if roots.is_empty() {
            bail!("No hay directorios permitidos para buscar");
        }
        let q = query.to_lowercase();
        // Walk bloqueante en spawn_blocking (I/O síncrono).
        let results = tokio::task::spawn_blocking(move || find_files_sync(&roots, &q))
            .await
            .context("búsqueda de archivos")??;
        if results.is_empty() {
            return Ok(ToolResult::success(
                tc.id.clone(),
                format!("Sin resultados para: {query}"),
            ));
        }
        let mut out = format!("Resultados para '{query}':\n");
        for r in &results {
            out.push_str(&format!("- {}\n", r.display()));
        }
        Ok(ToolResult::success(tc.id.clone(), out))
    }

    // === open_file (F2-2) ===
    async fn open_file(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.open_file {
            bail!("open_file deshabilitado en configuracion");
        }
        let path = Self::arg_string(&tc.arguments, "path")?;
        let validated = self.validate_path(path)?;
        if !validated.exists() {
            bail!(format!("No existe: {}", validated.display()));
        }
        let reveal = tc
            .arguments
            .get("reveal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        // REGLA: nunca sh -c, siempre Command con args.
        if reveal {
            // Mostrar en Dolphin (seleccionado); fallback a abrir el padre.
            match Command::new("dolphin")
                .arg("--select")
                .arg(&validated)
                .spawn()
            {
                Ok(_) => Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("Mostrando en Dolphin: {}", validated.display()),
                )),
                Err(_) => {
                    let parent = validated
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or(validated.clone());
                    Command::new("xdg-open")
                        .arg(&parent)
                        .spawn()
                        .with_context(|| "abriendo carpeta padre")?;
                    Ok(ToolResult::success(
                        tc.id.clone(),
                        format!(
                            "Dolphin no disponible; carpeta abierta: {}",
                            parent.display()
                        ),
                    ))
                }
            }
        } else {
            Command::new("xdg-open")
                .arg(&validated)
                .spawn()
                .with_context(|| format!("abriendo {}", validated.display()))?;
            Ok(ToolResult::success(
                tc.id.clone(),
                format!("Abierto: {}", validated.display()),
            ))
        }
    }

    // === open_url (F2-2) ===
    async fn open_url(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.open_url {
            bail!("open_url deshabilitado en configuracion");
        }
        let url = Self::arg_string(&tc.arguments, "url")?;
        let url = url.trim();
        // Solo http(s). Nada de file:/javascript:/data:.
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            bail!("Solo se permiten URLs http(s): {url}");
        }
        if url.contains([' ', '\n', '\r', '\t']) {
            bail!("URL inválida");
        }
        if url.len() > 2048 {
            bail!("URL demasiado larga");
        }
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .with_context(|| "abriendo URL en el navegador")?;
        Ok(ToolResult::success(
            tc.id.clone(),
            format!("URL abierta en el navegador: {url}"),
        ))
    }

    // === system_info (F2-3, solo lectura) ===
    async fn system_info(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.system_info {
            bail!("system_info deshabilitado en configuracion");
        }
        let info = tokio::task::spawn_blocking(collect_system_info)
            .await
            .context("info del sistema")?;
        Ok(ToolResult::success(tc.id.clone(), info))
    }

    // === notify (F2-3) ===
    async fn notify(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.notify {
            bail!("notify deshabilitado en configuracion");
        }
        let title = Self::arg_string(&tc.arguments, "title")?;
        let body = Self::arg_string(&tc.arguments, "body")?;
        let title: String = title.chars().take(100).collect();
        let body: String = body.chars().take(500).collect();
        if title.trim().is_empty() {
            bail!("Título vacío");
        }
        // REGLA: nunca sh -c, siempre Command con args.
        let status = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.Notifications",
                "--type=method_call",
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications.Notify",
                "string:KDE Assistant",
                "uint32:0",
                "string:kde-assistant",
                &format!("string:{title}"),
                &format!("string:{body}"),
                "string:",
                "array:string:",
                "dict:string:",
                "int32:-1",
            ])
            .status()
            .context("enviando notificación")?;
        if status.success() {
            Ok(ToolResult::success(
                tc.id.clone(),
                format!("Notificación enviada: {title}"),
            ))
        } else {
            bail!("El servicio de notificaciones no respondió")
        }
    }

    // === media (F4-2, playerctl) ===
    async fn media(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.media {
            bail!("media deshabilitado en configuracion");
        }
        let action = Self::arg_string(&tc.arguments, "action")?;
        let arg = match action {
            "play" | "pause" | "play-pause" | "next" | "previous" | "status" => action.to_string(),
            // Alias en español.
            "reproducir" => "play".to_string(),
            "pausar" => "pause".to_string(),
            "alternar" => "play-pause".to_string(),
            "siguiente" => "next".to_string(),
            "anterior" => "previous".to_string(),
            "estado" => "status".to_string(),
            other => {
                bail!("Acción no soportada: {other} (play|pause|play-pause|next|previous|status)")
            }
        };
        let out = run_cmd(&["playerctl", &arg], 10).await?;
        let text = out.trim();
        Ok(ToolResult::success(
            tc.id.clone(),
            if text.is_empty() {
                format!("Multimedia: {arg} ok")
            } else {
                format!("Multimedia ({arg}): {text}")
            },
        ))
    }

    // === volume (F4-2, wpctl con fallback pactl) ===
    async fn volume(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.volume {
            bail!("volume deshabilitado en configuracion");
        }
        let action = Self::arg_string(&tc.arguments, "action")?;
        match action {
            "get" | "estado" => {
                let out = match run_cmd(&["wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@"], 10).await
                {
                    Ok(o) => o,
                    Err(_) => run_cmd(&["pactl", "get-sink-volume", "@DEFAULT_SINK@"], 10).await?,
                };
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("Volumen: {}", out.trim()),
                ))
            }
            "set" => {
                let level = tc
                    .arguments
                    .get("level")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| anyhow!("Argumento 'level' (0.0-1.0) requerido para set"))?;
                if !(0.0..=1.0).contains(&level) {
                    bail!("level fuera de rango (0.0-1.0): {level}");
                }
                let pct = format!("{:.0}%", level * 100.0);
                let set_wp =
                    run_cmd(&["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", &pct], 10).await;
                if let Err(e) = set_wp {
                    let pactl_pct = format!("{:.0}%", level * 100.0);
                    run_cmd(
                        &["pactl", "set-sink-volume", "@DEFAULT_SINK@", &pactl_pct],
                        10,
                    )
                    .await
                    .with_context(|| format!("ni wpctl ni pactl disponibles ({e})"))?;
                }
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("Volumen al {pct}"),
                ))
            }
            "mute" | "silenciar" => {
                set_mute(true).await?;
                Ok(ToolResult::success(tc.id.clone(), "Audio silenciado"))
            }
            "unmute" | "activar" => {
                set_mute(false).await?;
                Ok(ToolResult::success(tc.id.clone(), "Audio activado"))
            }
            other => bail!("Acción no soportada: {other} (get|set|mute|unmute)"),
        }
    }

    // === brightness (F4-2, brightnessctl) ===
    async fn brightness(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.brightness {
            bail!("brightness deshabilitado en configuracion");
        }
        let action = Self::arg_string(&tc.arguments, "action")?;
        match action {
            "get" | "estado" => {
                let cur = run_cmd(&["brightnessctl", "get"], 10).await?;
                let max = run_cmd(&["brightnessctl", "max"], 10).await?;
                let (c, m): (f64, f64) = (
                    cur.trim().parse().unwrap_or(0.0),
                    max.trim().parse::<f64>().unwrap_or(1.0).max(1.0),
                );
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("Brillo: {:.0}%", c / m * 100.0),
                ))
            }
            "set" => {
                let level = tc
                    .arguments
                    .get("level")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| anyhow!("Argumento 'level' (1-100) requerido para set"))?;
                if !(1.0..=100.0).contains(&level) {
                    bail!("level fuera de rango (1-100): {level}");
                }
                run_cmd(&["brightnessctl", "set", &format!("{level:.0}%")], 10).await?;
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("Brillo al {level:.0}%"),
                ))
            }
            other => bail!("Acción no soportada: {other} (get|set)"),
        }
    }

    // === network_status (F4-2, solo lectura) ===
    async fn network_status(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.network_status {
            bail!("network_status deshabilitado en configuracion");
        }
        let mut lines = Vec::new();
        if let Ok(out) = run_cmd(&["nmcli", "-t", "-f", "STATE", "general"], 10).await {
            let state = out.trim().to_string();
            if !state.is_empty() {
                lines.push(format!("Red: {state}"));
            }
        }
        if let Ok(out) = run_cmd(
            &[
                "nmcli",
                "-t",
                "-f",
                "NAME,TYPE",
                "connection",
                "show",
                "--active",
            ],
            10,
        )
        .await
        {
            let conns: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
            if !conns.is_empty() {
                lines.push(format!("Conexiones: {}", conns.join(", ")));
            }
        }
        if let Ok(out) = run_cmd(&["bluetoothctl", "show"], 5).await {
            let powered = out
                .lines()
                .find(|l| l.trim().starts_with("Powered:"))
                .map(|l| l.trim().to_string())
                .unwrap_or_default();
            if !powered.is_empty() {
                lines.push(format!("Bluetooth: {powered}"));
            }
        }
        if lines.is_empty() {
            lines.push("Sin información de red (¿nmcli/bluetoothctl instalados?)".to_string());
        }
        Ok(ToolResult::success(tc.id.clone(), lines.join("\n")))
    }

    // === kdeconnect (F6) ===
    async fn kdeconnect(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.kdeconnect {
            bail!("kdeconnect deshabilitado en configuracion");
        }
        let action = Self::arg_string(&tc.arguments, "action")?;
        let device = tc
            .arguments
            .get("device")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        // REGLA: nunca sh -c, siempre Command con args (ver run_cmd).
        match action {
            "devices" | "dispositivos" => {
                let listed = run_cmd(&["kdeconnect-cli", "-l", "--id-only"], 10).await;
                let out = match listed {
                    Ok(o) => o,
                    Err(_) => run_cmd(&["kdeconnect-cli", "-l"], 10).await.map_err(|_| {
                        anyhow!(
                            "kdeconnect-cli no encontrado o sin respuesta (instala KDE Connect)"
                        )
                    })?,
                };
                let text = out.trim();
                Ok(ToolResult::success(
                    tc.id.clone(),
                    if text.is_empty() {
                        "Sin dispositivos KDE Connect visibles".to_string()
                    } else {
                        format!("Dispositivos KDE Connect:\n{text}")
                    },
                ))
            }
            "ping" | "ring" | "sonar" => {
                let dev = device.ok_or_else(|| anyhow!("Falta 'device' (id del móvil)"))?;
                let flag = if action == "ping" { "--ping" } else { "--ring" };
                run_cmd(&["kdeconnect-cli", "-d", dev, flag], 15)
                    .await
                    .map_err(|_| {
                        anyhow!("kdeconnect-cli no encontrado o el dispositivo no responde")
                    })?;
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("KDE Connect: {action} enviado a {dev}"),
                ))
            }
            "share_url" | "compartir_url" => {
                let dev = device.ok_or_else(|| anyhow!("Falta 'device' (id del móvil)"))?;
                let url = Self::arg_string(&tc.arguments, "url")?;
                let url = url.trim();
                if !(url.starts_with("https://") || url.starts_with("http://")) {
                    bail!("Solo URLs http(s)");
                }
                run_cmd(&["kdeconnect-cli", "-d", dev, "--share", url], 15).await?;
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("URL compartida con {dev}"),
                ))
            }
            "share_file" | "compartir_archivo" => {
                let dev = device.ok_or_else(|| anyhow!("Falta 'device' (id del móvil)"))?;
                let path = Self::arg_string(&tc.arguments, "path")?;
                let validated = self.validate_path(path)?;
                if !validated.exists() {
                    bail!(format!("No existe: {}", validated.display()));
                }
                run_cmd(
                    &[
                        "kdeconnect-cli",
                        "-d",
                        dev,
                        "--share",
                        &validated.to_string_lossy(),
                    ],
                    20,
                )
                .await?;
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("Archivo compartido con {dev}: {}", validated.display()),
                ))
            }
            "sms" => {
                let dev = device.ok_or_else(|| anyhow!("Falta 'device' (id del móvil)"))?;
                let number = Self::arg_string(&tc.arguments, "number")?;
                let number: String = number
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '+')
                    .collect();
                if number.chars().filter(|c| c.is_ascii_digit()).count() < 3 || number.len() > 20 {
                    bail!("Número inválido");
                }
                let text = Self::arg_string(&tc.arguments, "text")?;
                let text: String = text.chars().take(500).collect();
                if text.trim().is_empty() {
                    bail!("Texto vacío");
                }
                run_cmd(
                    &[
                        "kdeconnect-cli",
                        "-d",
                        dev,
                        "--destination",
                        &number,
                        "--send-sms",
                        &text,
                    ],
                    20,
                )
                .await?;
                Ok(ToolResult::success(
                    tc.id.clone(),
                    format!("SMS enviado vía {dev} al {number}"),
                ))
            }
            other => {
                bail!("Acción no soportada: {other} (devices|ping|ring|share_url|share_file|sms)")
            }
        }
    }

    // === remind_in (F4-2, persistente en F6) ===
    async fn remind_in(&self, tc: &ToolCall) -> Result<ToolResult> {
        let cfg = self.get_config_snapshot();
        if !cfg.tools.remind_in {
            bail!("remind_in deshabilitado en configuracion");
        }
        let minutes = tc
            .arguments
            .get("minutes")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow!("Argumento 'minutes' requerido"))?;
        if !(0.1..=1440.0).contains(&minutes) {
            bail!("minutes fuera de rango (0.1-1440): {minutes}");
        }
        let text = Self::arg_string(&tc.arguments, "text")?;
        let text: String = text.chars().take(500).collect();
        if text.trim().is_empty() {
            bail!("Texto vacío");
        }
        let fire_at = chrono::Utc::now() + chrono::Duration::seconds((minutes * 60.0) as i64);
        let fire_at_str = fire_at.to_rfc3339();
        // Persistir (sobrevive reinicios; el scheduler los dispara al arrancar).
        let reminder_id = self.sessions.as_ref().and_then(|s| {
            s.lock()
                .unwrap_or_else(|e| e.into_inner())
                .add_reminder(&fire_at_str, &text)
                .ok()
        });
        let text_for_task = text.clone();
        let sessions_for_task = self.sessions.clone();
        tokio::spawn(async move {
            let secs = (minutes * 60.0) as u64;
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            send_desktop_notification("Recordatorio", &text_for_task);
            if let (Some(sessions), Some(id)) = (sessions_for_task, reminder_id) {
                let _ = sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .mark_reminder_done(id);
            }
        });
        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Recordatorio en {minutes} min: {text} (persistente)"),
        ))
    }
}

/// Envía una notificación KDE (helper compartido, F6).
pub fn send_desktop_notification(title: &str, body: &str) {
    let _ = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.Notifications",
            "--type=method_call",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications.Notify",
            "string:KDE Assistant",
            "uint32:0",
            "string:kde-assistant",
            &format!("string:{title}"),
            &format!("string:{body}"),
            "string:",
            "array:string:",
            "dict:string:",
            "int32:-1",
        ])
        .status();
}

/// Dispara al arrancar los recordatorios pendientes (F6).
/// Los vencidos suenan con "(pendiente)"; los futuros se programan.
pub async fn fire_pending_reminders(
    sessions: std::sync::Arc<std::sync::Mutex<crate::backend::session_manager::SessionManager>>,
) {
    let pending = sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pending_reminders()
        .unwrap_or_default();
    if pending.is_empty() {
        return;
    }
    log::info!("Scheduler: {} recordatorio(s) pendiente(s)", pending.len());
    for r in pending {
        let now = chrono::Utc::now();
        let fire_at = chrono::DateTime::parse_from_rfc3339(&r.fire_at)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let sessions_c = sessions.clone();
        if fire_at <= now {
            send_desktop_notification("Recordatorio (pendiente)", &r.text);
            let _ = sessions_c
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .mark_reminder_done(r.id);
        } else {
            let wait = (fire_at - now).num_seconds().max(0) as u64;
            let text = r.text.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                send_desktop_notification("Recordatorio", &text);
                let _ = sessions_c
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .mark_reminder_done(r.id);
            });
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigSnapshot;

// === Helpers de comandos del sistema (F4-2, sin shell) ===

/// Ejecuta un binario con args (sin shell), con timeout. Retorna stdout.
async fn run_cmd(args: &[&str], timeout_secs: u64) -> Result<String> {
    let (bin, rest) = args.split_first().ok_or_else(|| anyhow!("sin comando"))?;
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        tokio::process::Command::new(bin)
            .args(rest)
            .stdin(std::process::Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| anyhow!("timeout ejecutando {bin}"))?
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("comando no encontrado: {bin}")
        } else {
            anyhow!("ejecutando {bin}: {e}")
        }
    })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("{bin} falló: {}", err.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn set_mute(mute: bool) -> Result<()> {
    let v = if mute { "1" } else { "0" };
    match run_cmd(&["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", v], 10).await {
        Ok(_) => Ok(()),
        Err(_) => {
            run_cmd(&["pactl", "set-sink-mute", "@DEFAULT_SINK@", v], 10).await?;
            Ok(())
        }
    }
}

// === Helpers ===

/// Información del sistema sin dependencias nuevas (lee /proc y /sys).
/// Todo best-effort: lo que falte se omite sin fallar.
fn collect_system_info() -> String {
    let mut lines = Vec::new();
    if let Some(os) = os_pretty_name() {
        lines.push(format!("Sistema: {os}"));
    }
    if let Some(k) = kernel_release() {
        lines.push(format!("Kernel: {k}"));
    }
    if let Some(up) = uptime_human() {
        lines.push(format!("Uptime: {up}"));
    }
    let (cpus, model) = cpu_summary();
    if cpus > 0 {
        lines.push(if model.is_empty() {
            format!("CPU: {cpus} hilos")
        } else {
            format!("CPU: {model} ({cpus} hilos)")
        });
    }
    if let Some((total, avail)) = mem_summary() {
        lines.push(format!(
            "RAM: {} libres de {}",
            human_bytes(avail),
            human_bytes(total)
        ));
    }
    if let Some(disk) = disk_summary() {
        lines.push(disk);
    }
    if let Some(bat) = battery_summary() {
        lines.push(bat);
    }
    if lines.is_empty() {
        return "Sin información del sistema disponible".to_string();
    }
    lines.join("\n")
}

fn read_first_line(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .next()
        .map(|s| s.to_string())
}

fn os_pretty_name() -> Option<String> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

fn kernel_release() -> Option<String> {
    // /proc/version: "Linux version 6.x..."; nos quedamos con el 3er token.
    let v = read_first_line("/proc/version")?;
    v.split_whitespace().nth(2).map(|s| s.to_string())
}

fn uptime_human() -> Option<String> {
    let v = read_first_line("/proc/uptime")?;
    let secs: u64 = v
        .split_whitespace()
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()?;
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    Some(if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    })
}

fn cpu_summary() -> (usize, String) {
    let content = match std::fs::read_to_string("/proc/cpuinfo") {
        Ok(c) => c,
        Err(_) => return (0, String::new()),
    };
    let mut count = 0usize;
    let mut model = String::new();
    for line in content.lines() {
        if line.starts_with("processor") {
            count += 1;
        } else if model.is_empty() && line.starts_with("model name") {
            model = line.split(':').nth(1).unwrap_or("").trim().to_string();
        }
    }
    (count, model)
}

fn mem_kb(key: &str) -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if line.starts_with(key) {
            // "MemTotal:       16384000 kB"
            let num: String = line.chars().filter(|c| c.is_ascii_digit()).collect();
            return num.parse().ok();
        }
    }
    None
}

fn mem_summary() -> Option<(u64, u64)> {
    let total_kb = mem_kb("MemTotal:")?;
    let avail_kb = mem_kb("MemAvailable:").or_else(|| mem_kb("MemFree:"))?;
    Some((total_kb * 1024, avail_kb * 1024))
}

fn human_bytes(b: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let f = b as f64;
    if f >= GB {
        format!("{:.1} GB", f / GB)
    } else {
        format!("{:.0} MB", f / MB)
    }
}

fn disk_summary() -> Option<String> {
    // df sin shell: binario + args.
    let out = std::process::Command::new("df")
        .args(["-h", "--output=avail,used,target", "/"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    lines.next()?; // cabecera
    let row = lines.next()?;
    let cols: Vec<&str> = row.split_whitespace().collect();
    if cols.len() < 2 {
        return None;
    }
    Some(format!("Disco /: {} libres (usados {})", cols[0], cols[1]))
}

fn battery_summary() -> Option<String> {
    let base = Path::new("/sys/class/power_supply");
    let entries = std::fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("BAT") {
            continue;
        }
        let cap_path = base.join(&name).join("capacity");
        let cap = std::fs::read_to_string(&cap_path).ok()?.trim().to_string();
        let status = std::fs::read_to_string(base.join(&name).join("status"))
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if cap.is_empty() {
            continue;
        }
        return Some(if status.is_empty() {
            format!("Batería {name}: {cap}%")
        } else {
            format!("Batería {name}: {cap}% ({status})")
        });
    }
    None
}

/// Búsqueda síncrona de archivos por subcadena (case-insensitive).
/// Límites: profundidad 6, 2000 entradas visitadas, 20 resultados.
/// Salta `.git`, `node_modules`, `target` y directorios ocultos.
fn find_files_sync(roots: &[PathBuf], query_lower: &str) -> Result<Vec<PathBuf>> {
    const MAX_DEPTH: usize = 6;
    const MAX_VISITED: usize = 2000;
    const MAX_RESULTS: usize = 20;
    const SKIP_DIRS: [&str; 3] = ["node_modules", "target", ".git"];

    let mut results = Vec::new();
    let mut visited = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = roots.iter().map(|r| (r.clone(), 0)).collect();

    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH || visited >= MAX_VISITED || results.len() >= MAX_RESULTS {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_VISITED || results.len() >= MAX_RESULTS {
                break;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push((path.clone(), depth + 1));
            }
            if name.contains(query_lower) {
                results.push(path);
            }
        }
    }
    results.sort();
    Ok(results)
}

/// Expande `~`, `~/` y `$HOME`/`~user` básico al home real.
fn expand_home(input: &str, home: &Path) -> String {
    if input == "~" {
        return home.to_string_lossy().to_string();
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return home.join(rest).to_string_lossy().to_string();
    }
    if let Some(rest) = input.strip_prefix("$HOME/") {
        return home.join(rest).to_string_lossy().to_string();
    }
    if input == "$HOME" {
        return home.to_string_lossy().to_string();
    }
    input.to_string()
}

/// Extensión segura para imágenes descargadas (allowlist).
fn sanitize_image_ext(url: &str) -> String {
    let raw = url
        .rsplit('.')
        .next()
        .and_then(|s| s.split('?').next())
        .and_then(|s| s.split('#').next())
        .unwrap_or("jpg")
        .to_lowercase();
    // Solo alfanumérico corto.
    let clean: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    match clean.as_str() {
        "jpg" | "jpeg" => "jpg".to_string(),
        "png" | "gif" | "webp" | "bmp" | "svg" => clean,
        _ => "jpg".to_string(),
    }
}

/// Heurística mínima de firma de imagen.
fn looks_like_image(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    // JPEG FF D8 FF, PNG 89 50 4E 47, GIF 47 49 46, WebP RIFF....WEBP, BMP 42 4D
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])
        || bytes.starts_with(&[0x47, 0x49, 0x46])
        || bytes.starts_with(&[0x42, 0x4D])
    {
        return true;
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return true;
    }
    // SVG textual
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
    if head.contains("<svg") {
        return true;
    }
    false
}

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

fn try_desktop(app_id: &str) -> Option<(String, Vec<String>)> {
    // F1-3: buscar en XDG_DATA_DIRS/applications (+ XDG_DATA_HOME).
    // Se puntúa por id/Name/Exec/Keywords y se lanza lo mejor si supera el umbral.
    let entries = scan_desktop_entries();
    if entries.is_empty() {
        return None;
    }
    let query = app_id.trim().to_lowercase();
    let mut best: Option<(&DesktopEntry, u32)> = None;
    for e in &entries {
        if let Some(score) = score_entry(&query, e) {
            if best.is_none_or(|(_, b)| score > b) {
                best = Some((e, score));
            }
        }
    }
    let (entry, score) = best?;
    if score < 30 {
        return None;
    }
    // Preferir gtk-launch con el id (respeta OnlyShowIn/DBusActivatable del .desktop).
    if !entry.id.is_empty() {
        return Some(("gtk-launch".to_string(), vec![entry.id.clone()]));
    }
    // Fallback: binario del Exec.
    let (bin, args) = split_exec(&entry.exec)?;
    Some((bin, args))
}

/// Entrada .desktop mínima para resolver apps.
#[derive(Debug, Clone)]
struct DesktopEntry {
    /// Id sin extensión (ej: "firefox").
    id: String,
    name: String,
    exec: String,
    keywords: String,
}

/// Directorios `applications` según XDG (+ fallbacks de la spec).
fn xdg_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(".local/share")
        });
    dirs.push(data_home.join("applications"));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for d in data_dirs.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    // Fallback clásico de Debian/Ubuntu.
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
}

/// Escanea todos los `*.desktop` (solo `[Desktop Entry]`, sin recursión profunda).
fn scan_desktop_entries() -> Vec<DesktopEntry> {
    scan_dirs(&xdg_app_dirs())
}

fn scan_dirs(dirs: &[PathBuf]) -> Vec<DesktopEntry> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() || !seen.insert(id.clone()) {
                continue;
            }
            if let Some(e) = parse_desktop_file(&path, &id) {
                out.push(e);
            }
        }
    }
    out
}

/// Parsea Name/Exec/Keywords/NoDisplay de un .desktop.
fn parse_desktop_file(path: &Path, id: &str) -> Option<DesktopEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut keywords = String::new();
    let mut generic = String::new();
    let mut hidden = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        // Ignorar claves localizadas (Name[es]=...) en favor de la base.
        match k {
            "Name" if name.is_empty() => name = v.trim().to_string(),
            "Exec" if exec.is_empty() => exec = v.trim().to_string(),
            "Keywords" if keywords.is_empty() => keywords = v.trim().to_string(),
            "GenericName" if generic.is_empty() => generic = v.trim().to_string(),
            "NoDisplay" => {
                if v.trim().eq_ignore_ascii_case("true") {
                    return None;
                }
            }
            "Hidden" if v.trim().eq_ignore_ascii_case("true") => {
                hidden = true;
            }
            _ => {}
        }
    }
    if hidden || name.is_empty() || exec.is_empty() {
        return None;
    }
    if generic.len() > keywords.len() {
        keywords = format!("{keywords} {generic}");
    }
    Some(DesktopEntry {
        id: id.to_string(),
        name,
        exec,
        keywords,
    })
}

/// Puntúa un entry contra la query (0-100). None = sin relación.
fn score_entry(query: &str, e: &DesktopEntry) -> Option<u32> {
    if query.is_empty() {
        return None;
    }
    let name_l = e.name.to_lowercase();
    let id_l = e.id.to_lowercase();
    let exec_bin = split_exec(&e.exec)
        .map(|(b, _)| b.to_lowercase())
        .unwrap_or_default();
    let exec_base = exec_bin.rsplit('/').next().unwrap_or(&exec_bin).to_string();
    let kw_l = e.keywords.to_lowercase();

    if query == id_l || query == format!("{id_l}.desktop") {
        return Some(100);
    }
    if query == name_l {
        return Some(90);
    }
    if query == exec_base {
        return Some(85);
    }
    if name_l.starts_with(query) || id_l.starts_with(query) || exec_base.starts_with(query) {
        return Some(70);
    }
    if name_l.contains(query) || id_l.contains(query) {
        return Some(55);
    }
    if kw_l.contains(query) {
        return Some(45);
    }
    // Todas las palabras contenidas en nombre+keywords.
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() > 1 {
        let hay = format!("{name_l} {kw_l} {id_l}");
        if words.iter().all(|w| hay.contains(w)) {
            return Some(35);
        }
    }
    None
}

/// Divide un Exec en (binario, args) quitando códigos de campo `%X`.
/// Retorna None si el binario está vacío o contiene metacaracteres de shell.
fn split_exec(exec: &str) -> Option<(String, Vec<String>)> {
    let mut parts: Vec<String> = Vec::new();
    for tok in exec.split_whitespace() {
        if tok.starts_with('%') {
            continue;
        }
        // Seguridad: rebotar Exec con shell (el .desktop es del sistema,
        // pero nunca está de más no ejecutar nada raro).
        if tok.contains([';', '&', '|', '`', '$', '\n']) {
            return None;
        }
        parts.push(tok.to_string());
    }
    if parts.is_empty() {
        return None;
    }
    let bin = parts.remove(0);
    Some((bin, parts))
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
    fn desktop_scan_scores_and_resolves() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("kda_desktop_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut f = std::fs::File::create(dir.join("falkon.desktop")).unwrap();
        writeln!(
            f,
            "[Desktop Entry]\nName=Falkon\nExec=falkon %u\nKeywords=Browser;Web;\n"
        )
        .unwrap();
        let entries = scan_dirs(std::slice::from_ref(&dir));
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(score_entry("falkon", e), Some(100));
        assert_eq!(score_entry("falk", e), Some(70));
        assert_eq!(score_entry("browser", e), Some(45));
        assert!(score_entry("zzz_no_match", e).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn desktop_hidden_or_nodisplay_skipped() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("kda_hidden_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut f = std::fs::File::create(dir.join("secret.desktop")).unwrap();
        writeln!(
            f,
            "[Desktop Entry]\nName=Secret\nExec=secret\nNoDisplay=true\n"
        )
        .unwrap();
        assert!(scan_dirs(std::slice::from_ref(&dir)).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn split_exec_strips_field_codes() {
        let (bin, args) = split_exec("firefox %u --private").unwrap();
        assert_eq!(bin, "firefox");
        assert_eq!(args, vec!["--private"]);
        assert!(split_exec("evil; rm -rf /").is_none());
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

    #[test]
    fn expand_home_variants() {
        let home = PathBuf::from("/home/test");
        assert_eq!(expand_home("~", &home), "/home/test");
        assert_eq!(expand_home("~/Doc", &home), "/home/test/Doc");
        assert_eq!(expand_home("$HOME/Doc", &home), "/home/test/Doc");
        assert_eq!(expand_home("/tmp/x", &home), "/tmp/x");
    }

    #[test]
    fn sanitize_ext_allowlist() {
        assert_eq!(sanitize_image_ext("https://x/y.png?z=1"), "png");
        assert_eq!(sanitize_image_ext("https://x/y.EXE"), "jpg");
        assert_eq!(sanitize_image_ext("https://x/y"), "jpg");
        assert_eq!(sanitize_image_ext("https://x/photo.webp"), "webp");
    }

    #[test]
    fn image_signature_detection() {
        assert!(looks_like_image(&[0xFF, 0xD8, 0xFF, 0x00]));
        assert!(looks_like_image(&[0x89, 0x50, 0x4E, 0x47]));
        assert!(!looks_like_image(b"hola mundo"));
        assert!(looks_like_image(b"<svg xmlns='x'></svg>"));
    }

    #[test]
    fn find_files_sync_respects_limits_and_skips() {
        let base = std::env::temp_dir().join(format!("kda_find_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::create_dir_all(base.join(".git")).unwrap();
        std::fs::create_dir_all(base.join("node_modules")).unwrap();
        std::fs::write(base.join("notas_importantes.md"), "x").unwrap();
        std::fs::write(base.join("sub").join("otras_notas.txt"), "x").unwrap();
        std::fs::write(base.join(".git").join("notas_secret.md"), "x").unwrap();
        std::fs::write(base.join("node_modules").join("notas_dep.md"), "x").unwrap();
        let r = find_files_sync(std::slice::from_ref(&base), "notas").unwrap();
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|p| p.starts_with(&base)));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn open_url_rejects_non_http() {
        let ex = dummy_executor();
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/plain,hola",
            "",
        ] {
            let mut args = HashMap::new();
            args.insert(
                "url".to_string(),
                serde_json::Value::String(bad.to_string()),
            );
            let res = ex
                .execute(&tc("open_url", args), &ToolCtx::auto(None))
                .await
                .unwrap();
            assert!(!res.success, "debería rechazar {bad}");
        }
    }

    #[tokio::test]
    async fn find_file_empty_query_errors() {
        let ex = dummy_executor();
        let mut args = HashMap::new();
        args.insert(
            "query".to_string(),
            serde_json::Value::String("  ".to_string()),
        );
        let res = ex
            .execute(&tc("find_file", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
    }

    #[test]
    fn system_info_returns_lines() {
        let info = collect_system_info();
        assert!(!info.is_empty());
        // En Linux siempre hay kernel/uptime; en otros SO al menos no panica.
        assert!(info.len() < 2000);
    }

    #[test]
    fn human_bytes_formats() {
        assert_eq!(human_bytes(512 * 1024 * 1024), "512 MB");
        assert!(human_bytes(2 * 1024 * 1024 * 1024).contains("GB"));
    }

    fn media_tc(action: &str) -> ToolCall {
        let mut args = HashMap::new();
        args.insert(
            "action".to_string(),
            serde_json::Value::String(action.to_string()),
        );
        tc("media", args)
    }

    #[tokio::test]
    async fn media_rejects_bad_action_without_spawning() {
        let ex = dummy_executor();
        let res = ex
            .execute(&media_tc("explotar"), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.content.contains("no soportada"));
    }

    #[tokio::test]
    async fn volume_validates_level() {
        let ex = dummy_executor();
        let mut args = HashMap::new();
        args.insert(
            "action".to_string(),
            serde_json::Value::String("set".to_string()),
        );
        args.insert("level".to_string(), serde_json::Value::from(9.9));
        let res = ex
            .execute(&tc("volume", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.content.contains("rango"));
    }

    #[tokio::test]
    async fn remind_validates_range() {
        let ex = dummy_executor();
        let mut args = HashMap::new();
        args.insert("minutes".to_string(), serde_json::Value::from(99999.0));
        args.insert(
            "text".to_string(),
            serde_json::Value::String("hola".to_string()),
        );
        let res = ex
            .execute(&tc("remind_in", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
    }

    fn kde_tc(action: &str) -> ToolCall {
        let mut args = HashMap::new();
        args.insert(
            "action".to_string(),
            serde_json::Value::String(action.to_string()),
        );
        tc("kdeconnect", args)
    }

    #[tokio::test]
    async fn kdeconnect_rejects_bad_action() {
        let ex = dummy_executor();
        let res = ex
            .execute(&kde_tc("hackear"), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
    }

    #[tokio::test]
    async fn kdeconnect_needs_device() {
        let ex = dummy_executor();
        for action in ["ping", "ring", "sms"] {
            let res = ex
                .execute(&kde_tc(action), &ToolCtx::auto(None))
                .await
                .unwrap();
            assert!(!res.success, "debería pedir device en {action}");
        }
    }

    #[tokio::test]
    async fn kdeconnect_validates_sms() {
        let ex = dummy_executor();
        let mut args = HashMap::new();
        args.insert(
            "action".to_string(),
            serde_json::Value::String("sms".to_string()),
        );
        args.insert(
            "device".to_string(),
            serde_json::Value::String("abc123".to_string()),
        );
        args.insert(
            "number".to_string(),
            serde_json::Value::String("xx".to_string()),
        );
        args.insert(
            "text".to_string(),
            serde_json::Value::String("hola".to_string()),
        );
        let res = ex
            .execute(&tc("kdeconnect", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error_result() {
        let ex = dummy_executor();
        let res = ex
            .execute(&tc("no_existe", HashMap::new()), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.content.contains("desconocida"));
    }
}
