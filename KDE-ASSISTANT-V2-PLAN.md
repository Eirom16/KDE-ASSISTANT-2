# KDE ASSISTANT v2 - Guia de Arquitectura y Desarrollo

## Vision General

KDE Assistant v2 es un asistente de escritorio para KDE Plasma Linux construido en **Rust + Qt 6/QML**. Combina la integracion nativa de KDE con la elegancia del sistema de diseño Apple (Cupertino), capacidades de agente con tool calling (OpenRouter), ejecucion de comandos del sistema, busqueda web, renderizado de imagenes en chat, invocacion por voz estilo Siri (orbe luminoso + chimes + wake word ML), e iconografia GitHub Primer Octicons.

---

## Errores que NO se deben cometer

### 1. Shell Escaping
- **NUNCA** pasar el prompt como argumento de shell con comillas. Cualquier caracter especial (comillas, backticks, `$`, `\n`) rompe el comando.
- **SOLUCION:** Usar `std::process::Command` de Rust que pasa argumentos como array, no como string shell. No hay escaping.

### 2. TTY Dependency
- El CLI de OpenCode (`opencode run`) a veces necesita un TTY para funcionar. Si se ejecuta sin TTY, puede colgar indefinidamente.
- **SOLUCION:** Usar una API HTTP estandar compatible con OpenAI (OpenRouter: `https://openrouter.ai/api/v1/chat/completions`). No depender del CLI.

### 3. Streaming
- **NUNCA** usar `exec()` o `Command::output()` que bufferiza toda la salida. El usuario ve "pensando" y nunca recibe tokens parciales.
- **SOLUCION:** Usar `BufReader` sobre `response.bytes_stream()` de reqwest y procesar lineas JSON SSE una por una. Cada token se envia al UI inmediatamente via signals QML.

### 4. Bloqueo del UI
- **NUNCA** hacer I/O sincrono (archivos, red, base de datos, audio) en el thread principal.
- **SOLUCION:** Todo debe ser async con `tokio`. Las operaciones de UI van en el thread de Qt, las operaciones pesadas en el runtime de tokio. Comunicacion via signals/slots.

### 5. Autenticacion
- **NUNCA** hardcodear API keys en el codigo fuente.
- **SOLUCION:** Leer de `~/.config/kde-assistant/config.json` (campo `ai.apiKey`). Fallback a variable de entorno `OPENROUTER_API_KEY`.

### 6. Seguridad del Renderer
- **NUNCA** dar acceso directo al filesystem o procesos al frontend QML.
- **SOLUCION:** El backend Rust expone metodos `Q_INVOKABLE` que QML puede llamar, pero no tiene acceso directo a archivos. Las herramientas (open_app, create_file, etc.) se ejecutan en Rust con validacion de rutas.

### 7. Sesiones
- **NUNCA** usar archivos JSON individuales por sesion. Se vuelven lentos con muchas sesiones y no permiten busquedas.
- **SOLUCION:** Usar SQLite con tablas `sessions` y `messages`. Una unica base de datos, consultas rapidas, indices.

### 8. Speech
- **NUNCA** depender de Web Speech API (solo funciona en Chrome/Electron).
- **SOLUCION:** Usar `whisper-rs` para STT local y `piper-tts` (motor neural ONNX) para TTS de alta calidad. Chimes generados con `rodio`.

### 9. Wake Word
- **NUNCA** usar detección simple de energia para el wake word (demasiados falsos positivos).
- **SOLUCION:** Usar un modelo ML dedicado (ONNX/openWakeWord) de ~50MB para deteccion precisa de "Hey KDE". Umbral de confianza configurable (default 0.8).

---

## Preparacion del Entorno

### Sistema Operativo
- KDE Plasma Linux (X11 o Wayland)
- Distribuciones recomendadas: Arch, CachyOS, Fedora, openSUSE

### Dependencias del Sistema

#### Arch Linux / CachyOS
```bash
sudo pacman -S \
  rust cargo \
  qt6-base qt6-declarative qt6-multimedia qt6-wayland \
  cmake \
  alsa-lib \
  pkg-config \
  openssl \
  sqlite \
  piper-tts \
  onnxruntime
```

#### Fedora
```bash
sudo dnf install \
  rust cargo \
  qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtmultimedia-devel \
  cmake \
  alsa-lib-devel \
  pkg-config \
  openssl-devel \
  sqlite-devel \
  piper-tts \
  onnxruntime
```

#### Ubuntu / Debian
```bash
sudo apt install \
  rust cargo \
  qt6-base-dev qt6-declarative-dev qt6-multimedia-dev \
  cmake \
  libasound2-dev \
  pkg-config \
  libssl-dev \
  libsqlite3-dev \
  piper-tts \
  libonnxruntime-dev
```

### Herramientas de Desarrollo
```bash
# Rust (via rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verificar versiones
rustc --version    # >= 1.75
cargo --version
qmake6 --version   # Qt 6.x
```

### API de IA (OpenRouter)
```
URL: https://openrouter.ai/api/v1/chat/completions
Formato: OpenAI-compatible (POST JSON)
Auth: Bearer token desde config.json o env OPENROUTER_API_KEY
Streaming: Server-Sent Events (SSE)
Tool Calling: Formato OpenAI standard (tools array + function schema)
Modelos gratuitos: openrouter/z-ai/glm-5.2:free, openrouter/mimo-v2.5-free, etc.
```

---

## Estructura del Proyecto

```
kde-assistant/
├── Cargo.toml
├── build.rs
├── resources.qrc                 # QML + Octicons embebidos
├── .gitignore
├── AGENTS.md
├── assets/
│   ├── octicons/                 # SVGs de GitHub Primer Octicons
│   │   ├── paper-airplane-16.svg
│   │   ├── search-16.svg
│   │   ├── rocket-16.svg
│   │   ├── file-added-16.svg
│   │   ├── pencil-16.svg
│   │   ├── image-16.svg
│   │   ├── gear-16.svg
│   │   ├── plus-16.svg
│   │   ├── sidebar-collapse-16.svg
│   │   ├── sidebar-expand-16.svg
│   │   ├── hubot-16.svg
│   │   ├── unmute-16.svg
│   │   ├── stop-16.svg
│   │   └── sync-16.svg
│   ├── chimes/                   # Sonidos de activacion/procesamiento
│   │   ├── activate.wav
│   │   ├── process.wav
│   │   └── deactivate.wav
│   └── models/                   # Modelos ML (no en git, descarga opcional)
│       └── wake_word.onnx        # Modelo openWakeWord (~50MB)
├── src/
│   ├── main.rs                   # Init Tokio + Qt Engine + QSystemTrayIcon
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── ai_service.rs         # HTTP client OpenRouter + SSE + tool calling
│   │   ├── tool_executor.rs      # Ejecucion segura de herramientas
│   │   ├── session_manager.rs    # SQLite CRUD
│   │   ├── speech_service.rs     # STT (whisper-rs) + TTS (piper-tts neural)
│   │   ├── audio_capture.rs      # cpal input + VAD + nivel de amplitud
│   │   ├── chime_player.rs       # rodio playback de chimes
│   │   ├── hotword.rs            # Wake word detection (ONNX/openWakeWord)
│   │   └── kde_integration.rs    # DBus, tray, atajos globales, KWin
│   └── models/
│       ├── mod.rs
│       ├── message.rs
│       ├── tool_call.rs          # Estructura para tool calls del LLM
│       └── config.rs
├── qml/
│   ├── Main.qml                  # Ventana flotante translucida + KWin blur
│   ├── Theme.qml                 # Tokens Apple Design (colors, radii, typography)
│   ├── Octicon.qml               # Componente reutilizable para SVG Octicons
│   ├── ChatView.qml              # Scrollview + burbujas + pills de tool calls
│   ├── MessageBubble.qml         # Burbuja usuario/asistente con radios Apple
│   ├── ToolCallBadge.qml         # Pill informativa de accion ejecutada
│   ├── ImageCard.qml             # Tarjeta para show_image (con zoom)
│   ├── InputBar.qml              # Floating capsule bar + boton mic circular
│   ├── VoiceOrb.qml              # Orbe luminoso Siri-style con SpringAnimation
│   ├── TypingIndicator.qml       # Tres puntos pulsantes
│   ├── SessionDrawer.qml         # Panel lateral en chips capsula
│   ├── SettingsDialog.qml        # Config (API, modelo, voz, shortcuts, tema)
│   ├── TrayMenu.qml              # Menu del system tray
│   └── WelcomeScreen.qml         # Pantalla de bienvenida
└── tests/
```

---

## Dependencias Rust (Cargo.toml)

```toml
[package]
name = "kde-assistant"
version = "0.1.0"
edition = "2021"

[dependencies]
# Qt
qml = { version = "1.0", features = ["qmlmacros"] }
qt_core = "1.0"
qt_gui = "1.0"
qt_quick = "1.0"

# Async
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"

# HTTP (OpenRouter API)
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Database
rusqlite = { version = "0.31", features = ["bundled"] }

# Speech
whisper-rs = "0.12"           # STT (futuro)
cpal = "0.15"                 # Audio input
# piper-tts se invoca como subproceso del binario del sistema
# Instalar: sudo pacman -S piper-tts
# Modelo: descargar .onnx y .onnx.json a ~/.local/share/kde-assistant/models/piper/
rodio = "0.19"                # Chimes playback + TTS playback

# Wake Word (ML)
ort = "2.0"                   # ONNX Runtime para openWakeWord
ndarray = "0.16"

# Utils
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
anyhow = "1"
thiserror = "1"
log = "0.4"
env_logger = "0.11"
dirs = "5"

[build-dependencies]
qt_build = "1.0"
```

---

## Flujo de Datos

```
QML UI ──Signal──→ Rust Backend ──HTTP+SSE──→ OpenRouter API
   ↑                    │                         │
   │                    │←──── SSE Stream ────────┘
   │                    │
   │                    ├──→ SQLite (Sessions/Messages)
   │                    ├──→ Whisper (STT)
   │                    ├──→ piper-tts (TTS neural)
   │                    ├──→ Rodio (Chimes)
   │                    ├──→ ONNX (Wake Word)
   │                    ├──→ cpal (Audio Capture + VAD)
   │                    └──→ DBus (KDE Tray/Shortcuts)

Bucle de Agente (Tool Calling):
  LLM Response → ¿tool_call? → ToolExecutor → Resultado → Reenvio a LLM → Respuesta Final
```

---

## Sistema de Herramientas (Tool Calling)

El asistente opera como un agente con un bucle ReAct. Las herramientas se envian al LLM en el campo `tools` del request:

### Herramientas Disponibles

1. **`open_app(name: string)`** — Lanza apps via `gtk-launch` / `.desktop` / `xdg-open`. Sin shell.
2. **`create_file(path: string, content: string)`** — Crea archivos en rutas autorizadas.
3. **`edit_file(path: string, mode: "overwrite" | "append", content: string)`** — Edita/anade texto atomico.
4. **`read_file(path: string)`** — Lee archivos para contexto del LLM.
5. **`web_search(query: string)`** — DuckDuckGo Lite o SearXNG con reqwest.
6. **`show_image(source: string, caption?: string)`** — Inyecta imagen (URL o file://) en el chat.

Cada ejecucion muestra una **pill informativa** en el chat con icono Octicon + texto descriptivo.

---

## Convenciones de Codigo

### Rust
- Indentacion: 4 espacios
- Naming: `snake_case` para funciones/variables, `PascalCase` para tipos
- Error handling: `anyhow::Result` para errores generales, `thiserror` para errores custom
- Clippy: `cargo clippy` sin warnings
- Formatting: `cargo fmt`

### QML
- Indentacion: 4 espacios
- Naming: `PascalCase` para tipos, `camelCase` para propiedades
- Signals: prefijo `on` (ej: `onMessageReceived`)
- Properties: declaradas con `property tipo nombre`
- Diseno: Seguir tokens Apple Design (ver apple-DESIGN.md y Theme.qml)

### Testing
```bash
cargo test                    # Unit tests
cargo test -- --nocapture     # Con output
cargo clippy                  # Lint
cargo fmt --check             # Verificar formato
```

### Build y Distribucion
```bash
cargo build --release         # Build optimizado
cargo build --release --target x86_64-unknown-linux-gnu
```

---

## Funcionalidades Requeridas

### Core
- [ ] Chat de texto con streaming SSE (token a token)
- [ ] Tool calling (agente ReAct con bucle de herramientas)
- [ ] Ejecutar apps del sistema (open_app)
- [ ] Crear/editar archivos (create_file, edit_file, read_file)
- [ ] Buscar en web (web_search)
- [ ] Mostrar imagenes en chat (show_image)
- [ ] Sesiones persistentes (SQLite)
- [ ] Panel lateral de sesiones
- [ ] Markdown renderizado (code blocks, bold, italic, listas)
- [ ] Selector de modelos (OpenRouter)
- [ ] Historial de mensajes con scroll

### Voz (Siri-style)
- [ ] Wake word ML "Hey KDE" (ONNX, umbral 0.8)
- [ ] Orbe luminoso con SpringAnimation (gradiente Action Blue → cian)
- [ ] Chimes de activacion/procesamiento/desactivacion (rodio)
- [ ] STT: Whisper local (whisper-rs)
- [ ] TTS: piper-tts (motor neural ONNX, alta calidad)
- [ ] Push-to-talk (Super+Shift+V)
- [ ] Barge-in: detener TTS al hablar o pulsar Escape
- [ ] Indicador visual de escucha (orbe pulsante)

### KDE Integration
- [ ] System tray con menu multifuncion
- [ ] Acciones rapidas en tray (buscar, abrir app, abrir carpeta)
- [ ] Sesiones recientes en tray
- [ ] Global shortcuts (configurables)
- [ ] Notificaciones nativas (DBus)
- [ ] Tema automatico Breeze Dark/Light con paleta Apple
- [ ] Soporte Wayland

### Configuracion
- [ ] API key y base URL (OpenRouter)
- [ ] Modelo de IA seleccionable
- [ ] Voz TTS seleccionable
- [ ] Idioma STT/TTS
- [ ] Wake word y umbral de confianza
- [ ] Atajos de teclado configurables
- [ ] Comportamiento de ventana (siempre arriba, minimize to tray)
- [ ] Chimes on/off

---

## Archivos Importantes

### .gitignore
```gitignore
/target
*.swp
*.swo
*~
.DS_Store
/assets/models/*.bin
/assets/models/*.onnx
/assets/models/*.gguf
*.wav
!/assets/chimes/*.wav
.env
```

### .env.example
```bash
OPENROUTER_API_KEY=         # API key para OpenRouter
KDE_ASSISTANT_MODEL=        # Optional: modelo por defecto (ej: openrouter/z-ai/glm-5.2:free)
KDE_ASSISTANT_LANGUAGE=     # Optional: idioma default (es-ES)
```
