//! Resource changes revoke a prepared model. Events complement, never replace, SHA-256.
use super::voice_tts::{resource_paths, Selection};
use anyhow::Result;
use rustix::fs::inotify;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    mem::MaybeUninit,
    os::unix::ffi::OsStringExt,
    path::PathBuf,
};
use tokio::io::unix::AsyncFd;

pub struct ResourceWatch {
    fd: AsyncFd<rustix::fd::OwnedFd>,
    names: HashMap<i32, Option<HashSet<OsString>>>,
}
impl ResourceWatch {
    pub fn new(selected: &Selection) -> Result<Self> {
        Self::paths(resource_paths(selected)?, &selected.runtime)
    }
    fn paths(paths: Vec<PathBuf>, runtime: &std::path::Path) -> Result<Self> {
        let fd = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)?;
        let mut names = HashMap::new();
        let mask = inotify::WatchFlags::MODIFY
            | inotify::WatchFlags::ATTRIB
            | inotify::WatchFlags::DELETE_SELF
            | inotify::WatchFlags::MOVE_SELF;
        let mut directories: HashMap<PathBuf, HashSet<OsString>> = HashMap::new();
        for path in paths {
            let wd = inotify::add_watch(&fd, &path, mask)?;
            names.insert(wd, None);
            let mut child = path;
            while let Some(parent) = child.parent() {
                let Some(name) = child.file_name() else { break };
                directories
                    .entry(parent.to_owned())
                    .or_default()
                    .insert(name.to_owned());
                // Follow the private runtime's directory chain up to its containing
                // directory. Other resources need only their direct parent.
                if !child.starts_with(runtime) {
                    break;
                }
                child = parent.to_owned();
            }
        }
        for (path, wanted) in directories {
            let wd = inotify::add_watch(
                &fd,
                &path,
                mask | inotify::WatchFlags::ONLYDIR
                    | inotify::WatchFlags::CREATE
                    | inotify::WatchFlags::DELETE
                    | inotify::WatchFlags::MOVED_FROM
                    | inotify::WatchFlags::MOVED_TO,
            )?;
            names.insert(wd, Some(wanted));
        }
        Ok(Self {
            fd: AsyncFd::new(fd)?,
            names,
        })
    }
    pub async fn changed(&self) -> Result<()> {
        loop {
            let mut ready = self.fd.readable().await?;
            let result = ready.try_io(|fd| {
                let mut buffer = [MaybeUninit::uninit(); 8192];
                let mut reader = inotify::Reader::new(fd.get_ref(), &mut buffer);
                loop {
                    let event = reader.next().map_err(std::io::Error::from)?;
                    let relevant = match (self.names.get(&event.wd()), event.file_name()) {
                        (Some(Some(wanted)), Some(name)) => {
                            wanted.contains(&OsString::from_vec(name.to_bytes().to_vec()))
                        }
                        _ => true, // self event, overflow, lost/unknown watch: revoke.
                    };
                    if relevant {
                        return Ok(());
                    }
                }
            });
            match result {
                Ok(result) => return result.map_err(Into::into),
                Err(_) => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::Duration};
    #[tokio::test]
    async fn watches_same_size_write_replace_and_permissions_not_unrelated_files() {
        for mutation in ["write", "replace", "permissions"] {
            let root = std::env::temp_dir().join(format!("r5-watch-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let file = root.join("model");
            fs::write(&file, b"abcd").unwrap();
            let watch = ResourceWatch::paths(vec![file.clone()], &root).unwrap();
            fs::write(root.join("unrelated"), b"safe").unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(25), watch.changed())
                    .await
                    .is_err()
            );
            match mutation {
                "write" => fs::write(&file, b"efgh").unwrap(),
                "replace" => {
                    fs::write(root.join("new"), b"abcd").unwrap();
                    fs::rename(root.join("new"), &file).unwrap();
                }
                _ => {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
                }
            }
            tokio::time::timeout(Duration::from_secs(1), watch.changed())
                .await
                .unwrap()
                .unwrap();
            drop(watch);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
