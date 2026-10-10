//! Each registry entry owns one supervisor. Network/approval futures are inline;
//! dispatched tools are drained before cancellation can become terminal.
use anyhow::{bail, Result};
use serde::Serialize;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::watch, task::JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Created,
    Running,
    WaitingForApproval,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}
impl TurnState {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}
struct Inner {
    state: TurnState,
    sequence: u64,
}
pub struct Turn {
    pub id: String,
    pub session_id: Option<String>,
    inner: Mutex<Inner>,
    changes: watch::Sender<TurnState>,
}
impl Turn {
    pub fn new(session_id: Option<String>) -> Arc<Self> {
        let (changes, _) = watch::channel(TurnState::Created);
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            inner: Mutex::new(Inner {
                state: TurnState::Created,
                sequence: 0,
            }),
            changes,
        })
    }
    pub fn state(&self) -> TurnState {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).state
    }
    pub fn transition(&self, next: TurnState) -> Result<()> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let valid = matches!(
            (inner.state, next),
            (
                TurnState::Created,
                TurnState::Running | TurnState::Failed | TurnState::Cancelling
            ) | (
                TurnState::Running,
                TurnState::WaitingForApproval | TurnState::Cancelling | TurnState::Failed
            ) | (
                TurnState::WaitingForApproval,
                TurnState::Running | TurnState::Cancelling | TurnState::Failed
            ) | (
                TurnState::Cancelling,
                TurnState::Cancelled | TurnState::Failed
            )
        );
        if !valid {
            bail!(
                "turn_state: transición inválida {:?} -> {next:?}",
                inner.state
            );
        }
        inner.state = next;
        self.changes.send_replace(next);
        Ok(())
    }
    pub fn cancel(&self) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.state.terminal() {
            return false;
        }
        inner.state = TurnState::Cancelling;
        self.changes.send_replace(TurnState::Cancelling);
        true
    }
    pub fn check(&self) -> Result<()> {
        let state = self.state();
        if matches!(state, TurnState::Cancelling | TurnState::Cancelled) {
            bail!("turn_cancelled: cancelación solicitada");
        }
        if state.terminal() {
            bail!("turn_terminal: operación terminada");
        }
        Ok(())
    }
    /// This check is the tool dispatch linearization point. Once dispatched,
    /// the future must be drained even if cancel wins before its first await.
    pub async fn dispatch<T>(&self, future: impl Future<Output = T>) -> Result<T> {
        self.check()?;
        let result = future.await;
        self.check()?;
        Ok(result)
    }
    pub async fn cancellation(&self) {
        let mut changes = self.changes.subscribe();
        loop {
            let state = *changes.borrow_and_update();
            if state == TurnState::Cancelling || state.terminal() {
                return;
            }
            if changes.changed().await.is_err() {
                return;
            }
        }
    }
    pub async fn wait_terminal(&self) {
        let mut changes = self.changes.subscribe();
        loop {
            if changes.borrow_and_update().terminal() {
                return;
            }
            if changes.changed().await.is_err() {
                return;
            }
        }
    }
    /// Completion/persistence and cancellation share the same linearization lock.
    pub fn complete(&self, persist: impl FnOnce() -> Result<()>) -> Result<()> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.state != TurnState::Running {
            bail!("turn_cancelled: no se publica respuesta");
        }
        persist()?;
        inner.state = TurnState::Completed;
        self.changes.send_replace(TurnState::Completed);
        Ok(())
    }
    pub fn envelope(&self, mut data: serde_json::Value) -> serde_json::Value {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.sequence += 1;
        data["turn_id"] = self.id.clone().into();
        data["session_id"] = serde_json::json!(self.session_id);
        data["sequence"] = inner.sequence.into();
        data["state"] = serde_json::json!(inner.state);
        data
    }
}

pub async fn cancellable<T>(
    turn: Option<&Arc<Turn>>,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    if let Some(turn) = turn {
        turn.check()?;
        tokio::select! { biased;
            _ = turn.cancellation() => bail!("turn_cancelled: cancelación solicitada"),
            result = future => result,
        }
    } else {
        future.await
    }
}

struct Entry {
    turn: Arc<Turn>,
    task: Option<JoinHandle<()>>,
}
impl Entry {
    fn diagnose_finished(&self) {
        if self.task.as_ref().is_some_and(JoinHandle::is_finished) && !self.turn.state().terminal()
        {
            log::error!(
                "supervisor_ended_without_terminal: turn {} session {:?}",
                self.turn.id,
                self.turn.session_id
            );
            // A finished owner without a terminal is a failure, never proof that
            // external effects were rolled back or cancellation was successful.
            let _ = self.turn.transition(TurnState::Failed);
        }
    }
}
#[derive(Default)]
pub struct TurnRegistry {
    entries: Mutex<HashMap<Option<String>, Entry>>,
}
impl TurnRegistry {
    fn key(session: &Option<String>) -> Option<String> {
        session.clone()
    }
    pub fn reserve(&self, session: Option<String>) -> Result<Arc<Turn>> {
        let key = Self::key(&session);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        // Finished handles remain owned until reaped; never replace an active owner.
        entries.retain(|_, e| {
            e.diagnose_finished();
            !e.task.as_ref().is_some_and(JoinHandle::is_finished)
        });
        if entries.contains_key(&key) {
            bail!("session_busy: hay un turno activo en esta sesión");
        }
        let turn = Turn::new(session);
        entries.insert(
            key,
            Entry {
                turn: turn.clone(),
                task: None,
            },
        );
        Ok(turn)
    }
    pub fn supervise(&self, turn: &Arc<Turn>, task: JoinHandle<()>) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(&Self::key(&turn.session_id)) {
            entry.task = Some(task);
        }
    }
    pub fn current(&self, session: &Option<String>) -> Option<Arc<Turn>> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&Self::key(session))
            .map(|entry| {
                entry.diagnose_finished();
                entry.turn.clone()
            })
    }
    pub fn find(&self, session: &Option<String>, id: &str) -> Option<Arc<Turn>> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&Self::key(session))
            .filter(|e| e.turn.id == id)
            .map(|e| {
                e.diagnose_finished();
                e.turn.clone()
            })
    }
    pub fn running_tasks(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter(|entry| !entry.task.as_ref().is_some_and(JoinHandle::is_finished))
            .count()
    }
    pub fn active_count(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter(|e| {
                e.diagnose_finished();
                !e.turn.state().terminal()
            })
            .count()
    }
    pub async fn cancel_and_wait(&self, turn: &Arc<Turn>) -> bool {
        turn.cancel();
        // This acknowledgment is bounded. A started tool may still be draining.
        tokio::time::timeout(Duration::from_secs(2), turn.wait_terminal())
            .await
            .is_ok()
    }
}
impl Drop for TurnRegistry {
    fn drop(&mut self) {
        for entry in self
            .entries
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            entry.turn.cancel();
        }
    }
}

/// Owned by the HTTP request or SSE body. Dropping it requests cancellation.
pub struct ClientLease(pub Arc<Turn>);
impl Drop for ClientLease {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
