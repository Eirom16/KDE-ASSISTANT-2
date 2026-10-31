//! Explicit, verified Piper 1.8.0 CPU runtime. Disposable or optional managed worker.
use super::voice_process;
use crate::models::config::SpeechConfig;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek},
    os::fd::AsRawFd,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::process::Command;

const SITE: &str = "lib/python3.14/site-packages";
const MODEL_HASH: &str = "40febfb1679c69a4505ff311dc136e121e3419a13a290ef264fdf43ddedd0fb1";
const CONFIG_HASH: &str = "7438c9b699c72b0c3388dae1b68d3f364dc66a2150fe554a1c11f03372957b2c";
const WORKER: &str = include_str!("piper_worker.py");
const RESOURCES: &str = include_str!("piper_resources.json");

#[derive(Clone)]
pub struct Selection {
    pub runtime: PathBuf,
    pub model: PathBuf,
    pub scale: f32,
    pub speaker: u32,
}
impl Selection {
    pub fn from_config(cfg: &SpeechConfig) -> Result<Self> {
        anyhow::ensure!(cfg.tts_engine == "piper", "Motor TTS local no compatible");
        let runtime = PathBuf::from(&cfg.piper_runtime);
        anyhow::ensure!(
            cfg.piper_runtime.len() <= 4096 && runtime.is_absolute(),
            "Piper no configurado: indica la ruta absoluta del entorno privado evaluado"
        );
        anyhow::ensure!(
            cfg.piper_length_scale.is_finite() && (0.5..=2.0).contains(&cfg.piper_length_scale),
            "Duración Piper inválida"
        );
        anyhow::ensure!(
            cfg.piper_speaker < 2,
            "Hablante Piper no disponible (0 o 1)"
        );
        let model = if Path::new(&cfg.piper_model).is_absolute() {
            PathBuf::from(&cfg.piper_model)
        } else {
            anyhow::ensure!(
                cfg.piper_model == "es_ES-sharvard-medium",
                "Voz Piper no evaluada"
            );
            dirs::data_local_dir()
                .context("Sin directorio de voces")?
                .join("kde-assistant/models/piper/es_ES-sharvard-medium.onnx")
        };
        anyhow::ensure!(
            cfg.piper_model.len() <= 4096 && model.extension().is_some_and(|ext| ext == "onnx"),
            "Ruta de modelo Piper inválida"
        );
        Ok(Self {
            runtime,
            model,
            scale: cfg.piper_length_scale,
            speaker: cfg.piper_speaker,
        })
    }
    /// Presence only; the disposable worker verifies every hash before execution.
    pub fn paths_present(&self) -> bool {
        self.runtime.join("bin/python").is_file()
            && self.runtime.join(SITE).join("piper/voice.py").is_file()
            && self.model.is_file()
            && self.model.with_extension("onnx.json").is_file()
    }
    pub fn stream_command(&self) -> Result<Command> {
        let mut cmd = voice_process::command(std::env::current_exe()?);
        cmd.arg("--voice-piper-stream")
            .arg(&self.runtime)
            .arg(&self.model)
            .arg(self.scale.to_string())
            .arg(self.speaker.to_string());
        Ok(cmd)
    }
    pub fn managed_command(&self) -> Result<Command> {
        let mut cmd = voice_process::command(std::env::current_exe()?);
        cmd.env_clear()
            .arg("--voice-piper-managed")
            .arg(&self.runtime)
            .arg(&self.model)
            .arg(self.scale.to_string())
            .arg(self.speaker.to_string());
        Ok(cmd)
    }
    pub fn command(&self, check: bool) -> Result<Command> {
        let mut command = voice_process::command(std::env::current_exe()?);
        command
            .arg(if check {
                "--voice-piper-check"
            } else {
                "--voice-piper-worker"
            })
            .arg(&self.runtime)
            .arg(&self.model);
        if !check {
            command
                .arg(self.scale.to_string())
                .arg(self.speaker.to_string());
        }
        Ok(command)
    }
}

fn verified_file(path: &Path, size: u64, expected: &str) -> Result<(File, [i128; 7])> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .context("Recurso Piper ausente o no accesible")?;
    let info = file.metadata()?;
    anyhow::ensure!(
        info.is_file()
            && info.len() == size
            && info.uid() == rustix::process::getuid().as_raw()
            && info.mode() & 0o022 == 0,
        "Recurso Piper incompleto o versión no evaluada"
    );
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut input = (&mut file).take(size + 1);
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    anyhow::ensure!(
        hex::encode(digest.finalize()) == expected,
        "Integridad de recurso Piper inválida"
    );
    let after = file.metadata()?;
    anyhow::ensure!(
        identity(&info) == identity(&after),
        "Recurso Piper cambió durante verificación"
    );
    file.rewind()?;
    Ok((file, identity(&after)))
}
fn identity(meta: &std::fs::Metadata) -> [i128; 7] {
    [
        meta.dev() as i128,
        meta.ino() as i128,
        meta.len() as i128,
        meta.mtime() as i128,
        meta.mtime_nsec() as i128,
        meta.ctime() as i128,
        meta.ctime_nsec() as i128,
    ]
}
fn verify_identity(file: &File, expected: [i128; 7]) -> Result<()> {
    anyhow::ensure!(
        identity(&file.metadata()?) == expected,
        "Recurso Piper cambió después de verificar su hash"
    );
    Ok(())
}
fn verify_file(path: &Path, size: u64, hash: &str) -> Result<()> {
    verified_file(path, size, hash).map(|_| ())
}
struct Verified {
    model: File,
    config: File,
    model_identity: [i128; 7],
    config_identity: [i128; 7],
    metrics: serde_json::Value,
}
pub fn validate(runtime: &Path, model: &Path) -> Result<()> {
    validate_open(runtime, model).map(|_| ())
}
fn validate_open(runtime: &Path, model: &Path) -> Result<Verified> {
    let started = Instant::now();
    anyhow::ensure!(
        runtime.is_absolute()
            && model.is_absolute()
            && model.extension().is_some_and(|ext| ext == "onnx"),
        "Rutas Piper absolutas y modelo .onnx requeridos"
    );
    let meta = std::fs::symlink_metadata(runtime).context("Entorno Piper no instalado")?;
    anyhow::ensure!(
        meta.is_dir()
            && meta.uid() == rustix::process::getuid().as_raw()
            && meta.mode() & 0o077 == 0,
        "Entorno Piper debe ser un directorio privado propio (0700)"
    );
    let python = runtime
        .join("bin/python")
        .canonicalize()
        .context("Python privado de Piper ausente")?;
    anyhow::ensure!(
        python == Path::new("/usr/bin/python3").canonicalize()?,
        "Intérprete Piper no evaluado"
    );
    let hashing = Instant::now();
    let (model_file, model_identity) = verified_file(model, 76_733_615, MODEL_HASH)?;
    let config = model.with_extension("onnx.json");
    let (config_file, config_identity) = verified_file(&config, 4903, CONFIG_HASH)?;
    let model_hash_ms = hashing.elapsed().as_secs_f64() * 1000.0;
    let metadata = super::voice_coordinator::read_voice_metadata(&config)?;
    anyhow::ensure!(
        super::voice_coordinator::piper_audio_options(&metadata, 0)? == 22050,
        "Formato Piper no evaluado"
    );
    let runtime_hashing = Instant::now();
    let resources: Vec<(String, u64, String)> = serde_json::from_str(RESOURCES)?;
    for (path, size, hash) in resources {
        verify_file(&runtime.join(SITE).join(path), size, &hash)?;
    }
    Ok(Verified {
        model: model_file,
        config: config_file,
        model_identity,
        config_identity,
        metrics: serde_json::json!({
            "validation_ms": started.elapsed().as_secs_f64() * 1000.0,
            "model_config_hash_ms": model_hash_ms,
            "runtime_hash_ms": runtime_hashing.elapsed().as_secs_f64() * 1000.0,
        }),
    })
}

/// Replace the restricted Rust child: Python retains the same owned PID and pipes.
/// Validation/loading/inference all remain killable by the coordinator.
pub fn worker(runtime: &Path, model: &Path, scale: f32, speaker: u32) -> Result<()> {
    anyhow::ensure!(
        scale.is_finite() && (0.5..=2.0).contains(&scale),
        "Duración Piper inválida"
    );
    anyhow::ensure!(speaker < 2, "Hablante Piper no disponible");
    launch(runtime, model, scale, speaker, "raw")
}
pub fn stream_worker(runtime: &Path, model: &Path, scale: f32, speaker: u32) -> Result<()> {
    anyhow::ensure!(
        scale.is_finite() && (0.5..=2.0).contains(&scale) && speaker < 2,
        "Opciones Piper inválidas"
    );
    let result = launch(runtime, model, scale, speaker, "framed");
    if let Err(ref error) = result {
        use std::io::Write;
        let message = serde_json::to_vec(&serde_json::json!({"error": error.to_string()}))?;
        let mut output = std::io::stdout().lock();
        output.write_all(b"KDA4PCM1")?;
        output.write_all(&(message.len() as u32).to_le_bytes())?;
        output.write_all(&message)?;
    }
    result
}
/// Offline experiment only. Two requests and fixed 3s idle TTL; no application setting.
pub fn warm_experiment(runtime: &Path, model: &Path, scale: f32, speaker: u32) -> Result<()> {
    anyhow::ensure!(
        scale.is_finite() && (0.5..=2.0).contains(&scale) && speaker < 2,
        "Opciones Piper inválidas"
    );
    launch(runtime, model, scale, speaker, "warm-experiment")
}
pub fn managed_worker(runtime: &Path, model: &Path, scale: f32, speaker: u32) -> Result<()> {
    anyhow::ensure!(
        scale.is_finite() && (0.5..=2.0).contains(&scale) && speaker < 2,
        "Opciones Piper inválidas"
    );
    launch(runtime, model, scale, speaker, "managed-v1")
}
/// Expected installed resource paths, for revocation watches (never a validity cache).
pub(crate) fn resource_paths(selected: &Selection) -> Result<Vec<PathBuf>> {
    let resources: Vec<(String, u64, String)> = serde_json::from_str(RESOURCES)?;
    let mut paths: Vec<_> = resources
        .into_iter()
        .map(|(path, _, _)| selected.runtime.join(SITE).join(path))
        .collect();
    paths.extend([
        selected.model.clone(),
        selected.model.with_extension("onnx.json"),
        selected.runtime.join("bin/python"),
        std::env::current_exe()?,
        PathBuf::from("/usr/bin/python3"),
        Path::new("/usr/bin/python3").canonicalize()?,
    ]);
    Ok(paths)
}
fn launch(runtime: &Path, model: &Path, scale: f32, speaker: u32, mode: &str) -> Result<()> {
    let verified = validate_open(runtime, model)?;
    // Compare against the identity at hashing time, never re-baseline a changed inode.
    verify_identity(&verified.model, verified.model_identity)?;
    verify_identity(&verified.config, verified.config_identity)?;
    // Pin the verified model/config inodes across exec, rather than reopening their paths.
    for fd in [&verified.model, &verified.config] {
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::empty())?;
    }
    let diagnostics = serde_json::json!({
        "native": verified.metrics,
        "exec_wall_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs_f64()*1000.0,
        "model_identity": verified.model_identity,
        "config_identity": verified.config_identity,
    });
    // Start the trusted system interpreter directly: even an altered pyvenv.cfg
    // must not redirect standard-library discovery before our verified imports.
    // Private packages are added explicitly under -S, without system/user site.
    let mut command = std::process::Command::new(Path::new("/usr/bin/python3").canonicalize()?);
    command
        .env_clear()
        .args([
            "-I",
            "-B",
            "-S",
            "-X",
            "pycache_prefix=/dev/null",
            "-c",
            WORKER,
        ])
        .arg(runtime.join(SITE))
        .arg(format!("/proc/self/fd/{}", verified.model.as_raw_fd()))
        .arg(scale.to_string())
        .arg(speaker.to_string())
        .arg("3")
        .arg("all")
        .arg(mode)
        .arg(format!("/proc/self/fd/{}", verified.config.as_raw_fd()))
        .arg(serde_json::to_string(&diagnostics)?)
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("OMP_NUM_THREADS", "1")
        .current_dir(runtime);
    // Do not inherit the desktop socket, HOME, bearer, keys, PYTHONPATH or user site.
    // -S also prevents executable .pth files in the runtime from being evaluated.
    Err(command.exec()).context("No se pudo iniciar Piper privado")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_runtime_and_validated_options_are_required() {
        let mut cfg = crate::models::Config::first_run().speech;
        assert!(Selection::from_config(&cfg).is_err());
        cfg.piper_runtime = "/nonexistent/private-venv".into();
        assert!(!Selection::from_config(&cfg).unwrap().paths_present());
        cfg.piper_speaker = 2;
        assert!(Selection::from_config(&cfg).is_err());
        cfg.piper_speaker = 0;
        cfg.piper_length_scale = f32::NAN;
        assert!(Selection::from_config(&cfg).is_err());
        cfg.piper_length_scale = 1.0;
        cfg.piper_model = "../unverified".into();
        assert!(Selection::from_config(&cfg).is_err());
    }
    #[test]
    fn verified_inode_is_retained_and_metadata_change_invalidates_identity() {
        let directory = std::env::temp_dir().join(format!("f4-piper-pin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("model");
        std::fs::write(&path, b"data").unwrap();
        let hash = hex::encode(Sha256::digest(b"data"));
        let (mut pinned, before) = verified_file(&path, 4, &hash).unwrap();
        std::fs::rename(&path, directory.join("old")).unwrap();
        std::fs::write(&path, b"oops").unwrap();
        let mut content = String::new();
        pinned.read_to_string(&mut content).unwrap();
        assert_eq!(content, "data");
        assert_ne!(identity(&std::fs::metadata(&path).unwrap()), before);
        assert!(verified_file(&path, 4, &hash).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn modification_after_hash_cannot_become_the_expected_exec_identity() {
        let directory =
            std::env::temp_dir().join(format!("f4-piper-race-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("model");
        std::fs::write(&path, b"data").unwrap();
        let hash = hex::encode(Sha256::digest(b"data"));
        let (file, expected) = verified_file(&path, 4, &hash).unwrap();
        verify_identity(&file, expected).unwrap();
        std::fs::write(&path, b"changed size").unwrap();
        assert!(verify_identity(&file, expected).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn resource_integrity_rejects_corruption_and_special_files() {
        let directory =
            std::env::temp_dir().join(format!("f4-piper-check-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("fixture");
        std::fs::write(&file, b"data").unwrap();
        let hash = hex::encode(Sha256::digest(b"data"));
        verify_file(&file, 4, &hash).unwrap();
        std::fs::write(&file, b"oops").unwrap();
        assert!(verify_file(&file, 4, &hash)
            .unwrap_err()
            .to_string()
            .contains("Integridad"));
        assert!(verify_file(&file, 5, &hash).is_err());
        assert!(verify_file(Path::new("/dev/null"), 0, &hash).is_err());
        std::os::unix::fs::symlink(&file, directory.join("link")).unwrap();
        assert!(verify_file(&directory.join("link"), 4, &hash).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
