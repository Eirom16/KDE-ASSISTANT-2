//! Narrow desktop authority: events for our UI only, never agent tool dispatch.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tokio::sync::broadcast;

pub const DBUS_PATH: &str = "/org/kde/assistant/Desktop";
pub const DBUS_INTERFACE: &str = "org.kde.assistant.Desktop1";
pub const DBUS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiAction {
    Show,
    Toggle,
    Status,
    PortalToggle(String),
}

pub struct Desktop {
    events: broadcast::Sender<UiAction>,
    stopping: AtomicBool,
    shortcut_status: Mutex<String>,
}
impl Default for Desktop {
    fn default() -> Self {
        Self {
            events: broadcast::channel(16).0,
            stopping: AtomicBool::new(false),
            shortcut_status: Mutex::new("disabled".into()),
        }
    }
}
impl Desktop {
    pub fn subscribe(&self) -> broadcast::Receiver<UiAction> {
        self.events.subscribe()
    }
    pub fn send(&self, action: UiAction) -> Result<()> {
        anyhow::ensure!(!self.stopping.load(Ordering::SeqCst), "desktop_stopping");
        self.events.send(action).context("ui_not_connected")?;
        Ok(())
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }
    pub fn set_shortcut_status(&self, status: &str) {
        *self
            .shortcut_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = status.to_owned();
        let _ = self.events.send(UiAction::Status);
    }
    pub fn shortcut_status(&self) -> String {
        self.shortcut_status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

/// The descriptor lock is the authority; a PID/name is never sufficient.
/// Never unlink a flock file: replacing its inode permits two owners.
pub struct InstanceLock {
    _file: File,
    pub bus_name: String,
}
pub enum InstanceState {
    Primary(InstanceLock),
    Existing(String),
}
pub fn profile_bus_name(config_dir: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    format!(
        "org.kde.assistant.p{}",
        hex::encode(Sha256::digest(config_dir.as_os_str().as_bytes()))
    )
}
impl InstanceLock {
    pub fn acquire(config_dir: &Path) -> Result<InstanceState> {
        crate::backend::private_files::private_dir(config_dir)?;
        let dir = config_dir.canonicalize()?;
        let bus_name = profile_bus_name(&dir);
        let path = dir.join("desktop.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC)
                    .bits() as i32,
            )
            .open(&path)
            .context("abriendo lock de instancia")?;
        let meta = file.metadata()?;
        anyhow::ensure!(
            meta.is_file()
                && meta.nlink() == 1
                && meta.uid() == rustix::process::getuid().as_raw()
                && meta.mode() & 0o077 == 0,
            "invalid_instance_lock"
        );
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {
                let current = std::fs::symlink_metadata(path)?;
                anyhow::ensure!(
                    current.dev() == meta.dev() && current.ino() == meta.ino(),
                    "instance_lock_replaced"
                );
                Ok(InstanceState::Primary(Self {
                    _file: file,
                    bus_name,
                }))
            }
            Err(rustix::io::Errno::WOULDBLOCK) => Ok(InstanceState::Existing(bus_name)),
            Err(e) => Err(e).context("adquiriendo lock de instancia"),
        }
    }
}

pub async fn session_bus() -> Result<zbus::Connection> {
    tokio::time::timeout(
        DBUS_TIMEOUT,
        zbus::connection::Builder::session()?
            .method_timeout(DBUS_TIMEOUT)
            .max_queued(32)
            .build(),
    )
    .await
    .context("dbus_connect_timeout")?
    .map_err(Into::into)
}
pub async fn activate(bus: &zbus::Connection, name: &str, toggle: bool) -> Result<()> {
    let proxy = zbus::Proxy::new(bus, name, DBUS_PATH, DBUS_INTERFACE).await?;
    proxy
        .call::<_, _, ()>(if toggle { "Toggle" } else { "Activate" }, &())
        .await?;
    Ok(())
}
pub struct DesktopService(pub std::sync::Arc<Desktop>);
#[zbus::interface(name = "org.kde.assistant.Desktop1")]
impl DesktopService {
    fn activate(&self) -> zbus::fdo::Result<()> {
        self.0
            .send(UiAction::Show)
            .map_err(|_| zbus::fdo::Error::Failed("UI unavailable".into()))
    }
    fn toggle(&self) -> zbus::fdo::Result<()> {
        self.0
            .send(UiAction::Toggle)
            .map_err(|_| zbus::fdo::Error::Failed("UI unavailable".into()))
    }
}
pub async fn register(
    bus: &zbus::Connection,
    name: &str,
    desktop: std::sync::Arc<Desktop>,
) -> Result<()> {
    bus.object_server()
        .at(DBUS_PATH, DesktopService(desktop))
        .await?;
    let result = bus
        .request_name_with_flags(name, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        .await?;
    if result != zbus::fdo::RequestNameReply::PrimaryOwner {
        bail!("desktop_name_not_owned");
    }
    Ok(())
}
