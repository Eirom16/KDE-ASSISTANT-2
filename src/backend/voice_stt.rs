//! Local STT selection; one restricted, supervised worker per voice operation.
use super::{speech_service::whisper_model_path_for, voice_process};
use crate::models::config::SpeechConfig;
use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;

pub enum Selection {
    Whisper { model: PathBuf, language: String },
    Vosk { library: PathBuf, model: PathBuf },
}
impl Selection {
    pub fn from_config(cfg: &SpeechConfig) -> Result<Self> {
        match cfg.stt_engine.as_str() {
            "whisper" => Ok(Self::Whisper {
                model: whisper_model_path_for(&cfg.stt_model)?,
                language: cfg.stt_language.clone(),
            }),
            "vosk" => {
                let language = cfg.stt_language.to_ascii_lowercase();
                anyhow::ensure!(
                    language == "es" || language.starts_with("es-"),
                    "Vosk small español requiere idioma es; no admite autodetección"
                );
                let library = PathBuf::from(&cfg.vosk_library);
                let model = PathBuf::from(&cfg.vosk_model);
                anyhow::ensure!(
                    cfg.vosk_library.len() <= 4096
                        && cfg.vosk_model.len() <= 4096
                        && library.is_absolute()
                        && model.is_absolute(),
                    "Configura rutas absolutas de libvosk y modelo español local"
                );
                Ok(Self::Vosk { library, model })
            }
            _ => anyhow::bail!("Motor STT local desconocido; selecciona Whisper o Vosk"),
        }
    }
    pub fn paths_present(&self) -> bool {
        match self {
            Self::Whisper { model, .. } => model.is_file(),
            Self::Vosk { library, model } => {
                library.is_file()
                    && model.is_dir()
                    && model.join("am/final.mdl").is_file()
                    && model.join("graph/HCLr.fst").is_file()
            }
        }
    }
    pub fn command(&self, check: bool) -> Result<Command> {
        let mut command = voice_process::command(std::env::current_exe()?);
        match self {
            Self::Whisper { model, language } => {
                if check {
                    command.arg("--voice-model-check").arg(model);
                } else {
                    command.arg("--voice-stt-worker").arg(model).arg(language);
                }
            }
            Self::Vosk { library, model } => {
                command
                    .arg(if check {
                        "--voice-vosk-check"
                    } else {
                        "--voice-vosk-worker"
                    })
                    .arg(library)
                    .arg(model)
                    .env("OPENBLAS_NUM_THREADS", "1")
                    .env("OMP_NUM_THREADS", "1");
            }
        }
        Ok(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_remains_whisper_and_unknown_engine_does_not_fallback() {
        let mut cfg = crate::models::Config::default().speech;
        assert!(matches!(
            Selection::from_config(&cfg).unwrap(),
            Selection::Whisper { .. }
        ));
        cfg.stt_engine = "remote".into();
        assert!(Selection::from_config(&cfg).is_err());
        cfg.stt_engine = "vosk".into();
        assert!(Selection::from_config(&cfg).is_err());
        cfg.vosk_library = "/nonexistent/libvosk.so".into();
        cfg.vosk_model = "/nonexistent/model".into();
        assert!(!Selection::from_config(&cfg).unwrap().paths_present());
        cfg.stt_language = "auto".into();
        assert!(Selection::from_config(&cfg).is_err());
    }
}
