//! Aprobaciones de herramientas sensibles (F4-1).
//!
//! El agente emite `ToolApprovalNeeded`; la UI responde con los identificadores
//! de consentimiento, llamada, turno y sesión. SQLite decide si sigue vigente.
//! Timeout (120s) = denegado.

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

const APPROVE_TIMEOUT_SECS: u64 = 120;

/// Política heredada de interacción; F2 clasifica el riesgo y exige consentimiento.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalPolicy {
    /// Valor heredado: F2 no lo usa como autorización de herramientas.
    pub confirm_yellow: bool,
    /// Las 🔴 siempre preguntan.
    pub confirm_red: bool,
    /// Si false (voz manos-libres), F2 deniega Yellow y Red al momento.
    pub interactive: bool,
}

impl ApprovalPolicy {
    pub fn from_config(confirm_sensitive: bool) -> Self {
        Self {
            confirm_yellow: confirm_sensitive,
            confirm_red: true,
            interactive: true,
        }
    }

    /// Política de voz heredada; F2 deniega Yellow y Red sin UI interactiva.
    pub fn voice() -> Self {
        Self {
            confirm_yellow: false,
            confirm_red: true,
            interactive: false,
        }
    }
}

pub struct ApprovalManager {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Registration precedes the UI event, eliminating immediate-approval races.
    pub fn register(&self, tool_call_id: &str) -> PendingApproval<'_> {
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(tool_call_id.to_string(), tx);
        PendingApproval {
            manager: self,
            id: tool_call_id.to_string(),
            rx: Some(rx),
        }
    }
    pub async fn request(&self, tool_call_id: &str, _policy_note: &str) -> bool {
        self.register(tool_call_id).wait().await
    }

    /// Resuelve un pedido pendiente. Retorna false si ya expiró o no existe.
    pub fn resolve(&self, tool_call_id: &str, approved: bool) -> bool {
        let tx = {
            let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            map.remove(tool_call_id)
        };
        match tx {
            Some(tx) => tx.send(approved).is_ok(),
            None => false,
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

pub struct PendingApproval<'a> {
    manager: &'a ApprovalManager,
    id: String,
    rx: Option<oneshot::Receiver<bool>>,
}
impl PendingApproval<'_> {
    pub async fn wait(mut self) -> bool {
        let Some(rx) = self.rx.take() else {
            return false;
        };
        tokio::time::timeout(std::time::Duration::from_secs(APPROVE_TIMEOUT_SECS), rx)
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(false)
    }
}
impl Drop for PendingApproval<'_> {
    fn drop(&mut self) {
        self.manager
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn approve_flow() {
        let m = Arc::new(ApprovalManager::new());
        let m2 = m.clone();
        let waiter = tokio::spawn(async move { m2.request("call_1", "test").await });
        // Dar tiempo a registrarse.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(m.pending_count(), 1);
        assert!(m.resolve("call_1", true));
        assert!(waiter.await.unwrap());
        assert_eq!(m.pending_count(), 0);
    }

    #[tokio::test]
    async fn deny_flow() {
        let m = Arc::new(ApprovalManager::new());
        let m2 = m.clone();
        let waiter = tokio::spawn(async move { m2.request("call_2", "test").await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(m.resolve("call_2", false));
        assert!(!waiter.await.unwrap());
    }

    #[test]
    fn resolve_unknown_is_false() {
        let m = ApprovalManager::new();
        assert!(!m.resolve("nope", true));
    }

    #[test]
    fn dropped_waiter_revokes_late_answer() {
        let m = ApprovalManager::new();
        let pending = m.register("abandoned");
        assert_eq!(m.pending_count(), 1);
        drop(pending);
        assert_eq!(m.pending_count(), 0);
        assert!(!m.resolve("abandoned", true));
    }
}
