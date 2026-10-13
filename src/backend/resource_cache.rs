//! Owned cache v1. Sweep only at backend startup, before QML can acquire resources.
//! Live workspaces hold a flock lease for the process lifetime; legacy /tmp is untouched.
use super::session_manager::SessionManager;
use anyhow::{ensure, Context, Result};
use rustix::fs::{self, AtFlags, Mode, OFlags, RenameFlags, ResolveFlags, CWD};
use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
const RESOLVE: ResolveFlags = ResolveFlags::NO_SYMLINKS
    .union(ResolveFlags::NO_MAGICLINKS)
    .union(ResolveFlags::BENEATH)
    .union(ResolveFlags::NO_XDEV);
const MAX_WORKSPACES: usize = 1024;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
const OWNER: &[u8] = b"KDE Assistant resource-cache v1\n";
struct Lease {
    path: PathBuf,
    root: File,
    directory: File,
    marker: File,
    reserved: u64,
}
static LEASES: Mutex<Vec<Lease>> = Mutex::new(Vec::new());
fn same_file(a: &File, b: &File) -> Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}
fn verify_lease(entry: &Lease) -> Result<()> {
    let parent = entry.path.parent().context("cache root missing")?;
    let root: File = fs::openat2(
        CWD,
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?
    .into();
    let dir = open_dir(
        &root,
        entry.path.file_name().context("cache identity missing")?,
    )?;
    ensure!(
        same_file(&root, &entry.root)?
            && same_file(&dir, &entry.directory)?
            && same_file(&lease(&dir)?, &entry.marker)?,
        "cache_identity_changed"
    );
    Ok(())
}
fn private(file: &File) -> Result<()> {
    let m = file.metadata()?;
    ensure!(
        m.uid() == rustix::process::getuid().as_raw() && m.mode() & 0o777 == 0o700,
        "cache_owner_invalid"
    );
    Ok(())
}
fn directory(parent: &File, name: &str) -> Result<File> {
    match fs::mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(e) => return Err(e.into()),
    }
    Ok(fs::openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )?
    .into())
}
fn root_at(cache: &Path) -> Result<(PathBuf, File)> {
    let base: File = fs::openat2(
        CWD,
        cache,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?
    .into();
    let app = directory(&base, "kde-assistant")?;
    let root = directory(&app, "resources-v1")?;
    private(&root)?;
    Ok((cache.join("kde-assistant/resources-v1"), root))
}
fn names(dir: &File) -> Result<Vec<std::ffi::OsString>> {
    let mut names = Vec::new();
    for item in fs::Dir::read_from(dir)? {
        let item = item?;
        let name = OsStr::from_bytes(item.file_name().to_bytes());
        if name != "." && name != ".." {
            ensure!(names.len() < MAX_WORKSPACES * 4, "cache_entry_limit");
            names.push(name.to_owned());
        }
    }
    Ok(names)
}
fn open_dir(root: &File, name: &OsStr) -> Result<File> {
    let dir: File = fs::openat2(
        root,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )?
    .into();
    private(&dir)?;
    Ok(dir)
}
fn lease(dir: &File) -> Result<File> {
    let mut file: File = fs::openat2(
        dir,
        ".lease",
        OFlags::RDWR | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )?
    .into();
    let m = file.metadata()?;
    ensure!(
        m.is_file()
            && m.nlink() == 1
            && m.uid() == rustix::process::getuid().as_raw()
            && m.len() == OWNER.len() as u64,
        "cache_identity_invalid"
    );
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(OWNER.len() as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes == OWNER, "cache_identity_invalid");
    Ok(file)
}
fn size(dir: &File) -> Result<u64> {
    let mut total = 0u64;
    for name in names(dir)? {
        let fd: File = fs::openat2(
            dir,
            &name,
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
            RESOLVE,
        )?
        .into();
        let m = fd.metadata()?;
        ensure!(m.is_file() && m.nlink() == 1, "cache_resource_invalid");
        total = total.saturating_add(m.len());
    }
    Ok(total)
}
pub fn allocate() -> Result<(PathBuf, File)> {
    allocate_at(&dirs::cache_dir().context("cache directory unavailable")?)
}
pub fn allocate_at(cache: &Path) -> Result<(PathBuf, File)> {
    let mut leases = LEASES.lock().unwrap_or_else(|e| e.into_inner());
    let (path, root) = root_at(cache)?;
    let entries = names(&root)?;
    ensure!(
        entries.len() < MAX_WORKSPACES,
        "cache_quota_exceeded: workspace limit"
    );
    let mut bytes = 0u64;
    for name in entries {
        let dir = open_dir(&root, &name)?;
        lease(&dir)?;
        bytes = bytes.saturating_add(size(&dir)?);
    }
    let reserved: u64 = leases
        .iter()
        .filter(|entry| entry.path.parent() == Some(path.as_path()))
        .map(|entry| entry.reserved)
        .sum();
    ensure!(
        bytes.saturating_add(reserved) < MAX_BYTES - MAX_RESOURCE_BYTES,
        "cache_quota_exceeded: storage limit"
    );
    let name = uuid::Uuid::new_v4().to_string();
    let dir = directory(&root, &name)?;
    let mut marker: File = fs::openat2(
        &dir,
        ".lease",
        OFlags::CREATE | OFlags::EXCL | OFlags::RDWR | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
        RESOLVE,
    )?
    .into();
    marker.write_all(OWNER)?;
    fs::flock(&marker, fs::FlockOperation::NonBlockingLockExclusive)?;
    let path = path.join(name);
    leases.push(Lease {
        path: path.clone(),
        root: root.try_clone()?,
        directory: dir.try_clone()?,
        marker,
        reserved: MAX_RESOURCE_BYTES,
    });
    Ok((path, dir))
}
/// Refuse nested directories, symlinks and unknown ownership; never traverse them.
fn remove_flat(root: &File, name: &OsStr, dir: &File) -> Result<()> {
    let entries = names(dir)?;
    size(dir)?;
    let marker = lease(dir)?;
    // Claim a fresh name atomically before destructive work. If the original name
    // changed, preserve the replacement and restore it without overwriting anything.
    let claimed = uuid::Uuid::new_v4().to_string();
    fs::renameat_with(root, name, root, &claimed, RenameFlags::NOREPLACE)?;
    let identity = (|| -> Result<()> {
        let actual = open_dir(root, OsStr::new(&claimed))?;
        ensure!(
            same_file(dir, &actual)? && same_file(&marker, &lease(&actual)?)?,
            "cache_identity_changed"
        );
        Ok(())
    })();
    if identity.is_err() {
        let _ = fs::renameat_with(root, &claimed, root, name, RenameFlags::NOREPLACE);
        return identity;
    }
    let result = (|| -> Result<()> {
        for file in &entries {
            if file != ".lease" {
                fs::unlinkat(dir, file, AtFlags::empty())?;
            }
        }
        fs::unlinkat(dir, ".lease", AtFlags::empty())?;
        fs::unlinkat(root, &claimed, AtFlags::REMOVEDIR)?;
        Ok(())
    })();
    if result.is_err() {
        // Retain recoverable ownership after a partial cleanup failure.
        if let Ok(fd) = fs::openat2(
            dir,
            ".lease",
            OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
            RESOLVE,
        ) {
            let mut restored: File = fd.into();
            let _ = restored.write_all(OWNER);
        }
        let _ = fs::renameat_with(root, &claimed, root, name, RenameFlags::NOREPLACE);
    }
    result
}
pub fn release_private_snapshot(path: &Path) -> Result<()> {
    let directory = path.parent().context("snapshot parent missing")?;
    let mut leases = LEASES.lock().unwrap_or_else(|e| e.into_inner());
    let index = leases
        .iter()
        .position(|entry| entry.path == directory)
        .context("cache_not_owned")?;
    let entry = &leases[index];
    verify_lease(entry)?;
    remove_flat(
        &entry.root,
        directory.file_name().context("snapshot identity missing")?,
        &entry.directory,
    )?;
    leases.swap_remove(index);
    Ok(())
}
/// Create through the pinned workspace descriptor; never overwrite or follow a link.
pub(crate) fn create_resource(path: &Path) -> Result<File> {
    let directory = path.parent().context("resource parent missing")?;
    let leases = LEASES.lock().unwrap_or_else(|e| e.into_inner());
    let entry = leases
        .iter()
        .find(|entry| entry.path == directory)
        .context("cache_not_owned")?;
    verify_lease(entry)?;
    let name = path.file_name().context("resource identity missing")?;
    ensure!(name != ".lease", "cache_identity_invalid");
    Ok(fs::openat2(
        &entry.directory,
        name,
        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
        RESOLVE,
    )?
    .into())
}
pub(crate) fn write_resource(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() as u64 <= MAX_RESOURCE_BYTES,
        "cache_resource_limit"
    );
    create_resource(path)?.write_all(bytes)?;
    Ok(())
}
pub fn check_resource(path: &Path) -> Result<()> {
    let directory = path.parent().context("resource parent missing")?;
    let leases = LEASES.lock().unwrap_or_else(|e| e.into_inner());
    let entry = leases
        .iter()
        .find(|entry| entry.path == directory)
        .context("cache_not_owned")?;
    verify_lease(entry)?;
    let file: File = fs::openat2(
        &entry.directory,
        path.file_name().context("resource identity missing")?,
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLVE,
    )?
    .into();
    let m = file.metadata()?;
    ensure!(
        m.is_file()
            && m.nlink() == 1
            && m.uid() == rustix::process::getuid().as_raw()
            && m.len() <= MAX_RESOURCE_BYTES,
        "cache_resource_limit"
    );
    Ok(())
}
/// Before exposing a resource, validate workspace size and release its admission reserve.
pub fn publish_resource(path: &Path) -> Result<()> {
    check_resource(path)?;
    let directory = path.parent().context("resource parent missing")?;
    let mut leases = LEASES.lock().unwrap_or_else(|e| e.into_inner());
    let entry = leases
        .iter_mut()
        .find(|entry| entry.path == directory)
        .context("cache_not_owned")?;
    verify_lease(entry)?;
    ensure!(
        size(&entry.directory)? <= MAX_RESOURCE_BYTES,
        "cache_resource_limit"
    );
    entry.reserved = 0;
    Ok(())
}
/// Unpublished resources cannot be used by QML and may be removed on failure/drop.
pub struct PendingResource(Option<PathBuf>);
impl PendingResource {
    pub fn new(directory: &Path) -> Self {
        Self(Some(directory.join(".unpublished")))
    }
    pub fn keep(mut self) {
        self.0 = None;
    }
}
impl Drop for PendingResource {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            if let Err(error) = release_private_snapshot(path) {
                log::warn!("Unpublished cache cleanup pending: {error}");
            }
        }
    }
}
/// Failure leaves evidence/resources for the next sweep. No runtime sweep or VACUUM.
pub fn sweep_at(
    cache: &Path,
    protected: impl Fn(&str) -> Result<bool>,
    minimum_age: Duration,
) -> Result<serde_json::Value> {
    let (path, root) = root_at(cache)?;
    let mut removed = 0;
    let mut skipped = 0;
    let mut failures = 0;
    for name in names(&root)? {
        let work = (|| -> Result<bool> {
            ensure!(
                uuid::Uuid::parse_str(&name.to_string_lossy()).is_ok(),
                "cache_unknown_identity"
            );
            let dir = open_dir(&root, &name)?;
            let marker = lease(&dir)?;
            if fs::flock(&marker, fs::FlockOperation::NonBlockingLockExclusive).is_err() {
                return Ok(false);
            }
            if marker.metadata()?.modified()?.elapsed().unwrap_or_default() < minimum_age
                || protected(&path.join(&name).to_string_lossy())?
            {
                return Ok(false);
            }
            remove_flat(&root, &name, &dir)?;
            Ok(true)
        })();
        match work {
            Ok(true) => removed += 1,
            Ok(false) => skipped += 1,
            Err(_) => failures += 1,
        }
    }
    Ok(
        serde_json::json!({"removed":removed,"skipped":skipped,"failures":failures,"max_bytes":MAX_BYTES,"max_workspaces":MAX_WORKSPACES}),
    )
}
pub fn startup(sessions: &SessionManager) -> Result<()> {
    let cache = dirs::cache_dir().context("cache directory unavailable")?;
    // Qt hasn't started yet. Persisted references and other process leases remain live.
    let stats = sweep_at(
        &cache,
        |directory| sessions.resource_is_referenced(directory),
        Duration::from_secs(7 * 86400),
    )?;
    log::info!("Owned resource cache maintenance: {stats}");
    Ok(())
}
