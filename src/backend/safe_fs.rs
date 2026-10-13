//! Acceso Linux anclado a la raíz autorizada. No hay fallback por pathname.
use anyhow::{bail, Context, Result};
use rustix::fs::{self, AtFlags, Mode, OFlags, RenameFlags, ResolveFlags, CWD};
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use crate::models::Config;

const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_MAGICLINKS)
    .union(ResolveFlags::NO_XDEV);

pub(super) struct SafePath {
    root: File,
    relative: PathBuf,
    pub display: PathBuf,
}

impl SafePath {
    pub fn resolve(cfg: &Config, path: &str) -> Result<Self> {
        if !cfg.tools.fs_enabled() {
            bail!("Acceso al sistema de archivos deshabilitado");
        }
        let home = dirs::home_dir().context("HOME no disponible; acceso denegado")?;
        let display = checked_absolute(&super::tool_executor::expand_home(path, &home))?;
        for root in &cfg.tools.allowed_paths {
            let root_path = checked_absolute(&super::tool_executor::expand_home(root, &home))?;
            if let Ok(relative) = display.strip_prefix(&root_path) {
                // La raíz también se abre sin symlinks, incluso en sus ancestros.
                // ENOSYS/EINVAL/EPERM se propagan: nunca open/canonicalize de fallback.
                let root = fs::openat2(
                    CWD,
                    &root_path,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
                )
                .context("abriendo raíz autorizada con openat2")?;
                return Ok(Self {
                    root: root.into(),
                    relative: relative.into(),
                    display,
                });
            }
        }
        bail!("Path no permitido (allowed_paths): {}", display.display())
    }

    pub fn search_root(cfg: &Config, path: &str) -> Result<Option<Self>> {
        let result = (|| -> Result<Self> {
            let resolved = Self::resolve(cfg, path)?;
            // Fijar el directorio completo también cubre raíces solapadas: resolve
            // puede haber abierto una raíz padre, dejando componentes relativos.
            let root = resolved.open(OFlags::RDONLY | OFlags::DIRECTORY)?;
            Ok(Self {
                root,
                relative: PathBuf::new(),
                display: resolved.display,
            })
        })();
        match result {
            Ok(root) => Ok(Some(root)),
            Err(error)
                if error.downcast_ref::<rustix::io::Errno>() == Some(&rustix::io::Errno::NOENT) =>
            {
                Ok(None)
            }
            Err(error) => Err(error)
                .with_context(|| format!("Raíz de búsqueda insegura o inaccesible: {path}")),
        }
    }

    pub fn snapshot(&self) -> Result<PathBuf> {
        let mut source = self.open(OFlags::RDONLY | OFlags::NONBLOCK)?;
        check_regular(&source)?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut source)
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(bytes.len() <= 32 * 1024 * 1024, "cache_resource_limit");
        let (directory, root) = private_workspace()?;
        let pending = super::resource_cache::PendingResource::new(&directory);
        let name = self.relative.file_name().context("archivo sin nombre")?;
        let snapshot = Self {
            root,
            relative: name.into(),
            display: directory.join(name),
        };
        snapshot.create(&bytes)?;
        super::resource_cache::publish_resource(&snapshot.display)?;
        pending.keep();
        Ok(snapshot.display)
    }

    pub fn find(&self, query: &str) -> Result<Vec<PathBuf>> {
        let mut stack = vec![(
            self.open(OFlags::RDONLY | OFlags::DIRECTORY)?,
            self.display.clone(),
            0,
        )];
        let mut results = Vec::new();
        let mut visited = 0;
        while let Some((dir, display, depth)) = stack.pop() {
            if depth > 6 || visited >= 2000 || results.len() >= 20 {
                continue;
            }
            for entry in fs::Dir::read_from(&dir)? {
                let entry = entry?;
                let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
                if name == "." || name == ".." {
                    continue;
                }
                visited += 1;
                if visited > 2000 || results.len() >= 20 {
                    break;
                }
                // O_PATH sin O_NOFOLLOW: NO_SYMLINKS rechaza incluso el último link.
                let fd = match fs::openat2(
                    &dir,
                    name,
                    OFlags::PATH | OFlags::CLOEXEC,
                    Mode::empty(),
                    RESOLVE,
                ) {
                    Ok(fd) => File::from(fd),
                    Err(_) => continue,
                };
                let metadata = fd.metadata()?;
                let lower = name.to_string_lossy().to_lowercase();
                if metadata.is_dir()
                    && depth < 6
                    && !lower.starts_with('.')
                    && !matches!(lower.as_str(), "node_modules" | "target")
                {
                    // Reabrir desde el descriptor fijado, nunca desde display.
                    let child = fs::openat2(
                        &fd,
                        ".",
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                        Mode::empty(),
                        RESOLVE,
                    )?;
                    stack.push((child.into(), display.join(name), depth + 1));
                }
                if (metadata.is_dir() || (metadata.is_file() && metadata.nlink() == 1))
                    && lower.contains(query)
                {
                    results.push(display.join(name));
                }
            }
        }
        results.sort();
        Ok(results)
    }

    fn open(&self, flags: OFlags) -> Result<File> {
        let path = if self.relative.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &self.relative
        };
        Ok(fs::openat2(
            &self.root,
            path,
            flags | OFlags::CLOEXEC,
            Mode::empty(),
            RESOLVE,
        )
        .context("resolviendo archivo autorizado con openat2")?
        .into())
    }

    pub fn read(&self) -> Result<Vec<u8>> {
        let mut file = self.open(OFlags::RDONLY | OFlags::NONBLOCK)?;
        check_regular(&file)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .context("leyendo descriptor autorizado")?;
        Ok(bytes)
    }

    pub fn is_dir(&self) -> Result<bool> {
        match self.open_optional()? {
            Some(file) => Ok(file.metadata()?.is_dir()),
            None => Ok(false),
        }
    }

    fn open_optional(&self) -> Result<Option<File>> {
        let path = if self.relative.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &self.relative
        };
        match fs::openat2(
            &self.root,
            path,
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
            RESOLVE,
        ) {
            Ok(fd) => Ok(Some(fd.into())),
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(e) => Err(e).context("destino inseguro o inaccesible"),
        }
    }

    fn check_destination(&self, overwrite: bool) -> Result<()> {
        if let Some(file) = self.open_optional()? {
            check_regular(&file)?;
            if !overwrite {
                bail!("El archivo ya existe: {}", self.display.display());
            }
        }
        Ok(())
    }

    fn child(&self, name: &std::ffi::OsStr) -> Result<Self> {
        Ok(Self {
            root: self.root.try_clone()?,
            relative: self.relative.join(name),
            display: self.display.join(name),
        })
    }

    fn backup(&self) -> Result<Self> {
        let relative = self.relative.with_extension("bak");
        if relative == self.relative {
            bail!("El backup coincide con el archivo principal");
        }
        Ok(Self {
            root: self.root.try_clone()?,
            relative,
            display: self.display.with_extension("bak"),
        })
    }

    // Todos los componentes se resuelven con openat2; mkdirat recibe sólo un basename.
    fn parent(&self, create: bool) -> Result<(File, OsString)> {
        let name = self
            .relative
            .file_name()
            .context("No se puede escribir la raíz autorizada")?
            .to_os_string();
        let mut dir = self.root.try_clone()?;
        for component in self
            .relative
            .parent()
            .context("archivo sin padre")?
            .components()
        {
            let Component::Normal(name) = component else {
                bail!("Componente inseguro");
            };
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
            let fd = match fs::openat2(&dir, name, flags, Mode::empty(), RESOLVE) {
                Ok(fd) => fd,
                Err(rustix::io::Errno::NOENT) if create => {
                    match fs::mkdirat(&dir, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                        Err(e) => return Err(e).context("creando directorio autorizado"),
                    }
                    fs::openat2(&dir, name, flags, Mode::empty(), RESOLVE)
                        .context("abriendo directorio creado")?
                }
                Err(e) => return Err(e).context("abriendo directorio autorizado"),
            };
            dir = fd.into();
        }
        Ok((dir, name))
    }

    pub fn create(&self, content: &[u8]) -> Result<()> {
        self.check_destination(false)?;
        self.replace(content, false, true)
    }

    pub fn edit(&self, mode: &str, content: &str) -> Result<()> {
        if !matches!(mode, "overwrite" | "append") {
            bail!("Modo no soportado: {mode}");
        }
        // Leer y autorizar ambos objetos antes de cualquier escritura.
        let original = self.read()?;
        if mode == "overwrite" {
            let backup = self.backup()?;
            backup.check_destination(true)?;
            backup
                .replace(&original, true, false)
                .context("backup obligatorio")?;
            self.replace(content.as_bytes(), true, false)
        } else {
            let mut updated =
                String::from_utf8(original).context("append requiere UTF-8 válido")?;
            updated.push_str(content);
            self.replace(updated.as_bytes(), true, false)
        }
    }

    pub fn copy_to(&self, destination: Self, overwrite: bool) -> Result<PathBuf> {
        let bytes = self.read()?;
        let destination = if destination.is_dir()? {
            destination.child(self.relative.file_name().context("origen sin nombre")?)?
        } else {
            destination
        };
        if self.display == destination.display {
            bail!("Origen y destino coinciden");
        }
        destination.check_destination(overwrite)?;
        destination.replace(&bytes, overwrite, true)?;
        Ok(destination.display)
    }

    fn replace(&self, content: &[u8], overwrite: bool, create_parents: bool) -> Result<()> {
        let (parent, name) = self.parent(create_parents)?;
        let temp = format!(".kda-{}", uuid::Uuid::new_v4());
        let fd = fs::openat2(
            &parent,
            &temp,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
            RESOLVE,
        )
        .context("creando temporal privado")?;
        let mut file = File::from(fd);
        let result = (|| -> Result<()> {
            file.write_all(content)?;
            file.sync_all()?;
            // renameat2 sólo recibe basenames y nunca sigue el destino. El modo
            // NOREPLACE mantiene creación exclusiva incluso si aparece un link.
            fs::renameat_with(
                &parent,
                &temp,
                &parent,
                &name,
                if overwrite {
                    RenameFlags::empty()
                } else {
                    RenameFlags::NOREPLACE
                },
            )
            .context("publicando archivo autorizado")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::unlinkat(&parent, &temp, AtFlags::empty());
        }
        result
    }
}

fn check_regular(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    // También se niegan hardlinks: pueden aliasar datos fuera de la allowlist.
    if !metadata.is_file() || metadata.nlink() != 1 {
        bail!("Sólo archivos regulares sin hardlinks están permitidos");
    }
    Ok(())
}

fn checked_absolute(path: &str) -> Result<PathBuf> {
    if path.trim().is_empty() || path.contains('\0') || path.len() > 4096 {
        bail!("Path inválido");
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        bail!("Path no permitido: se requiere ruta absoluta sin '..'");
    }
    Ok(path)
}

// Cache de vistas: nombre impredecible y 0700 desde mkdirat; sin fallback compartido.
pub(super) fn private_workspace() -> Result<(PathBuf, File)> {
    super::resource_cache::allocate()
}
