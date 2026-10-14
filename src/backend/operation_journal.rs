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
        #[cfg(test)]
        crash_checkpoint("transaction");
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
    conn.execute_batch(include_str!("../../migrations/004_storage_retention.sql"))?;
    conn.execute_batch(include_str!("../../migrations/005_permission_engine.sql"))?;
    #[cfg(test)]
    crash_checkpoint("migration");
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
    #[cfg(test)]
    crash_checkpoint("audit");
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
        conn.execute("UPDATE permission_requests SET state='revoked' WHERE state IN ('pending','approved') AND operation_id IN (SELECT id FROM operation_journal WHERE state='abandoned')",[])?;
        #[cfg(test)]
        crash_checkpoint("recovery");
        conn.execute_batch("INSERT OR IGNORE INTO turn_recovery(turn_id,previous_state,recovered_at)
            SELECT id,state,strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM conversation_turns
            WHERE state IN ('created','running','waiting_for_approval','cancelling');
            UPDATE conversation_turns SET state='failed' WHERE state IN ('created','running','waiting_for_approval','cancelling');")?;
        Ok(())
    })
}

impl SessionManager {
    pub fn prepare_permission(
        &self,
        turn_id: &str,
        call: &crate::models::ToolCall,
        plan: &super::permission_engine::Plan,
        origin: &str,
    ) -> Result<(String, Option<String>)> {
        ensure!(
            matches!(origin, "chat" | "voice" | "manual"),
            "invalid_tool_origin"
        );
        let operation = uuid::Uuid::new_v4().to_string();
        let consent = (plan.risk != super::permission_engine::Risk::Green)
            .then(|| uuid::Uuid::new_v4().to_string());
        atomic(&self.conn, || {
            let (session, state): (Option<String>, String) = self.conn.query_row(
                "SELECT session_id,state FROM conversation_turns WHERE id=?1",
                [turn_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            ensure!(state == "running", "journal_turn_not_running");
            let now = Utc::now();
            let stamp = now.to_rfc3339();
            self.conn.execute("INSERT INTO tool_audit(timestamp,session_id,tool,args,permission,decided) VALUES(?1,?2,?3,'[redacted]',?4,'prepared')", params![stamp,session,call.name,plan.risk.as_str()])?;
            let anchor = self.conn.last_insert_rowid();
            self.conn.execute("INSERT INTO operation_journal(id,turn_id,session_id,kind,tool,state,prepared_at,updated_at,permission,consent,audit_id) VALUES(?1,?2,?3,'tool',?4,'prepared',?5,?5,?6,'pending',?7)", params![operation,turn_id,session,call.name,stamp,plan.risk.as_str(),anchor])?;
            event(&self.conn, &operation, "prepared", anchor)?;
            self.conn.execute("INSERT INTO permission_requests(operation_id,consent_id,tool_call_id,fingerprint,risk,principal,origin,state,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,'local_authenticated',?6,?7,?8,?9)", params![operation,consent,call.id,plan.fingerprint,plan.risk.as_str(),origin,if consent.is_some() {"pending"} else {"auto"},now.timestamp(),now.timestamp()+120])?;
            Ok(())
        })?;
        Ok((operation, consent))
    }

    pub fn answer_permission(
        &self,
        consent_id: &str,
        tool_call_id: &str,
        turn_id: &str,
        session_id: Option<&str>,
        approved: bool,
    ) -> Result<()> {
        atomic(&self.conn, || {
            let (operation, expires): (String, i64) = self.conn.query_row(
                "SELECT operation_id,expires_at FROM permission_requests WHERE consent_id=?1 AND tool_call_id=?2 AND state='pending'",
                params![consent_id,tool_call_id], |r| Ok((r.get(0)?,r.get(1)?)))?;
            let valid: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM operation_journal j JOIN conversation_turns t ON t.id=j.turn_id WHERE j.id=?1 AND j.turn_id=?2 AND j.session_id IS ?3 AND j.state='prepared' AND t.state='running')",params![operation,turn_id,session_id],|r|r.get(0))?;
            ensure!(
                valid && expires > Utc::now().timestamp(),
                "consent_expired_or_wrong_context"
            );
            let changed=self.conn.execute("UPDATE permission_requests SET state=?2,answered_at=?3 WHERE operation_id=?1 AND state='pending'",params![operation,if approved {"approved"} else {"denied"},Utc::now().timestamp()])?;
            ensure!(changed == 1, "consent_already_answered");
            if !approved {
                transition(
                    &self.conn,
                    &operation,
                    OperationState::Denied,
                    "denied",
                    None,
                )?;
            }
            Ok(())
        })
    }

    pub fn deny_permission(&self, operation: &str) -> Result<()> {
        atomic(&self.conn, || {
            let state: String = self.conn.query_row(
                "SELECT state FROM operation_journal WHERE id=?1",
                [operation],
                |r| r.get(0),
            )?;
            if state == "denied" {
                return Ok(());
            }
            self.conn.execute("UPDATE permission_requests SET state='denied',answered_at=?2 WHERE operation_id=?1 AND state IN ('pending','approved')",params![operation,Utc::now().timestamp()])?;
            transition(
                &self.conn,
                operation,
                OperationState::Denied,
                "denied",
                None,
            )
        })
    }

    pub fn dispatch_permission(
        &self,
        operation: &str,
        turn: &str,
        session: Option<&str>,
        call: &crate::models::ToolCall,
        fingerprint: &str,
    ) -> Result<String> {
        atomic(&self.conn, || {
            let (owner_turn,owner_session,owner_tool,op_state,turn_state,stored,status,expires): (String,Option<String>,String,String,String,String,String,i64)=self.conn.query_row(
                "SELECT j.turn_id,j.session_id,j.tool,j.state,t.state,p.fingerprint,p.state,p.expires_at FROM operation_journal j JOIN conversation_turns t ON t.id=j.turn_id JOIN permission_requests p ON p.operation_id=j.id WHERE j.id=?1",[operation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?;
            ensure!(
                owner_turn == turn
                    && owner_session.as_deref() == session
                    && owner_tool == call.name
                    && op_state == "prepared"
                    && turn_state == "running",
                "operation_identity_mismatch"
            );
            ensure!(
                stored == fingerprint && expires > Utc::now().timestamp(),
                "consent_resource_or_arguments_changed"
            );
            ensure!(
                matches!(status.as_str(), "auto" | "approved"),
                "consent_missing_or_consumed"
            );
            let consent = if status == "auto" { "auto" } else { "approved" };
            let changed=self.conn.execute("UPDATE permission_requests SET state='consumed',consumed_at=?2 WHERE operation_id=?1 AND state=?3",params![operation,Utc::now().timestamp(),status])?;
            ensure!(changed == 1, "consent_already_consumed");
            transition(
                &self.conn,
                operation,
                OperationState::Authorized,
                consent,
                None,
            )?;
            transition(
                &self.conn,
                operation,
                OperationState::Dispatched,
                consent,
                None,
            )?;
            Ok(consent.to_owned())
        })
    }
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
        self.operation_page(session, limit, false, None)
    }
    pub fn operation_page(
        &self,
        session: Option<&str>,
        limit: i64,
        recovery_only: bool,
        before: Option<i64>,
    ) -> Result<Vec<Operation>> {
        let mut stmt = self.conn.prepare("SELECT id,turn_id,session_id,kind,tool,state,prepared_at,updated_at,consent,audit_id,result_bytes FROM operation_journal WHERE (?1 IS NULL OR session_id=?1) AND (?3=0 OR state IN ('unknown_outcome','failed','abandoned')) AND (?4 IS NULL OR audit_id<?4) ORDER BY audit_id DESC LIMIT ?2")?;
        let rows = stmt.query_map(
            params![session, limit.clamp(1, 500), recovery_only, before],
            |r| {
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
            },
        )?;
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
                if result.outcome_uncertain {
                    OperationState::UnknownOutcome
                } else if result.success {
                    OperationState::Succeeded
                } else {
                    OperationState::Failed
                },
                consent,
                Some(result.content.len().min(i64::MAX as usize) as i64),
            )?;
            if result.outcome_uncertain {
                audit(&self.conn, id, "unknown_outcome", consent)?;
            }
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
            self.conn.execute("UPDATE permission_requests SET state='revoked' WHERE state IN ('pending','approved') AND operation_id IN (SELECT id FROM operation_journal WHERE turn_id=?1 AND state='abandoned')",[turn_id])?;
            self.finish_turn(turn_id, terminal)
        })
    }
    pub fn regeneration_is_safe(&self, session: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM operation_journal WHERE (turn_id=(SELECT turn_id FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1) OR turn_id IN (SELECT turn_id FROM turn_inputs WHERE user_message_id=(SELECT id FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1))) AND state IN ('dispatched','succeeded','failed','unknown_outcome')",[session],|r|r.get(0))?;
        Ok(count == 0)
    }
}

// Compiled only in the unit-test executable, armed exclusively by our child fixture.
#[cfg(test)]
fn crash_checkpoint(label: &str) {
    if std::env::var("KDA_F1B3_HOOK").as_deref() == Ok(label) {
        let root =
            std::path::PathBuf::from(std::env::var_os("KDA_F1B3_UNIT_ROOT").expect("child root"));
        std::fs::write(root.join("ready"), label).expect("child checkpoint");
        loop {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::DirBuilderExt,
        path::PathBuf,
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    #[ignore = "SIGKILL child fixture only"]
    fn crash_worker() {
        let root = PathBuf::from(std::env::var_os("KDA_F1B3_UNIT_ROOT").unwrap());
        let stage = std::env::var("KDA_F1B3_UNIT_STAGE").unwrap();
        if stage == "migration" || stage == "recovery" {
            std::env::set_var("KDA_F1B3_HOOK", &stage);
        }
        let db = SessionManager::open_at(root.join("sessions.db")).unwrap();
        db.begin_turn("crash", Some("fixture"), None).unwrap();
        if stage == "transaction" {
            std::env::set_var("KDA_F1B3_HOOK", "transaction");
            db.prepare_operation("crash", "create_file").unwrap();
        } else if stage == "audit" {
            let op = db.prepare_operation("crash", "create_file").unwrap();
            db.transition_operation(&op, OperationState::Authorized, "auto", None)
                .unwrap();
            db.transition_operation(&op, OperationState::Dispatched, "auto", None)
                .unwrap();
            std::env::set_var("KDA_F1B3_HOOK", "audit");
            db.transition_operation(&op, OperationState::Succeeded, "auto", Some(12))
                .unwrap();
        }
        panic!("checkpoint not reached");
    }
    fn kill_at(stage: &str) {
        let root = std::env::temp_dir().join(format!("kda-unit-crash-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let path = root.join("sessions.db");
        let db = SessionManager::open_at(path.clone()).unwrap();
        db.conn.execute("INSERT INTO sessions(id,title,created_at,updated_at) VALUES('fixture','fixture','now','now')",[]).unwrap();
        db.add_message(
            "fixture",
            &crate::models::Message::assistant("already complete"),
        )
        .unwrap();
        if stage == "recovery" {
            db.begin_turn("crash", Some("fixture"), None).unwrap();
            let op = db.prepare_operation("crash", "create_file").unwrap();
            db.transition_operation(&op, OperationState::Authorized, "auto", None)
                .unwrap();
            db.transition_operation(&op, OperationState::Dispatched, "auto", None)
                .unwrap();
        }
        if stage == "migration" {
            db.conn.execute_batch("DROP TABLE operation_events; DROP TABLE operation_journal; DROP TABLE turn_recovery; DROP TABLE recovery_migrations;").unwrap();
        }
        drop(db);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "backend::operation_journal::tests::crash_worker",
                "--ignored",
                "--nocapture",
            ])
            .env("KDA_F1B3_UNIT_ROOT", &root)
            .env("KDA_F1B3_UNIT_STAGE", stage)
            .env_remove("KDA_F1B3_HOOK")
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.join("ready").exists() {
            if child.try_wait().unwrap().is_some() {
                panic!("child failed before checkpoint");
            }
            if Instant::now() > deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("checkpoint timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill().unwrap();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(child.wait().unwrap().signal(), Some(9));
        let sql = Connection::open(&path).unwrap();
        if stage == "migration" {
            assert_eq!(
                sql.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name='operation_journal'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
        }
        if stage == "recovery" || stage == "audit" {
            assert_eq!(
                sql.query_row("SELECT state FROM operation_journal", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                "dispatched"
            );
        }
        if stage == "transaction" {
            assert_eq!(
                sql.query_row("SELECT COUNT(*) FROM operation_journal", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                sql.query_row("SELECT COUNT(*) FROM tool_audit", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        for _ in 0..3 {
            let db = SessionManager::open_at(path.clone()).unwrap();
            assert_eq!(db.get_messages("fixture").unwrap().len(), 1);
            let ops = db.list_operations(None, 50).unwrap();
            if stage == "recovery" || stage == "audit" {
                assert_eq!(ops[0].state, "unknown_outcome");
            }
            assert_eq!(
                db.conn
                    .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
            assert_eq!(
                db.conn
                    .query_row(
                        "SELECT COUNT(*) FROM recovery_migrations WHERE version=3",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                1
            );
            assert_eq!(
                db.conn
                    .query_row(
                        "SELECT COUNT(*) FROM operation_events WHERE state='unknown_outcome'",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                i64::from(stage == "recovery" || stage == "audit")
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sigkill_during_actual_migration() {
        kill_at("migration");
    }
    #[test]
    fn sigkill_during_actual_recovery() {
        kill_at("recovery");
    }
    #[test]
    fn sigkill_before_journal_commit() {
        kill_at("transaction");
    }
    #[test]
    fn sigkill_during_actual_audit_write() {
        kill_at("audit");
    }

    #[test]
    fn full_and_commit_errors_rollback_and_release_connection() {
        let db = SessionManager::in_memory_for_tests().unwrap();
        let pages: i64 = db
            .conn
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap();
        db.conn
            .pragma_update(None, "max_page_count", pages)
            .unwrap();
        let error=atomic(&db.conn,|| {
            db.conn.execute("INSERT INTO user_facts(key,value,updated_at) VALUES('full',zeroblob(1000000),'now')",[])?;
            Ok(())
        }).unwrap_err();
        let sqlite = error.downcast_ref::<rusqlite::Error>().unwrap();
        assert_eq!(
            sqlite.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DiskFull)
        );
        assert!(db.conn.is_autocommit());
        db.conn.execute_batch("PRAGMA max_page_count=100000; PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED)").unwrap();
        assert!(atomic(&db.conn, || {
            db.conn.execute("INSERT INTO child VALUES(42)", [])?;
            Ok(())
        })
        .is_err());
        assert!(db.conn.is_autocommit());
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM child", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(atomic(&db.conn, || {
            db.conn.execute("INSERT INTO parent VALUES(42)", [])?;
            Ok(())
        })
        .is_ok());
    }
    #[test]
    fn transaction_unwind_rolls_back_before_supervisor_continues() {
        let db = SessionManager::in_memory_for_tests().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<()> = atomic(&db.conn, || {
                db.conn.execute(
                    "INSERT INTO user_facts VALUES('panic','synthetic','now')",
                    [],
                )?;
                panic!("controlled unwind");
            });
        }));
        assert!(result.is_err());
        assert!(db.conn.is_autocommit());
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM user_facts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        db.create_turn("after-unwind", None).unwrap();
    }
}
