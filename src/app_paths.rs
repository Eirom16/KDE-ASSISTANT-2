use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

const DATA_DIR_ENV: &str = "KDE_ASSISTANT_DATA_DIR";

/// Localiza los recursos QML tanto en desarrollo como en una instalación FHS.
pub fn data_dir() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("resolviendo la ruta del ejecutable")?;
    let current_dir = std::env::current_dir().context("resolviendo el directorio actual")?;
    let explicit = std::env::var_os(DATA_DIR_ENV).map(PathBuf::from);

    resolve_data_dir_from(
        &executable,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &current_dir,
        explicit.as_deref(),
    )
}

/// Resuelve el directorio que contiene `qml/Main.qml` por prioridad estable.
pub fn resolve_data_dir_from(
    executable: &Path,
    manifest_dir: &Path,
    current_dir: &Path,
    explicit: Option<&Path>,
) -> Result<PathBuf> {
    let installed = executable
        .parent()
        .and_then(Path::parent)
        .map(|prefix| prefix.join("share/kde-assistant"));

    let candidates = explicit
        .into_iter()
        .map(Path::to_path_buf)
        .chain(installed)
        .chain([manifest_dir.to_path_buf(), current_dir.to_path_buf()]);

    let mut checked = Vec::new();
    for candidate in candidates {
        if candidate.join("qml/Main.qml").is_file() {
            return Ok(candidate);
        }
        checked.push(candidate);
    }

    let checked = checked
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "no se encontró qml/Main.qml; instala los recursos o define {DATA_DIR_ENV}. Rutas comprobadas: {checked}"
    )
}
