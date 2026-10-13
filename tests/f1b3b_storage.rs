//! Synthetic private SQLite and resources. Never touches user conversations.
use kde_assistant_lib::{
    backend::{
        conversation_turn::TurnRegistry,
        history_context::{validate_groups, validate_request},
        operation_journal::OperationState as State,
        resource_cache,
        session_manager::SessionManager,
    },
    models::{Config, Message, ToolCall},
};
use rusqlite::{Connection, OptionalExtension};
use std::{
    fs,
    os::unix::fs::{symlink, DirBuilderExt},
    path::PathBuf,
    process::Command,
    time::Duration,
};
struct Private(PathBuf);
impl Private {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kda-f1b3b-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("sessions.db")
    }
    fn manager(&self) -> SessionManager {
        SessionManager::open_at(self.db()).unwrap()
    }
    fn sql(&self) -> Connection {
        Connection::open(self.db()).unwrap()
    }
}
impl Drop for Private {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixture() -> (Private, SessionManager, String) {
    let p = Private::new();
    let db = p.manager();
    let id = db.create_session("private").unwrap().id;
    (p, db, id)
}
fn call(id: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: "system_info".into(),
        arguments: Default::default(),
    }
}
fn unit(db: &SessionManager, sid: &str, label: &str, tool: bool) {
    let id = uuid::Uuid::new_v4().to_string();
    db.begin_turn(&id, Some(sid), Some(&Message::user(label)))
        .unwrap();
    let mut output = vec![];
    if tool {
        output.extend([
            Message::assistant_with_tools("", vec![call("a"), call("b")]),
            Message::tool("b", "FAILURE: no effect confirmed"),
            Message::tool("a", "result"),
        ]);
    }
    output.push(Message::assistant(format!("answer {label}")));
    db.publish_turn(Some(sid), &id, &output).unwrap();
}
fn select(db: &SessionManager, sid: &str, cfg: &Config) -> anyhow::Result<Vec<Message>> {
    db.select_context(
        Some(sid),
        vec![Message::system("essential")],
        Some(Message::user("new explicit input")),
        &cfg.ai,
        &[],
    )
}
fn integrity(p: &Private) {
    let sql = p.sql();
    assert_eq!(
        sql.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(sql
        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
        .optional()
        .unwrap()
        .is_none());
}
fn unknown(db: &SessionManager, sid: &str, turn: &str) -> String {
    db.begin_turn(turn, Some(sid), Some(&Message::user("private request")))
        .unwrap();
    let op = db.prepare_operation(turn, "create_file").unwrap();
    db.authorize_operation(&op, turn, "create_file", "auto")
        .unwrap();
    db.transition_operation(&op, State::Dispatched, "auto", None)
        .unwrap();
    db.settle_turn(turn, "failed").unwrap();
    op
}
#[test]
fn short_history_preserves_multi_call_causality_and_failures() {
    let (_p, db, sid) = fixture();
    unit(&db, &sid, "one", true);
    let msgs = select(&db, &sid, &Config::default()).unwrap();
    validate_groups(&msgs).unwrap();
    assert_eq!(msgs.len(), 7);
    assert!(serde_json::to_string(&msgs).unwrap().contains("FAILURE"));
}
#[test]
fn long_history_selects_complete_suffix_without_summary() {
    let (_p, db, sid) = fixture();
    for i in 0..100 {
        unit(&db, &sid, &format!("turn-{i}"), true);
    }
    db.set_summary(&sid, "SILENT_SUMMARY_SECRET", 500).unwrap();
    let mut cfg = Config::default();
    cfg.ai.context_window_tokens = 5000;
    cfg.ai.max_tokens = 1000;
    let msgs = select(&db, &sid, &cfg).unwrap();
    validate_groups(&msgs).unwrap();
    let text = serde_json::to_string(&msgs).unwrap();
    assert!(text.contains("turn-99"));
    assert!(!text.contains("turn-0\""));
    assert!(!text.contains("SILENT_SUMMARY_SECRET"));
    assert_eq!(db.count_messages(&sid).unwrap(), 500);
}
#[test]
fn cutoff_never_splits_assistant_and_results() {
    let (_p, db, sid) = fixture();
    unit(&db, &sid, &"old".repeat(1000), true);
    unit(&db, &sid, "recent", true);
    let mut cfg = Config::default();
    cfg.ai.context_window_tokens = 3000;
    cfg.ai.max_tokens = 500;
    let msgs = select(&db, &sid, &cfg).unwrap();
    assert_eq!(
        msgs.iter()
            .filter(|m| matches!(m, Message::Tool { .. }))
            .count(),
        2
    );
    validate_groups(&msgs).unwrap();
}
#[test]
fn oversized_latest_input_and_response_fail_explicitly() {
    let (p, db, sid) = fixture();
    unit(&db, &sid, &"x".repeat(10000), false);
    let mut cfg = Config::default();
    cfg.ai.context_window_tokens = 2000;
    cfg.ai.max_tokens = 500;
    assert!(select(&db, &sid, &cfg)
        .unwrap_err()
        .to_string()
        .contains("budget"));
    cfg.ai.max_tokens = cfg.ai.context_window_tokens;
    assert!(select(&db, &sid, &cfg).is_err());
    let legacy = db.create_session("oversized legacy output").unwrap().id;
    db.add_message(&legacy, &Message::user("legacy")).unwrap();
    db.add_message(&legacy, &Message::assistant("")).unwrap();
    // The large invalid tool-call payload must be rejected by size before JSON parsing.
    p.sql().execute("UPDATE messages SET tool_result=CAST(zeroblob(4*1024*1024) AS TEXT) WHERE session_id=?1 AND role='assistant'",[&legacy]).unwrap();
    let error = select(&db, &legacy, &Config::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("context_budget_exceeded"), "{error}");
    integrity(&p);
}
#[test]
fn essential_instructions_cannot_be_cropped() {
    let (_p, db, sid) = fixture();
    db.add_message(&sid, &Message::system("security".repeat(2000)))
        .unwrap();
    unit(&db, &sid, "recent", false);
    let mut cfg = Config::default();
    cfg.ai.max_context_bytes = 1000;
    assert!(select(&db, &sid, &cfg).is_err());
}
#[test]
fn legacy_null_turn_ids_remain_causal() {
    let (_p, db, sid) = fixture();
    for m in [
        Message::user("legacy"),
        Message::assistant_with_tools("", vec![call("old-id")]),
        Message::tool("old-id", "old result"),
        Message::assistant("old final"),
    ] {
        db.add_message(&sid, &m).unwrap();
    }
    let msgs = select(&db, &sid, &Config::default()).unwrap();
    assert_eq!(msgs.len(), 6);
    validate_groups(&msgs).unwrap();
}
#[test]
fn incomplete_legacy_groups_are_errors_never_fabricated() {
    let (p, db, sid) = fixture();
    db.add_message(&sid, &Message::user("old")).unwrap();
    db.add_message(
        &sid,
        &Message::assistant_with_tools("", vec![call("missing")]),
    )
    .unwrap();
    assert!(select(&db, &sid, &Config::default())
        .unwrap_err()
        .to_string()
        .contains("incomplete"));
    assert_eq!(db.count_messages(&sid).unwrap(), 2);
    p.sql()
        .execute(
            "UPDATE messages SET tool_result='invalid' WHERE role='assistant'",
            [],
        )
        .unwrap();
    assert!(select(&db, &sid, &Config::default())
        .unwrap_err()
        .to_string()
        .contains("malformed"));
}
#[test]
fn orphan_duplicate_and_missing_results_fail() {
    for msgs in [
        vec![Message::tool("a", "orphan")],
        vec![
            Message::assistant_with_tools("", vec![call("a")]),
            Message::tool("a", "one"),
            Message::tool("a", "duplicate"),
        ],
        vec![
            Message::assistant_with_tools("", vec![call("a"), call("b")]),
            Message::tool("a", "one"),
        ],
    ] {
        assert!(validate_groups(&msgs).is_err());
    }
}
#[test]
fn cancelled_failed_and_previous_retry_unknown_are_visible_in_context() {
    let (_p, db, sid) = fixture();
    db.begin_turn("original", Some(&sid), Some(&Message::user("unanswered")))
        .unwrap();
    db.settle_turn("original", "cancelled").unwrap();
    db.begin_retry_turn("retry", &sid).unwrap();
    let op = db.prepare_operation("retry", "create_file").unwrap();
    db.authorize_operation(&op, "retry", "create_file", "auto")
        .unwrap();
    db.transition_operation(&op, State::Dispatched, "auto", None)
        .unwrap();
    db.settle_turn("retry", "failed").unwrap();
    let text = serde_json::to_string(&select(&db, &sid, &Config::default()).unwrap()).unwrap();
    assert!(text.contains("inciertas"));
    assert!(!db.regeneration_is_safe(&sid).unwrap());
    assert_eq!(db.count_messages(&sid).unwrap(), 1);
}
#[test]
fn regeneration_input_is_bounded() {
    let (_p, db, sid) = fixture();
    db.add_message(&sid, &Message::user("x".repeat(5000)))
        .unwrap();
    assert!(db.retry_input(&sid, 1000).is_err());
    assert_eq!(db.retry_input(&sid, 6000).unwrap().len(), 5000);
}
#[test]
fn live_request_growth_is_checked_each_iteration() {
    let cfg = Config::default();
    let msgs = vec![
        Message::system("essential"),
        Message::user("x".repeat(40000)),
    ];
    assert!(validate_request(&cfg.ai, &msgs, &[], cfg.ai.max_tokens).is_err());
}
#[test]
fn uncertain_deletion_requires_confirmation_and_leaves_no_orphans() {
    let (p, db, sid) = fixture();
    unknown(&db, &sid, "one");
    unknown(&db, &sid, "two");
    p.sql()
        .execute(
            "INSERT INTO turn_recovery VALUES('one','running','fixture')",
            [],
        )
        .unwrap();
    let other = db.create_session("independent").unwrap().id;
    unit(&db, &other, "keep", true);
    assert!(db
        .delete_session(&sid)
        .unwrap_err()
        .to_string()
        .contains("confirmation"));
    assert_eq!(db.list_operations(Some(&sid), 50).unwrap().len(), 2);
    db.delete_session_confirmed(&sid, true).unwrap();
    for table in [
        "messages",
        "conversation_turns",
        "tool_audit",
        "operation_journal",
    ] {
        assert_eq!(
            p.sql()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE session_id=?1"),
                    [&sid],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
    assert_eq!(
        p.sql()
            .query_row("SELECT COUNT(*) FROM operation_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        p.sql()
            .query_row("SELECT COUNT(*) FROM turn_recovery", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        p.sql()
            .query_row("SELECT COUNT(*) FROM turn_inputs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(db.count_messages(&other).unwrap(), 5);
    integrity(&p);
}
#[test]
fn deletion_failure_mid_transaction_rolls_back_every_table() {
    let (p, db, sid) = fixture();
    unknown(&db, &sid, "one");
    p.sql().execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON messages BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(db.delete_session_confirmed(&sid, true).is_err());
    assert_eq!(db.list_operations(Some(&sid), 50).unwrap().len(), 1);
    assert_eq!(db.count_messages(&sid).unwrap(), 1);
    assert_eq!(
        p.sql()
            .query_row("SELECT COUNT(*) FROM operation_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        4
    );
    integrity(&p);
    p.sql().execute_batch("DROP TRIGGER fail_delete").unwrap();
    db.delete_session_confirmed(&sid, true).unwrap();
    integrity(&p);
}
#[test]
fn deletion_rejects_persisted_and_reserved_active_turns() {
    let (p, db, sid) = fixture();
    db.begin_turn("active", Some(&sid), Some(&Message::user("live")))
        .unwrap();
    assert!(db
        .delete_session_confirmed(&sid, true)
        .unwrap_err()
        .to_string()
        .contains("busy"));
    let registry = TurnRegistry::default();
    registry.reserve(Some(sid.clone())).unwrap();
    assert!(registry
        .while_session_idle(&sid, || db.delete_session_confirmed(&sid, true))
        .is_err());
    integrity(&p);
}
#[test]
fn exact_session_matching_preserves_independent_metadata() {
    let (p, db, sid) = fixture();
    let other = db.create_session("independent").unwrap().id;
    unknown(&db, &sid, "one");
    unknown(&db, &other, "other");
    db.delete_session_confirmed(&format!("{sid}%"), true)
        .unwrap_err();
    db.delete_session_confirmed("' OR 1=1 --", true)
        .unwrap_err();
    db.delete_session_confirmed(&sid, true).unwrap();
    assert_eq!(db.list_operations(Some(&other), 50).unwrap().len(), 1);
    integrity(&p);
}
#[test]
fn parent_constraints_remain_enabled_and_deletion_handles_legacy_audit() {
    let (p, db, sid) = fixture();
    db.add_message(&sid, &Message::user("legacy")).unwrap();
    db.record_tool_audit(
        Some(&sid),
        "read_file",
        "OLD_PRIVATE_ARGS",
        true,
        1,
        "green",
        "auto",
    )
    .unwrap();
    assert!(p
        .sql()
        .execute("DELETE FROM sessions WHERE id=?1", [&sid])
        .is_err());
    db.delete_session(&sid).unwrap();
    assert_eq!(
        p.sql()
            .query_row("SELECT COUNT(*) FROM tool_audit", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    integrity(&p);
}
#[test]
fn migrations_repeat_and_preserve_pre_f1b2_uuid_history() {
    let p = Private::new();
    p.sql().execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);CREATE TABLE messages(id INTEGER PRIMARY KEY,session_id TEXT NOT NULL,role TEXT NOT NULL,content TEXT NOT NULL,tool_name TEXT,tool_call_id TEXT,tool_result TEXT,image_url TEXT,timestamp TEXT NOT NULL);INSERT INTO sessions VALUES('original-uuid','legacy','date','date');INSERT INTO messages VALUES(1,'original-uuid','user','unchanged',NULL,NULL,NULL,NULL,'date');").unwrap();
    for _ in 0..3 {
        let db = p.manager();
        assert_eq!(db.get_messages("original-uuid").unwrap().len(), 1);
        assert_eq!(
            p.sql()
                .query_row(
                    "SELECT COUNT(*) FROM recovery_migrations WHERE version=4",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        integrity(&p);
    }
}
#[test]
fn diagnostics_and_paged_recovery_are_private_and_non_destructive() {
    let (_p, db, sid) = fixture();
    for i in 0..3 {
        unknown(&db, &sid, &format!("turn{i}"));
    }
    let page = db.operation_page(Some(&sid), 2, true, None).unwrap();
    let older = db
        .operation_page(Some(&sid), 2, true, Some(page[1].audit_id))
        .unwrap();
    assert_eq!(older.len(), 1);
    let text = serde_json::to_string(&page).unwrap();
    assert!(!text.contains("private request"));
    assert!(!text.contains("arguments"));
    let diag = db.storage_diagnostics().unwrap();
    assert_eq!(diag["unknown_outcome"], 3);
    assert_eq!(diag["automatic_destructive_retention"], false);
    assert_eq!(db.list_operations(Some(&sid), 50).unwrap().len(), 3);
}
#[test]
fn cache_symlinks_are_not_followed_and_cleanup_can_recover() {
    let p = Private::new();
    let (path, dir) = resource_cache::allocate_at(&p.0).unwrap();
    fs::write(path.join("image.svg"), "fixture").unwrap();
    let outside = p.0.join("outside");
    fs::write(&outside, "KEEP").unwrap();
    symlink(&outside, path.join("evil")).unwrap();
    let stats = resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::ZERO).unwrap();
    assert_eq!(stats["removed"], 0);
    assert_eq!(fs::read_to_string(&outside).unwrap(), "KEEP");
    drop(dir);
}
#[test]
fn active_open_and_persistent_resources_are_retained() {
    let p = Private::new();
    let (path, _dir) = resource_cache::allocate_at(&p.0).unwrap();
    fs::write(path.join("image.svg"), "fixture").unwrap();
    let open = fs::File::open(path.join("image.svg")).unwrap();
    let stats = resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::ZERO).unwrap();
    assert_eq!(stats["skipped"], 1);
    assert!(path.join("image.svg").exists());
    drop(open);
}
// Every process touching Backend/Qt gets its own XDG roots and no provider credentials.
fn isolated(name: &str) -> bool {
    if std::env::var("KDA_F1B3B_CHILD").as_deref() == Ok(name) {
        return false;
    }
    let p = Private::new();
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", name, "--nocapture", "--include-ignored"])
        .env("KDA_F1B3B_CHILD", name);
    for (k, _) in std::env::vars() {
        if k.ends_with("_API_KEY") || k.to_uppercase().ends_with("_PROXY") {
            cmd.env_remove(k);
        }
    }
    for (k, d) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_DATA_HOME", "data"),
    ] {
        let path = p.0.join(d);
        fs::create_dir(&path).unwrap();
        cmd.env(k, path);
    }
    assert!(cmd.status().unwrap().success());
    true
}
#[test]
#[ignore = "owned cache child; executed only by parent tests"]
fn cache_worker() {
    let cache = PathBuf::from(std::env::var_os("KDA_F1B3B_CACHE").unwrap());
    let (path, _) = resource_cache::allocate_at(&cache).unwrap();
    fs::write(path.join("image.svg"), "PRIVATE_CACHE_CONTENT").unwrap();
    fs::write(cache.join("workspace"), path.to_string_lossy().as_bytes()).unwrap();
}
fn inactive_cache(p: &Private) -> PathBuf {
    assert!(Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "cache_worker", "--ignored"])
        .env("KDA_F1B3B_CACHE", &p.0)
        .status()
        .unwrap()
        .success());
    PathBuf::from(fs::read_to_string(p.0.join("workspace")).unwrap())
}
#[test]
fn inactive_symlink_cleanup_failure_retries_without_touching_target() {
    let p = Private::new();
    let path = inactive_cache(&p);
    let target = p.0.join("KEEP");
    fs::write(&target, "outside").unwrap();
    symlink(&target, path.join("evil")).unwrap();
    let stats = resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::ZERO).unwrap();
    assert_eq!(stats["failures"], 1);
    assert_eq!(stats["removed"], 0);
    assert!(path.join(".lease").exists());
    assert_eq!(fs::read_to_string(&target).unwrap(), "outside");
    fs::remove_file(path.join("evil")).unwrap();
    let stats = resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::ZERO).unwrap();
    assert_eq!(stats["removed"], 1);
    assert!(!path.exists());
    assert_eq!(fs::read_to_string(&target).unwrap(), "outside");
}
#[test]
fn referenced_and_young_cache_survive_repeated_sweeps() {
    let p = Private::new();
    let path = inactive_cache(&p);
    for _ in 0..3 {
        assert_eq!(
            resource_cache::sweep_at(&p.0, |_| Ok(true), Duration::ZERO).unwrap()["skipped"],
            1
        );
    }
    assert!(path.exists());
    assert_eq!(
        resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::from_secs(86400)).unwrap()
            ["skipped"],
        1
    );
    assert_eq!(
        resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::ZERO).unwrap()["removed"],
        1
    );
}
#[test]
fn cache_ownerless_and_symlink_roots_are_never_removed() {
    let p = Private::new();
    let (path, _) = resource_cache::allocate_at(&p.0).unwrap();
    let unknown = path.parent().unwrap().join("unknown");
    fs::create_dir(&unknown).unwrap();
    fs::write(unknown.join("KEEP"), "keep").unwrap();
    assert_eq!(
        resource_cache::sweep_at(&p.0, |_| Ok(false), Duration::ZERO).unwrap()["failures"],
        1
    );
    assert!(unknown.join("KEEP").exists());
    let outside = Private::new();
    let linked = p.0.join("linked-cache");
    symlink(&outside.0, &linked).unwrap();
    assert!(resource_cache::allocate_at(&linked).is_err());
    assert!(fs::read_dir(&outside.0).unwrap().next().is_none());
}
#[test]
fn cache_quota_prevents_unbounded_accumulation() {
    let p = Private::new();
    let (path, _) = resource_cache::allocate_at(&p.0).unwrap();
    let file = fs::File::create(path.join("huge")).unwrap();
    file.set_len(257 * 1024 * 1024).unwrap();
    assert!(resource_cache::allocate_at(&p.0)
        .unwrap_err()
        .to_string()
        .contains("quota"));
    assert!(resource_cache::check_resource(&path.join("huge")).is_err());
}
#[test]
fn selected_system_messages_keep_causal_positions() {
    let (_p, db, sid) = fixture();
    db.add_message(&sid, &Message::user("old")).unwrap();
    db.add_message(&sid, &Message::system("instruction mid-turn"))
        .unwrap();
    db.add_message(&sid, &Message::assistant("answer")).unwrap();
    let messages = select(&db, &sid, &Config::default()).unwrap();
    assert!(matches!(&messages[1],Message::User{content} if content=="old"));
    assert!(matches!(&messages[2],Message::System{content} if content=="instruction mid-turn"));
}
#[test]
fn interrupted_outputs_and_post_deletion_writes_are_rejected() {
    let (p, db, sid) = fixture();
    db.begin_turn("failed", Some(&sid), Some(&Message::user("input")))
        .unwrap();
    db.settle_turn("failed", "failed").unwrap();
    db.add_turn_message(&sid, "failed", &Message::assistant("fake success"))
        .unwrap();
    assert!(select(&db, &sid, &Config::default()).is_err());
    db.delete_session(&sid).unwrap();
    assert!(db.add_message(&sid, &Message::user("late")).is_err());
    integrity(&p);
}
#[test]
fn http_recovery_deletion_auth_errors_and_context_use_real_router() {
    if isolated("http_recovery_deletion_auth_errors_and_context_use_real_router") {
        return;
    }
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        use kde_assistant_lib::backend::{http_server, Backend};
        let cfg = Config::first_run();
        cfg.save().await.unwrap();
        let backend = Backend::new().await.unwrap();
        let state = backend.http_state().unwrap();
        let sid = backend
            .sessions
            .lock()
            .unwrap()
            .create_session("private")
            .unwrap()
            .id;
        {
            let db = backend.sessions.lock().unwrap();
            unknown(&db, &sid, "previous");
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
        let auth = |request: reqwest::RequestBuilder| {
            request
                .header("Host", "127.0.0.1:8765")
                .bearer_auth(&state.local_token)
        };
        assert_eq!(
            client
                .get(format!("{base}/api/storage"))
                .header("Host", "127.0.0.1:8765")
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
        let diagnostics: serde_json::Value = auth(client.get(format!("{base}/api/storage")))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(diagnostics["unknown_outcome"], 1);
        let records: serde_json::Value = auth(client.get(format!(
            "{base}/api/tools/operations?recovery_only=true&session_id={sid}"
        )))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(records[0]["state"], "unknown_outcome");
        assert!(!records.to_string().contains("private request"));
        backend
            .sessions
            .lock()
            .unwrap()
            .record_tool_audit(
                Some(&sid),
                "read_file",
                "HISTORICAL_PRIVATE_ARGS",
                true,
                1,
                "green",
                "auto",
            )
            .unwrap();
        let audit: serde_json::Value = auth(client.get(format!("{base}/api/tools/audit")))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(!audit.to_string().contains("HISTORICAL_PRIVATE_ARGS"));
        assert!(backend
            .sessions
            .lock()
            .unwrap()
            .list_tool_audit(50)
            .unwrap()
            .iter()
            .any(|row| row.args == "HISTORICAL_PRIVATE_ARGS"));
        let url = format!("{base}/api/session?session_id={sid}");
        assert_eq!(
            auth(client.delete(&url)).send().await.unwrap().status(),
            409
        );
        assert!(backend
            .sessions
            .lock()
            .unwrap()
            .get_session(&sid)
            .unwrap()
            .is_some());
        assert_eq!(
            auth(client.delete(format!("{url}&erase_uncertain_evidence=true")))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            auth(client.delete(&url)).send().await.unwrap().status(),
            404
        );
        let records: serde_json::Value =
            auth(client.get(format!("{base}/api/tools/operations?session_id={sid}")))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
        assert_eq!(records, serde_json::json!([]));
        let active = backend
            .sessions
            .lock()
            .unwrap()
            .create_session("active")
            .unwrap()
            .id;
        state.chat_tasks.reserve(Some(active.clone())).unwrap();
        assert_eq!(
            auth(client.delete(format!("{base}/api/session?session_id={active}")))
                .send()
                .await
                .unwrap()
                .status(),
            409
        );
        let bounded = backend
            .sessions
            .lock()
            .unwrap()
            .create_session("bounded")
            .unwrap()
            .id;
        {
            let mut cfg = state.config.write().await;
            cfg.ai.context_window_tokens = 100;
            cfg.ai.max_tokens = 50;
        }
        let response = auth(client.post(format!("{base}/api/chat/complete")))
            .json(&serde_json::json!({"session_id":bounded,"message":"too large"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 502);
        let error: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error["code"], "context_budget_exceeded");
        let sql = Connection::open(
            PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap())
                .join("kde-assistant/sessions.db"),
        )
        .unwrap();
        sql.execute_batch("ALTER TABLE operation_journal RENAME TO journal_fault")
            .unwrap();
        assert_eq!(
            auth(client.get(format!("{base}/api/tools/operations")))
                .send()
                .await
                .unwrap()
                .status(),
            500
        );
        sql.execute_batch("ALTER TABLE journal_fault RENAME TO operation_journal")
            .unwrap();
        server.abort();
        let _ = server.await;
    });
}
async fn qml_fixture(main: bool) {
    use axum::{
        extract::Query,
        http::StatusCode,
        routing::{delete, get},
        Json, Router,
    };
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    let deletion = Arc::new(AtomicUsize::new(0));
    let observed = deletion.clone();
    let app=Router::new().route("/api/tools/operations",get(|Query(q):Query<HashMap<String,String>>|async move {
        let session=q.get("session_id").map(String::as_str).unwrap_or("");
        if session=="slow"{tokio::time::sleep(Duration::from_secs(1)).await;}
        if session=="error"{return (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"synthetic"})));}
        let rows=match session{"empty"|"deleted"=>serde_json::json!([]),"legacy"=>serde_json::json!([{"tool":"old tool"}]),_=>serde_json::json!([{"tool":"create_file","state":"unknown_outcome","id":"fixture-op","session_id":"unknown","prepared_at":"2026-10-12T00:00:00Z"}])};
        (StatusCode::OK,Json(rows))
    })).route("/api/session",delete(move |Query(q):Query<HashMap<String,String>>|{let observed=observed.clone();async move {
        if q.get("erase_uncertain_evidence").map(String::as_str)!=Some("true") {return (StatusCode::CONFLICT,Json(serde_json::json!({"error":"uncertain_evidence_confirmation_required"})));}
        observed.fetch_add(1,Ordering::SeqCst);(StatusCode::OK,Json(serde_json::json!({"status":"ok"})))
    }})).route("/api/sessions",get(||async{Json(serde_json::json!([]))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let p = Private::new();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let module =
        kde_assistant_lib::backend::auth::write_qml_auth_module(&"a".repeat(64), false).unwrap();
    let source = if main {
        include_str!("qml/F1B3BMain.qml")
    } else {
        include_str!("qml/F1B3BRecovery.qml")
    }
    .replace("F1B3B_BACKEND", &base)
    .replace(
        "F1B3B_QML_DIR",
        &format!("file://{}", repo.join("qml").display()),
    );
    let path = p.0.join("Recovery.qml");
    fs::write(&path, source).unwrap();
    let logpath = p.0.join("qml.log");
    let log = fs::File::create(&logpath).unwrap();
    let child = Command::new("qml6")
        .args(["--verbose", "-apptype", "widget", "-I"])
        .arg(&repo)
        .arg("-I")
        .arg(module)
        .arg(path)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_ASSUME_STDERR_HAS_CONSOLE", "1")
        .env("QML_DISABLE_DISK_CACHE", "1")
        .env("QML_XHR_ALLOW_FILE_READ", "1")
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let mut child = kde_assistant_lib::owned_process::OwnedChild::new(child);
    let status = tokio::time::timeout(Duration::from_secs(16), async {
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("QML timeout");
    let output = fs::read_to_string(logpath).unwrap();
    server.abort();
    let _ = server.await;
    assert!(
        status.success() && output.contains("F1B3B_UI_PASS"),
        "{status}\n{output}"
    );
    assert_eq!(deletion.load(Ordering::SeqCst), usize::from(main));
}
#[test]
#[ignore = "Qt/QML offscreen with private XDG and mock API"]
fn qml_recovery_states_and_stale_responses() {
    if isolated("qml_recovery_states_and_stale_responses") {
        return;
    }
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(qml_fixture(false));
}
#[test]
#[ignore = "Qt/QML offscreen with private XDG and mock API"]
fn qml_session_banner_and_explicit_evidence_confirmation() {
    if isolated("qml_session_banner_and_explicit_evidence_confirmation") {
        return;
    }
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(qml_fixture(true));
}

#[test]
fn legacy_deleted_session_evidence_is_preserved_until_explicit_scoped_erase() {
    let p = Private::new();
    let db = p.manager();
    let sid = db.create_session("legacy deletion").unwrap().id;
    unknown(&db, &sid, "retained");
    drop(db);
    // Model the old deletion, before migration 004 guarded parent removal.
    let sql = p.sql();
    // Only this synthetic seed models an old connection with enforcement off.
    // Re-enable it and assert the real orphan before exercising production deletion.
    sql.pragma_update(None, "foreign_keys", false).unwrap();
    sql.execute_batch("DROP TRIGGER storage_session_delete;DROP TRIGGER storage_message_delete;")
        .unwrap();
    sql.execute("DELETE FROM messages WHERE session_id=?1", [&sid])
        .unwrap();
    sql.execute("DELETE FROM sessions WHERE id=?1", [&sid])
        .unwrap();
    sql.pragma_update(None, "foreign_keys", true).unwrap();
    assert!(sql
        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
        .optional()
        .unwrap()
        .is_some());
    drop(sql);
    let db = p.manager();
    assert!(
        db.storage_diagnostics().unwrap()["orphan_links"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert_eq!(
        db.list_operations(Some(&sid), 50).unwrap()[0].state,
        "unknown_outcome"
    );
    assert!(db
        .delete_session(&sid)
        .unwrap_err()
        .to_string()
        .contains("confirmation"));
    db.delete_session_confirmed(&sid, true).unwrap();
    assert_eq!(db.storage_diagnostics().unwrap()["orphan_links"], 0);
    integrity(&p);
}
#[test]
fn image_preview_retains_snapshot_and_failed_render_cleans_pending_resources() {
    if isolated("image_preview_retains_snapshot_and_failed_render_cleans_pending_resources") {
        return;
    }
    use kde_assistant_lib::backend::tool_executor::{ToolCtx, ToolExecutor};
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let p = Private::new();
        let source = p.0.join("image.svg");
        fs::write(
            &source,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>",
        )
        .unwrap();
        let mut cfg = Config::first_run();
        cfg.tools.preview_document = true;
        cfg.tools.allowed_paths = vec![p.0.display().to_string()];
        let tools = ToolExecutor::new(std::sync::Arc::new(tokio::sync::RwLock::new(cfg)));
        let preview = |path: PathBuf| ToolCall {
            id: uuid::Uuid::new_v4().to_string(),
            name: "preview_document".into(),
            arguments: serde_json::from_value(serde_json::json!({"path":path})).unwrap(),
        };
        let result = tools
            .execute(&preview(source), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(result.success);
        let image = PathBuf::from(result.image_url.unwrap());
        assert!(image.exists());
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << >> /Contents 4 0 R >>",
            "<< /Length 0 >>\nstream\n\nendstream",
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 5\n0000000000 65535 f \n");
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"));
        let pdf_path = p.0.join("synthetic.pdf");
        fs::write(&pdf_path, pdf).unwrap();
        let rendered = tools.execute(&preview(pdf_path), &ToolCtx::auto(None)).await.unwrap();
        assert!(rendered.success, "{}", rendered.content);
        assert!(fs::read(rendered.image_url.unwrap()).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
        let root = PathBuf::from(std::env::var_os("XDG_CACHE_HOME").unwrap())
            .join("kde-assistant/resources-v1");
        let before = fs::read_dir(&root).unwrap().count();
        let unsupported = p.0.join("bad.unknown");
        fs::write(&unsupported, "private source").unwrap();
        let failed = tools
            .execute(&preview(unsupported), &ToolCtx::auto(None))
            .await
            .unwrap();
        assert!(!failed.success);
        assert_eq!(fs::read_dir(&root).unwrap().count(), before);
        assert!(image.exists());
    });
}
#[test]
fn unpublished_guard_cleans_on_error_and_quota_reserves_concurrent_writers() {
    let p = Private::new();
    let (path, _) = resource_cache::allocate_at(&p.0).unwrap();
    {
        let _pending = resource_cache::PendingResource::new(&path);
        fs::write(path.join("partial"), "private partial").unwrap();
    }
    assert!(!path.exists());
    let mut allocated = 0;
    loop {
        match resource_cache::allocate_at(&p.0) {
            Ok(_) => allocated += 1,
            Err(e) => {
                assert!(e.to_string().contains("quota"));
                break;
            }
        }
    }
    assert!(allocated > 0 && allocated < 9);
}

#[test]
fn dispatched_operation_blocks_deletion_even_with_terminal_turn() {
    let (p, db, sid) = fixture();
    db.begin_turn("terminal-owner", Some(&sid), None).unwrap();
    let op = db
        .prepare_operation("terminal-owner", "create_file")
        .unwrap();
    db.authorize_operation(&op, "terminal-owner", "create_file", "auto")
        .unwrap();
    db.transition_operation(&op, State::Dispatched, "auto", None)
        .unwrap();
    db.finish_turn("terminal-owner", "failed").unwrap();
    assert!(db
        .delete_session_confirmed(&sid, true)
        .unwrap_err()
        .to_string()
        .contains("busy"));
    integrity(&p);
}

#[test]
fn replaced_workspace_cannot_publish_or_delete_another_live_resource() {
    let p = Private::new();
    let (first, _) = resource_cache::allocate_at(&p.0).unwrap();
    let (second, _) = resource_cache::allocate_at(&p.0).unwrap();
    fs::write(second.join("KEEP"), "published other resource").unwrap();
    resource_cache::publish_resource(&second.join("KEEP")).unwrap();
    let displaced = first.with_file_name("displaced");
    fs::rename(&first, &displaced).unwrap();
    fs::rename(&second, &first).unwrap();
    let publish = resource_cache::publish_resource(&first.join("KEEP"));
    let cleanup = resource_cache::release_private_snapshot(&first.join("unused"));
    assert!(
        publish.is_err() && cleanup.is_err(),
        "publish={publish:?}; cleanup={cleanup:?}"
    );
    assert_eq!(
        fs::read_to_string(first.join("KEEP")).unwrap(),
        "published other resource"
    );
    assert!(displaced.join(".lease").exists());
    fs::rename(&first, &second).unwrap();
    fs::rename(&displaced, &first).unwrap();
    resource_cache::release_private_snapshot(&first.join("unused")).unwrap();
    assert!(second.join("KEEP").exists());
}

#[test]
fn image_download_does_not_follow_planted_output_symlinks() {
    if isolated("image_download_does_not_follow_planted_output_symlinks") {
        return;
    }
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        use kde_assistant_lib::backend::tool_executor::{ToolCtx, ToolExecutor};
        use std::sync::Arc;
        let p = Private::new();
        let target = p.0.join("KEEP");
        fs::write(&target, "untouched private fixture").unwrap();
        let cache = PathBuf::from(std::env::var_os("XDG_CACHE_HOME").unwrap())
            .join("kde-assistant/resources-v1");
        let start = chrono::Utc::now().timestamp_millis();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/image.png", listener.local_addr().unwrap());
        let planted_target = target.clone();
        let app = axum::Router::new().route(
            "/image.png",
            axum::routing::get(move || {
                let cache = cache.clone();
                let target = planted_target.clone();
                async move {
                    let workspace = fs::read_dir(cache).unwrap().next().unwrap().unwrap().path();
                    let end = chrono::Utc::now().timestamp_millis();
                    assert!(end - start < 10000, "fixture clock or scheduling failure");
                    for timestamp in start - 2..=end + 2 {
                        symlink(&target, workspace.join(format!("{timestamp}.png"))).unwrap();
                    }
                    (
                        [("content-type", "image/png")],
                        b"\x89PNG\r\n\x1a\nsynthetic image".as_slice(),
                    )
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut cfg = Config::first_run();
        cfg.tools.show_image = true;
        let tools = ToolExecutor::new(Arc::new(tokio::sync::RwLock::new(cfg)));
        let result = tools
            .execute(
                &ToolCall {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: "show_image".into(),
                    arguments: serde_json::from_value(serde_json::json!({"source":url})).unwrap(),
                },
                &ToolCtx::auto(None),
            )
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(fs::read(&target).unwrap(), b"untouched private fixture");
        server.abort();
        let _ = server.await;
    });
}
