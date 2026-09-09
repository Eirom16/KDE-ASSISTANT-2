//! Aprobaciones de herramientas sensibles (F4-1).
//!
//! Las tools 🟡/🔴 no se ejecutan hasta que el usuario las aprueba en la UI:
//! el agente emite `ToolApprovalNeeded`, la UI responde
//! `POST /api/tools/approve {tool_call_id, approved}` y el agente continúa.
//! Timeout (120s) = denegado.

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

const APPROVE_TIMEOUT_SECS: u64 = 120;

/// Política de confirmación (sale de `tools.confirm_sensitive`).
#[derive(Debug, Clone, Copy)]
pub struct ApprovalPolicy {
    /// Si false, las 🟡 se ejecutan sin preguntar (modo potencia).
    pub confirm_yellow: bool,
    /// Las 🔴 siempre preguntan.
    pub confirm_red: bool,
    /// Si false (voz manos-libres), no se espera: las 🔴 se deniegan al momento.
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

    /// Política para voz manos-libres: 🟡 automáticas, 🔴 denegadas al momento.
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

    /// Registra un pedido y espera la decisión (timeout = denegado).
    pub async fn request(&self, tool_call_id: &str, policy_note: &str) -> bool {
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(tool_call_id.to_string(), tx);
        }
        log::info!("Approval pedido para {tool_call_id} ({policy_note}), esperando...");
        let decided =
            tokio::time::timeout(std::time::Duration::from_secs(APPROVE_TIMEOUT_SECS), rx)
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or(false);
        {
            let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            map.remove(tool_call_id);
        }
        if decided {
            log::info!("Approval {tool_call_id}: aprobada por el usuario");
        } else {
            log::info!("Approval {tool_call_id}: denegada o timeout");
        }
        decided
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

    #[cfg(test)]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
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
}
