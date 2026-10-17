//! Tool Executor - Ejecucion segura de herramientas del agente
//!
//! Reglas criticas (ver AGENTS.md):
//! - NUNCA usar sh -c con strings
//! - Siempre std::process::Command con array de argumentos
//! - Validar paths antes de escribir/leer
//! - Los adaptadores legacy de red/proceso siguen presentes para fixtures,
//!   pero F2 los bloquea en cualquier dispatcher productivo.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::backend::document_suite;
use crate::backend::safe_fs::SafePath;
use crate::backend::session_manager::SessionManager;
use crate::models::{Config, ToolCall, ToolResult};

/// Contexto de una ejecución (F4-3): para la auditoría.
#[derive(Debug, Clone)]
pub struct ToolCtx {
    pub session_id: Option<String>,
    /// Dato heredado del caller; nunca se usa como autorización F2.
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
    quota: std::sync::Arc<tokio::sync::Semaphore>,
    memory: std::sync::Arc<std::sync::Mutex<ToolMemory>>,
    #[cfg(test)]
    pub(crate) create_gate: Option<std::sync::Arc<TestCreateGate>>,
}

/// Test-only barrier after SEC-01 authorization, before real private-file I/O.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestCreateGate {
    pub started: tokio::sync::Notify,
    pub release: tokio::sync::Notify,
}

#[derive(Debug, Default)]
struct ToolMemory {
    last_opened_app: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EditArgs {
    path: String,
    mode: String,
    content: String,
}

impl ToolExecutor {
    /// Synthetic regressions only. Not compiled into the default library.
    #[cfg(any(test, feature = "legacy-fixtures"))]
    pub fn legacy_fixture(config: std::sync::Arc<tokio::sync::RwLock<Config>>) -> Self {
        Self {
            config,
            sessions: None,
            quota: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
            memory: std::sync::Arc::new(std::sync::Mutex::new(ToolMemory::default())),
            #[cfg(test)]
            create_gate: None,
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
            quota: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
            memory: std::sync::Arc::new(std::sync::Mutex::new(ToolMemory::default())),
            #[cfg(test)]
            create_gate: None,
        }
    }

    pub fn permission_plan(&self, call: &ToolCall) -> Result<super::permission_engine::Plan> {
        super::permission_engine::plan(call, &self.get_config_snapshot()?)
    }
    pub fn prepare_for_turn(&self, turn_id: &str, call: &ToolCall) -> Result<String> {
        self.prepare_for_turn_with_consent(turn_id, call, "chat")
            .map(|(operation, _)| operation)
    }
    pub fn prepare_for_turn_with_consent(
        &self,
        turn_id: &str,
        call: &ToolCall,
        origin: &str,
    ) -> Result<(String, Option<String>)> {
        let plan = self.permission_plan(call)?;
        let sessions = self
            .sessions
            .as_ref()
            .context("journal storage unavailable")?;
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare_permission(turn_id, call, &plan, origin)
    }
    pub fn journal_state(
        &self,
        operation: &str,
        state: super::operation_journal::OperationState,
        consent: &str,
        bytes: Option<i64>,
    ) -> Result<()> {
        let sessions = self
            .sessions
            .as_ref()
            .context("journal storage unavailable")?;
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .transition_operation(operation, state, consent, bytes)
    }
    pub fn deny_permission(&self, operation: &str) -> Result<()> {
        self.sessions
            .as_ref()
            .context("journal storage unavailable")?
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .deny_permission(operation)
    }
    pub fn record_policy_denial_for_turn(&self, turn_id: &str, call: &ToolCall) -> Result<()> {
        let manager = self
            .sessions
            .as_ref()
            .context("journal storage unavailable")?;
        let manager = manager.lock().unwrap_or_else(|e| e.into_inner());
        let operation = manager.prepare_operation(turn_id, &call.name)?;
        manager.transition_operation(
            &operation,
            super::operation_journal::OperationState::Denied,
            "denied",
            None,
        )
    }
    pub async fn execute_for_turn(
        &self,
        call: &ToolCall,
        _ctx: &ToolCtx,
        turn: &std::sync::Arc<super::conversation_turn::Turn>,
        operation: &str,
    ) -> Result<ToolResult> {
        let plan = self.permission_plan(call)?;
        let consent = if plan.risk == super::permission_engine::Risk::Green {
            "auto"
        } else {
            "approved"
        };
        let _permit = super::conversation_turn::cancellable(Some(turn), async {
            Ok(tokio::time::timeout(
                std::time::Duration::from_secs(10),
                self.quota.clone().acquire_owned(),
            )
            .await
            .context("tool_quota_wait_timeout")??)
        })
        .await?;
        turn.dispatch_recorded(
            || {
                self.sessions
                    .as_ref()
                    .context("journal storage unavailable")?
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .dispatch_permission(
                        operation,
                        &turn.id,
                        turn.session_id.as_deref(),
                        call,
                        &plan.fingerprint,
                    )
                    .map(|_| ())
            },
            self.execute_and_record(call, consent, operation, &plan),
        )
        .await
    }
    async fn execute_and_record(
        &self,
        call: &ToolCall,
        consent: &str,
        operation: &str,
        plan: &super::permission_engine::Plan,
    ) -> Result<ToolResult> {
        let started = std::time::Instant::now();
        let result = self.execute_raw_authorized(call, plan).await?;
        let sessions = self
            .sessions
            .as_ref()
            .context("journal storage unavailable")?;
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record_operation_result(
                operation,
                consent,
                &result,
                started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            )
            .context("operation_result_not_persisted: external outcome requires inspection")?;
        Ok(result)
    }
    pub async fn execute(&self, tool_call: &ToolCall, ctx: &ToolCtx) -> Result<ToolResult> {
        let Some(sessions) = &self.sessions else {
            #[cfg(any(test, feature = "legacy-fixtures"))]
            return self.execute_raw(tool_call).await;
            #[cfg(not(any(test, feature = "legacy-fixtures")))]
            bail!("journal storage unavailable");
        };
        let plan = match self.permission_plan(tool_call) {
            Ok(plan) => plan,
            Err(error) => {
                if super::tool_registry::all_tools()
                    .iter()
                    .any(|tool| tool.function.name == tool_call.name)
                {
                    self.record_denial(tool_call, ctx.session_id.clone())?;
                }
                return Err(error);
            }
        };
        let _permit = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.quota.clone().acquire_owned(),
        )
        .await
        .context("tool_quota_wait_timeout")??;
        let turn_id = uuid::Uuid::new_v4().to_string();
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .begin_turn(&turn_id, ctx.session_id.as_deref(), None)?;
        let (operation, _) = self.prepare_for_turn_with_consent(&turn_id, tool_call, "manual")?;
        if plan.risk != super::permission_engine::Risk::Green {
            let manager = sessions.lock().unwrap_or_else(|e| e.into_inner());
            manager.deny_permission(&operation)?;
            manager.settle_turn(&turn_id, "completed")?;
            anyhow::bail!("consent_required: manual dispatch cannot grant itself approval");
        }
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dispatch_permission(
                &operation,
                &turn_id,
                ctx.session_id.as_deref(),
                tool_call,
                &plan.fingerprint,
            )?;
        let result = self
            .execute_and_record(tool_call, "auto", &operation, &plan)
            .await;
        sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .settle_turn(
                &turn_id,
                if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                },
            )?;
        result
    }
    async fn execute_raw(&self, tool_call: &ToolCall) -> Result<ToolResult> {
        let id = tool_call.id.clone();
        let result = match self.get_config_snapshot() {
            Ok(cfg) => self.execute_with_config(tool_call, &cfg).await,
            Err(e) => Err(e),
        };

        match result {
            Ok(mut r) => {
                r.tool_call_id = id;
                Ok(r)
            }
            Err(e) => Ok(ToolResult::error(id, e.to_string())),
        }
    }

    async fn execute_raw_authorized(
        &self,
        call: &ToolCall,
        plan: &super::permission_engine::Plan,
    ) -> Result<ToolResult> {
        if call.name == "system_info" {
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(2), self.execute_raw(call))
                    .await;
            return Ok(match result {
                Ok(Ok(mut value)) => {
                    value.content = value.content.chars().take(32_000).collect();
                    value
                }
                Ok(Err(error)) => ToolResult::error(call.id.clone(), error.to_string()),
                Err(_) => ToolResult::error(call.id.clone(), "system_info_timeout"),
            });
        }
        let cfg = self.get_config_snapshot()?;
        let result: Result<ToolResult> = async {
            match call.name.as_str() {
                "read_file" => {
                    anyhow::ensure!(cfg.tools.read_file, "read_file disabled");
                    let path = Self::arg_string(&call.arguments, "path")?;
                    let safe = SafePath::resolve(&cfg, path)?;
                    let identity = plan.identity.clone();
                    let bytes =
                        tokio::task::spawn_blocking(move || safe.read_bound(&identity)).await??;
                    let content = String::from_utf8(bytes).context("archivo no es UTF-8 válido")?;
                    let clipped: String = content.chars().take(32_000).collect();
                    Ok(ToolResult::success(call.id.clone(), clipped))
                }
                "create_file" => {
                    anyhow::ensure!(cfg.tools.create_file, "create_file disabled");
                    let path = Self::arg_string(&call.arguments, "path")?;
                    let content = Self::arg_string(&call.arguments, "content")?
                        .as_bytes()
                        .to_vec();
                    let safe = SafePath::resolve(&cfg, path)?;
                    let display = safe.display.clone();
                    let identity = plan.identity.clone();
                    #[cfg(test)]
                    if let Some(gate) = &self.create_gate {
                        gate.started.notify_one();
                        gate.release.notified().await;
                    }
                    tokio::task::spawn_blocking(move || safe.create_bound(&identity, &content))
                        .await??;
                    Ok(ToolResult::success(
                        call.id.clone(),
                        format!("Archivo creado: {}", display.display()),
                    ))
                }
                "edit_file" => {
                    anyhow::ensure!(cfg.tools.edit_file, "edit_file disabled");
                    let path = Self::arg_string(&call.arguments, "path")?;
                    let mode = Self::arg_string(&call.arguments, "mode")?.to_owned();
                    let content = Self::arg_string(&call.arguments, "content")?.to_owned();
                    let safe = SafePath::resolve(&cfg, path)?;
                    let display = safe.display.clone();
                    let identity = plan.identity.clone();
                    let mode_for_worker = mode.clone();
                    tokio::task::spawn_blocking(move || {
                        safe.edit_bound(&identity, &mode_for_worker, &content)
                    })
                    .await??;
                    Ok(ToolResult::success(
                        call.id.clone(),
                        format!("Archivo editado: {} (mode={mode})", display.display()),
                    ))
                }
                _ => anyhow::bail!("tool_disabled_by_f2_policy"),
            }
        }
        .await;
        Ok(result.unwrap_or_else(|error| {
            let uncertain = error
                .downcast_ref::<super::safe_fs::UncertainEdit>()
                .is_some();
            let result = ToolResult::error(call.id.clone(), error.to_string());
            if uncertain {
                result.uncertain()
            } else {
                result
            }
        }))
    }

    // Una sola versión autoriza el flag y todas las rutas de la operación.
    async fn execute_with_config(&self, tool_call: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        match tool_call.name.as_str() {
            "open_app" => self.open_app(tool_call, cfg).await,
            "create_file" => self.create_file(tool_call, cfg).await,
            "edit_file" => self.edit_file(tool_call, cfg).await,
            "read_file" => self.read_file(tool_call, cfg).await,
            "web_search" => self.web_search(tool_call, cfg).await,
            "show_image" => self.show_image(tool_call, cfg).await,
            "find_file" => self.find_file(tool_call, cfg).await,
            "find_document" => self.find_document(tool_call, cfg).await,
            "preview_document" => self.preview_document(tool_call, cfg).await,
            "open_file" => self.open_file(tool_call, cfg).await,
            "copy_file" => self.copy_file(tool_call, cfg).await,
            "open_url" => self.open_url(tool_call, cfg).await,
            "list_open_apps" => self.list_open_apps(tool_call, cfg).await,
            "focus_app" => self.focus_app(tool_call, cfg).await,
            "close_app" => self.close_app(tool_call, cfg).await,
            "system_info" => self.system_info(tool_call, cfg).await,
            "notify" => self.notify(tool_call, cfg).await,
            "media" => self.media(tool_call, cfg).await,
            "volume" => self.volume(tool_call, cfg).await,
            "brightness" => self.brightness(tool_call, cfg).await,
            "network_status" => self.network_status(tool_call, cfg).await,
            "remind_in" => self.remind_in(tool_call, cfg).await,
            "kdeconnect" => self.kdeconnect(tool_call, cfg).await,
            other => Err(anyhow!("Herramienta desconocida: {other}")),
        }
    }

    /// Registra una denegación del usuario (F4-1/F4-3): no se ejecutó nada.
    pub fn record_denial(&self, tool_call: &ToolCall, session_id: Option<String>) -> Result<()> {
        let Some(sessions) = &self.sessions else {
            return Ok(());
        };
        let turn_id = uuid::Uuid::new_v4().to_string();
        let sessions = sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.begin_turn(&turn_id, session_id.as_deref(), None)?;
        let operation = sessions.prepare_operation(&turn_id, &tool_call.name)?;
        sessions.transition_operation(
            &operation,
            super::operation_journal::OperationState::Denied,
            "denied",
            None,
        )?;
        sessions.settle_turn(&turn_id, "completed")
    }

    fn arg_string<'a>(args: &'a HashMap<String, serde_json::Value>, key: &str) -> Result<&'a str> {
        args.get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Argumento '{key}' requerido y debe ser string"))
    }

    fn opt_arg_string<'a>(
        args: &'a HashMap<String, serde_json::Value>,
        key: &str,
    ) -> Option<&'a str> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    fn get_config_snapshot(&self) -> Result<Config> {
        self.config
            .try_read()
            .map(|c| c.clone())
            .context("configuracion no disponible; operacion denegada")
    }

    // === open_app ===
    async fn open_app(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
        self.remember_opened_app(name);

        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Aplicacion '{name}' abierta"),
        ))
    }

    // === create_file ===
    async fn create_file(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.create_file {
            bail!("create_file deshabilitado en configuracion");
        }
        let path = Self::arg_string(&tc.arguments, "path")?;
        let content = Self::arg_string(&tc.arguments, "content")?;

        let validated = SafePath::resolve(cfg, path)?;
        let display = validated.display.clone();
        let content = content.as_bytes().to_vec();
        #[cfg(test)]
        if let Some(gate) = &self.create_gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        tokio::task::spawn_blocking(move || validated.create(&content)).await??;

        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Archivo creado: {}", display.display()),
        ))
    }

    // === edit_file ===
    async fn edit_file(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.edit_file {
            bail!("edit_file deshabilitado en configuracion");
        }
        let raw = serde_json::to_string(&tc.arguments)?;
        let args: EditArgs = serde_json::from_str(&raw)?;

        let validated = SafePath::resolve(cfg, &args.path)?;
        let display = validated.display.clone();
        let mode = args.mode.clone();
        tokio::task::spawn_blocking(move || validated.edit(&args.mode, &args.content)).await??;
        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Archivo editado: {} (mode={})", display.display(), mode),
        ))
    }

    // === read_file ===
    async fn read_file(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.read_file {
            bail!("read_file deshabilitado en configuracion");
        }
        let path = Self::arg_string(&tc.arguments, "path")?;
        let validated = SafePath::resolve(cfg, path)?;
        let bytes = tokio::task::spawn_blocking(move || validated.read()).await??;
        let content = String::from_utf8(bytes).context("archivo no es UTF-8 válido")?;

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
    async fn web_search(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.web_search {
            bail!("web_search deshabilitado en configuracion");
        }
        let query = Self::arg_string(&tc.arguments, "query")?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
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
    async fn show_image(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
            let cache_dir = crate::backend::safe_fs::private_workspace()?.0;
            let pending = crate::backend::resource_cache::PendingResource::new(&cache_dir);

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
            let mut stream = resp.bytes_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                anyhow::ensure!(
                    bytes.len().saturating_add(chunk.len()) <= MAX_IMAGE_BYTES,
                    "Imagen demasiado grande (máx 10MB)"
                );
                bytes.extend_from_slice(&chunk);
            }
            // Verificación mínima de firma (JPEG/PNG/GIF/WebP/BMP).
            if !looks_like_image(&bytes) {
                log::warn!("show_image: la descarga no parece imagen conocida, se guarda igual");
            }
            let write_path = dest.clone();
            tokio::task::spawn_blocking(move || {
                crate::backend::resource_cache::write_resource(&write_path, &bytes)
            })
            .await??;
            crate::backend::resource_cache::publish_resource(&dest)?;
            pending.keep();
            dest.to_string_lossy().to_string()
        } else {
            let path = source.strip_prefix("file://").unwrap_or(source);
            let validated = SafePath::resolve(cfg, path)?;
            tokio::task::spawn_blocking(move || validated.snapshot())
                .await??
                .to_string_lossy()
                .to_string()
        };

        let msg = if caption.is_empty() {
            format!("Imagen: {local_path}")
        } else {
            format!("{caption} ({local_path})")
        };
        Ok(ToolResult::success(tc.id.clone(), msg).with_image(&local_path))
    }

    // === find_file (F2-2) ===
    async fn find_file(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.find_file {
            bail!("find_file deshabilitado en configuracion");
        }
        let query = Self::arg_string(&tc.arguments, "query")?;
        if query.trim().is_empty() {
            bail!("Argumento 'query' vacío");
        }
        let results = search_authorized(
            cfg,
            Self::opt_arg_string(&tc.arguments, "dir"),
            query,
            false,
        )
        .await?;
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

    // === find_document ===
    async fn find_document(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.find_document {
            bail!("find_document deshabilitado en configuracion");
        }
        let query = Self::arg_string(&tc.arguments, "query")?;
        if query.trim().is_empty() {
            bail!("Argumento 'query' vacío");
        }
        let results =
            search_authorized(cfg, Self::opt_arg_string(&tc.arguments, "dir"), query, true).await?;
        if results.is_empty() {
            return Ok(ToolResult::success(
                tc.id.clone(),
                format!("Sin documentos para: {query}"),
            ));
        }
        let mut out = format!("Documentos para '{query}':\n");
        for r in &results {
            out.push_str(&format!("- {}\n", r.display()));
        }
        Ok(ToolResult::success(tc.id.clone(), out))
    }

    // === preview_document ===
    async fn preview_document(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.preview_document {
            bail!("preview_document deshabilitado en configuracion");
        }
        let path = Self::arg_string(&tc.arguments, "path")?;
        let validated = SafePath::resolve(cfg, path)?;
        let display = validated.display.clone();
        let page = tc
            .arguments
            .get("page")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .clamp(1, 999) as u32;
        let preview_path = tokio::task::spawn_blocking(move || {
            let snapshot = validated.snapshot()?;
            let result = preview_document_sync(&snapshot, page);
            // Image inputs are already a private display resource, not a render source.
            if !result.as_ref().is_ok_and(|preview| preview == &snapshot) {
                if let Err(error) =
                    crate::backend::resource_cache::release_private_snapshot(&snapshot)
                {
                    log::warn!("Private snapshot cleanup pending: {error}");
                }
            }
            let preview = result?;
            crate::backend::resource_cache::check_resource(&preview)?;
            Ok::<_, anyhow::Error>(preview)
        })
        .await??;
        Ok(ToolResult::success(
            tc.id.clone(),
            format!(
                "Vista previa de {}{}",
                display.display(),
                if page > 1 {
                    format!(" (página {page})")
                } else {
                    String::new()
                }
            ),
        )
        .with_image(preview_path.to_string_lossy()))
    }

    // === open_file (F2-2) ===
    async fn open_file(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.open_file {
            bail!("open_file deshabilitado en configuracion");
        }
        let _ = tc;
        bail!("open_file bloqueado hasta F2: delegación por pathname sin capacidad segura")
    }

    // === copy_file ===
    async fn copy_file(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.copy_file {
            bail!("copy_file deshabilitado en configuracion");
        }
        let source = Self::arg_string(&tc.arguments, "source")?;
        let destination = Self::arg_string(&tc.arguments, "destination")?;
        let overwrite = tc
            .arguments
            .get("overwrite")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let src = SafePath::resolve(cfg, source)?;
        let dst = SafePath::resolve(cfg, destination)?;
        let display = src.display.clone();
        let destination =
            tokio::task::spawn_blocking(move || src.copy_to(dst, overwrite)).await??;
        Ok(ToolResult::success(
            tc.id.clone(),
            format!(
                "Copiado: {} -> {}",
                display.display(),
                destination.display()
            ),
        ))
    }

    // === open_url (F2-2) ===
    async fn open_url(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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

    // === list_open_apps ===
    async fn list_open_apps(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.list_open_apps {
            bail!("list_open_apps deshabilitado en configuracion");
        }
        let windows = tokio::task::spawn_blocking(list_windows_sync)
            .await
            .context("listando ventanas")??;
        if windows.is_empty() {
            return Ok(ToolResult::success(
                tc.id.clone(),
                "No pude detectar ventanas abiertas. En Wayland instala kdotool o usa una sesión X11 para control de ventanas.",
            ));
        }
        let mut out = String::from("Ventanas abiertas:\n");
        for w in windows.iter().take(30) {
            out.push_str(&format!(
                "- {}{}{}{}\n",
                w.title,
                if w.class_name.is_empty() { "" } else { " · " },
                w.class_name,
                w.pid.map(|pid| format!(" · pid {pid}")).unwrap_or_default()
            ));
        }
        Ok(ToolResult::success(tc.id.clone(), out))
    }

    // === focus_app ===
    async fn focus_app(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.focus_app {
            bail!("focus_app deshabilitado en configuracion");
        }
        let name = Self::arg_string(&tc.arguments, "name")?;
        let query = name.to_string();
        let focused = tokio::task::spawn_blocking(move || focus_window_sync(&query))
            .await
            .context("enfocando ventana")??;
        Ok(ToolResult::success(
            tc.id.clone(),
            format!("Ventana enfocada: {}", focused.title),
        ))
    }

    // === close_app ===
    async fn close_app(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.close_app {
            bail!("close_app deshabilitado en configuracion");
        }
        let explicit_name = Self::opt_arg_string(&tc.arguments, "name").map(str::to_string);
        let remembered_name = if explicit_name.is_none() {
            self.last_opened_app()
        } else {
            None
        };
        let target_name = explicit_name.clone().or_else(|| remembered_name.clone());
        let allow_active_fallback = explicit_name.is_none();
        let closed = tokio::task::spawn_blocking(move || {
            if let Some(name) = target_name.as_deref() {
                match close_window_or_app_sync(Some(name)) {
                    Ok(msg) => {
                        if remembered_name.is_some() {
                            Ok(format!("{msg} (última app abierta por Jarvis: {name})"))
                        } else {
                            Ok(msg)
                        }
                    }
                    Err(err) if allow_active_fallback => {
                        log::warn!(
                            "No se pudo cerrar la última app recordada '{name}': {err}; intentando ventana activa"
                        );
                        close_window_or_app_sync(None)
                    }
                    Err(err) => Err(err),
                }
            } else {
                close_window_or_app_sync(None)
            }
        })
        .await
        .context("cerrando app")??;
        if explicit_name.is_none() {
            self.clear_last_opened_app();
        }
        Ok(ToolResult::success(tc.id.clone(), closed))
    }

    // === system_info (F2-3, solo lectura) ===
    async fn system_info(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
        if !cfg.tools.system_info {
            bail!("system_info deshabilitado en configuracion");
        }
        let info = tokio::task::spawn_blocking(collect_system_info)
            .await
            .context("info del sistema")?;
        Ok(ToolResult::success(tc.id.clone(), info))
    }

    // === notify (F2-3) ===
    async fn notify(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
    async fn media(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
    async fn volume(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
    async fn brightness(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
    async fn network_status(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
    async fn kdeconnect(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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
                bail!("kdeconnect share_file bloqueado hasta F2: delegación por pathname")
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
    async fn remind_in(&self, tc: &ToolCall, cfg: &Config) -> Result<ToolResult> {
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

    fn remember_opened_app(&self, name: &str) {
        if let Ok(mut memory) = self.memory.lock() {
            memory.last_opened_app = Some(name.trim().to_string());
        }
    }

    fn last_opened_app(&self) -> Option<String> {
        self.memory
            .lock()
            .ok()
            .and_then(|memory| memory.last_opened_app.clone())
    }

    fn clear_last_opened_app(&self) {
        if let Ok(mut memory) = self.memory.lock() {
            memory.last_opened_app = None;
        }
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

#[derive(Debug, Clone)]
struct WindowInfo {
    id: String,
    title: String,
    class_name: String,
    pid: Option<u32>,
}

fn list_windows_sync() -> Result<Vec<WindowInfo>> {
    if !command_exists("xdotool") {
        bail!("No hay backend de ventanas disponible. Instala kdotool para KDE Wayland o wmctrl/xdotool para X11.");
    }
    let ids = cmd_output(&["xdotool", "search", "--onlyvisible", "--name", "."])?;
    let mut windows = Vec::new();
    for id in ids
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(80)
    {
        let title = cmd_output(&["xdotool", "getwindowname", id]).unwrap_or_default();
        let class_name = cmd_output(&["xdotool", "getwindowclassname", id]).unwrap_or_default();
        let pid = cmd_output(&["xdotool", "getwindowpid", id])
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        let title = title.trim().to_string();
        let class_name = class_name.trim().to_string();
        if title.is_empty() && class_name.is_empty() {
            continue;
        }
        windows.push(WindowInfo {
            id: id.to_string(),
            title,
            class_name,
            pid,
        });
    }
    Ok(windows)
}

fn focus_window_sync(query: &str) -> Result<WindowInfo> {
    let window = find_window_by_query(query)?;
    cmd_status(&["xdotool", "windowactivate", &window.id])?;
    Ok(window)
}

fn close_window_or_app_sync(name: Option<&str>) -> Result<String> {
    if let Some(name) = name {
        if command_exists("xdotool") {
            if let Ok(window) = find_window_by_query(name) {
                cmd_status(&["xdotool", "windowclose", &window.id])?;
                return Ok(format!("Ventana cerrada: {}", window.title));
            }
        }
        if terminate_exact_process(name)? {
            return Ok(format!("Proceso cerrado con SIGTERM: {name}"));
        }
        bail!("No encontré una ventana o proceso exacto para cerrar: {name}");
    }

    if !command_exists("xdotool") {
        bail!("Cerrar la ventana activa requiere xdotool/kdotool");
    }
    let id = cmd_output(&["xdotool", "getactivewindow"])?;
    let id = id.trim();
    if id.is_empty() {
        bail!("No hay ventana activa detectable");
    }
    let title = cmd_output(&["xdotool", "getwindowname", id]).unwrap_or_default();
    cmd_status(&["xdotool", "windowclose", id])?;
    Ok(format!(
        "Ventana activa cerrada{}",
        if title.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", title.trim())
        }
    ))
}

fn find_window_by_query(query: &str) -> Result<WindowInfo> {
    let q = query.to_lowercase();
    let windows = list_windows_sync()?;
    windows
        .into_iter()
        .find(|w| w.title.to_lowercase().contains(&q) || w.class_name.to_lowercase().contains(&q))
        .ok_or_else(|| anyhow!("No encontré ventana abierta para: {query}"))
}

fn terminate_exact_process(_name: &str) -> Result<bool> {
    bail!("process_termination_disabled: no verifiable ownership; third-party process consent contract required")
}

#[cfg(test)]
fn process_candidates(name: &str) -> Vec<String> {
    let lower = name.trim().to_lowercase();
    let mut out = vec![lower.clone()];
    match lower.as_str() {
        "dolphin" | "archivos" | "files" => out.push("dolphin".into()),
        "administrador de tareas"
        | "gestor de recursos"
        | "monitor del sistema"
        | "system monitor"
        | "plasma system monitor"
        | "plasma-systemmonitor"
        | "org.kde.plasma-systemmonitor" => out.extend([
            "plasma-systemmonitor".into(),
            "org.kde.plasma-systemmonitor".into(),
            "ksysguard".into(),
        ]),
        "brave" => out.extend([
            "brave".into(),
            "brave-browser".into(),
            "brave-origin".into(),
        ]),
        "firefox" | "navegador" | "browser" => out.push("firefox".into()),
        "chrome" | "chromium" | "google-chrome" => {
            out.extend(["chrome".into(), "chromium".into(), "google-chrome".into()])
        }
        "konsole" | "terminal" | "term" => out.push("konsole".into()),
        "kate" | "editor" => out.push("kate".into()),
        "okular" | "pdf" => out.push("okular".into()),
        _ => {}
    }
    if let Some((_program, args)) = try_desktop(&lower) {
        if let Some(first) = args.first() {
            out.push(first.clone());
        }
    }
    if let Some(entry) = scan_desktop_entries()
        .into_iter()
        .find(|e| score_entry(&lower, e).unwrap_or(0) >= 55)
    {
        if let Some((bin, _)) = split_exec(&entry.exec) {
            out.push(
                Path::new(&bin)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&bin)
                    .to_string(),
            );
        }
    }
    out
}

fn command_exists(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|p| p.join(bin).is_file())
}

fn cmd_output(args: &[&str]) -> Result<String> {
    let (bin, rest) = args.split_first().ok_or_else(|| anyhow!("sin comando"))?;
    let out = Command::new(bin)
        .args(rest)
        .stdin(std::process::Stdio::null())
        .output()
        .with_context(|| format!("ejecutando {bin}"))?;
    if !out.status.success() {
        bail!(
            "{bin} falló: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn cmd_status(args: &[&str]) -> Result<()> {
    let (bin, rest) = args.split_first().ok_or_else(|| anyhow!("sin comando"))?;
    let status = Command::new(bin)
        .args(rest)
        .stdin(std::process::Stdio::null())
        .status()
        .with_context(|| format!("ejecutando {bin}"))?;
    if !status.success() {
        bail!("{bin} falló con estado {status}");
    }
    Ok(())
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

async fn search_authorized(
    cfg: &Config,
    base: Option<&str>,
    query: &str,
    documents: bool,
) -> Result<Vec<PathBuf>> {
    let paths = match base {
        Some(path) => vec![SafePath::resolve(cfg, path)?],
        None => {
            let mut paths = Vec::new();
            for path in &cfg.tools.allowed_paths {
                if let Some(root) = SafePath::search_root(cfg, path)? {
                    paths.push(root);
                }
            }
            paths
        }
    };
    if paths.is_empty() {
        bail!("No hay ninguna raíz utilizable para buscar en allowed_paths");
    }
    let query = query.to_lowercase();
    tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        for path in paths {
            found.extend(path.find(&query)?);
        }
        if documents {
            found.retain(|path| is_document_like(path));
        }
        found.sort();
        found.dedup();
        found.truncate(20);
        Ok(found)
    })
    .await?
}

/// Búsqueda síncrona de archivos por subcadena (case-insensitive).
/// Límites: profundidad 6, 2000 entradas visitadas, 20 resultados.
/// Salta `.git`, `node_modules`, `target` y directorios ocultos.
#[cfg(test)]
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

#[cfg(test)]
fn find_documents_sync(roots: &[PathBuf], query_lower: &str) -> Result<Vec<PathBuf>> {
    let docs = find_files_sync(roots, query_lower)?;
    Ok(docs
        .into_iter()
        .filter(|p| is_document_like(p))
        .take(20)
        .collect())
}

fn is_document_like(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "gif"
            | "bmp"
            | "svg"
            | "txt"
            | "md"
            | "markdown"
            | "log"
            | "csv"
            | "docx"
            | "odt"
            | "ods"
            | "xlsx"
    )
}

fn preview_document_sync(path: &Path, page: u32) -> Result<PathBuf> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg"
    ) {
        return Ok(path.to_path_buf());
    }
    let cache_dir = writable_preview_cache_dir()?;
    let pending = crate::backend::resource_cache::PendingResource::new(&cache_dir);
    let clean_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("documento")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(40)
        .collect::<String>();
    let stem = format!("{}_{}", chrono::Utc::now().timestamp_millis(), clean_stem);
    let preview = match ext.as_str() {
        "pdf" => render_pdf_preview(path, page, &cache_dir, &stem),
        "txt" | "md" | "markdown" | "log" | "csv" => render_text_preview(path, &cache_dir, &stem),
        "docx" | "odt" | "ods" | "xlsx" => {
            render_office_placeholder_preview(path, &cache_dir, &stem)
        }
        other => bail!("Tipo de documento no soportado para preview: {other}"),
    }?;
    crate::backend::resource_cache::publish_resource(&preview)?;
    pending.keep();
    Ok(preview)
}

fn render_pdf_preview(path: &Path, page: u32, cache_dir: &Path, stem: &str) -> Result<PathBuf> {
    let out = cache_dir.join(format!("{stem}.png"));
    let output = crate::backend::resource_cache::create_resource(&out)?;
    let status = Command::new("pdftoppm")
        .args(["-f", &page.to_string(), "-singlefile", "-png", "-r", "144"])
        .arg(path)
        .stdout(output)
        .status()
        .context("ejecutando pdftoppm")?;
    if !status.success() {
        bail!("pdftoppm no pudo renderizar {}", path.display());
    }
    if !out.exists() {
        bail!("pdftoppm no generó la imagen esperada");
    }
    Ok(out)
}

fn writable_preview_cache_dir() -> Result<PathBuf> {
    Ok(crate::backend::safe_fs::private_workspace()?.0)
}

fn render_text_preview(path: &Path, cache_dir: &Path, stem: &str) -> Result<PathBuf> {
    const MAX_LINES: usize = 42;
    const MAX_COLS: usize = 92;
    let content =
        std::fs::read_to_string(path).with_context(|| format!("leyendo {}", path.display()))?;
    let lines = content
        .lines()
        .take(MAX_LINES)
        .map(|l| {
            let mut s = l.chars().take(MAX_COLS).collect::<String>();
            if l.chars().count() > MAX_COLS {
                s.push('…');
            }
            s
        })
        .collect::<Vec<_>>();
    let width = 1000;
    let line_h = 22;
    let height = 96 + (lines.len().max(1) as i32 * line_h);
    let title = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("documento");
    let mut body = String::new();
    for (i, line) in lines.iter().enumerate() {
        body.push_str(&format!(
            r#"<text x="32" y="{}" class="line">{}</text>"#,
            74 + (i as i32 * line_h),
            xml_escape(line)
        ));
    }
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<style>
.bg{{fill:#1d1d1f}} .panel{{fill:#2a2a2c;stroke:#3a3a3c;stroke-width:1}}
.title{{fill:#ffffff;font:600 22px Inter,Noto Sans,sans-serif}}
.line{{fill:#f5f5f7;font:15px monospace;white-space:pre}}
</style>
<rect class="bg" width="100%" height="100%" rx="18"/>
<rect class="panel" x="16" y="16" width="968" height="{panel_h}" rx="14"/>
<text x="32" y="48" class="title">{title}</text>
{body}
</svg>"##,
        panel_h = height - 32,
        title = xml_escape(title)
    );
    let out = cache_dir.join(format!("{stem}.svg"));
    crate::backend::resource_cache::write_resource(&out, svg.as_bytes())?;
    Ok(out)
}

fn render_office_placeholder_preview(path: &Path, cache_dir: &Path, stem: &str) -> Result<PathBuf> {
    let meta = std::fs::metadata(path).with_context(|| format!("leyendo {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("documento")
        .to_uppercase();
    let preview = match document_suite::extract_preview(path) {
        Ok(preview) => preview,
        Err(err) => document_suite::DocumentPreview {
            title: path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("documento")
                .to_string(),
            kind: ext.clone(),
            lines: vec!["No se pudo extraer texto de este documento todavía.".to_string()],
            note: format!("Preview interna disponible como tarjeta. Detalle: {err}"),
        },
    };
    let size = human_size(meta.len());
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "desconocido".to_string())
        })
        .unwrap_or_else(|| "desconocido".to_string());
    let content_lines = if preview.lines.is_empty() {
        vec!["Sin texto extraíble en esta vista previa.".to_string()]
    } else {
        preview.lines.clone()
    };
    let line_h = 24;
    let content_y = 314;
    let content_h = (content_lines.len().min(18) as i32 * line_h).max(line_h);
    let height = 440 + content_h;
    let mut body = String::new();
    for (i, line) in content_lines.iter().take(18).enumerate() {
        body.push_str(&format!(
            r#"<text x="72" y="{}" class="docline">{}</text>"#,
            content_y + (i as i32 * line_h),
            xml_escape(line)
        ));
    }
    let width = 1000;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<style>
.bg{{fill:#1d1d1f}} .card{{fill:#2a2a2c;stroke:#3a3a3c;stroke-width:1}}
.accent{{fill:#2997ff}} .title{{fill:#ffffff;font:700 28px Inter,Noto Sans,sans-serif}}
.label{{fill:#8e8e93;font:600 15px Inter,Noto Sans,sans-serif;letter-spacing:.8px}}
.value{{fill:#f5f5f7;font:18px Inter,Noto Sans,sans-serif}}
.hint{{fill:#c7c7cc;font:16px Inter,Noto Sans,sans-serif}}
.badge{{fill:#0a84ff;font:800 30px Inter,Noto Sans,sans-serif}}
.docpanel{{fill:#1f1f21;stroke:#3a3a3c;stroke-width:1}}
.docline{{fill:#f5f5f7;font:15px Inter,Noto Sans,sans-serif;white-space:pre}}
</style>
<rect class="bg" width="100%" height="100%" rx="28"/>
<rect class="card" x="28" y="28" width="944" height="{card_h}" rx="24"/>
<rect class="accent" x="64" y="74" width="132" height="164" rx="22"/>
<text x="130" y="168" text-anchor="middle" class="badge">{kind}</text>
<text x="232" y="102" class="label">DOCUMENTO</text>
<text x="232" y="146" class="title">{title}</text>
<text x="232" y="214" class="label">TAMAÑO</text>
<text x="232" y="246" class="value">{size}</text>
<text x="232" y="306" class="label">MODIFICADO</text>
<text x="232" y="338" class="value">{modified}</text>
<text x="64" y="292" class="label">CONTENIDO</text>
<rect class="docpanel" x="56" y="304" width="888" height="{doc_h}" rx="16"/>
{body}
<text x="64" y="{hint_y}" class="hint">{note}</text>
<text x="64" y="{hint2_y}" class="hint">Para editar o ver el documento completo, pide “ábrelo” y KDE usará tu office predeterminado.</text>
</svg>"##,
        card_h = height - 56,
        doc_h = content_h + 28,
        hint_y = height - 96,
        hint2_y = height - 64,
        title = xml_escape(&preview.title),
        kind = xml_escape(&preview.kind),
        size = xml_escape(&size),
        modified = xml_escape(&modified),
        note = xml_escape(&preview.note),
        body = body
    );
    let out = cache_dir.join(format!("{stem}.svg"));
    crate::backend::resource_cache::write_resource(&out, svg.as_bytes())?;
    Ok(out)
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Expande `~`, `~/` y `$HOME`/`~user` básico al home real.
pub(super) fn expand_home(input: &str, home: &Path) -> String {
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

    if matches!(
        name_lower.as_str(),
        "administrador de tareas"
            | "gestor de recursos"
            | "monitor del sistema"
            | "system monitor"
            | "plasma system monitor"
            | "plasma-systemmonitor"
            | "org.kde.plasma-systemmonitor"
    ) {
        return Some(("plasma-systemmonitor".to_string(), Vec::new()));
    }

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
        ToolExecutor::legacy_fixture(cfg)
    }

    fn tc(name: &str, args: HashMap<String, serde_json::Value>) -> ToolCall {
        ToolCall {
            id: "t1".into(),
            name: name.into(),
            arguments: args,
        }
    }

    #[tokio::test]
    async fn f1a_snapshot_stays_coherent_after_config_update() {
        use std::os::unix::fs::DirBuilderExt;
        let root = std::env::temp_dir().join(format!("kda-snapshot-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let mut cfg = Config::default();
        cfg.tools.allowed_paths = vec![root.display().to_string()];
        let shared = Arc::new(RwLock::new(cfg));
        let executor = ToolExecutor::legacy_fixture(shared.clone());
        let snapshot = executor.get_config_snapshot().unwrap();
        let mut guard = shared.write().await;
        guard.tools.create_file = false;
        guard.tools.allowed_paths.clear();
        let call = tc("create_file", serde_json::from_value(serde_json::json!({"path": root.join("fixture.txt"), "content": "snapshot fixture"})).unwrap());
        let result = executor
            .execute_with_config(&call, &snapshot)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(
            std::fs::read_to_string(root.join("fixture.txt")).unwrap(),
            "snapshot fixture"
        );
        drop(guard);
        assert!(
            !executor
                .execute(&call, &ToolCtx::auto(None))
                .await
                .unwrap()
                .success
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolve_common_aliases() {
        assert!(resolve_app("firefox").is_some());
        assert!(resolve_app("dolphin").is_some());
        assert!(resolve_app("terminal").is_some());
        assert_eq!(
            resolve_app("gestor de recursos"),
            Some(("plasma-systemmonitor".to_string(), Vec::new()))
        );
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

    #[test]
    fn find_documents_filters_document_types() {
        let base = std::env::temp_dir().join(format!("kda_docs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("factura.pdf"), "x").unwrap();
        std::fs::write(base.join("factura.png"), "x").unwrap();
        std::fs::write(base.join("factura.bin"), "x").unwrap();
        let found = find_documents_sync(std::slice::from_ref(&base), "factura").unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|p| is_document_like(p)));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn text_preview_generates_svg_image() {
        let base = std::env::temp_dir().join(format!("kda_preview_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let doc = base.join("nota.md");
        std::fs::write(&doc, "# Hola\nLinea <segura> & limpia").unwrap();
        let (cache, _) = crate::backend::resource_cache::allocate_at(&base).unwrap();
        let out = render_text_preview(&doc, &cache, "nota").unwrap();
        let svg = std::fs::read_to_string(&out).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("&lt;segura&gt; &amp; limpia"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn office_preview_generates_internal_card_without_suite() {
        let base = std::env::temp_dir().join(format!("kda_office_preview_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let doc = base.join("contrato.docx");
        std::fs::write(&doc, "placeholder").unwrap();
        let (cache, _) = crate::backend::resource_cache::allocate_at(&base).unwrap();
        let out = render_office_placeholder_preview(&doc, &cache, "contrato").unwrap();
        let svg = std::fs::read_to_string(&out).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("contrato.docx"));
        assert!(svg.contains("Preview interna"));
        assert!(svg.contains("DOCX"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn process_candidates_for_common_apps_are_exact_names() {
        let candidates = process_candidates("brave");
        assert!(candidates.contains(&"brave".to_string()));
        assert!(candidates.contains(&"brave-origin".to_string()));
        assert!(candidates.iter().all(|c| c
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))));
    }

    #[tokio::test]
    async fn show_image_file_uri_outside_allowed_paths_is_denied() {
        let ex = dummy_executor();
        let mut args = HashMap::new();
        args.insert(
            "source".to_string(),
            serde_json::Value::String("file:///etc/passwd".to_string()),
        );
        let r = ex
            .execute(&tc("show_image", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!r.success, "esperaba rechazo: {}", r.content);
        assert!(
            r.content.contains("no permitido") || r.content.contains("allowed_paths"),
            "contenido inesperado: {}",
            r.content
        );
    }

    #[tokio::test]
    async fn preview_document_text_returns_image_url() {
        let base = std::env::temp_dir().join(format!("kda_preview_exec_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let doc = base.join("nota.md");
        std::fs::write(&doc, "contenido de prueba").unwrap();
        let mut cfg = Config::default();
        cfg.tools.allowed_paths = vec![base.to_string_lossy().to_string()];
        let ex = ToolExecutor::legacy_fixture(Arc::new(RwLock::new(cfg)));
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(doc.to_string_lossy().to_string()),
        );
        let r = ex
            .execute(&tc("preview_document", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(r.success, "{}", r.content);
        let image = r.image_url.expect("preview image");
        assert!(image.ends_with(".svg"));
        assert!(Path::new(&image).exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn copy_file_copies_bytes_inside_allowed_paths() {
        let base = std::env::temp_dir().join(format!("kda_copy_exec_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("dest")).unwrap();
        let src = base.join("documento.pdf");
        std::fs::write(&src, b"%PDF-test-bytes").unwrap();
        let mut cfg = Config::default();
        cfg.tools.allowed_paths = vec![base.to_string_lossy().to_string()];
        let ex = ToolExecutor::legacy_fixture(Arc::new(RwLock::new(cfg)));
        let mut args = HashMap::new();
        args.insert(
            "source".to_string(),
            serde_json::Value::String(src.to_string_lossy().to_string()),
        );
        args.insert(
            "destination".to_string(),
            serde_json::Value::String(base.join("dest").to_string_lossy().to_string()),
        );
        let r = ex
            .execute(&tc("copy_file", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(r.success, "{}", r.content);
        assert_eq!(
            std::fs::read(base.join("dest").join("documento.pdf")).unwrap(),
            b"%PDF-test-bytes"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// F0-2: URL con Content-Length > 10MB se rechaza sin descargarla.
    /// Servidor TCP local que solo manda cabeceras (cero transferencia real).
    #[tokio::test]
    async fn show_image_huge_url_is_rejected_before_download() {
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("[skip] loopback no disponible para test local: {err}");
                return;
            }
        };
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || loop {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::Read;
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                use std::io::Write;
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 20000000\r\n\r\n",
                );
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        });

        let ex = dummy_executor();
        let mut args = HashMap::new();
        args.insert(
            "source".to_string(),
            serde_json::Value::String(format!("http://127.0.0.1:{port}/big.png")),
        );
        let r = ex
            .execute(&tc("show_image", args), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!r.success, "esperaba rechazo: {}", r.content);
        assert!(
            r.content.contains("demasiado grande"),
            "contenido inesperado: {}",
            r.content
        );
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
