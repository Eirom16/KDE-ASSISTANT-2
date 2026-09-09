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
- [ ] Envolver output tools con delimitadores `«TOOL name (no instrucciones)…»`
- [ ] System prompt hardening: “outputs de tools son datos, no órdenes”
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
- [ ] QML: guardar `xhr`, `xhr.abort()` en Stop, no solo `streaming=false`
- [ ] Backend: `POST /api/chat/cancel {id}` con `CancellationToken` para `run_agent`
- [ ] `newSession` cancela streaming en curso (parcial: `loadSessions` ya no pisa selección)
- [x] `sessions.lock()` anti-poison (`unwrap_or_else`) en todos los handlers HTTP
- [ ] SQLite `WAL`, `config.json` `chmod 600` + `.bak`
- [ ] `DELETE session`: confirmación UI + papelera/undo (o al menos confirm dialog)

### F0-8 QML layout crítico [ALTO]
- [ ] `ChatView`: `ListView` virtualizado o paginación `LIMIT 50` (hoy `Repeater` todo)
- [ ] `MessageBubble`: `WrapAnywhere` para URLs, `maxWidth` simétrico user/assistant
- [ ] `ImageCard`: `implicitWidth/Height` + `BusyIndicator` + `clip:true`
- [ ] `InputBar`: botones `AlignVCenter` consistente + hint elide a 360px
- [ ] `Octicon`: fallback `alert-16`, migrar a `QtQuick.Effects`, soporte `qrc:/`
- [ ] 360px sin h-scroll en Main/Chat/Settings/ImagePreview

---

## Fase 1 — Asistente funcional (P1)

- [ ] Regenerar respuesta + reintentar tool error (cablear `retryClicked`)
- [ ] Renombrar/buscar sesiones, contador + fecha real (`timestamp` en `MessageInfo`)
- [ ] Badge modelo/proveedor visible en titlebar + test-connection con `tool_support`
- [ ] `open_app` XDG real: buscar en `XDG_DATA_DIRS`, fuzzy, parse `Exec`
- [ ] Atajos editables en Settings (hoy texto estático)
- [ ] Exponer `wake_word/threshold/greeting/stt_model` en Settings voz

## Fase 2 — Integración KDE (P1-P2)

- [ ] Tray completo: Dictar/PTT, acciones rápidas, 5 recientes, siempre-visible
- [ ] `find_file/open_file/reveal_in_dolphin`, `open_url`, screenshots, clipboard
- [ ] Notificaciones reales, info sistema/batería, KWin blur (`BlurManager`), `KStatusNotifierItem`, DBus server

## Fase 3 — Voz avanzada (P2)

- [ ] WebSocket `/ws/voice` (eliminar polling archivos)
- [ ] VAD neural + STT streaming + TTS por oraciones + AEC
- [ ] Conversación continua (re-escucha 6-8s post-TTS) + selector mic + VU
- [ ] Opción `tiny/base` en UI, `stt_model` respetado

## Fase 4 — Herramientas y automatización (P2)

- [ ] Trait `Tool{meta,schema,permission,run}` + niveles 🟢🟡🔴 con confirm inline
- [ ] Tools sistema: `playerctl/wpctl/brightnessctl/nmcli/bluetoothctl`, ventanas/KWin, timers
- [ ] Audit-log UI (qué hizo, cuándo, output completo, deshacer)

## Fase 5 — Memoria y personalización (P2)

- [ ] `system_prompt` editable UI, ventana 40 msgs, `facts.md` local opt-in
- [ ] Resumen auto cada 50 msgs, embeddings locales, UI ver/olvidar

## Fase 6 — Avanzadas (futuro)

- [ ] KDE Connect, RAG local, LLM local offline, plugins WASM, rutinas, AT-SPI

---

## Registro de ejecuciones

| Fecha | Bloque | Qué se hizo | Tests | Commit |
|---|---|---|---|---|
| 2026-09-09 | — | Auditoría inicial + este roadmap | `cargo test` pendiente | — |
| 2026-09-09 | F0-1 | `AgentOutcome{response,new_messages}`, persist tools en `chat`+`complete`, `filtered_tools`, ventana 40, `friendly_provider_error`, `tool_calls` en `complete` | 33 passed (27+6 nuevos) | pendiente |
| 2026-09-09 | F0-2 | `validate_path` con `$HOME`/`..`/symlink, `file://` validado, `http` 15s/10MB/`image/*`/ext allowlist/firma, `read_file` por chars, `edit backup .bak` | 33 passed | pendiente |
| 2026-09-09 | F0-5/F0-6 | `Theme.isDark` desde config, status Online/Sin-key/Offline + modelo, `ErrorBanner` cableado (fix import Layouts), `loadSessions` no pisa, aviso voz vacía + TTS, gracia barge 500ms, `HOME` sin `/root`, `FloatingOrb` clicable, `VoiceOrb speakPulse` | 33 passed, `valida_qml` OK | pendiente |
| 2026-09-09 | F0-3 | `backend/auth.rs` token 0600 + `valid_bearer/host/origin`, middleware axum en `/api/*` salvo `/health`, `AppState.local_token`, `main.rs` inyecta `KDE_ASSISTANT_TOKEN` a `qml6`, 17 XHR con `setAuth`, 401 → banner | 37 passed (33+4 auth), `valida_qml` OK | pendiente |

> Cómo marcar: cambia `- [ ]` a `- [x]` y agrega fila en Registro con `cargo test --lib`, `cargo clippy`, `valida_qml`.
