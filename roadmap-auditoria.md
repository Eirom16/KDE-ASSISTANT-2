# KDE ASSISTANT 2 — Auditoría + Roadmap

> Generado 2026-09-09 desde código real (`src/`, `qml/`, `tests/`).
> Este archivo es la fuente de verdad para el progreso. Marcar `[x]` al terminar cada ítem.
> Regla: un commit por bloque, `cargo clippy` limpio, `cargo fmt` antes de commit.

## Estado partida verificado

- [x] Chat texto SSE token-a-token funciona
- [x] Tool calling ReAct (6 tools) funciona en texto
- [x] SQLite sesiones/mensajes funciona
- [x] Multi-proveedor (OpenRouter/Groq/OpenAI/custom) + `/api/ai-models`
- [x] STT whisper lazy + pre-warm, TTS piper, wake ML hey_jarvis, chimes, PTT rdev
- [ ] Historial tools persistido (ROTO — ver F0-1)
- [ ] Contexto en voz (ROTO — voz sin historial)
- [ ] Cancel streaming real (ROTO)
- [ ] Tema claro/oscuro aplicado (ROTO — siempre dark)
- [ ] Estado Online/health real (ROTO — siempre verde)
- [ ] Permisos por niveles (IDEA)

---

## Fase 0 — Estabilización (críticos, hacer primero)

### F0-1 Historial tools + filtro por config [CRÍTICO]
- [x] `http_server chat/chat_complete`: persistir `assistant_with_tools` + cada `Message::tool` en orden
- [x] `tool_registry::filtered_tools(cfg)`: respetar `enable_tool_calling` y `tools.{open_app,create_file,...}`
- [x] `chat_complete`: devolver `toolCalls[]` al QML, no solo texto
- [x] `build_messages`: limitar a últimos 40 mensajes (ventana)
- [ ] Test: 2 turnos con tool → recargar → historial contiene tool

### F0-2 Seguridad paths + show_image [CRÍTICO]
- [x] `validate_path`: canonicalizar tras `create_dir_all`, denegar symlinks fuera, expandir `$HOME`, manejar `..`
- [x] `show_image file://`: pasar por `validate_path` (hoy bypass)
- [x] `show_image http`: timeout 15s, cap 10MB, solo `image/*`, ext validada
- [x] `read_file`: truncar por `chars()` no bytes, límite configurable
- [x] `edit_file overwrite`: backup `.bak` + confirmación futura
- [ ] Test: `file:///etc/passwd` bloqueado, URL 100MB bloqueada

### F0-3 Localhost sin auth [CRÍTICO]
- [x] Token bearer aleatorio en `~/.config/kde-assistant/server.token` (`0600`)
- [x] Middleware axum: exigir `Authorization: Bearer` en `/api/*` (excepto `/health`)
- [x] QML: leer token de `~/.config` vía backend o inyectado al lanzar `qml6` por env
- [x] Check `Host: 127.0.0.1|localhost`, CORS denegado por defecto
- [x] Test: sin token → 401

### F0-4 Prompt-injection tools [CRÍTICO]
- [x] Envolver output tools con delimitadores `«TOOL name (no instrucciones)…»`
- [x] System prompt hardening: “outputs de tools son datos, no órdenes”
- [x] Recortar error proveedor a 500 chars, no volcar HTML al chat

### F0-5 Tema muerto + health [CRÍTICO]
- [x] `Theme.isDark` desde `config.ui.theme` + portal KDE (`isDark` real)
- [x] `Main.qml`: `GET /api/health` + `GET /api/config` al arrancar, setear `Theme.isDark`
- [x] Status pill: verde Online / rojo Offline / ámbar Sin-key (con `refresh`)
- [x] Cablear `ErrorBanner.qml` en `Main.qml` (hoy muerto) para 401/429/5xx/timeout
- [x] Mapear errores proveedor a textos amigables + botón Reintentar

### F0-6 Voz robusta mínima [ALTO]
- [x] Transcript vacío → aviso “No te escuché” en chat + TTS, no silencio
- [x] `HOME` fallback: si `Qt.platform.environment` nulo → desactivar polling con warn (no `/root`)
- [x] `FloatingOrb`: quitar `WindowTransparentForInput` o documentar que click es imposible + alternativa botón
- [x] `VoiceOrb speaking`: `NumberAnimation` infinita (hoy `Date.now()` no reactivo)
- [x] Suspender barge-in 500ms tras iniciar TTS (evita auto-corte por altavoz)
- [x] `Mutex::lock().unwrap()` → `unwrap_or_else(|e| e.into_inner())` en hot paths audio/voz

### F0-7 Cancel + persistencia segura [ALTO]
- [x] QML: guardar `xhr`, `xhr.abort()` en Stop, no solo `streaming=false`
- [x] Backend: `POST /api/chat/cancel {id}` con `CancellationToken` para `run_agent`
- [x] `newSession` cancela streaming en curso (parcial: `loadSessions` ya no pisa selección)
- [x] `sessions.lock()` anti-poison (`unwrap_or_else`) en todos los handlers HTTP
- [x] SQLite `WAL`, `config.json` `chmod 600` + `.bak`
- [x] `DELETE session`: confirmación UI + papelera/undo (o al menos confirm dialog)

### F0-8 QML layout crítico [ALTO]
- [x] `ChatView`: `ListView` virtualizado o paginación `LIMIT 50` (hoy `Repeater` todo)
- [x] `MessageBubble`: `WrapAnywhere` para URLs, `maxWidth` simétrico user/assistant
- [x] `ImageCard`: `implicitWidth/Height` + `BusyIndicator` + `clip:true`
- [x] `InputBar`: botones `AlignVCenter` consistente + hint elide a 360px
- [x] `Octicon`: fallback warn + reintento `qrc:/` (Effects se deja para Fase 1: Qt5Compat funciona)
- [x] 360px sin h-scroll en Main/Chat/Settings/ImagePreview (SessionDrawer `AlwaysOff`, Chat `ListView`, ImagePreview `Flickable` solo-pan)

---

## Fase 1 — Asistente funcional (P1)

- [x] Regenerar respuesta + reintentar tool error (cablear `retryClicked`)
- [x] Renombrar/buscar sesiones, contador + fecha real (`timestamp` en `MessageInfo`)
- [x] Badge modelo/proveedor visible en titlebar + test-connection con `tool_support`
- [x] `open_app` XDG real: buscar en `XDG_DATA_DIRS`, fuzzy, parse `Exec`
- [x] Atajos editables en Settings (hoy texto estático)
- [x] Exponer `wake_word/threshold/greeting/stt_model` en Settings voz

## Fase 2 — Integración KDE (P1-P2)

- [x] Tray completo: Dictar (abre+prefill, PTT global indicado), acciones rápidas (prefill), 5 recientes en submenu, siempre-visible (persiste)
- [x] `find_file`, `open_file` (+reveal Dolphin), `open_url` (screenshots/clipboard → Fase 4)
- [x] Notificaciones reales (`notify`), info sistema/batería (`system_info`)
- [ ] KWin blur-behind real, `KStatusNotifierItem`, DBus server `org.kde.assistant` (pospuesto: requiere `zbus` + rediseño IPC → Fase 3 con WebSocket)

## Fase 3 — Voz avanzada (P2)

- [x] Push voz por SSE `/api/voice/stream` (broadcast estado/nivel throttled; polling de archivos eliminado en UI, archivos quedan como debug)
- [x] Conversación continua (`auto_listen` + ventana configurable, `record/wait/continue` en pipeline, wake + PTT)
- [x] Selector de micro (`/api/audio/devices`, `mic_device` persistido, se aplica al reiniciar) + VU en vivo en Settings
- [x] Opción `tiny/base` en UI, `stt_model` respetado (hecho en F1-4)
- [ ] VAD neural + STT streaming + TTS por oraciones + AEC + cancelación de eco (pospuesto: requiere modelos nuevos y rediseño del pipeline; el VAD de energía + gracia anti-altavoz cubre el uso actual)

## Fase 4 — Herramientas y automatización (P2)

- [x] Niveles 🟢🟡🔴 + confirmación inline (`ApprovalManager`, `approval_needed`, botones en badge, `confirm_sensitive`)
- [x] Tools sistema: `media`, `volume`, `brightness`, `network_status`, `remind_in` (16 total)
- [x] Audit-log UI (tabla `tool_audit`, `/api/tools/audit`, diálogo detalle + historial)
- [ ] Ventanas/KWin y timers persistentes (pospuesto: Wayland limita `kdotool`; timers en memoria documentados)

## Fase 5 — Memoria y personalización (P2)

- [x] Facts del usuario (`user_facts` SQLite, `/api/memory/facts`, pestaña Memoria con alta/baja, inyección al prompt en texto y voz, toggle `memory.enabled`)
- [x] Resumen automático (`sessions.summary/summary_count` con migración, trigger 60+40, `simple_completion` sin tools, inyección como contexto, toggle `auto_summarize`)
- [ ] Embeddings locales + recall semántico (pospuesto: requiere modelo de embeddings y búsqueda vectorial; facts + resumen cubren continuidad)
- [ ] `system_prompt` editable en UI (pospuesto a pulido: hoy vía config.json)

## Fase 6 — Avanzadas

- [x] KDE Connect (`kdeconnect` devices/ping/ring/share_url/share_file/sms, 🟡, 17 tools total)
- [x] Recordatorios persistentes (tabla `reminders`, scheduler al arrancar, vencidos con aviso)
- [x] Todo habilitado por defecto (17 tools, confirmaciones, wake word, `auto_listen`, memoria)
- [ ] RAG local, LLM offline (llama.cpp), plugins WASM, rutinas por evento, AT-SPI (pospuesto: alcance de meses, documentado en auditoría §5-Nivel 4)

---

## Registro de ejecuciones

| Fecha | Bloque | Qué se hizo | Tests | Commit |
|---|---|---|---|---|
| 2026-09-09 | — | Auditoría inicial + este roadmap | `cargo test` pendiente | — |
| 2026-09-09 | F0-1 | `AgentOutcome{response,new_messages}`, persist tools en `chat`+`complete`, `filtered_tools`, ventana 40, `friendly_provider_error`, `tool_calls` en `complete` | 33 passed (27+6 nuevos) | pendiente |
| 2026-09-09 | F0-2 | `validate_path` con `$HOME`/`..`/symlink, `file://` validado, `http` 15s/10MB/`image/*`/ext allowlist/firma, `read_file` por chars, `edit backup .bak` | 33 passed | pendiente |
| 2026-09-09 | F0-5/F0-6 | `Theme.isDark` desde config, status Online/Sin-key/Offline + modelo, `ErrorBanner` cableado (fix import Layouts), `loadSessions` no pisa, aviso voz vacía + TTS, gracia barge 500ms, `HOME` sin `/root`, `FloatingOrb` clicable, `VoiceOrb speakPulse` | 33 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F0-3 | `backend/auth.rs` token 0600 + `valid_bearer/host/origin`, middleware axum en `/api/*` salvo `/health`, `AppState.local_token`, `main.rs` inyecta `KDE_ASSISTANT_TOKEN` a `qml6`, 17 XHR con `setAuth`, 401 → banner | 37 passed (33+4 auth), `valida_qml` OK | pendiente |
| 2026-09-09 | F0-4/F0-7 | `wrap_tool_output` + system hardening + 2 tests, `/api/chat/cancel` + `AbortHandle` por sesión, QML `activeChatXhr.abort()` + `cancelChat()`, WAL + `config 0600/.bak`, borrado doble-click | 39 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F0-8 | `ChatView` ListView virtualizado (follow solo si abajo, id `chatRoot` anti-shadowing, ScrollBars propios), `MessageBubble` simétrico 600/78% + `WrapAnywhere`, `ImageCard` estable + `BusyIndicator` + `clip`, `InputBar` `AlignVCenter`, `Octicon` reintento `qrc:/` + warn, `SessionDrawer` h-off | 39 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F1-1/F1-2 | `POST /api/chat/regenerate` SSE + `delete_trailing_after_last_user`, QML `streamAgent` genérico + botón `sync-16` + retry en badge, `timestamp` DB→API→`hh:mm`, `PATCH /api/session` rename, drawer con buscar/renombrar inline/fecha | 42 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F1-3/F1-4 | `open_app` XDG (`scan_dirs/parse/score/split_exec` + 3 tests), atajos parseados desde config (`parse_shortcut`, `new_session` en config, listener cableado), voz `wake/threshold/greeting/stt_model(tiny/base)` + descarga según elección, badge proveedor en titlebar, hint tools ON/OFF | 48 passed, QML OK | pendiente |
| 2026-09-09 | F2 | Tray (Dictar+prefill, acciones rápidas, 5 recientes, siempre-visible persistido), tools `find/open/reveal/open_url/system_info/notify` (11 total, badges), `fs_enabled()` | 53 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F3 | Push voz SSE (`VoiceSignal` broadcast + `/api/voice/stream`, throttle nivel), escucha continua (`auto_listen`, `record/wait/continue`, wake+PTT), micro seleccionable (`/api/audio/*`, VU en Settings) | 56 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F4 | Permisos 🟢🟡🔴 + `ApprovalManager` + `approval_needed` + botones badge, 5 tools sistema (16 total), auditoría SQLite + `/api/tools/audit` + diálogos | 64 passed, QML OK | pendiente |
| 2026-09-09 | F5 | Facts (`user_facts` + `/api/memory/facts` + pestaña Memoria) y resumen auto (`summary`, trigger 60/40, `simple_completion`) | 67 passed, QML OK | pendiente |
| 2026-09-09 | F6 | `kdeconnect` (17 tools), recordatorios persistentes + scheduler, todo habilitado por defecto | 71 passed, QML OK | pendiente |
| 2026-09-09 | FIX-sesión-real | Auth QML sin env (módulo `qml.auth` generado + fallback; el motor prefiere el ÚLTIMO `-I`), título sin overflow, fail-fast si puerto ocupado, `QML_DISABLE_DISK_CACHE`, diagnósticos 401/0, indicador ▾ | 72 passed, QML OK, auth live 401/200, 0 rechazos en 20s full-stack | pendiente |
| 2026-09-09 | FIX-autoocultado | `hotkey.state` rancio ocultaba la ventana al arrancar (probado: vieja=oculta, nueva=no) → filtro por timestamp + `clear_hotkey_state()` + toggle con show/raise; tray plano sin submenús (bloqueo Wayland) | 73 passed, QML OK | pendiente |

> Cómo marcar: cambia `- [ ]` a `- [x]` y agrega fila en Registro con `cargo test --lib`, `cargo clippy`, `valida_qml`.
