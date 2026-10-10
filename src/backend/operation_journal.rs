//! Write-ahead intent and conservative recovery. No arguments, paths or output text.
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;

use super::session_manager::SessionManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Prepared,
    Authorized,
    Dispatched,
    Succeeded,
    Failed,
    Denied,
    UnknownOutcome,
    Abandoned,
}
impl OperationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Authorized => "authorized",
            Self::Dispatched => "dispatched",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::UnknownOutcome => "unknown_outcome",
            Self::Abandoned => "abandoned",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Operation {
    pub id: String,
    pub turn_id: String,
    pub session_id: Option<String>,
    pub kind: String,
    pub tool: String,
    pub state: String,
    pub prepared_at: String,
    pub updated_at: String,
    pub consent: String,
    pub audit_id: i64,
    pub result_bytes: Option<i64>,
}

/// Own the outer transaction, including COMMIT errors and automatic SQLite rollback.
pub(crate) fn atomic<T>(conn: &Connection, work: impl FnOnce() -> Result<T>) -> Result<T> {
    ensure!(conn.is_autocommit(), "journal_transaction_already_open");
    conn.execute_batch("BEGIN IMMEDIATE")?;
    struct RollbackOnUnwind<'a>(&'a Connection);
    impl Drop for RollbackOnUnwind<'_> {
        fn drop(&mut self) {
            if !self.0.is_autocommit() {
                if let Err(error) = self.0.execute_batch("ROLLBACK") {
                    log::error!("journal unwind rollback: {error}");
                }
            }
        }
    }
    let _rollback = RollbackOnUnwind(conn);
    let result = work().and_then(|value| {
        conn.execute_batch("COMMIT")?;
        Ok(value)
    });
    if result.is_err() && !conn.is_autocommit() {
        if let Err(cleanup) = conn.execute_batch("ROLLBACK") {
            return result.context(format!("journal rollback failed: {cleanup}"));
        }
    }
    result
}

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("../../migrations/003_operation_journal.sql"))?;
    Ok(())
}

fn audit(conn: &Connection, id: &str, state: &str, consent: &str) -> Result<i64> {
    let anchor: i64 = conn.query_row(
        "SELECT audit_id FROM operation_journal WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    let changed = conn.execute(
        "UPDATE tool_audit SET timestamp=?2,success=?3,decided=?4 WHERE id=?1",
        params![
            anchor,
            Utc::now().to_rfc3339(),
            state == "succeeded",
            consent
        ],
    )?;
    ensure!(changed == 1, "operation_audit_missing");
    Ok(anchor)
}

fn event(conn: &Connection, id: &str, state: &str, audit_id: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO operation_events(operation_id,state,timestamp,audit_id)
         VALUES(?1,?2,?3,?4)",
        params![id, state, Utc::now().to_rfc3339(), audit_id],
    )?;
    Ok(())
}

fn transition(
    conn: &Connection,
    id: &str,
    next: OperationState,
    consent: &str,
    bytes: Option<i64>,
) -> Result<()> {
    let state = next.as_str();
    let previous: String = conn.query_row(
        "SELECT state FROM operation_journal WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    let valid = matches!(
        (previous.as_str(), next),
        (
            "prepared",
            OperationState::Authorized | OperationState::Denied | OperationState::Abandoned
        ) | (
            "authorized",
            OperationState::Dispatched | OperationState::Abandoned
        ) | (
            "dispatched",
            OperationState::Succeeded | OperationState::Failed | OperationState::UnknownOutcome
        )
    );
    ensure!(valid, "invalid_operation_transition: {previous} -> {state}");
    ensure!(
        next != OperationState::Authorized || matches!(consent, "auto" | "approved"),
        "operation_consent_missing"
    );
    ensure!(
        next != OperationState::Denied || consent == "denied",
        "operation_denial_missing"
    );
    if previous != "prepared" {
        let recorded: String = conn.query_row(
            "SELECT consent FROM operation_journal WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        ensure!(recorded == consent, "operation_consent_changed");
    }
    let audit_id: i64 = if matches!(
        next,
        OperationState::Succeeded | OperationState::Failed | OperationState::Denied
    ) {
        audit(conn, id, state, consent)?
    } else {
        conn.query_row(
            "SELECT audit_id FROM operation_journal WHERE id=?1",
            [id],
            |r| r.get(0),
        )?
    };
    conn.execute("UPDATE operation_journal SET state=?2,updated_at=?3,consent=?4,audit_id=?5,result_bytes=?6 WHERE id=?1",
        params![id,state,Utc::now().to_rfc3339(),consent,audit_id,bytes])?;
    event(conn, id, state, audit_id)
}

pub(crate) fn recover(conn: &Connection) -> Result<()> {
    atomic(conn, || {
        let ids = {
            let mut stmt = conn.prepare("SELECT id,state FROM operation_journal WHERE state IN ('prepared','authorized','dispatched')")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, state) in ids {
            let consent: String = conn.query_row(
                "SELECT consent FROM operation_journal WHERE id=?1",
                [&id],
                |r| r.get(0),
            )?;
            transition(
                conn,
                &id,
                if state == "dispatched" {
                    OperationState::UnknownOutcome
                } else {
                    OperationState::Abandoned
                },
                &consent,
                None,
            )?;
        }
        conn.execute_batch("INSERT OR IGNORE INTO turn_recovery(turn_id,previous_state,recovered_at)
            SELECT id,state,strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM conversation_turns
            WHERE state IN ('created','running','waiting_for_approval','cancelling');
            UPDATE conversation_turns SET state='failed' WHERE state IN ('created','running','waiting_for_approval','cancelling');")?;
        Ok(())
    })
}

impl SessionManager {
    /// Operation IDs always originate here, never in ToolCtx or provider output.
    pub fn prepare_operation(&self, turn_id: &str, tool: &str) -> Result<String> {
        ensure!(
            super::tool_registry::all_tools()
                .iter()
                .any(|t| t.function.name == tool),
            "unknown journal tool"
        );
        let id = uuid::Uuid::new_v4().to_string();
        atomic(&self.conn, || {
            let (session, state): (Option<String>, String) = self.conn.query_row(
                "SELECT session_id,state FROM conversation_turns WHERE id=?1",
                [turn_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            ensure!(state == "running", "journal_turn_not_running");
            let now = Utc::now().to_rfc3339();
            // A single redacted audit anchor records the attempt, with decided='prepared'.
            self.conn.execute("INSERT INTO tool_audit(timestamp,session_id,tool,args,permission,decided) VALUES(?1,?2,?3,'[redacted]',?4,'prepared')", params![now,session,tool,super::tool_registry::permission(tool).as_str()])?;
            let anchor = self.conn.last_insert_rowid();
            self.conn.execute("INSERT INTO operation_journal(id,turn_id,session_id,kind,tool,state,prepared_at,updated_at,permission,consent,audit_id) VALUES(?1,?2,?3,'tool',?4,'prepared',?5,?5,?6,'pending',?7)", params![id,turn_id,session,tool,now,super::tool_registry::permission(tool).as_str(),anchor])?;
            event(&self.conn, &id, "prepared", anchor)?;
            Ok(())
        })?;
        Ok(id)
    }
    pub fn transition_operation(
        &self,
        id: &str,
        state: OperationState,
        consent: &str,
        result_bytes: Option<i64>,
    ) -> Result<()> {
        ensure!(
            matches!(consent, "pending" | "auto" | "approved" | "denied"),
            "invalid consent metadata"
        );
        atomic(&self.conn, || {
            transition(&self.conn, id, state, consent, result_bytes)
        })
    }
    /// Bounded, newest-first metadata; never returns tool arguments or output text.
    pub fn list_operations(&self, session: Option<&str>, limit: i64) -> Result<Vec<Operation>> {
        let mut stmt = self.conn.prepare("SELECT id,turn_id,session_id,kind,tool,state,prepared_at,updated_at,consent,audit_id,result_bytes FROM operation_journal WHERE (?1 IS NULL OR session_id=?1) ORDER BY prepared_at DESC,id DESC LIMIT ?2")?;
        let rows = stmt.query_map(params![session, limit.clamp(1, 500)], |r| {
            Ok(Operation {
                id: r.get(0)?,
                turn_id: r.get(1)?,
                session_id: r.get(2)?,
                kind: r.get(3)?,
                tool: r.get(4)?,
                state: r.get(5)?,
                prepared_at: r.get(6)?,
                updated_at: r.get(7)?,
                consent: r.get(8)?,
                audit_id: r.get(9)?,
                result_bytes: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn authorize_operation(
        &self,
        id: &str,
        turn: &str,
        tool: &str,
        consent: &str,
    ) -> Result<()> {
        atomic(&self.conn, || {
            let owner: (String, String) = self.conn.query_row(
                "SELECT turn_id,tool FROM operation_journal WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            ensure!(
                owner.0 == turn && owner.1 == tool,
                "operation_identity_mismatch"
            );
            transition(&self.conn, id, OperationState::Authorized, consent, None)
        })
    }
    pub fn record_operation_result(
        &self,
        id: &str,
        consent: &str,
        result: &crate::models::ToolResult,
        duration_ms: u64,
    ) -> Result<()> {
        atomic(&self.conn, || {
            transition(
                &self.conn,
                id,
                if result.success {
                    OperationState::Succeeded
                } else {
                    OperationState::Failed
                },
                consent,
                Some(result.content.len().min(i64::MAX as usize) as i64),
            )?;
            self.conn.execute("UPDATE tool_audit SET duration_ms=?2 WHERE id=(SELECT audit_id FROM operation_journal WHERE id=?1)",params![id,duration_ms.min(i64::MAX as u64) as i64])?;
            Ok(())
        })
    }
    pub fn begin_turn(
        &self,
        turn_id: &str,
        session: Option<&str>,
        user: Option<&crate::models::Message>,
    ) -> Result<()> {
        atomic(&self.conn, || {
            if let Some(sid) = session {
                ensure!(self.get_session(sid)?.is_some(), "session_not_found");
            }
            self.create_turn(turn_id, session)?;
            if let (Some(sid), Some(message)) = (session, user) {
                let user_id = self.add_turn_message(sid, turn_id, message)?;
                self.conn.execute(
                    "INSERT INTO turn_inputs(turn_id,user_message_id) VALUES(?1,?2)",
                    params![turn_id, user_id],
                )?;
            }
            Ok(())
        })
    }
    pub fn begin_retry_turn(&self, turn_id: &str, session: &str) -> Result<()> {
        atomic(&self.conn, || {
            ensure!(self.get_session(session)?.is_some(), "session_not_found");
            let (message, role): (i64, String) = self.conn.query_row(
                "SELECT id,role FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1",
                [session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            // The HTTP precheck precedes awaits; history can change before this write.
            ensure!(role == "user", "regeneration_requires_unanswered_user");
            ensure!(
                self.regeneration_is_safe(session)?,
                "regeneration_requires_new_message"
            );
            self.create_turn(turn_id, Some(session))?;
            self.conn.execute(
                "INSERT INTO turn_inputs(turn_id,user_message_id) VALUES(?1,?2)",
                params![turn_id, message],
            )?;
            Ok(())
        })
    }
    /// Finalization does not alter recorded outcomes, even when cancellation won.
    pub fn settle_turn(&self, turn_id: &str, terminal: &str) -> Result<()> {
        atomic(&self.conn, || {
            let ids = {
                let mut stmt = self.conn.prepare("SELECT id,state,consent FROM operation_journal WHERE turn_id=?1 AND state IN ('prepared','authorized','dispatched')")?;
                let rows = stmt.query_map([turn_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for (id, state, consent) in ids {
                transition(
                    &self.conn,
                    &id,
                    if state == "dispatched" {
                        OperationState::UnknownOutcome
                    } else {
                        OperationState::Abandoned
                    },
                    &consent,
                    None,
                )?;
            }
            self.finish_turn(turn_id, terminal)
        })
    }
    pub fn regeneration_is_safe(&self, session: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM operation_journal WHERE (turn_id=(SELECT turn_id FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1) OR turn_id IN (SELECT turn_id FROM turn_inputs WHERE user_message_id=(SELECT id FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1))) AND state IN ('dispatched','succeeded','failed','unknown_outcome')",[session],|r|r.get(0))?;
        Ok(count == 0)
    }
}
