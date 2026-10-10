//! Publicación atómica de bootstrap privado; los errores nunca se ignoran.
use anyhow::{bail, Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.uid() != rustix::process::getuid().as_raw() {
        bail!("Directorio privado inválido: {}", path.display());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub fn read_private(path: &Path) -> Result<String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.nlink() != 1 || meta.uid() != rustix::process::getuid().as_raw() {
        bail!("Archivo privado inválido: {}", path.display());
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(text)
}

/// `create_only` publica sin reemplazar una configuración/token concurrente.
pub fn write_private(path: &Path, content: &str, create_only: bool) -> Result<()> {
    let parent = path.parent().context("archivo sin directorio")?;
    private_dir(parent)?;
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.is_file() || meta.nlink() != 1 || meta.uid() != rustix::process::getuid().as_raw()
        {
            bail!("Destino privado inválido: {}", path.display());
        }
    }
    let temp = parent.join(format!(".bootstrap-{}", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        if create_only {
            fs::hard_link(&temp, path)?;
            fs::remove_file(&temp)?;
        } else {
            fs::rename(&temp, path)?;
        }
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}

/// Sólo la colisión de publicación exclusiva permite releer al ganador.
/// Un error de escritura/sync sigue siendo fatal aunque el destino ya exista.
pub fn allow_creation_race(result: Result<()>) -> Result<()> {
    match result {
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::AlreadyExists) =>
        {
            Ok(())
        }
        other => other,
    }
}
