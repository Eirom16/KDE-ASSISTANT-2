# DESKTOP AGENT — Auditoría y Diseño de Arquitectura

> Documento de diseño para la evolución de `FloatingOrb` → `AssistantCharacter`.
> Fuente: `KDE-ASSISTANT-2_Desktop-Agent-Prompt-v2.md` (prompt maestro).
> Estado: **auditoría completa + diseño aprobado para implementar** (Fases 1–8).
> Regla: nada de esto modifica archivos existentes hasta que se indique;
> toda la implementación aterriza primero en archivos nuevos.

---

## 1. Auditoría del repositorio (estado real, verificado)

### Arquitectura actual

```
kde-assistant (Rust/Tokio)
├── Backend: axum HTTP en 127.0.0.1:8765 (token Bearer local por instancia)
├── SSE: /api/chat (token/tool_call/tool_result/approval_needed/done/error)
│         /api/voice/stream (state: idle|listening|processing|speaking, level)
├── Voz: cpal → VAD → whisper-rs (STT) / piper-tts (TTS) / rodio (chimes)
├── Wake word: openWakeWord ONNX (hey_jarvis), HotwordEvent::Detected en main.rs
├── Hotkeys: rdev en thread → hotkey.state + acciones PTT/toggle/menu
├── SQLite: sesiones/mensajes/auditoría (session_manager)
└── UI: subproceso `qml6 qml/Main.qml` (QT_QPA_PLATFORM=xcb → XWayland;
        comentario en main.rs: en nativo Wayland "la ventana a veces no mapea")
```

### Puente backend ↔ QML (hechos)

- **No hay QML embebido en Rust ni context properties**: todo es HTTP/SSE con
  `XMLHttpRequest` desde QML + módulo generado `qml.auth` (token) +
  `~/.cache/kde-assistant/hotkey.state` (polling a 300ms para hotkeys).
- El chat ya emite por SSE exactamente los eventos que necesitan las escenas:
  `tool_call` (inicio, con nombre), `tool_result` (fin, con contenido/imagen),
  `approval_needed`, `done`, `error`. La voz emite `state`/`level`.
- `main.rs` ya supervisa al proceso qml6 y mata huérfanos (`cull_stale_qml`,
  bucle `try_wait`). Añadir un segundo proceso QML exige extender esa lógica.

### FloatingOrb auditado (§20)

- `qml/components/FloatingOrb.qml` (75 líneas): `Window` transparente,
  `Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint`, 190×190 fija.
- **Posición**: esquina inferior derecha con `Screen.desktopAvailableWidth/Height`
  (márgenes 28/64). **Fija**, no persistente, no arrastrable, sin multi-monitor.
- **Instanciación**: `Main.qml:1083` con `voiceState`/`amplitude` de la raíz.
- **Señales**: ninguna propia. Contiene un `VoiceOrb` cuyo `clicked()` **no está
  conectado** (el comentario "clic = barge-in" está desactualizado: hoy el clic
  en el orbe flotante no hace nada). Hallazgo de auditoría.
- **Conexión con backend**: indirecta — Main.qml escucha `/api/voice/stream` y
  reenvía `voiceState`. No tiene canal propio.
- Ciclo de vida: `visible: voiceState !== "idle"` (solo existe durante la voz).

### Entorno verificado del sistema del usuario

| Pieza | Estado |
|---|---|
| Qt / qml6 | 6.11.2 ✅ |
| Sesión | KDE Plasma 6.7.4, **Wayland**, KWin 6.7.4 |
| `org.kde.layershell` QML module | **instalado** (`/usr/lib/qt6/qml/org/kde/layershell/`) ✅ |
| qt6-svg, qt6-multimedia | 6.11.2 ✅ (Image SVG + SoundEffect disponibles) |
| Tests | `tests/agent_smoke.rs`, `src/bin/valida_qml.rs` (smoke offscreen de Main.qml) |

### Limitaciones técnicas documentadas (§13 — evaluación + documentación)

1. **UI en XWayland** (decisión ya tomada en main.rs por bug de mapeo en nativo).
   Esto inhabilita layer-shell en el proceso actual; los flags X11
   (`Qt.Tool|StaysOnTop`) funcionan bien vía XWayland hoy.
2. **Posición global del cursor**: en Wayland nativo un cliente no puede sondear
   el puntero (privacidad). En XWayland se puede vía XQueryPointer. → El gaze
   al cursor será *best-effort* (mirar cuando el cursor está sobre/cerca del
   personaje; miradas ociosas el resto del tiempo). No prometer mirada global.
3. **Geometría de ventanas ajenas**: no hay API estable sin KWin script.
   Estrategia: zonas visuales aproximadas (zona de archivos, esquina de panel…)
   configuradas por heurística de pantalla; KWin script opcional en Fase 7.
4. **qml6 runtime puro**: sin plugins C++ propios → layer-shell se consume como
   módulo QML (`import org.kde.layershell 1.0`) pero hay que protegerlo con
   carga dinámica (`Qt.createComponent`) para no romper XWayland/offscreen.
5. **input-region / click-through**: con layer-shell es nativo (`inputRegion`);
   en ventana XWayland no es accesible desde QML puro → se aproxima haciendo la
   ventana exactamente del tamaño del personaje.

---

## 2. Decisiones de diseño (con justificación)

### D1 — El personaje convive y luego absorbe al FloatingOrb

- **Fases 1–3 (fundación/vida/movimiento)**: `AssistantCharacter` vive en una
  **nueva ventana `AgentWindow`** dentro del **mismo proceso qml6** (XWayland).
  Cero riesgo para Main.qml y FloatingOrb, que siguen intactos.
  Config `character.enabled` (default **false** hasta Fase 5).
- **Fase 4 (overlay)**: migración a **proceso QML separado** lanzado por main.rs
  (`qml6 qml/agent/AgentMain.qml` con `QT_QPA_PLATFORM=wayland`) **solo si**
  layer-shell está disponible; fallback a la ventana integrada si no.
  Justificación: layer-shell da overlay real (sin foco, click-through,
  multi-monitor, sobre/bajo ventanas) sin C++; y proceso separado aísla fallos
  y permite wayland nativo sin tocar la ventana principal (que tiene un bug
  conocido de mapeo en nativo). Coste: supervisión de un segundo hijo.
- **Fase 5+**: cuando el personaje cubre los estados de voz (wake/listening/
  processing/speaking), `AgentWindow` sustituye a `FloatingOrb` (que se apaga
  con `character.enabled` y se mantiene el código como fallback).

### D2 — Avatar: Blobatar por capas, assets locales, sin WebView

Blobatar (MIT, `github.com/Alain00/blobatar`): generador determinista
nombre→SVG (viewBox 100×100), 14 expresiones exactamente las del prompt,
silueta `capsule` disponible en gen 2, `hue=225`. La morfología entre
expresiones requiere CSS (no existe en QML), y el gaze es un driver JS que
traslada un grupo `.mo-eyes`.

- **Spike (primera tarea de implementación)**: generar offline los 14 SVG del
  personaje fijo (nombre/traits fijados, `hue=225`, `shape=capsule`,
  `background=none`) con `@blobatar/cli` y **inspeccionar la estructura**.
- **Plan A (preferido)**: separar capas — cuerpo (1 SVG estático) + ojos/boca
  como items QML posicionados por un mapa de expresiones (JSON extraído de los
  SVG: posición/escala/forma de cada rasgo por expresión). Así **blink** (párpado
  o scaleY de ojos), **gaze** (mover pupilas, travel ≈3% del viewBox) y
  **squash&stretch** (scale del cuerpo) son nativos QML y baratos. Cambio de
  expresión = crossfade de los rasgos de cara (150–200ms), no morph.
- **Plan B (fallback)**: 14 SVG completos + crossfade; blink/gaze limitados
  (párpados overlay sobre los ojos si la geometría lo permite).
- **Offline**: los SVG/JSON se **comitean** en `assets/character/`; la app no
  llama a blobatar.dev en runtime (cumple §2, privacidad).
- **Licencia**: MIT → `assets/character/ATTRIBUTION.md` con copyright y enlace.

### D3 — Contrato de intención: el backend deriva el intent, no el LLM

El prompt (§9) muestra al LLM emitiendo `{tool, visual, mood}`. Decisión
justificada (§38): **el intent se deriva determinísticamente del `tool_call`
en el backend**, en `tool_registry`/`agent_intent.rs` con una tabla central
herramienta→escena/mood (cumple §8/§28 "tabla como configuración central").
Razones: los modelos vía OpenRouter/Groq no garantizan campos extra estables;
el tool_call ya ES la intención; y elimina una vía de inyección visual. El
formato JSON del intent **sigue siendo el del prompt** (schema validado con
whitelist en el lado QML), solo que lo emite Rust:

```json
{
  "type": "intent",
  "tool": "find_file",
  "visual": "search",
  "mood": "focused",
  "scene": "SearchScene",
  "phase": "start",          // start | progress | done | error | cancel
  "priority": "NORMAL",       // CRITICAL | ERROR | SECURITY | IMPORTANT | NORMAL | IDLE
  "target": {"kind": "zone", "zone": "files"},
  "message": "encontró 3 archivos"
}
```

- **Bus**: nuevo SSE `GET /api/agent/stream` (mismo patrón que `voice/stream`:
  XHR persistente + reconexión 2s). El QML del chat no se toca.
- Orígenes de intents: `run_agent` (tool_call/tool_result/error/approval),
  wake word (`HotwordEvent::Detected` → intent `wake`), PTT, chat por texto
  (`thinking` mientras streamea, `happy` al `done`), recordatorios, errores HTTP.
- **Cancelación real**: `/api/chat/cancel` ya existe → intent `phase:"cancel"`.

### D4 — Controladores en QML puro (deterministas, testables offscreen)

Nuevo módulo `qml/agent/` (qmldir `module qml.agent`), todo `QtObject`/Item QML:

```
qml/agent/
├── AgentMain.qml            # entry del proceso overlay (Fase 4)
├── AgentWindow.qml          # ventana contenedora (XWayland hoy; layer shell F4)
├── AssistantCharacter.qml   # visual: cuerpo + cara + squash; API pública
├── CharacterController.qml  # fachada: prioridades, cola, dispatch a subsistemas
├── ExpressionController.qml # 14 expresiones + blink + transiciones + anti-flap
├── MoodController.qml       # estado emocional: valor, intensidad, decaimiento
├── GazeController.qml       # mirada con inercia (cursor/ventana/zona/punto)
├── MovementController.qml   # locomoción: walk/run/jump/bounce/return + física
├── AnimationController.qml  # anims con nombre: breathe/blink/think/celebrate…
├── SceneController.qml      # registro+ejecución de escenas, cancelación
├── IntentRouter.qml         # SSE → validación whitelist → SceneController
├── IdleBehavior.qml         # vida autónoma (parpadeo, postura, dormir/despertar)
├── DebugPanel.qml           # panel §31 (forzar expresión/escena, FPS; off en prod)
└── scenes/
    ├── Scene.qml            # clase base: start/progress/done/error/cancel
    ├── SearchScene.qml  OpenAppScene.qml  FileScene.qml  SystemScene.qml
    ├── ErrorScene.qml   SuccessScene.qml  DownloadScene.qml  ScreenshotScene.qml
    └── GenericScene.qml     # fallback de herramientas desconocidas (§8)
```

- **ExpressionController**: propiedades `expression`, `mood`, `blinking`,
  `looking` (conceptuales del §3). Anti-flap: mínimo 900ms por expresión,
  cola de 1 transición pendiente, crossfade 160ms.
- **MoodController**: `mood` + `intensity 0..1` + `decaySeconds`; transiciones
  suaves; el mood nunca salta por cada token del LLM (solo intents completos).
- **GazeController**: `lookAt(point/zone)`, inercia (lerp + micro-saccades con
  ruido poco frecuente), descanso al centro; cursor solo si disponible (D-L2).
- **MovementController**: física ligera §16 — velocidad, aceleración, gravedad
  opcional, suelo virtual, bordes de pantalla, rebote (QML `Behavior`/springs +
  integración por frame con `FrameAnimation` solo cuando se mueve; sin timers en
  reposo. `Screen.virtualX/virtualY` para multi-monitor X11; clamp a
  `desktopAvailable` para respetar paneles).
- **PrioritySystem** (§18): `CRITICAL > ERROR > SECURITY > IMPORTANT > NORMAL
  > IDLE`; una escena en ejecución de prioridad menor se cancela limpia; IDLE
  nunca interrumpe trabajo; cola acotada (máx 4, descarta IDLE duplicados).
- **Modos de presencia** (§11) `character.mode`: `minimal | reactive |
  companion | cinematic` — un objeto de restricciones por modo (qué controlador
  puede mover/expresar, frecuencia idle, escenas sí/no). Persistido en config.

### D5 — Backend: `agent_intent.rs` + `sound_controller.rs`

```
src/backend/
├── agent_intent.rs      # Intent struct (serde), tabla tool→(scene,mood,visual),
                         # broadcast tokio::sync::broadcast, dedupe, prioridad
├── sound_controller.rs  # ¡Nuevo! cues con cooldown, prioridad, ducking TTS,
                         # rodio; respeta character.sound.enabled/volume
└── http_server.rs       # +GET /api/agent/stream (SSE)  +POST /api/agent/feedback
                         # (QML reporta hover/click/drag para logging futuro)
src/models/config.rs     # + CharacterConfig {enabled, mode, reduced_motion,
                         #  sounds_enabled, sfx_volume, click_through, monitor,
                         #  home_position:{x,y}, follow_fullscreen:bool}
```

- `AgentIntent` se emite desde: `ai_service::run_agent` (hook en tool_call /
  tool_result / error), `main.rs` (wake), hotkeys (PTT), reminders, approvals.
- Sonido (§24.5): biblioteca local ≤40 cues (`assets/audio/{character,
  interaction,assistant,movement,special}/`), mapa evento→cue centralizado en
  Rust (tabla §24.5), cooldown por cue (p.ej. hover ≥2s), ducking de SFX cuando
  TTS habla, "modo silencioso". `chime_player` se mantiene para TTS/voz; el
  controller nuevo gestiona SFX. Las licencias van a `assets/audio/ATTRIBUTION.md`.
- **Wake chime** (§24.6): distintivo, <400ms, propio; `wake` corta idle con la
  animación wake + lookAt(usuario) antes de `listening`.
- Presupuesto de audio (§24.9): contador de cues; en `minimal` solo
  wake/success/error; nada de sonidos por micro-movimientos.

### D6 — Voz/personaje y regla voz-texto quedan intactos

El pipeline de voz actual no cambia. El personaje **refleja** estados (no los
orquesta): `listening → looking at user + listening`, `processing → thinking`,
`speaking → talking bob`, `idle → returnTo idle`. `barge-in` y recordatorios
siguen funcionando igual; el personaje es una proyección visual del agente.

---

## 3. Mapeo tool → escena (tabla central inicial, §8/§28)

| Tool / evento | Escena | Mood | Visual |
|---|---|---|---|
| open_app | OpenAppScene | curious→focused | lookAt(zona app) + mover + actuar |
| find_file | SearchScene | thinking→focused | mover a zona archivos + search + inspect |
| read_file | FileScene | focused | inspect |
| create_file / edit_file | FileScene | excited / focused | escribir (bob) |
| web_search | SearchScene | focused | search mirando zona navegador |
| show_image (con descarga) | DownloadScene | focused→happy | esperar + celebrate corto |
| system_info / network_status | SystemScene | focused | inspect |
| volume / brightness / media | SystemScene | focused | acción breve |
| notify / remind_in | SuccessScene | happy | attention cue |
| kdeconnect | SystemScene | focused | lookAt zona panel/móvil |
| approval_needed | (inline) | concerned | esperar decisión, señal visual |
| tool error / SSE error | ErrorScene | confused→concerned | interrumpir, volver, ErrorBanner ya existente |
| cancel | (cualquiera) | concerned | cancelar escena → return |
| done (chat) | — | happy (si hubo tool) | settle |
| wake word | WakeScene (inline) | sleepy→attentive | wake anim + wake chime + lookAt usuario |
| unknown tool | GenericScene | focused | movimiento corto genérico |

Prioridades: error/approval=ERROR/IMPORTANT, tools=NORMAL..IMPORTANT,
idle autónomo=IDLE, wake=IMPORTANT.

---

## 4. Riesgos y bloqueos anticipados

| Riesgo | Mitigación |
|---|---|
| Layer-shell requiere Wayland nativo; main.qml va en XWayland por bug de mapeo | Proceso separado del agente en nativo (F4) + fallback ventana XWayland; decisión medida con prueba en sesión real |
| Posición global del cursor indisponible en Wayland | Gaze al cursor solo aproximado (hover/última interacción); documentado (§13/D-L2) |
| Validación offscreen (`valida_qml.rs`) sin layer-shell | `AgentWindow` carga layer shell vía `Qt.createComponent` dinámico; si falta, modo ventana clásica → el smoke test offscreen sigue pasando |
| 2º proceso qml6: huérfanos | Reusar patrón `cull_stale_qml` + supervisión (patrón distinto: `qml6.*agent/AgentMain.qml`) |
| Rendimiento Intel UHD | Solo transform/opacity; `layer.enabled` prohibido en el personaje; timers de idle ligados a modo; `FrameAnimation` solo en movimiento; medición CPU en §21 al final de cada fase |
| Morfología de expresiones sin CSS | Crossfade de rasgos (A/B test en spike); blink/gaze son items QML independientes del SVG |

---

## 5. Plan de implementación (por fases del prompt maestro)

- **Fase 1 — Fundación** ✅ *(implementada)*: spike Blobatar resuelto —
  la expresión es un **pose numérico de 13 canales** (esx/esy/tilt/edy/edx,
  asimetrías esx2/esy2/tilt2/edy2, lock, heat, shake, rock, bdy) aplicado a
  frames de ojos fijos. QML los interpola con lerp → **morph real entre
  expresiones** (mejor que el crossfade previsto). Datos en
  `qml/agent/CharacterData.js` (generado) + `assets/character/` (SVG
  referencia + layout.json + licencia MIT). Componentes:
  `AssistantCharacter.qml`, `ExpressionController.qml` (anti-flap 900ms,
  morph 260/480ms), `AgentAnimationController.qml` (blink aleatorio
  3.2–6.5s, breathe, appear/disappear), `CharacterController.qml`
  (fachada: eventos + mood + prioridades básicas), `AgentWindow.qml`
  (contenedor XWayland, no montado aún en Main), `DevPreview.qml`
  (harness de 14 expresiones + eventos). Smoke: `cargo run --bin
  valida_agent` ✅. QA visual: 14/14 vs referencia ✅.
  *Pendiente de coordinación (otro agente en TrayMenu): montaje opcional
  de AgentWindow desde Main.qml tras config `character.enabled`.*
- **Fase 2 — Vida ✅**: `GazeController` (inercia+overshoot+micro-saccades),
  `MoodController` (8 moods, dwell, TTL, intensidad), `IdleBehavior` (glances,
  posture, sleep/wake con Zzz), `CharacterController` reescrito (fold de
  mood+gaze+idle), `AgentAnimationController` (blink 3.2–6.5s random, breathe
  period por mood, appear/disappear). Montado en `Main.qml` con config
  `character.*`. `CharacterConfig` añadido a `config.rs`. Smoke: `cargo run
  --bin valida_agent` ✅. Checkboxes §5/§6/§33 Fase 2 marcados.
- **Fase 3 — Movimiento ✅**: `MovementController` (física Euler 60FPS: pos/vel/acc,
  fricción 0.92/0.98, gravedad 1200, groundY, home, prioridades 6 niveles),
  `walkTo`/`runTo` (aceleración 3×speed, squash/stretch visual), `jump`
  (onGround guard, vy=-650, anticipación squash, rebote damping 0.65),
  `bounce(intensity)`, `returnHome`, `appear`/`disappear` (scale 0↔1 + Back easing),
  `startDrag/updateDrag/endDrag` (MouseArea), `cancel()`, prioridades
  (drag=6 > scene=5 > jump/bounce=4 > run=3 > walk/return=2 > idle=1).
  `CharacterController` expone eventos movimiento. `AgentWindow` integra
  `moveCtrl` + drag en MouseArea. Smoke: `cargo run --bin valida_agent` ✅.
  Checkboxes §4/§33 Fase 3 marcados.
- **Fase 4 — Overlay ✅**: `AgentOverlay.qml` (LayerShell layer=Overlay,
  keyboardInteractivity=None, inputRegion dinámico clickThrough/interactive),
  `AgentMain.qml` (proceso nativo Wayland, AgentWindow como Item con x/y bind
  a MovementController), `AgentWindowStandalone.qml` (wrapper XWayland para
  compatibilidad Fases 2-3). Multi-monitor: LayerShell anclajes
  left/right/top/bottom cubren desktopAvailableWidth/Height; DPI/scaling
  heredado; posición persistente via MovementController.homeX/Y + config.json;
  recuperación Screen.onDesktopAvailableWidth/HeightChanged. Checkboxes
  §10/§11/§33 Fase 4 marcados. Smoke: `cargo run --bin valida_agent` ✅.
  Fallback XWayland (AgentWindowStandalone) conservado en Main.qml.
- **Fase 5 — Escenas ✅**: `SceneController` (registro dinámico, prioridades
  drag=6 > scene=5 > jump/bounce=4 > run=3 > walk/return=2 > idle=1,
  inyección de dependencias, cancelación), 8 escenas coreografiadas:
  `SearchScene` (6 fases), `OpenAppScene` (5 fases), `FileScene` (4 fases),
  `SystemScene` (3 fases), `ErrorScene` (4 fases), `SuccessScene` (3 fases),
  `DownloadScene` (3 fases), `ScreenshotScene` (3 fases). Cada fase controla
  expresión (request/flash), mood (setMood+TTL), movimiento (walk/run/bounce/return),
  mirada (lookAtZone/rest), duración (Timer), objetivo (params), finalización
  (signal finished), cancelación (skip to finish). Mapeo tool→escena: 17 tools
  a 8 escenas con fallback "system". `CharacterController.handleEvent("tool:*")`
  dispara escenas. Smoke: `cargo run --bin valida_agent` ✅. Checkboxes
  §7/§8/§33 Fase 5 marcados.
- **Fase 6 — Tool calling visual ✅**: `IntentRouter` (whitelist 17 tools
  → 8 visual intents con priority/cancelable/approvalNeeded, parser/validador,
  fallback "system", prioridades 6 niveles no interrumpir mayor, cancelación
  end-to-end). `CharacterController.onToolResult/onToolApproved` callbacks
  desde backend. `handleEvent("tool:*")` → `IntentRouter.processToolCall`.
  Integración completa: tool call SSE → visual intent → escena → resultado
  → success/error scene. Checkboxes §9/§33 Fase 6 marcados.
- **Fase 7 — Inteligencia visual ✅**: `VisualGuide` (showArrow/highlight/pulse/path,
  fade-in/out, duration configurable, habilitado en companion/cinematic),
  `GazeController.desktopZones` (coords globales -> lookAtScreenPoint para overlay
  Wayland nativo), `SearchScene` + `IntentRouter.tool:find_file` (backend real +
  escena visual sincronizada, no finge resultados, cancelación end-to-end),
  `CharacterController.handleEvent("guide:*")` (arrow/highlight/pulse/path/hide).
  Checkboxes §12/§14/§33 Fase 7 marcados.
- **Fase 8 — Pulido ✅**: `DebugPanel` (toggle Ctrl+Shift+D, muestra mood/expr/anim/scene/target/pos/FPS, force buttons), `SettingsDialog` tab Personaje (enabled/mode/reduced-motion/sleep/size + chimes/TTS/wake en Voz), `reducedMotion` propagado a AnimationController/MovementController/VisualGuide, `applyModeConstraints` pausa idle/gaze/movement/guide en minimal, click-through overlay, config.json character.* persistence, `cargo clippy`/`cargo fmt` limpio, `valida_agent` offscreen OK, ~30MB RAM release. Checkboxes §21/§22/§23/§24/§31/§32/§33/§35/§36 Fase 8 marcados.

Orden con el repo: cada fase = commit(s) propios; nunca tocar `TrayMenu.qml`
mientras el otro agente trabaje en él.

---

## 6. Checklist de arranque (siguiente sesión)

1. `npm i -D` nada: usar `bunx @blobatar/cli` o `npx @blobatar/cli` una vez;
   guardar 14 SVG + extraer mapa de rasgos → `assets/character/` + ATTRIBUTION.
2. Crear `qml/agent/` + `qmldir` + `AssistantCharacter.qml` (expresiones,
   blink, gaze básico) — archivos nuevos, sin tocar los existentes.
3. `ExpressionController` con whitelist de las 14 expresiones + anti-flap.
4. `AgentWindow.qml` montada en `Main.qml` tras `enabled` por config
   (única línea en archivo existente, coordinada cuando el otro agente termine).
5. Smoke test offscreen + captura Xvfb para validar visual.

> Al completarse Fase 1 implementada se marcan los checkboxes del prompt
> maestro correspondientes (§2, §3, §20 parcial, §33 Fase 1) y se actualiza
> `AGENTS.md` con los módulos nuevos.
