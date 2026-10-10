//! Un guard sólo termina y recoge el hijo que creó esta instancia.
use std::ops::{Deref, DerefMut};
use std::process::Child;

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
