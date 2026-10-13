//! Explicit deletion; automatic destructive retention is disabled.
use super::{operation_journal::atomic, session_manager::SessionManager};
use anyhow::{ensure, Result};
use rusqlite::params;
impl SessionManager {
    pub fn delete_session_confirmed(&self, id: &str, erase_uncertain_evidence: bool) -> Result<()> {
        atomic(&self.conn, || {
            let exists = self.get_session(id)?.is_some();
            let retained: bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE session_id=?1) OR EXISTS(SELECT 1 FROM conversation_turns WHERE session_id=?1) OR EXISTS(SELECT 1 FROM operation_journal WHERE session_id=?1) OR EXISTS(SELECT 1 FROM tool_audit WHERE session_id=?1)",[id],|r|r.get(0))?;
            // An old legacy deletion may have removed the session but left its evidence.
            ensure!(exists || retained, "session_not_found");
            let active: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE session_id=?1 AND state NOT IN ('completed','cancelled','failed')) OR EXISTS(SELECT 1 FROM operation_journal WHERE session_id=?1 AND state IN ('prepared','authorized','dispatched'))",[id],|r|r.get(0))?;
            ensure!(!active, "session_busy: active turn or operation");
            let uncertain: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM operation_journal WHERE session_id=?1 AND state='unknown_outcome')",[id],|r|r.get(0))?;
            ensure!(
                !uncertain || erase_uncertain_evidence,
                "uncertain_evidence_confirmation_required"
            );
            // Exact ownership only; verify old links before touching shared records.
            let inconsistent: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM operation_journal j JOIN conversation_turns t ON t.id=j.turn_id WHERE (j.session_id=?1 OR t.session_id=?1) AND j.session_id IS NOT t.session_id) OR EXISTS(SELECT 1 FROM turn_inputs i JOIN conversation_turns t ON t.id=i.turn_id JOIN messages m ON m.id=i.user_message_id WHERE (t.session_id=?1 OR m.session_id=?1) AND t.session_id IS NOT m.session_id) OR EXISTS(SELECT 1 FROM operation_journal j JOIN tool_audit a ON a.id=j.audit_id WHERE (j.session_id=?1 OR a.session_id=?1) AND j.session_id IS NOT a.session_id)",[id],|r|r.get(0))?;
            ensure!(!inconsistent, "storage_ownership_inconsistent");
            self.conn.execute("DELETE FROM operation_events WHERE operation_id IN (SELECT id FROM operation_journal WHERE session_id=?1)",[id])?;
            self.conn
                .execute("DELETE FROM operation_journal WHERE session_id=?1", [id])?;
            self.conn.execute("DELETE FROM turn_recovery WHERE turn_id IN (SELECT id FROM conversation_turns WHERE session_id=?1)",[id])?;
            self.conn.execute("DELETE FROM turn_inputs WHERE turn_id IN (SELECT id FROM conversation_turns WHERE session_id=?1)",[id])?;
            self.conn
                .execute("DELETE FROM tool_audit WHERE session_id=?1", [id])?;
            self.conn
                .execute("DELETE FROM messages WHERE session_id=?1", [id])?;
            self.conn
                .execute("DELETE FROM conversation_turns WHERE session_id=?1", [id])?;
            self.conn
                .execute("DELETE FROM sessions WHERE id=?1", params![id])?;
            Ok(())
        })
    }
    pub fn storage_diagnostics(&self) -> Result<serde_json::Value> {
        let scalar = |sql| self.conn.query_row(sql, [], |r| r.get::<_, i64>(0));
        let pages = scalar("PRAGMA page_count")?;
        let page_size = scalar("PRAGMA page_size")?;
        let wal = self.db_file_bytes("-wal");
        Ok(
            serde_json::json!({"automatic_destructive_retention":false,"retention_period_days":null,"logical_db_bytes":pages*page_size,"wal_bytes":wal,"orphan_links":scalar("SELECT (SELECT COUNT(*) FROM turn_inputs i WHERE NOT EXISTS(SELECT 1 FROM messages WHERE id=i.user_message_id) OR NOT EXISTS(SELECT 1 FROM conversation_turns WHERE id=i.turn_id)) + (SELECT COUNT(*) FROM operation_events e WHERE NOT EXISTS(SELECT 1 FROM operation_journal WHERE id=e.operation_id) OR NOT EXISTS(SELECT 1 FROM tool_audit WHERE id=e.audit_id)) + (SELECT COUNT(*) FROM operation_journal j WHERE NOT EXISTS(SELECT 1 FROM conversation_turns WHERE id=j.turn_id) OR NOT EXISTS(SELECT 1 FROM tool_audit WHERE id=j.audit_id) OR (j.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE id=j.session_id))) + (SELECT COUNT(*) FROM messages m WHERE NOT EXISTS(SELECT 1 FROM sessions WHERE id=m.session_id)) + (SELECT COUNT(*) FROM conversation_turns t WHERE t.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE id=t.session_id)) + (SELECT COUNT(*) FROM turn_recovery r WHERE NOT EXISTS(SELECT 1 FROM conversation_turns WHERE id=r.turn_id)) + (SELECT COUNT(*) FROM tool_audit a WHERE a.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE id=a.session_id))")?,"free_pages":scalar("PRAGMA freelist_count")?,"sessions":scalar("SELECT COUNT(*) FROM sessions")?,"messages":scalar("SELECT COUNT(*) FROM messages")?,"conversation_turns":scalar("SELECT COUNT(*) FROM conversation_turns")?,"turn_inputs":scalar("SELECT COUNT(*) FROM turn_inputs")?,"turn_recovery":scalar("SELECT COUNT(*) FROM turn_recovery")?,"tool_audit":scalar("SELECT COUNT(*) FROM tool_audit")?,"operation_events":scalar("SELECT COUNT(*) FROM operation_events")?,"operations":scalar("SELECT COUNT(*) FROM operation_journal")?,"unknown_outcome":scalar("SELECT COUNT(*) FROM operation_journal WHERE state='unknown_outcome'")?,"active_operations":scalar("SELECT COUNT(*) FROM operation_journal WHERE state IN ('prepared','authorized','dispatched')")?}),
        )
    }
    pub(crate) fn resource_is_referenced(&self, directory: &str) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE substr(image_url,1,length(?1))=?1)",
            [format!("{directory}/")],
            |r| r.get(0),
        )?)
    }
}
