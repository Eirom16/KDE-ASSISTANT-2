//! TTS (Text-to-Speech) engines
//!
//! Wrapper alrededor de motores TTS externos. Por ahora solo piper-tts
//! (motor neural ONNX, alta calidad, ~70MB por modelo).
//!
//! El binario `piper-tts` debe estar instalado en el sistema.
//! En Arch/CachyOS: `sudo pacman -S piper-tts`
//! Los modelos de voz (`.onnx` + `.onnx.json`) se buscan en
//! `~/.local/share/kde-assistant/models/piper/`.

pub mod piper;

pub use piper::{PiperEngine, PiperModelInfo};
