//! Private SQLite and controlled child processes only; no real backend capabilities.
use kde_assistant_lib::{
    backend::{
        conversation_turn::{Turn, TurnState},
        operation_journal::OperationState as State,
        session_manager::SessionManager,
        tool_executor::{ToolCtx, ToolExecutor},
    },
    models::{Config, Message, ToolCall},
};
use rusqlite::Connection;
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Private(PathBuf);
impl Private {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kda-f1b3a-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("sessions.db")
    }
    fn manager(&self) -> SessionManager {
        SessionManager::open_at(self.db()).unwrap()
    }
}
impl Drop for Private {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
fn fixture(p: &Private) -> (SessionManager, String) {
    let db = p.manager();
    let sid = db.create_session("fixture").unwrap().id;
    db.add_message(&sid, &Message::assistant("completed before crash"))
        .unwrap();
    (db, sid)
}
fn call(p: &Private) -> ToolCall {
    ToolCall {
        id: uuid::Uuid::new_v4().to_string(),
        name: "create_file".into(),
        arguments: serde_json::from_value(
            serde_json::json!({"path":p.0.join("effect.txt"),"content":"PRIVATE_FIXTURE_SECRET"}),
        )
        .unwrap(),
    }
}
fn executor(p: &Private, db: SessionManager) -> (ToolExecutor, Arc<Mutex<SessionManager>>) {
    let mut cfg = Config::default();
    cfg.tools.create_file = true;
    cfg.tools.allowed_paths = vec![p.0.display().to_string()];
    let db = Arc::new(Mutex::new(db));
    (
        ToolExecutor::with_audit(Arc::new(tokio::sync::RwLock::new(cfg)), db.clone()),
        db,
    )
}
fn execute_approved(
    exec: &ToolExecutor,
    db: &Arc<Mutex<SessionManager>>,
    sid: &str,
    call: &ToolCall,
) -> anyhow::Result<kde_assistant_lib::models::ToolResult> {
    let turn = Turn::new(Some(sid.to_owned()));
    turn.transition(TurnState::Running)?;
    db.lock().unwrap().begin_turn(&turn.id, Some(sid), None)?;
    let (op, consent) = exec.prepare_for_turn_with_consent(&turn.id, call, "chat")?;
    turn.transition(TurnState::WaitingForApproval)?;
    db.lock().unwrap().answer_permission(
        &consent.ok_or_else(|| anyhow::anyhow!("missing consent"))?,
        &call.id,
        &turn.id,
        Some(sid),
        true,
    )?;
    turn.transition(TurnState::Running)?;
    let result = rt().block_on(exec.execute_for_turn(
        call,
        &ToolCtx::auto(Some(sid.to_owned())),
        &turn,
        &op,
    ));
    db.lock().unwrap().settle_turn(
        &turn.id,
        if result.is_ok() {
            "completed"
        } else {
            "failed"
        },
    )?;
    result
}
fn integrity(p: &Private) {
    let sql = Connection::open(p.db()).unwrap();
    assert_eq!(
        sql.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert_eq!(
        sql.query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .unwrap(),
        None
    );
}
use rusqlite::OptionalExtension;

#[test]
#[ignore = "controlled crash worker; invoked only by this suite"]
fn crash_worker() {
    let root = PathBuf::from(std::env::var_os("KDA_F1B3_ROOT").unwrap());
    let stage = std::env::var("KDA_F1B3_STAGE").unwrap();
    let db = SessionManager::open_at(root.join("sessions.db")).unwrap();
    let sid = std::env::var("KDA_F1B3_SESSION").unwrap();
    db.begin_turn(
        "interrupted",
        Some(&sid),
        Some(&Message::user("pending user")),
    )
    .unwrap();
    let pause = || {
        fs::write(root.join("ready"), b"ready").unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    if stage == "before_intent" {
        pause();
    }
    let id = db.prepare_operation("interrupted", "create_file").unwrap();
    fs::write(root.join("operation"), &id).unwrap();
    if stage == "prepared" {
        pause();
    }
    db.transition_operation(&id, State::Authorized, "auto", None)
        .unwrap();
    if stage == "authorized" {
        pause();
    }
    if stage == "transaction" {
        // A real, uncommitted SQLite transaction containing intermediate rows.
        let sql = Connection::open(root.join("sessions.db")).unwrap();
        sql.execute_batch("BEGIN IMMEDIATE").unwrap();
        sql.execute(
            "UPDATE operation_journal SET state='dispatched' WHERE id=?1",
            [&id],
        )
        .unwrap();
        sql.execute("INSERT INTO messages(session_id,role,content,timestamp) VALUES(?1,'assistant','uncommitted','now')",[&sid]).unwrap();
        pause();
    }
    db.transition_operation(&id, State::Dispatched, "auto", None)
        .unwrap();
    if stage == "dispatch_without_effect" {
        pause();
    }
    // Controlled executor: append exactly once; a replay would be observable.
    use std::io::Write;
    let mut effect = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("effect.txt"))
        .unwrap();
    effect.write_all(b"one effect\n").unwrap();
    effect.sync_all().unwrap();
    if stage == "dispatched" {
        pause();
    }
    if stage == "cancellation" {
        db.finish_turn("interrupted", "cancelling").unwrap();
        pause();
    }
    if stage == "audit" {
        let sql = Connection::open(root.join("sessions.db")).unwrap();
        sql.execute_batch("BEGIN IMMEDIATE").unwrap();
        sql.execute("UPDATE tool_audit SET success=1,decided='auto' WHERE id=(SELECT audit_id FROM operation_journal WHERE id=?1)",[&id]).unwrap();
        pause();
    }
    db.transition_operation(
        &id,
        if stage == "failed_result" {
            State::Failed
        } else {
            State::Succeeded
        },
        "auto",
        Some(10),
    )
    .unwrap();
    pause();
}
fn crash(stage: &str, expected: Option<&str>, effect: bool) {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    drop(db);
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", "crash_worker", "--ignored", "--nocapture"])
        .env("KDA_F1B3_ROOT", &p.0)
        .env("KDA_F1B3_STAGE", stage)
        .env("KDA_F1B3_SESSION", &sid)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    for (key, _) in std::env::vars() {
        if key.ends_with("_API_KEY") || key.to_uppercase().ends_with("_PROXY") {
            cmd.env_remove(key);
        }
    }
    for (key, name) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        cmd.env(key, p.0.join(name));
    }
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !p.0.join("ready").exists() {
        if child.try_wait().unwrap().is_some() {
            panic!("worker exited before checkpoint");
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("worker checkpoint timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Child::kill is SIGKILL on Unix; only the exact child we created is targeted.
    child.kill().unwrap();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(child.wait().unwrap().signal(), Some(9));
    for _ in 0..3 {
        let db = p.manager();
        let ops = db.list_operations(Some(&sid), 50).unwrap();
        if let Some(expected) = expected {
            assert_eq!(ops.len(), 1);
            assert_eq!(ops[0].state, expected);
        } else {
            assert!(ops.is_empty());
        }
        let messages = db.get_messages(&sid).unwrap();
        assert_eq!(messages.len(), 2);
        assert!(
            matches!(&messages[0],Message::Assistant{content,..} if content == "completed before crash")
        );
        let sql = Connection::open(p.db()).unwrap();
        assert_eq!(
            sql.query_row(
                "SELECT state FROM conversation_turns WHERE id='interrupted'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "failed"
        );
        if expected == Some("unknown_outcome") {
            assert!(!db.regeneration_is_safe(&sid).unwrap());
            assert!(db
                .publish_turn(
                    Some(&sid),
                    "interrupted",
                    &[Message::assistant("invented success")]
                )
                .is_err());
        }
        if effect {
            assert_eq!(fs::read(p.0.join("effect.txt")).unwrap(), b"one effect\n");
        } else {
            assert!(!p.0.join("effect.txt").exists());
        }
        let registry = kde_assistant_lib::backend::conversation_turn::TurnRegistry::default();
        assert!(registry.reserve(Some(sid.clone())).is_ok());
        integrity(&p);
    }
}
#[test]
fn crash_before_intent() {
    crash("before_intent", None, false);
}
#[test]
fn crash_after_intent() {
    crash("prepared", Some("abandoned"), false);
}
#[test]
fn crash_after_authorization() {
    crash("authorized", Some("abandoned"), false);
}
#[test]
fn crash_after_dispatch() {
    crash("dispatched", Some("unknown_outcome"), true);
}
#[test]
fn crash_after_result_before_turn_finalization() {
    crash("result", Some("succeeded"), true);
}
#[test]
fn crash_during_sqlite_transaction() {
    crash("transaction", Some("abandoned"), false);
}
#[test]
fn crash_during_cancellation() {
    crash("cancellation", Some("unknown_outcome"), true);
}
#[test]
fn crash_during_audit_transaction() {
    crash("audit", Some("unknown_outcome"), true);
}

#[test]
fn insert_and_dispatch_failures_never_execute() {
    for sql in [
        "CREATE TRIGGER inject BEFORE INSERT ON operation_journal BEGIN SELECT RAISE(ABORT,'insert_failure'); END;",
        "CREATE TRIGGER inject BEFORE INSERT ON tool_audit BEGIN SELECT RAISE(ABORT,'audit_failure'); END;",
        "CREATE TRIGGER inject BEFORE UPDATE ON operation_journal WHEN NEW.state='dispatched' BEGIN SELECT RAISE(ROLLBACK,'dispatch_failure'); END;",
    ] {
        let p = Private::new();let (db,sid)=fixture(&p);let (exec,db)=executor(&p,db);
        Connection::open(p.db()).unwrap().execute_batch(sql).unwrap();
        assert!(execute_approved(&exec,&db,&sid,&call(&p)).is_err());
        assert!(!p.0.join("effect.txt").exists());
        Connection::open(p.db()).unwrap().execute_batch("DROP TRIGGER inject").unwrap();
        assert!(db.lock().unwrap().create_turn("after-error",None).is_ok());
        integrity(&p);
    }
}
#[test]
fn result_or_audit_failure_never_reports_success() {
    for sql in [
        "CREATE TRIGGER inject BEFORE UPDATE ON operation_journal WHEN NEW.state='succeeded' BEGIN SELECT RAISE(ABORT,'result_failure'); END;",
        "CREATE TRIGGER inject BEFORE UPDATE ON tool_audit BEGIN SELECT RAISE(ROLLBACK,'audit_failure'); END;",
    ] {
        let p=Private::new();let (db,sid)=fixture(&p);let (exec,db)=executor(&p,db);
        Connection::open(p.db()).unwrap().execute_batch(sql).unwrap();
        let err=execute_approved(&exec,&db,&sid,&call(&p)).unwrap_err();
        assert!(err.to_string().contains("operation_result_not_persisted"));
        assert!(p.0.join("effect.txt").exists());
        let ops=db.lock().unwrap().list_operations(Some(&sid),50).unwrap();
        assert_eq!(ops[0].state,"unknown_outcome");
        assert!(!db.lock().unwrap().list_tool_audit(10).unwrap()[0].success);
        assert_eq!(db.lock().unwrap().get_messages(&sid).unwrap().len(),1);
        Connection::open(p.db()).unwrap().execute_batch("DROP TRIGGER inject").unwrap();
        integrity(&p);
    }
}
#[test]
fn simulated_disk_full_fails_closed_and_rolls_back() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    let (exec, db) = executor(&p, db);
    let sql = Connection::open(p.db()).unwrap();
    // Model the failed write boundary deterministically, without filling a real disk.
    sql.execute_batch("CREATE TRIGGER disk_failure BEFORE INSERT ON operation_events BEGIN SELECT RAISE(ABORT,'simulated SQLITE_FULL'); END;").unwrap();
    assert!(execute_approved(&exec, &db, &sid, &call(&p)).is_err());
    assert!(db
        .lock()
        .unwrap()
        .list_operations(Some(&sid), 50)
        .unwrap()
        .is_empty());
    assert!(db.lock().unwrap().list_tool_audit(10).unwrap().is_empty());
    assert!(!p.0.join("effect.txt").exists());
    sql.execute_batch("DROP TRIGGER disk_failure").unwrap();
    assert!(
        execute_approved(&exec, &db, &sid, &call(&p))
            .unwrap()
            .success
    );
    integrity(&p);
}
#[test]
fn busy_database_times_out_without_effect_and_recovers() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    let (exec, _db) = executor(&p, db);
    let lock = Connection::open(p.db()).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let start = Instant::now();
    assert!(execute_approved(&exec, &_db, &sid, &call(&p)).is_err());
    assert!(start.elapsed() >= Duration::from_secs(4));
    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(!p.0.join("effect.txt").exists());
    lock.execute_batch("ROLLBACK").unwrap();
    assert!(
        execute_approved(&exec, &_db, &sid, &call(&p))
            .unwrap()
            .success
    );
    integrity(&p);
}
#[test]
fn cancellation_before_dispatch_is_persisted_as_abandoned() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    let (exec, db) = executor(&p, db);
    let turn = Turn::new(Some(sid.clone()));
    turn.transition(TurnState::Running).unwrap();
    db.lock()
        .unwrap()
        .begin_turn(&turn.id, Some(&sid), None)
        .unwrap();
    let tc = call(&p);
    let op = exec.prepare_for_turn(&turn.id, &tc).unwrap();
    turn.cancel();
    assert!(rt()
        .block_on(exec.execute_for_turn(&tc, &ToolCtx::auto(Some(sid)), &turn, &op))
        .is_err());
    db.lock()
        .unwrap()
        .settle_turn(&turn.id, "cancelled")
        .unwrap();
    assert_eq!(
        db.lock().unwrap().list_operations(None, 50).unwrap()[0].state,
        "abandoned"
    );
    assert!(!p.0.join("effect.txt").exists());
    integrity(&p);
}
#[test]
fn privacy_denials_transitions_and_schema_are_enforced() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    let (exec, db) = executor(&p, db);
    assert!(
        execute_approved(&exec, &db, &sid, &call(&p))
            .unwrap()
            .success
    );
    exec.record_denial(&call(&p), Some(sid.clone())).unwrap();
    let db = db.lock().unwrap();
    let ops = db.list_operations(Some(&sid), 50).unwrap();
    assert_eq!(ops.len(), 2);
    assert!(ops.iter().any(|o| o.state == "denied"));
    for op in &ops {
        assert!(uuid::Uuid::parse_str(&op.id).is_ok());
        assert!(db
            .transition_operation(&op.id, State::Dispatched, "auto", None)
            .is_err());
    }
    let serialized = serde_json::to_string(&ops).unwrap();
    assert!(!serialized.contains("PRIVATE_FIXTURE_SECRET"));
    assert!(!serialized.contains("effect.txt"));
    for audit in db.list_tool_audit(10).unwrap() {
        assert_eq!(audit.args, "[redacted]");
    }
    let sql = Connection::open(p.db()).unwrap();
    assert!(sql
        .execute("UPDATE operation_journal SET state='prepared'", [])
        .is_err());
    assert!(sql.execute("INSERT INTO operation_events(operation_id,state,timestamp,audit_id) VALUES('missing','succeeded','now',1)",[]).is_err());
    assert_eq!(
        sql.query_row("SELECT COUNT(*) FROM operation_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        6
    );
    integrity(&p);
}
#[test]
fn second_backend_cannot_recover_live_operations() {
    let p = Private::new();
    let (db, _) = fixture(&p);
    assert!(SessionManager::open_at(p.db()).is_err());
    drop(db);
    assert!(SessionManager::open_at(p.db()).is_ok());
}

#[test]
fn retries_keep_the_original_user_link_and_cannot_hide_effects() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    db.begin_turn("original", Some(&sid), Some(&Message::user("request")))
        .unwrap();
    db.settle_turn("original", "failed").unwrap();
    assert!(db.regeneration_is_safe(&sid).unwrap());
    db.begin_retry_turn("retry", &sid).unwrap();
    let op = db.prepare_operation("retry", "create_file").unwrap();
    db.transition_operation(&op, State::Authorized, "auto", None)
        .unwrap();
    db.transition_operation(&op, State::Dispatched, "auto", None)
        .unwrap();
    db.settle_turn("retry", "failed").unwrap();
    assert!(!db.regeneration_is_safe(&sid).unwrap());
    assert!(db.begin_retry_turn("retry-again", &sid).is_err());
    db.begin_turn(
        "explicit-new",
        Some(&sid),
        Some(&Message::user("inspected effect; new explicit request")),
    )
    .unwrap();
    assert!(db.regeneration_is_safe(&sid).unwrap());
    integrity(&p);
}
#[test]
fn retry_rechecks_unanswered_input_inside_transaction() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    db.begin_turn("original", Some(&sid), Some(&Message::user("request")))
        .unwrap();
    db.settle_turn("original", "failed").unwrap();
    assert!(db.regeneration_is_safe(&sid).unwrap());
    // A manual attachment can change history while HTTP awaits the config lock.
    db.add_message(&sid, &Message::tool("manual", "attachment"))
        .unwrap();
    assert!(db.begin_retry_turn("stale-retry", &sid).is_err());
    let sql = Connection::open(p.db()).unwrap();
    assert_eq!(
        sql.query_row(
            "SELECT COUNT(*) FROM conversation_turns WHERE id='stale-retry'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    integrity(&p);
}

#[test]
fn legacy_user_retry_remains_blocked_after_repeated_restarts() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    db.add_message(&sid, &Message::user("legacy user without Turn ID"))
        .unwrap();
    db.begin_retry_turn("legacy-retry", &sid).unwrap();
    let op = db.prepare_operation("legacy-retry", "create_file").unwrap();
    db.transition_operation(&op, State::Authorized, "auto", None)
        .unwrap();
    db.transition_operation(&op, State::Dispatched, "auto", None)
        .unwrap();
    db.settle_turn("legacy-retry", "cancelled").unwrap();
    drop(db);
    for _ in 0..3 {
        let db = p.manager();
        assert!(!db.regeneration_is_safe(&sid).unwrap());
        assert!(db.begin_retry_turn("forbidden-retry", &sid).is_err());
        assert_eq!(
            db.list_operations(Some(&sid), 50).unwrap()[0].state,
            "unknown_outcome"
        );
        assert_eq!(db.get_messages(&sid).unwrap().len(), 2);
    }
    integrity(&p);
}

#[test]
fn nonexistent_session_cannot_dispatch_or_persist_untrusted_metadata() {
    let p = Private::new();
    let (db, _) = fixture(&p);
    let (exec, db) = executor(&p, db);
    let result = rt().block_on(exec.execute(
        &call(&p),
        &ToolCtx::auto(Some("/private/arbitrary-session-secret".into())),
    ));
    assert!(result.is_err());
    assert!(!p.0.join("effect.txt").exists());
    assert!(db
        .lock()
        .unwrap()
        .list_operations(None, 50)
        .unwrap()
        .is_empty());
    integrity(&p);
}
#[test]
fn recovery_error_rolls_back_all_changes_and_next_start_can_recover() {
    let p = Private::new();
    let (db, sid) = fixture(&p);
    db.begin_turn("interrupted", Some(&sid), Some(&Message::user("pending")))
        .unwrap();
    let op = db.prepare_operation("interrupted", "create_file").unwrap();
    db.transition_operation(&op, State::Authorized, "auto", None)
        .unwrap();
    db.transition_operation(&op, State::Dispatched, "auto", None)
        .unwrap();
    drop(db);
    let sql = Connection::open(p.db()).unwrap();
    sql.execute_batch("CREATE TRIGGER fail_recovery BEFORE INSERT ON turn_recovery BEGIN SELECT RAISE(ABORT,'recovery_failure'); END;").unwrap();
    assert!(SessionManager::open_at(p.db()).is_err());
    assert_eq!(
        sql.query_row("SELECT state FROM operation_journal", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "dispatched"
    );
    assert_eq!(
        sql.query_row(
            "SELECT COUNT(*) FROM operation_events WHERE state='unknown_outcome'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    sql.execute_batch("DROP TRIGGER fail_recovery").unwrap();
    assert_eq!(
        p.manager().list_operations(None, 50).unwrap()[0].state,
        "unknown_outcome"
    );
    integrity(&p);
}
#[test]
fn legacy_migration_preserves_messages_and_explicit_storage_settings() {
    let p = Private::new();
    let sql = Connection::open(p.db()).unwrap();
    sql.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL); CREATE TABLE messages(id INTEGER PRIMARY KEY AUTOINCREMENT,session_id TEXT NOT NULL,role TEXT NOT NULL,content TEXT NOT NULL,tool_name TEXT,tool_call_id TEXT,tool_result TEXT,image_url TEXT,timestamp TEXT NOT NULL); INSERT INTO sessions VALUES('old','old','now','now'); INSERT INTO messages(session_id,role,content,timestamp) VALUES('old','assistant','legacy completed','now');").unwrap();
    for _ in 0..3 {
        let db = p.manager();
        assert_eq!(db.get_messages("old").unwrap().len(), 1);
        assert_eq!(
            sql.query_row("SELECT turn_id FROM messages", [], |r| r
                .get::<_, Option<String>>(0))
                .unwrap(),
            None
        );
        assert_eq!(
            sql.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        assert_eq!(
            sql.query_row(
                "SELECT COUNT(*) FROM recovery_migrations WHERE version=3",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
    integrity(&p);
}

fn isolated_http(name: &str) -> bool {
    if std::env::var("KDA_F1B3_HTTP").as_deref() == Ok(name) {
        return false;
    }
    let p = Private::new();
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", name, "--nocapture"])
        .env("KDA_F1B3_HTTP", name);
    for (key, _) in std::env::vars() {
        if key.ends_with("_API_KEY") || key.to_uppercase().ends_with("_PROXY") {
            cmd.env_remove(key);
        }
    }
    for (key, name) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        cmd.env(key, p.0.join(name));
    }
    cmd.env("ORT_DYLIB_PATH", p.0.join("absent.so"));
    assert!(cmd.status().unwrap().success());
    true
}
async fn http_journal(fail_result: bool) {
    use axum::{routing::post, Router};
    use kde_assistant_lib::backend::{http_server, Backend};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let p = Private::new();
    let file = p.0.join("http-effect.txt");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = Config::first_run();
    cfg.ai.provider = "custom".into();
    cfg.ai.api_key = "synthetic-fixture".into();
    cfg.ai.base_url = format!("http://{}", listener.local_addr().unwrap());
    cfg.ai.enable_tool_calling = true;
    cfg.tools.create_file = true;
    cfg.tools.confirm_sensitive = false;
    cfg.tools.allowed_paths = vec![p.0.display().to_string()];
    cfg.save().await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let destination = file.clone();
    let app=Router::new().route("/chat/completions",post(move || {
        let calls=observed.clone();let file=destination.clone();async move {
            if calls.fetch_add(1,Ordering::SeqCst)==0 {
                format!("data: {}\n\ndata: [DONE]\n\n",serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"untrusted-provider-id","function":{"name":"create_file","arguments":serde_json::json!({"path":file,"content":"PRIVATE_HTTP_SECRET"}).to_string()}}]},"finish_reason":"tool_calls"}]}))
            } else {"data: {\"choices\":[{\"delta\":{\"content\":\"completed\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".into()}
        }
    }));
    let provider = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let backend = Backend::new().await.unwrap();
    let state = backend.http_state().unwrap();
    let sid = backend
        .sessions
        .lock()
        .unwrap()
        .create_session("HTTP journal")
        .unwrap()
        .id;
    let dbpath =
        PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap()).join("kde-assistant/sessions.db");
    if fail_result {
        Connection::open(&dbpath).unwrap().execute_batch("CREATE TRIGGER fail_result BEFORE UPDATE ON operation_journal WHEN NEW.state='succeeded' BEGIN SELECT RAISE(ABORT,'result_failure'); END;").unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = http_server::router(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let response = client
        .post(format!("{base}/api/chat"))
        .header("Host", "127.0.0.1:8765")
        .bearer_auth(&state.local_token)
        .json(&serde_json::json!({"session_id":sid,"message":"synthetic request"}))
        .send()
        .await
        .unwrap();
    let collected = tokio::spawn(async move { response.text().await.unwrap() });
    tokio::time::timeout(Duration::from_secs(3), async {
        while state.approvals.pending_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (consent_id,tool_call_id,turn_id):(String,String,String)=Connection::open(&dbpath).unwrap().query_row(
        "SELECT p.consent_id,p.tool_call_id,j.turn_id FROM permission_requests p JOIN operation_journal j ON j.id=p.operation_id WHERE j.session_id=?1 AND p.state='pending' ORDER BY p.created_at DESC LIMIT 1",
        [&sid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(client.post(format!("{base}/api/tools/approve"))
        .header("Host","127.0.0.1:8765").bearer_auth(&state.local_token)
        .json(&serde_json::json!({"consent_id":consent_id,"tool_call_id":tool_call_id,"turn_id":turn_id,"session_id":sid,"approved":true}))
        .send().await.unwrap().status(),200);
    let stream = collected.await.unwrap();
    assert!(file.exists());
    if fail_result {
        assert!(stream.contains("event: error"));
        assert!(!stream.contains("event: done"));
        assert!(!stream.contains("event: tool_result"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    } else {
        assert!(stream.contains("event: done"));
        assert!(stream.contains("event: tool_result"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
    let url = format!("{base}/api/tools/operations?session_id={sid}");
    assert_eq!(
        client
            .get(&url)
            .header("Host", "127.0.0.1:8765")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let records: serde_json::Value = client
        .get(&url)
        .header("Host", "127.0.0.1:8765")
        .bearer_auth(&state.local_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1);
    for session in ["nonexistent-session", "' OR 1=1 --"] {
        let response = client
            .get(format!("{base}/api/tools/operations"))
            .query(&[("session_id", session), ("limit", "100000")])
            .header("Host", "127.0.0.1:8765")
            .bearer_auth(&state.local_token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.json::<serde_json::Value>().await.unwrap(),
            serde_json::json!([])
        );
    }
    assert_eq!(
        records[0]["state"],
        if fail_result {
            "unknown_outcome"
        } else {
            "succeeded"
        }
    );
    let id = records[0]["turn_id"].as_str().unwrap();
    assert!(stream.contains(id));
    assert!(!records.to_string().contains("PRIVATE_HTTP_SECRET"));
    assert_eq!(
        backend
            .sessions
            .lock()
            .unwrap()
            .get_messages(&sid)
            .unwrap()
            .len(),
        if fail_result { 1 } else { 4 }
    );
    assert_eq!(state.chat_tasks.active_count(), 0);
    server.abort();
    provider.abort();
    let _ = server.await;
    let _ = provider.await;
    let old_database = Arc::downgrade(&backend.sessions);
    drop(client);
    drop(state);
    drop(backend);
    tokio::time::timeout(Duration::from_secs(3), async {
        while old_database.upgrade().is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let restored = SessionManager::open_at(dbpath).unwrap();
    assert_eq!(
        restored.list_operations(Some(&sid), 50).unwrap()[0].state,
        if fail_result {
            "unknown_outcome"
        } else {
            "succeeded"
        }
    );
    assert_eq!(fs::read_to_string(file).unwrap(), "PRIVATE_HTTP_SECRET");
}
#[test]
fn http_success_and_authenticated_journal_match_turn() {
    if isolated_http("http_success_and_authenticated_journal_match_turn") {
        return;
    }
    rt().block_on(http_journal(false));
}
#[test]
fn http_result_write_failure_has_no_tool_success_or_done() {
    if isolated_http("http_result_write_failure_has_no_tool_success_or_done") {
        return;
    }
    rt().block_on(http_journal(true));
}

#[test]
fn http_manual_denial_is_persisted_and_write_failure_is_closed() {
    if isolated_http("http_manual_denial_is_persisted_and_write_failure_is_closed") {
        return;
    }
    rt().block_on(async {
        use kde_assistant_lib::backend::{http_server, Backend};
        Config::first_run().save().await.unwrap();
        let backend = Backend::new().await.unwrap();
        let state = backend.http_state().unwrap();
        let sid = backend.sessions.lock().unwrap().create_session("denial").unwrap().id;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = http_server::router(state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap();
        let private = Private::new();
        let effect = private.0.join("denied.txt");
        let request = || client.post(format!("{base}/api/tools/execute"))
            .header("Host", "127.0.0.1:8765").bearer_auth(&state.local_token)
            .json(&serde_json::json!({"name":"edit_file","session_id":sid,"arguments":{"path":effect,"mode":"append","content":"PRIVATE_DENIAL_SECRET"}}));
        assert_eq!(request().send().await.unwrap().status(), 403);
        let records = backend.sessions.lock().unwrap().list_operations(Some(&sid), 50).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].state, "denied");
        assert!(!effect.exists());
        let sql = Connection::open(PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap()).join("kde-assistant/sessions.db")).unwrap();
        sql.execute_batch("CREATE TRIGGER reject_denial BEFORE UPDATE ON operation_journal WHEN NEW.state='denied' BEGIN SELECT RAISE(ABORT,'denial_failure'); END;").unwrap();
        assert_eq!(request().send().await.unwrap().status(), 500);
        assert!(!effect.exists());
        assert_eq!(sql.query_row("SELECT COUNT(*) FROM operation_events WHERE state='dispatched'", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        assert_eq!(sql.query_row("SELECT COUNT(*) FROM operation_journal WHERE state='denied'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        server.abort();
        let _ = server.await;
    });
}

#[test]
fn dispatched_marker_without_an_effect_is_still_unknown() {
    crash("dispatch_without_effect", Some("unknown_outcome"), false);
}
#[test]
fn persisted_failure_survives_without_claiming_effect_rollback() {
    crash("failed_result", Some("failed"), true);
}
