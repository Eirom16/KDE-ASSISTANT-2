---
name: rust-backend
description: Convenciones y patrones para desarrollo Rust en el backend de KDE Assistant v2
metadata:
  audience: developers
  stack: rust
  module: src/backend/
---

## Qué hago

Proporciono convenciones, patrones y reglas específicas para escribir código Rust en el backend de KDE Assistant v2. Incluye manejo de errores, async patterns, y integración con Qt/QML.

## Cuándo usarme

Usa esta skill cuando:
- Erites o modifiques código Rust en `src/backend/`
- Agregues nuevas dependencias en `Cargo.toml`
- Implementes nuevos módulos o funciones
- Refactorices código existente
- Depures errores de compilación o clippy

## Convenciones Rust del Proyecto

### Estructura de módulos
```
src/
├── main.rs              # Init Tokio + Qt Engine + SystemTray
├── lib.rs               # Re-exports (backend + models)
├── backend/
│   ├── mod.rs           # Backend struct + initialization
│   ├── ai_service.rs    # HTTP client + SSE streaming + tool calling
│   ├── tool_executor.rs # Ejecución de herramientas
│   ├── session_manager.rs # SQLite CRUD
│   ├── speech_service.rs  # STT+TTS orchestration
│   ├── stt.rs           # WhisperEngine
│   ├── tts/piper.rs     # Piper TTS engine
│   ├── voice_pipeline.rs  # Orquesta STT -> LLM -> TTS
│   ├── audio_capture.rs   # cpal + VAD
│   ├── chime_player.rs    # rodio WAV playback
│   ├── hotword.rs         # Wake word detection
│   ├── wakeword_ml.rs     # openWakeWord ONNX
│   ├── hotkey_listener.rs # rdev global hotkeys
│   ├── model_downloader.rs # Auto-download ML models
│   ├── http_server.rs     # Axum HTTP server + SSE
│   └── kde_integration.rs # DBus, tray, hotkeys
└── models/
    ├── mod.rs
    ├── config.rs        # Config struct + load/save
    ├── message.rs       # Message enum + StreamEvent
    └── tool_call.rs     # ToolCall, ToolResult, Tool schema
```

### Reglas Críticas

1. **Sin Shell Escaping**
   ```rust
   // CORRECTO
   Command::new("gtk-launch").arg("firefox").spawn()?;
   // INCORRECTO - PROHIBIDO
   Command::new("sh").arg("-c").arg("gtk-launch firefox").spawn()?;
   ```

2. **Streaming Token a Token**
   ```rust
   // CORRECTO: stream + BufReader
   let mut stream = response.bytes_stream();
   while let Some(chunk) = stream.next().await {
       // procesar SSE y enviar cada token via signal
   }
   // INCORRECTO: nunca exec() o Command::output()
   ```

3. **I/O Nunca en Thread de Qt**
   - Todo I/O en Tokio runtime
   - UI solo se actualiza via signals/slots desde Tokio al thread de Qt
   - Usar `Q_INVOKABLE` para exponer métodos Rust a QML

4. **Error Handling**
   ```rust
   // Errores generales
   use anyhow::Result;
   // Errores custom
   #[derive(Error, Debug)]
   enum MyError {
       #[error("Failed to connect: {0}")]
       Connection(String),
   }
   // Nunca unwrap() en producción
   ```

5. **Sin Claves Hardcoded**
   - API key desde config o env var
   - Nunca en código fuente

### Naming Conventions
- Funciones/variables: `snake_case`
- Tipos: `PascalCase`
- Constantes: `SCREAMING_SNAKE_CASE`
- Módulos: `snake_case`

### Dependencias Principales
- **Async:** `tokio` (full), `futures-util`
- **HTTP:** `reqwest` (rustls-tls, streaming, gzip)
- **HTTP Server:** `axum`
- **DB:** `rusqlite` (bundled)
- **Audio:** `cpal`, `rodio`, `hound`
- **STT:** `whisper-rs`
- **ONNX:** `ort` (load-dynamic)
- **Hotkeys:** `rdev`
- **Serde:** `serde` + `serde_json`

### Comandos Útiles
```bash
cargo build                    # Debug
cargo build --release          # Release
cargo test                     # Tests
cargo clippy                   # Linter
cargo fmt                      # Formatear
cargo fmt --check              # Verificar formato
cargo check                    # Verificar compilación
```

### Después de Cada Cambio
1. `cargo fmt` para formatear
2. `cargo clippy` para lint
3. `cargo test` para verificar tests
4. `cargo build` para verificar compilación
