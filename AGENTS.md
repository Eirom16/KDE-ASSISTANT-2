# AGENTS.md — KDE Assistant v2

> Contexto operativo y reglas para cualquier modelo AI/agente que trabaje en este proyecto.

## Project

**KDE Assistant v2** es un asistente de escritorio de IA para **KDE Plasma Linux**, construido en **Rust** (backend) + **Qt 6 / QML** (UI), con estetica **Apple Design (Cupertino)**, capacidades de **agente con tool calling** (OpenRouter API), **invocacion por voz estilo Siri** (orbe luminoso, chimes, wake word ML) y **system tray multifuncion** en KDE Plasma.

## Stack

- **Backend:** Rust 1.75+ con Tokio async
- **UI:** Qt 6.11+ / QML con QtQuick Controls 2
- **API LLM:** OpenRouter (`https://openrouter.ai/api/v1/chat/completions`) — compatible con OpenAI, tool calling y SSE streaming
- **STT:** whisper-rs (local, modelo ggml-base.bin)
- **TTS:** piper-tts (motor neural) (local)
- **Chimes:** rodio (WAVs embebidos)
- **Wake Word:** ONNX / openWakeWord (~50MB, deteccion ML)
- **Audio capture:** cpal + WebRTC VAD
- **DB:** SQLite (rusqlite bundled)
- **HTTP:** reqwest con streaming SSE
- **Iconos:** GitHub Primer Octicons (SVG embebidos en .qrc)
- **Escritorio:** DBus `org.kde.assistant`, QSystemTrayIcon, rdev global hotkeys, KWin blur

## Documentos de Referencia

1. **`KDE-ASSISTANT-V2-PLAN.md`** — Arquitectura, reglas de ingenieria, setup del entorno, estructura del proyecto, dependencias Rust, esquema SQLite, fases de implementacion.
2. **`KDE-ASSISTANT-V2-PROMPT.md`** — Especificacion funcional completa, diseno de UI detallado, comportamiento del asistente, configuracion, pipeline de voz, bucle de tool calling.
3. **`apple-DESIGN.md`** — Sistema de diseno Cupertino (tokens de color, tipografia, espaciado, radios, componentes).

**Lee estos 3 archivos antes de hacer cualquier cambio significativo.**

## Commands

```bash
# Build
cargo build                      # Debug
cargo build --release            # Release optimizado

# Test
cargo test                       # Unit tests
cargo test -- --nocapture        # Con output

# Lint y formato
cargo clippy                     # Linter
cargo fmt                        # Formatear
cargo fmt --check                # Verificar formato sin modificar

# Qt/QML
qmake6 --version                 # Verificar Qt 6
ls assets/octicons/              # Verificar Octicons descargados

# Octicons
ls assets/octicons/*.svg | wc -l # Contar iconos disponibles

# Ejecutar
cargo run                        # Debug
cargo run --release              # Release

# Git
git status
git add -A
git commit -m "..."
```

## Reglas Criticas (NO ROMPER)

### 1. Sin Shell Escaping
```rust
// CORRECTO: array de argumentos
Command::new("gtk-launch").arg("firefox").spawn()?;

// INCORRECTO: nunca usar sh -c con string
Command::new("sh").arg("-c").arg("gtk-launch firefox").spawn()?;  // PROHIBIDO
```

### 2. Sin Dependencia TTY
- Usar siempre la API HTTP de OpenRouter directamente, no el CLI de opencode.
- No ejecutar `opencode run` en subprocess.

### 3. Streaming Token a Token (NO bufferizar)
```rust
// CORRECTO: stream + BufReader
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    // procesar SSE y enviar cada token via signal
}

// INCORRECTO: nunca exec() o Command::output()
```

### 4. I/O Nunca en Thread de Qt
- Todo I/O (HTTP, SQLite, audio, archivos) en Tokio runtime.
- UI solo se actualiza via signals/slots desde Tokio al thread de Qt.
- Usar `Q_INVOKABLE` para exponer metodos Rust a QML, nunca acceso directo.

### 5. Sin Claves Hardcoded
- API key desde `~/.config/kde-assistant/config.json` o `OPENROUTER_API_KEY` env.
- Nunca escribir API keys en el codigo fuente.

### 6. SQLite, no JSON
- Sesiones y mensajes en SQLite unico con indices.
- No usar archivos JSON individuales por sesion.

### 7. Voz Local
- STT: `whisper-rs` local, no Web Speech API.
- TTS: `piper-tts` local (motor neural ONNX).
- Chimes: WAVs embebidos + `rodio`.

### 8. Wake Word ML
- Usar ONNX/openWakeWord, no deteccion de energia simple.
- Umbral por defecto 0.8, configurable.

### 9. Tool Calling
- Usar formato OpenAI standard (`tools` array + `function` schema).
- Bucle ReAct: LLM responde con `tool_call` -> ejecutar -> reenviar resultado -> continuar.
- Max 8 iteraciones del bucle (prevenir loops infinitos).

### 10. Iconos Siempre desde Octicons
- NUNCA usar iconos Breeze o SF Symbols.
- Cargar SVGs desde `assets/octicons/` via `Octicon.qml`.
- Mapear nombres logicos a archivos SVG en `Octicon.qml`.

## Convenciones de Codigo

### Rust
- Indentacion: **4 espacios**
- Naming: `snake_case` para funciones/variables, `PascalCase` para tipos, `SCREAMING_SNAKE_CASE` para constantes
- Error handling: `anyhow::Result` para errores generales, `thiserror` para errores custom con `#[derive(Error)]`
- No usar `unwrap()` en produccion, solo en tests
- Comentarios: solo cuando el codigo no es obvio, preferentemente explicar el "por que" no el "que"
- `cargo clippy` debe pasar sin warnings
- `cargo fmt` antes de cada commit

### QML
- Indentacion: **4 espacios**
- Naming: `PascalCase` para tipos, `camelCase` para propiedades y signals
- Signals: prefijo `on` (ej: `onMessageReceived`, `onTokenReceived`)
- Properties: `property tipo nombre` con tipo explicito
- Components: una sola responsabilidad, reutilizables
- Usar tokens de `Theme.qml`, nunca valores hardcoded

### Git Commits
- Mensajes en espanol, concisos
- Prefijos: `feat:`, `fix:`, `refactor:`, `docs:`, `style:`, `test:`, `chore:`
- Ejemplo: `feat: agregar tool call badge al MessageBubble`

## Arquitectura de Modulos

```
src/
├── main.rs                   # Init Tokio + Qt Engine + SystemTray
├── backend/
│   ├── mod.rs                # Re-exports y setup
│   ├── ai_service.rs         # HTTP client OpenRouter + SSE + tool calling
│   ├── tool_executor.rs      # Ejecucion segura de herramientas
│   ├── session_manager.rs    # SQLite CRUD (sessions, messages)
│   ├── speech_service.rs     # STT (whisper-rs) + TTS (piper-tts)
│   ├── stt.rs                # WhisperEngine (lazy load ggml-base.bin)
│   ├── voice_pipeline.rs     # Orquesta STT -> LLM -> TTS + buffer grabacion
│   ├── model_downloader.rs   # Descarga automatica de modelos (whisper/piper/oww)
│   ├── wakeword_ml.rs        # openWakeWord ONNX (hey jarvis) via ort load-dynamic
│   ├── audio_capture.rs      # cpal input + VAD + nivel de amplitud
│   ├── chime_player.rs       # rodio playback de chimes
│   ├── hotword.rs            # Wake word ML (con fallback heuristico energia+ZCR)
│   ├── hotkey_listener.rs    # Atajos globales via rdev (PTT press/release)
│   ├── http_server.rs        # API HTTP local para la UI QML (chat/sessions/config)
│   └── kde_integration.rs    # DBus, tray, atajos, KWin
└── models/
    ├── mod.rs                # Re-exports
    ├── message.rs            # Struct Message (role, content, tool_call_id, image_url)
    ├── tool_call.rs          # Struct ToolCall + Tool schema
    └── config.rs             # Struct Config + load/save
```

## Sistema de Diseno (Resumen Rapido)

| Token | Valor Dark | Valor Light | Uso |
|---|---|---|---|
| `canvas` | `#1d1d1f` | `#f5f5f7` | Fondo ventana |
| `surface` | `#2a2a2c` | `#ffffff` | Burbujas asistente, cards |
| `primary` | `#2997ff` | `#0066cc` | Burbuja usuario, botones, acentos |
| `ink` | `#ffffff` | `#1d1d1f` | Texto principal |
| `inkMuted` | `#7a7a7a` | `#6d6f72` | Texto secundario |
| `hairline` | `#3a3a3c` | `#e0e0e0` | Bordes 1px |
| `radiusLg` | 18px | 18px | Cards, burbujas |
| `radiusPill` | 9999px | 9999px | InputBar, botones primarios |
| `error` | `#ff453a` | `#ff3b30` | Errores |
| `success` | `#30d158` | `#34c759` | Exito |

Tipografia: **Inter** (sustituto SF Pro), tracking negativo en titulares, line-height holgado en body.

Ver `apple-DESIGN.md` para tokens completos.

## Bucle de Agente (Tool Calling)

```rust
// Pseudo-codigo
loop {
    let response = ai_service.chat(messages, tools).await?;
    
    if let Some(tool_call) = response.tool_call {
        // Mostrar ToolCallBadge en UI
        ui::show_tool_call(tool_call.clone());
        
        // Ejecutar herramienta
        let result = tool_executor.execute(&tool_call).await?;
        
        // Mostrar resultado
        ui::show_tool_result(&result);
        
        // Agregar al historial y continuar
        messages.push(Message::tool(tool_call.id, result));
        
        if iteration >= max_iterations { break; }
        continue;
    }
    
    // Respuesta final
    return Ok(response.content);
}
```

## Herramientas Expuestas al LLM

| Nombre | Descripcion | Icono |
|---|---|---|
| `open_app(name)` | Lanza app del sistema | `rocket-16.svg` |
| `create_file(path, content)` | Crea archivo | `file-added-16.svg` |
| `edit_file(path, mode, content)` | Edita/append | `pencil-16.svg` |
| `read_file(path)` | Lee archivo | `file-16.svg` |
| `web_search(query)` | Busca en web | `search-16.svg` |
| `show_image(source, caption?)` | Inyecta imagen en chat | `image-16.svg` |

Todas se definen en `src/backend/tool_executor.rs` con sus schemas JSON.

## Comandos Utiles para Agentes

```bash
# Ver estructura del proyecto
ls -R src/ qml/ assets/

# Buscar TODOs
grep -rn "TODO\|FIXME\|XXX" src/ qml/

# Verificar que compila
cargo check

# Ver warnings
cargo build 2>&1 | grep warning

# Contar lineas
wc -l src/**/*.rs qml/*.qml

# Buscar archivos
find . -name "*.rs" -path "*/src/*"
find . -name "*.qml"
```

## Testing

```bash
cargo test                                    # Todos los tests
cargo test backend::ai_service                # Modulo especifico
cargo test -- --nocapture --test-threads=1    # Con output, secuencial
```

## Distribucion

```bash
# Build release
cargo build --release

# El binario estara en target/release/kde-assistant

# Crear .desktop file para integracion KDE
# ~/.local/share/applications/kde-assistant.desktop
```

## Troubleshooting Comun

- **"Qt platform plugin could not be initialized"** — Instalar `qt6-wayland` o variables `QT_QPA_PLATFORM=wayland`/`xcb`.
- **"libonnxruntime not found"** — La app lo descarga sola (~11MB a `~/.local/share/kde-assistant/lib/`). Si falla la descarga, instalar paquete `onnxruntime` del sistema o definir `ORT_DYLIB_PATH`.
- **Whisper model no descargado** — Descarga manual desde HuggingFace o esperar primer arranque (descarga automatica).
- **Wake Word no detecta** — Verificar que los 3 modelos ONNX existen en `~/.local/share/kde-assistant/models/wakeword/` (hey_jarvis_v0.1.onnx, melspectrogram.onnx, embedding_model.onnx). Verificar `libonnxruntime.so` (la app lo descarga sola (~11MB a `~/.local/share/kde-assistant/lib/`). Si falla la descarga, instalar paquete `onnxruntime` del sistema o definir `ORT_DYLIB_PATH`). Verificar umbral `wake_word_threshold` (default 0.5, recomendado por openWakeWord; subir si hay falsos positivos).
- **Tool call no ejecuta** — Verificar que `enableToolCalling: true` en config y que la API key tiene acceso a modelos con tool calling.
- **TTS suena robotico** — Asegurarse de que `piper-tts` esta instalado (no espeak-ng). En Arch: `sudo pacman -S piper-tts`. Descargar un modelo neural de https://huggingface.co/rhasspy/piper-voices (ej. `es_ES-sharvard-medium`) a `~/.local/share/kde-assistant/models/piper/`.
- **"piper-tts no encontrado"** — Verificar `which piper-tts`. Si no esta, instalar con el gestor de paquetes o descargar binario desde https://github.com/rhasspy/piper/releases.
- **"no hay modelos piper"** — El motor esta pero sin voces. Descargar archivos `.onnx` + `.onnx.json` de HuggingFace a `~/.local/share/kde-assistant/models/piper/`.

## Estado Actual del Proyecto

Ver `git log --oneline` para historial y `KDE-ASSISTANT-V2-PLAN.md` seccion "Pasos de Implementacion" para fases.

## Filosofia

- **Privacidad primero:** Voz y archivos locales, solo el prompt va a la nube.
- **Rendimiento:** Rust + Tokio, nunca bloquear UI.
- **Estetica Apple:** Minimalismo, frosted glass, un solo color interactivo, tracking negativo.
- **Open source friendly:** MIT/Apache, sin dependencias proprietarias, Octicons (MIT), Whisper (MIT), piper-tts (Apache-2.0).
- **KDE nativo:** DBus, KGlobalAccel, KWin blur, Breeze detection.

## Contacto y Contexto

- **Repositorio Git:** local (`git init` en raiz del proyecto)
- **Documentacion detallada:** los 3 archivos .md en raiz
- **Para preguntas:** consultar primero PLAN.md, PROMPT.md, apple-DESIGN.md, luego este AGENTS.md
