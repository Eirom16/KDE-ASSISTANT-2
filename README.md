# KDE Assistant v2

Asistente de escritorio para KDE Plasma Linux con estetica Apple Design (Cupertino), capacidades de agente con tool calling, invocacion por voz estilo Siri y system tray multifuncion.

## Stack

- **Backend:** Rust + Tokio
- **UI:** Qt 6 / QML (lanzada via `qml6`)
- **AI:** OpenRouter API (compatible con OpenAI, tool calling, SSE streaming)
- **Voz:** whisper-rs (STT) + espeak-ng/piper-tts (TTS) + ONNX openWakeWord
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
sudo pacman -S rust cargo qt6-declarative cmake alsa-lib espeak-ng
```

Para los modelos ML (Fase 5+):
```bash
sudo pacman -S onnxruntime
```

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
    "base_url": "https://openrouter.ai/api/v1",
    "api_key": "<tu-api-key>",
    "model": "openrouter/z-ai/glm-5.2:free",
    "enable_tool_calling": true
  }
}
```

Tambien puedes usar la variable de entorno `OPENROUTER_API_KEY`.

## Tests

```bash
cargo test --lib           # 9 tests unitarios
./target/debug/valida_qml  # Verifica que el QML carga sin errores
./target/debug/snap_ui     # Captura screenshots para QA visual
```

## Estado

- **Fase 1:** Scaffold ✓
- **Fase 2:** Backend core (AI, tool executor, agente ReAct) ✓
- **Fase 3:** QML UI estilo Apple (17 componentes) ✓
- **Fase 4:** Tool calling UI + dialogs (ErrorBanner, ImagePreview, Settings) ✓
- **Fase 5:** Voz Siri (cpal + whisper-rs + espeak-ng + ONNX hotword) — *pendiente*
- **Fase 6:** KDE Integration (DBus, System Tray, KGlobalAccel) — *pendiente*
- **Fase 7:** Polish final — *pendiente*
