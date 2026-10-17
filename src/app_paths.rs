use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

const DATA_DIR_ENV: &str = "KDE_ASSISTANT_DATA_DIR";

pub fn validate_resources(root: &Path) -> Result<()> {
    for resource in [
        "qml/Main.qml",
        "qml/qmldir",
        "qml/Theme.qml",
        "qml/Octicon.qml",
        "qml/components/TrayMenu.qml",
        "assets/icons/kde-assistant.svg",
        "assets/octicons/plus-16.svg",
    ] {
        anyhow::ensure!(
            root.join(resource).is_file(),
            "Falta recurso requerido: {resource}; revisa KDE_ASSISTANT_DATA_DIR o la instalación"
        );
    }
    Ok(())
}

/// Localiza los recursos QML tanto en desarrollo como en una instalación FHS.
pub fn data_dir() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("resolviendo la ruta del ejecutable")?;
    let explicit = std::env::var_os(DATA_DIR_ENV).map(PathBuf::from);

    resolve_data_dir_from(
        &executable,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new("/"),
        explicit.as_deref(),
    )
}

/// Resuelve el directorio que contiene `qml/Main.qml` por prioridad estable.
pub fn resolve_data_dir_from(
    executable: &Path,
    manifest_dir: &Path,
    _current_dir: &Path,
    explicit: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(explicit) = explicit {
        anyhow::ensure!(
            explicit.is_absolute(),
            "{DATA_DIR_ENV} debe ser una ruta absoluta"
        );
        anyhow::ensure!(
            explicit.join("qml/Main.qml").is_file(),
            "Faltan recursos: {DATA_DIR_ENV}/qml/Main.qml"
        );
        return explicit
            .canonicalize()
            .context("resolviendo recursos explícitos");
    }
    let installed = executable
        .parent()
        .and_then(Path::parent)
        .map(|prefix| prefix.join("share/kde-assistant"));

    let candidates = installed.into_iter().chain([manifest_dir.to_path_buf()]);

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
