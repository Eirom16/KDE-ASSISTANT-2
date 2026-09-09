# KDE Assistant v2

Asistente de escritorio para KDE Plasma Linux con estetica Apple Design (Cupertino), capacidades de agente con tool calling, invocacion por voz estilo Siri y system tray multifuncion.

## Stack

- **Backend:** Rust + Tokio
- **UI:** Qt 6 / QML (lanzada via `qml6`)
- **AI:** OpenRouter API (compatible con OpenAI, tool calling, SSE streaming)
- **Voz:** whisper-rs (STT local) + piper-tts (TTS neural ONNX) + wake word ML (openWakeWord "hey jarvis")
- **DB:** SQLite (rusqlite)
- **Iconos:** GitHub Primer Octicons

## Documentacion

- `KDE-ASSISTANT-V2-PLAN.md` — Arquitectura y reglas de ingenieria
- `KDE-ASSISTANT-V2-PROMPT.md` — Especificacion funcional
- `apple-DESIGN.md` — Sistema de diseno Apple
- `AGENTS.md` — Contexto para modelos AI

## Build y Ejecucion

### Dependencias del sistema (Arch / CachyOS)

```bash
sudo pacman -S rust cargo qt6-declarative cmake alsa-lib piper-tts
```

> ONNX Runtime, modelos Whisper/Piper/wakeword: la app los descarga sola
> al primer arranque. No hay que instalar nada mas.

### Compilar y ejecutar

```bash
# Debug (compila rapido)
cargo run

# Release (optimizado, recomendado para uso real)
cargo run --release

# Sin UI (solo backend, util para validar el agente)
cargo run -- --ui-off
```

El binario principal es `kde-assistant`. Al lanzarlo:
1. Inicializa el backend Rust (logs en stdout/stderr)
2. Lanza la UI QML como subproceso de `qml6`
3. Aparece la ventana flotante con el WelcomeScreen

## Estructura del proyecto

```
kde-assistant/
├── src/                    # Backend Rust
│   ├── main.rs             # Entry point
│   ├── lib.rs              # Re-exports
│   ├── backend/            # Modulos: AI, tools, session, speech, etc.
│   └── models/             # Config, Message, ToolCall
├── qml/                    # UI QML
│   ├── Main.qml
│   ├── Theme.qml           # Tokens Apple Design
│   ├── Octicon.qml         # Wrapper para Octicons
│   └── components/         # 17 componentes reusables
├── assets/
│   └── octicons/           # 36 SVGs de GitHub Primer Octicons
├── tests/                  # Tests de integracion
└── resources.qrc           # Recursos Qt (futuro)
```

## Configuracion

Editar `~/.config/kde-assistant/config.json`:

```json
{
  "ai": {
    "provider": "groq",
    "base_url": "https://api.groq.com/openai/v1",
    "api_key": "<tu-api-key>",
    "model": "llama-3.3-70b-versatile",
    "enable_tool_calling": true
  },
  "speech": {
    "tts_engine": "piper",
    "piper_model": "es_ES-sharvard-medium",
    "piper_length_scale": 1.0,
    "auto_speak": false
  }
}
```

Tambien puedes usar variables de entorno (`OPENROUTER_API_KEY`, `GROQ_API_KEY` u `OPENAI_API_KEY` segun el proveedor).

## Proveedores LLM

El backend habla el dialecto OpenAI (`/chat/completions` + SSE + tool calling).
En Configuracion elige proveedor y la app rellena la URL:

| Proveedor | URL | API key |
|---|---|---|
| OpenRouter | `https://openrouter.ai/api/v1` | `sk-or-...` o `OPENROUTER_API_KEY` |
| Groq | `https://api.groq.com/openai/v1` | `gsk-...` o `GROQ_API_KEY` |
| OpenAI | `https://api.openai.com/v1` | `sk-...` o `OPENAI_API_KEY` |
| Personalizado | la que escribas | la que corresponda |

Nota: el tool calling necesita un modelo capaz (ej. en Groq, `llama-3.3-70b-versatile`).

## Cómo funciona (arquitectura)

KDE Assistant v2 usa una arquitectura de dos procesos:

- **Backend Rust** (`kde-assistant`): Tokio runtime que expone el servidor HTTP en `http://127.0.0.1:8765`
- **UI QML** (`qml6 qml/Main.qml`): Lanzada como subproceso, consume la API HTTP

La UI envía mensajes al backend via `POST /api/chat/complete` y recibe la respuesta del agente (streaming SSE disponible via `/api/chat`). El backend persiste sesiones y mensajes en SQLite.

## Voz (TTS + STT)

### TTS (piper-tts)
El motor TTS es **piper-tts** (neural ONNX, alta calidad). Necesita:

1. **Binario:** `sudo pacman -S piper-tts` (Arch/CachyOS)
2. **Modelo de voz:** se descarga **automaticamente** al primer arranque (desde HuggingFace) a `~/.local/share/kde-assistant/models/piper/`

### STT (whisper-rs)
El reconocimiento de voz usa **whisper-rs** local con modelo `ggml-base.bin` (~140MB), que se descarga **automaticamente** al primer arranque.

- **Idioma:** auto-detecta, o forzar via `speech.stt_language` en config (`"es"`, `"en"`, `"auto"`)
- El modelo se carga de forma perezosa (solo cuando se usa STT)

### Descarga automatica de modelos
`model_downloader.rs` descarga los modelos que falten (whisper, piper) al iniciar, con progreso y verificacion sha256. La primera ejecucion puede tardar unos minutos en descargar ~200MB.

### Wake word ("hey jarvis", ML)
La deteccion usa el modelo openWakeWord `hey_jarvis_v0.1.onnx` (~1.3MB) mas `melspectrogram.onnx` y `embedding_model.onnx` (~2.3MB). Se descargan automaticamente a `~/.local/share/kde-assistant/models/wakeword/`.

Requiere `libonnxruntime.so` v1.29.0, que la app descarga y gestiona sola
(~11MB a `~/.local/share/kde-assistant/lib/`). No hay que instalar nada:
si ya tienes `onnxruntime` del sistema lo usa, si no lo descarga.

Di "hey jarvis" para activar la escucha. Umbral configurable en `speech.wake_word_threshold` (default 0.8).

Probar el pipeline completo: `cargo run --bin test_stt` (TTS -> STT round-trip)

## Atajos globales

Se usan via `rdev` (cross-platform, no requiere KGlobalAccel):

| Shortcut | Accion |
|----------|--------|
| `Super+Shift+A` | Mostrar/ocultar ventana |
| `Super+Shift+V` | Push-to-talk (toggle microfono) |
| `Ctrl+Shift+K` | Nueva sesion |

El backend escribe a `~/.cache/kde-assistant/hotkey.state` con timestamp; el QML hace polling cada 300ms via `XMLHttpRequest` y reacciona.

Desactivar con flag: `cargo run -- --no-shortcuts`

## Integracion KDE

- **DBus signals:** `org.kde.assistant.Chat.HotkeyTriggered`, `.ShowWindow`, `.SendMessage`
- **Notificaciones nativas:** via `dbus-send org.freedesktop.Notifications.Notify`
- **Deteccion de tema:** via `dbus-send org.freedesktop.portal.Settings` (color-scheme)
- **Translucidez:** via `Qt.WA_TranslucentBackground` en QML + blur KWin
- **Icono de bandeja:** `assets/icons/kde-assistant.svg` (+ PNG 16–128 generados con `rsvg-convert`); instalar con:
  ```bash
  for s in 16 22 24 32 48 64 128; do
    d=~/.local/share/icons/hicolor/${s}x${s}/apps; mkdir -p "$d"
    rsvg-convert -w $s -h $s assets/icons/kde-assistant.svg -o "$d/kde-assistant.png"
  done
  mkdir -p ~/.local/share/icons/hicolor/scalable/apps
  cp assets/icons/kde-assistant.svg ~/.local/share/icons/hicolor/scalable/apps/
  ```

## Tests

```bash
cargo test --lib           # 23 tests unitarios
./target/debug/valida_qml  # Verifica que el QML carga sin errores
./target/debug/snap_ui     # Captura screenshots para QA visual
./target/debug/test_voice  # Prueba TTS + chimes
./target/debug/test_stt    # Prueba STT (TTS -> whisper round-trip)
```

## Endpoints HTTP (localhost:8765)

La UI QML se comunica con el backend Rust via este servidor HTTP:

| Método | Path | Descripción |
|--------|------|-------------|
| `GET`  | `/api/health`        | Health check (sin auth) |
| `POST` | `/api/chat`          | Streaming SSE (token/tool_call/done/error) |
| `POST` | `/api/chat/complete` | Respuesta JSON completa + `tool_calls` |
| `POST` | `/api/chat/regenerate` | Re-genera última respuesta (SSE) |
| `POST` | `/api/chat/cancel`   | Cancela el agente en curso (`{session_id?}`) |
| `GET`  | `/api/sessions`      | Lista sesiones |
| `POST` | `/api/session`       | Crea sesión (body: `{"title":"..."}`) |
| `PATCH` | `/api/session`      | Renombra sesión (`{"id","title"}`) |
| `GET`  | `/api/messages?session_id=X` | Mensajes de una sesión (con `timestamp`) |
| `POST` | `/api/ai-models` | Lista modelos de la API (`{base_url?, api_key?, provider?}`) |
| `GET`  | `/api/config` / `POST` | Lee/actualiza configuración |
| `GET`  | `/api/voice/stream`  | Push SSE de voz (`state`/`level`) |
| `GET`  | `/api/audio/devices` | Micrófonos disponibles + actual |
| `POST` | `/api/audio/device`  | Guarda micrófono (`{"name"}`) |
| `GET`  | `/api/tools`         | Catálogo tools (permiso + flag) |
| `POST` | `/api/tools/approve` | Resuelve confirmación (`{tool_call_id, approved}`) |
| `GET`  | `/api/tools/audit?limit=N` | Historial de ejecuciones |
| `POST` | `/api/speak` / `/api/speak/stop` | Reproduce/detiene TTS |

## Estado

- **Fase 1:** Scaffold ✓
- **Fase 2:** Backend core (AI, tool executor, agente ReAct) ✓
- **Fase 3:** QML UI estilo Apple (17 componentes) ✓
- **Fase 4:** Tool calling UI + dialogs ✓
- **Fase 5:** Voz (cpal + whisper-rs STT + piper-tts TTS) ✓
- **Fase 6:** KDE Integration (System Tray, rdev hotkeys, DBus) ✓
- **Fase 7:** Pipeline de voz (descarga de modelos + STT + TTS + wake word) ✓
- **Fase 8:** IPC QML ↔ Backend via HTTP local (chat funcional) ✓
- **Fase 9:** Wake word ML openWakeWord ("hey jarvis") ✓
- **Nota:** push-to-talk (press/release) y persistencia de Settings ya implementados en fases previas
