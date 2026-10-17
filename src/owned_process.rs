//! Un guard sólo termina y recoge el hijo que creó esta instancia.
use std::ops::{Deref, DerefMut};
use std::process::Child;

/// Linux ties death to the actual thread calling spawn, not the whole runtime.
/// UI launch sites must stay on the process leader, never a disposable worker.
/// The PPID check covers process death before prctl; it does not detect a dead
/// creator thread reparented within a still-live thread group.
/// No PID search and no signals to an unrelated process.
pub fn terminate_with_parent(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    let parent = rustix::process::getpid();
    // SAFETY: only async-signal-safe Linux prctl/getppid run between fork/exec.
    unsafe {
        command.pre_exec(move || {
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))?;
            if rustix::process::getppid() != Some(parent) {
                return Err(std::io::Error::from_raw_os_error(3));
            }
            Ok(())
        });
    }
}

pub struct OwnedChild(Child);
impl OwnedChild {
    pub fn new(child: Child) -> Self {
        Self(child)
    }
}
impl Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
