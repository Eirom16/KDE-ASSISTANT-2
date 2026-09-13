---
name: voice-pipeline
description: Pipeline de voz: STT (whisper-rs), TTS (piper-tts), wake word (openWakeWord), chimes (rodio), y audio capture (cpal)
metadata:
  audience: developers
  module: src/backend/
  audio: true
---

## Qué hago

Proporciono guías y patrones para el pipeline de voz completo de KDE Assistant: captura de audio, detección de wake word, speech-to-text, text-to-text, y chimes sonoros.

## Cuándo usarme

Usa esta skill cuando:
- Modifiques el pipeline de voz (`voice_pipeline.rs`)
- Trabajes con whisper-rs (STT) o piper-tts (TTS)
- Implementes o depures wake word detection
- Agregues o modifiques chimes sonoros
- Depures problemas de audio (captura, reproducción)
- Configures el AudioCapture o ChimePlayer

## Arquitectura del Pipeline

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  AudioCapture│────>│  WakeWord    │────>│  VoiceOrb   │
│  (cpal + VAD)│     │  (ONNX/ml)  │     │  (QML)      │
└─────────────┘     └──────────────┘     └─────────────┘
                           │
                           ▼
                    ┌──────────────┐     ┌─────────────┐
                    │  STT         │────>│  LLM        │
                    │  (whisper-rs)│     │  (OpenRouter)│
                    └──────────────┘     └─────────────┘
                                               │
                                               ▼
                                        ┌─────────────┐
                                        │  TTS        │
                                        │  (piper-tts)│
                                        └─────────────┘
                                               │
                                               ▼
                                        ┌─────────────┐
                                        │  ChimePlayer│
                                        │  (rodio)    │
                                        └─────────────┘
```

## Componentes Principales

### AudioCapture (`audio_capture.rs`)
```rust
// Captura de micrófono con cpal
// Incluye VAD (Voice Activity Detection) para detectar silencio
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// Configuración típica
// Sample rate: 16000 Hz (whisper necesita 16kHz)
// Channels: 1 (mono)
// Format: f32

// VAD params
// Min speech duration: 500ms
// Min silence duration: 700ms
// Speech threshold: 0.5
```

### Wake Word (`hotword.rs` + `wakeword_ml.rs`)
```rust
// Detección de wake word "Hey Jarvis" usando openWakeWord
// Modelo ONNX cargado via ort (load-dynamic)

// Umbral por defecto: 0.5 (openWakeWord default)
// Configurable en config.json: wake_word_threshold

// Modelos necesarios (~50MB total):
// - hey_jarvis_v0.1.onnx
// - melspectrogram.onnx
// - embedding_model.onnx

// Ubicación: ~/.local/share/kde-assistant/models/wakeword/
```

### STT (`stt.rs`)
```rust
// Speech-to-Text usando whisper-rs (local)
// Modelo: ggml-base.bin (~140MB)
// Idioma: español (configurable)

// WhisperEngine struct
pub struct WhisperEngine {
    model_path: PathBuf,
    language: String,
    // ...
}

// Ejemplo de uso
let stt = WhisperEngine::new(model_path, "es")?;
let text = stt.transcribe(&audio_data)?;
```

### TTS (`tts/piper.rs`)
```rust
// Text-to-Speech usando piper-tts (motor neural ONNX)
// Voces descargadas desde HuggingFace
// Ubicación: ~/.local/share/kde-assistant/models/piper/

// Ejemplo de uso
let tts = PiperEngine::new(model_dir)?;
let audio = tts.synthesize("Hola, ¿cómo estás?")?;

// Voces recomendadas para español:
// - es_ES-sharvard-medium (neural, buena calidad)
// - es_ES-davefx-medium (alternativa)
```

### ChimePlayer (`chime_player.rs`)
```rust
// Reproducción de chimes WAV usando rodio
// WAVs embebidos en el binario via include_bytes!()

// Chimes disponibles:
// - activation.wav    # Al activar por voz
// - deactivation.wav  # Al desactivar
// - error.wav         # Error de reconocimiento
// - success.wav       # Éxito de tool call
// - thinking.wav      # Mientras piensa

// Reproducción no bloqueante
let player = ChimePlayer::new()?;
player.play("activation")?;
```

### VoicePipeline (`voice_pipeline.rs`)
```rust
// Orquestación completa del pipeline
// 1. Escucha micrófono (AudioCapture)
// 2. Detecta wake word (Hotword)
// 3. Muestra VoiceOrb animada
// 4. Graba audio hasta silencio
// 5. Transcribe con STT
// 6. Envía a LLM
// 7. Responde con TTS
// 8. reproduce chime de éxito/error

// Estados del pipeline:
// Idle -> Listening -> Processing -> Speaking -> Idle
```

## Modelos Necesarios

### Descarga Automática
```rust
// model_downloader.rs descarga modelos automáticamente
// en el primer arranque

// Modelos:
// 1. whisper ggml-base.bin (~140MB)
//    -> ~/.local/share/kde-assistant/models/whisper/
// 2. piper voice (~50MB)
//    -> ~/.local/share/kde-assistant/models/piper/
// 3. openWakeWord ONNX (~50MB)
//    -> ~/.local/share/kde-assistant/models/wakeword/
// 4. ONNX Runtime (~11MB)
//    -> ~/.local/share/kde-assistant/lib/
```

### Descarga Manual
```bash
# Whisper
wget -P ~/.local/share/kde-assistant/models/whisper/ \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# Piper (ejemplo español)
# Descargar de https://huggingface.co/rhasspy/piper-voices
# Necesario: archivo.onnx + archivo.onnx.json

# openWakeWord
# Descargar desde https://github.com/dscripka/openWakeWord
```

## Troubleshooting

### Whisper no detecta audio
- Verificar sample rate: 16000 Hz
- Verificar que el modelo está descargado
- Probar con audio grabado previamente

### Piper suena robótico
- Verificar que se usa piper-tts (no espeak-ng)
- Descargar modelo neural de HuggingFace
- Verificar que el modelo .onnx está en la carpeta correcta

### Wake Word no detecta
- Verificar los 3 modelos ONNX existen
- Verificar umbral (default 0.5, subir si hay falsos positivos)
- Probar con "Hey Jarvis" claro y pausado

### Audio no se graba
- Verificar permisos de micrófono
- Verificar que cpal puede acceder al dispositivo
- Probar con `arecord` directamente

### Chimes no suenan
- Verificar que los WAVs están embebidos en el binario
- Verificar que rodio puede reproducir
- Probar con `aplay` directamente
