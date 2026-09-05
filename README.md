# KDE Assistant v2

Asistente de escritorio para KDE Plasma Linux con estetica Apple Design (Cupertino), capacidades de agente con tool calling, invocacion por voz estilo Siri y system tray multifuncion.

## Stack

- **Backend:** Rust + Tokio
- **UI:** Qt 6 / QML
- **AI:** OpenRouter API (compatible con OpenAI, tool calling, SSE streaming)
- **Voz:** whisper-rs (STT) + espeak-ng/piper-tts (TTS) + ONNX openWakeWord
- **DB:** SQLite (rusqlite)
- **Iconos:** GitHub Primer Octicons

## Documentacion

- `KDE-ASSISTANT-V2-PLAN.md` — Arquitectura y reglas de ingenieria
- `KDE-ASSISTANT-V2-PROMPT.md` — Especificacion funcional
- `apple-DESIGN.md` — Sistema de diseno Apple
- `AGENTS.md` — Contexto para modelos AI

## Build

```bash
# Dependencias del sistema (Arch)
sudo pacman -S rust cargo qt6-base qt6-declarative qt6-multimedia cmake alsa-lib espeak-ng onnxruntime

# Compilar
cargo build --release

# Ejecutar
./target/release/kde-assistant
```

## Estado

Fase 1: Scaffold — Estructura base del proyecto.
