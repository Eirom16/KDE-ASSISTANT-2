//! Deterministic loopback HTTP/1.1 fixtures. Each case runs with private XDG.
use futures_util::StreamExt;
use kde_assistant_lib::{
    backend::{ai_service::AiService, http_server, Backend},
    models::Config,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, Notify},
    task::{JoinHandle, JoinSet},
};

struct Private(PathBuf);
impl Drop for Private {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn isolated(name: &str) -> bool {
    if std::env::var("KDA_F1B2_CHILD").as_deref() == Ok(name) {
        return false;
    }
    let private = Private(std::env::temp_dir().join(format!("kda-f1b2-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&private.0).unwrap();
    fs::set_permissions(&private.0, fs::Permissions::from_mode(0o700)).unwrap();
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", name, "--nocapture"])
        .env("KDA_F1B2_CHILD", name);
    if name.starts_with("qml_") {
        cmd.arg("--ignored");
    }
    for (key, _) in std::env::vars() {
        if key.ends_with("_API_KEY") || key.to_ascii_uppercase().ends_with("_PROXY") {
            cmd.env_remove(key);
        }
    }
    for (key, dir) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        let path = private.0.join(dir);
        fs::create_dir(&path).unwrap();
        cmd.env(key, path);
    }
    cmd.env("ORT_DYLIB_PATH", private.0.join("absent.so"));
    assert!(cmd.status().unwrap().success(), "{name}");
    true
}
fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
fn token(text: &str) -> Vec<u8> {
    format!(
        "data: {}\n\n",
        json!({"choices":[{"delta":{"content":text}}]})
    )
    .into_bytes()
}
fn complete(text: &str) -> Vec<u8> {
    let mut bytes = token(text);
    bytes.extend_from_slice(b"data: [DONE]\n\n");
    bytes
}
fn tool(name: &str, args: &str) -> Vec<u8> {
    format!("data: {}\n\ndata: [DONE]\n\n",json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"provider-arbitrary-id","function":{"name":name,"arguments":args}}]},"finish_reason":"tool_calls"}]})).into_bytes()
}
struct Plan {
    initial: Vec<u8>,
    hold: bool,
    abrupt: bool,
    fragment: bool,
    status: u16,
    delay_headers: bool,
}
impl Plan {
    fn hold(initial: Vec<u8>) -> Self {
        Self {
            initial,
            hold: true,
            abrupt: false,
            fragment: false,
            status: 200,
            delay_headers: false,
        }
    }
    fn reply(initial: Vec<u8>) -> Self {
        Self {
            initial,
            hold: false,
            abrupt: false,
            fragment: true,
            status: 200,
            delay_headers: false,
        }
    }
}
struct Seen {
    request: Value,
    closed: AtomicBool,
    late_attempt: AtomicBool,
    release: Notify,
}
struct Mock {
    url: String,
    seen: mpsc::Receiver<Arc<Seen>>,
    task: JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn write_chunk(socket: &mut tokio::net::TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    socket
        .write_all(format!("{:x}\r\n", bytes.len()).as_bytes())
        .await?;
    socket.write_all(bytes).await?;
    socket.write_all(b"\r\n").await
}
async fn mock(plans: Vec<Plan>) -> Mock {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let plans = Arc::new(Mutex::new(VecDeque::from(plans)));
    let (tx, seen) = mpsc::channel(32);
    let task = tokio::spawn(async move {
        let mut children = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let (mut socket,_) = accepted.unwrap(); let tx = tx.clone(); let plans = plans.clone();
                    children.spawn(async move {
                        let mut bytes = Vec::new(); let mut header_end = None; let mut length = 0;
                        loop {
                            let mut byte = [0u8;1]; if socket.read_exact(&mut byte).await.is_err() {return;}
                            bytes.push(byte[0]);
                            if header_end.is_none() && bytes.ends_with(b"\r\n\r\n") {
                                let headers = String::from_utf8(bytes.clone()).unwrap();
                                length = headers.lines().find_map(|line|line.to_lowercase().strip_prefix("content-length:").map(|v|v.trim().parse::<usize>().unwrap())).unwrap();
                                header_end = Some(bytes.len());
                            }
                            if header_end.is_some_and(|end|bytes.len() == end+length) {break;}
                        }
                        let record = Arc::new(Seen {request:serde_json::from_slice(&bytes[header_end.unwrap()..]).unwrap(),closed:AtomicBool::new(false),late_attempt:AtomicBool::new(false),release:Notify::new()});
                        let plan = plans.lock().unwrap().pop_front().expect("unexpected retry/request");
                        tx.send(record.clone()).await.unwrap();
                        socket.write_all(format!("HTTP/1.1 {} OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",plan.status).as_bytes()).await.unwrap();
                        if !plan.initial.is_empty() {
                            if plan.fragment {for byte in plan.initial {if write_chunk(&mut socket,&[byte]).await.is_err() {return;} tokio::task::yield_now().await;}}
                            else {write_chunk(&mut socket,&plan.initial).await.unwrap();}
                        }
                        if plan.abrupt {return;}
                        if plan.hold {
                            let mut probe = [0u8;1];
                            tokio::select! {
                                read = socket.read(&mut probe) => {
                                    assert!(matches!(read,Ok(0)|Err(_)),"unexpected request pipelining");
                                    record.closed.store(true,Ordering::SeqCst);
                                    record.late_attempt.store(true,Ordering::SeqCst);
                                    let _ = write_chunk(&mut socket,&complete("LATE-A")).await;
                                    return;
                                }
                                _ = record.release.notified() => {let _ = write_chunk(&mut socket,&complete("released")).await;}
                            }
                        }
                        let _ = socket.write_all(b"0\r\n\r\n").await;
                    });
                }
                _ = children.join_next(), if !children.is_empty() => {}
            }
        }
    });
    Mock { url, seen, task }
}
struct Fixture {
    backend: Backend,
    state: http_server::AppState,
    base: String,
    server: JoinHandle<()>,
    client: reqwest::Client,
    sid: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new(url: &str, timeout: Duration) -> Self {
        let mut cfg = Config::first_run();
        cfg.ai.provider = "custom".into();
        cfg.ai.base_url = url.into();
        cfg.ai.model = "fixture".into();
        cfg.ai.api_key = "local-fixture".into();
        cfg.save().await.unwrap();
        let backend = Backend::new().await.unwrap();
        let mut state = backend.http_state().unwrap();
        state.ai = Arc::new(
            AiService::with_timeout(backend.config.clone(), timeout)
                .await
                .unwrap(),
        );
        let sid = backend
            .sessions
            .lock()
            .unwrap()
            .create_session("fixture")
            .unwrap()
            .id;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = http_server::router(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            backend,
            state,
            base,
            server,
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            sid,
        }
    }
    fn post(&self, path: &str, body: Value) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}{path}", self.base))
            .header("Host", "127.0.0.1:8765")
            .bearer_auth(&self.state.local_token)
            .json(&body)
    }
    async fn begin(&self, sid: &str) -> Reader {
        let response = self
            .post("/api/chat", json!({"session_id":sid,"message":"hello"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let mut reader = Reader {
            stream: Box::pin(response.bytes_stream()),
            parser: kde_assistant_lib::backend::sse_parser::SseParser::default(),
            events: Vec::new(),
        };
        reader.until("turn").await;
        reader
    }
    async fn cancel(&self, sid: &str, id: &str) -> Value {
        let response = self
            .post("/api/chat/cancel", json!({"session_id":sid,"turn_id":id}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        response.json().await.unwrap()
    }
    fn messages(&self, sid: &str) -> Vec<kde_assistant_lib::models::Message> {
        self.backend
            .sessions
            .lock()
            .unwrap()
            .get_messages(sid)
            .unwrap()
    }
}
struct Reader {
    stream: std::pin::Pin<
        Box<dyn futures_util::Stream<Item = Result<axum::body::Bytes, reqwest::Error>> + Send>,
    >,
    parser: kde_assistant_lib::backend::sse_parser::SseParser,
    events: Vec<(String, Value)>,
}
impl Reader {
    fn id(&self) -> String {
        self.events[0].1["turn_id"].as_str().unwrap().into()
    }
    async fn until(&mut self, event: &str) {
        self.until_count(event, 1).await;
    }
    async fn until_count(&mut self, event: &str, count: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if self.events.iter().filter(|(name, _)| name == event).count() >= count {
                    return;
                }
                let bytes = self.stream.next().await.expect("missing event").unwrap();
                for event in self.parser.push(&bytes).unwrap() {
                    self.events
                        .push((event.event, serde_json::from_str(&event.data).unwrap()));
                }
            }
        })
        .await
        .unwrap();
    }
    async fn end(&mut self) {
        while let Some(chunk) = self.stream.next().await {
            for event in self.parser.push(&chunk.unwrap()).unwrap() {
                self.events
                    .push((event.event, serde_json::from_str(&event.data).unwrap()));
            }
        }
        assert_eq!(
            self.events
                .iter()
                .filter(|(name, _)| matches!(name.as_str(), "done" | "cancelled" | "error"))
                .count(),
            1
        );
        assert!(self
            .events
            .iter()
            .all(|(_, value)| value["session_id"] == self.events[0].1["session_id"]));
        let mut last = 0;
        for (_, value) in &self.events {
            assert_eq!(value["turn_id"], self.id());
            assert!(value["sequence"].as_u64().unwrap() > last);
            last = value["sequence"].as_u64().unwrap();
        }
    }
}
async fn seen(mock: &mut Mock) -> Arc<Seen> {
    tokio::time::timeout(Duration::from_secs(3), mock.seen.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(4), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn cancel_before_first_token_closes_connection() {
    if isolated("cancel_before_first_token_closes_connection") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(Vec::new())]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let mut a = f.begin(&f.sid).await;
        let request = seen(&mut m).await;
        let ack = f.cancel(&f.sid, &a.id()).await;
        assert_eq!(ack["cancellation_effective"], true);
        a.until("cancelled").await;
        a.end().await;
        eventually(|| request.closed.load(Ordering::SeqCst)).await;
        assert_eq!(f.state.chat_tasks.active_count(), 0);
        eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
        assert_eq!(
            f.state.ai.activity.snapshot()["active"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(f.messages(&f.sid).len(), 1);
        assert!(!a.events.iter().any(|(n, _)| n == "done"));
    });
}
#[test]
fn cancel_tokens_then_b_isolated_from_late_a() {
    if isolated("cancel_tokens_then_b_isolated_from_late_a") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![
            Plan::hold(token("partial-A")),
            Plan::hold(token("B niño🙂")),
        ])
        .await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let mut a = f.begin(&f.sid).await;
        let first = seen(&mut m).await;
        a.until("token").await;
        f.cancel(&f.sid, &a.id()).await;
        a.until("cancelled").await;
        a.end().await;
        eventually(|| first.closed.load(Ordering::SeqCst)).await;
        assert!(first.late_attempt.load(Ordering::SeqCst));
        let mut b = f.begin(&f.sid).await;
        let second = seen(&mut m).await;
        assert!(!second.request.to_string().contains("partial-A"));
        b.until("token").await;
        let stale = f
            .post(
                "/api/chat/cancel",
                json!({"session_id":f.sid,"turn_id":a.id()}),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(stale.status(), 404);
        assert!(!second.closed.load(Ordering::SeqCst));
        second.release.notify_one();
        b.until("done").await;
        b.end().await;
        assert_ne!(a.id(), b.id());
        let text = serde_json::to_string(&f.messages(&f.sid)).unwrap();
        assert!(!text.contains("partial-A") && !text.contains("LATE-A"));
        assert!(text.contains("B niño🙂"));
        assert_eq!(f.messages(&f.sid).len(), 3);
        let c = f
            .post(
                "/api/chat/cancel",
                json!({"session_id":f.sid,"turn_id":a.id()}),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(c.status(), 404);
    });
}
#[test]
fn approval_cancel_releases_waiter_and_late_decision() {
    if isolated("approval_cancel_releases_waiter_and_late_decision") {
        return;
    }
    rt().block_on(async {
        let private = dirs::data_local_dir().unwrap().join("effect.txt");
        let args = json!({"path":private,"content":"must not exist"}).to_string();
        let mut m = mock(vec![Plan::reply(tool("create_file", &args))]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        {
            let mut cfg = f.backend.config.write().await;
            cfg.ai.enable_tool_calling = true;
            cfg.tools.create_file = true;
            cfg.tools.allowed_paths =
                vec![dirs::data_local_dir().unwrap().to_string_lossy().into()];
        }
        let mut a = f.begin(&f.sid).await;
        seen(&mut m).await;
        a.until("approval_needed").await;
        let call = a
            .events
            .iter()
            .find(|(n, _)| n == "approval_needed")
            .unwrap()
            .1["tool_call_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_ne!(call, "provider-arbitrary-id");
        assert_eq!(f.state.approvals.pending_count(), 1);
        f.cancel(&f.sid, &a.id()).await;
        a.until("cancelled").await;
        a.end().await;
        assert_eq!(f.state.approvals.pending_count(), 0);
        assert!(!f.state.approvals.resolve(&call, true));
        assert!(!private.exists());
        assert_eq!(f.state.chat_tasks.active_count(), 0);
        eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
        assert_eq!(
            f.state.ai.activity.snapshot()["active"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    });
}
#[test]
fn simultaneous_same_session_rejected_before_provider() {
    if isolated("simultaneous_same_session_rejected_before_provider") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(Vec::new())]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let body = json!({"session_id":f.sid,"message":"hello"});
        let (one, two) = tokio::join!(
            f.post("/api/chat", body.clone()).send(),
            f.post("/api/chat", body).send()
        );
        let one = one.unwrap();
        let two = two.unwrap();
        assert!(
            (one.status() == 200 && two.status() == 409)
                || (one.status() == 409 && two.status() == 200)
        );
        seen(&mut m).await;
        assert_eq!(f.state.chat_tasks.active_count(), 1);
        assert_eq!(f.messages(&f.sid).len(), 1);
        assert_eq!(f.backend.http_state().unwrap().chat_tasks.active_count(), 1);
        let regenerate = f
            .post("/api/chat/regenerate", json!({"session_id":f.sid}))
            .send()
            .await
            .unwrap();
        assert_eq!(regenerate.status(), 409);
        assert_eq!(f.messages(&f.sid).len(), 1);
        drop(one);
        drop(two);
        eventually(|| f.state.chat_tasks.active_count() == 0).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), m.seen.recv())
                .await
                .is_err()
        );
    });
}
#[test]
fn cancellation_is_scoped_to_session_and_turn() {
    if isolated("cancellation_is_scoped_to_session_and_turn") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(Vec::new()), Plan::hold(Vec::new())]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let sid2 = f
            .backend
            .sessions
            .lock()
            .unwrap()
            .create_session("other")
            .unwrap()
            .id;
        let mut a = f.begin(&f.sid).await;
        let first = seen(&mut m).await;
        let mut b = f.begin(&sid2).await;
        let second = seen(&mut m).await;
        let wrong = f
            .post(
                "/api/chat/cancel",
                json!({"session_id":sid2,"turn_id":a.id()}),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status(), 404);
        f.cancel(&f.sid, &a.id()).await;
        a.until("cancelled").await;
        eventually(|| first.closed.load(Ordering::SeqCst)).await;
        assert!(!second.closed.load(Ordering::SeqCst));
        assert_eq!(f.state.chat_tasks.active_count(), 1);
        second.release.notify_one();
        b.until("done").await;
        b.end().await;
        assert_eq!(f.messages(&sid2).len(), 2);
    });
}
#[test]
fn client_disconnect_cancels_idle_provider() {
    if isolated("client_disconnect_cancels_idle_provider") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(Vec::new())]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let a = f.begin(&f.sid).await;
        let request = seen(&mut m).await;
        drop(a);
        eventually(|| request.closed.load(Ordering::SeqCst)).await;
        eventually(|| f.state.chat_tasks.active_count() == 0).await;
        assert_eq!(f.messages(&f.sid).len(), 1);
    });
}
#[test]
fn provider_timeout_releases_connection() {
    if isolated("provider_timeout_releases_connection") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(Vec::new())]).await;
        let f = Fixture::new(&m.url, Duration::from_millis(150)).await;
        let mut a = f.begin(&f.sid).await;
        let request = seen(&mut m).await;
        a.until("error").await;
        a.end().await;
        eventually(|| request.closed.load(Ordering::SeqCst)).await;
        assert_eq!(a.events.last().unwrap().1["code"], "provider_timeout");
        assert_eq!(f.messages(&f.sid).len(), 1);
        assert_eq!(f.state.chat_tasks.active_count(), 0);
        eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
        assert_eq!(
            f.state.ai.activity.snapshot()["active"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    });
}
#[test]
fn malformed_truncated_error_and_abrupt_streams_never_succeed() {
    if isolated("malformed_truncated_error_and_abrupt_streams_never_succeed") {
        return;
    }
    rt().block_on(async {
        let mut abrupt = Plan::reply(token("partial"));
        abrupt.abrupt = true;
        let mut m = mock(vec![
            Plan::reply(b"data: {bad}\n\n".to_vec()),
            Plan::reply(token("missing-done")),
            Plan::reply(b"event: error\ndata: {\"error\":\"rejected\"}\n\n".to_vec()),
            abrupt,
            Plan::reply(b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{".to_vec()),
        ])
        .await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        for expected in [
            "invalid_provider_stream",
            "invalid_provider_stream",
            "provider_error",
            "provider_transport",
            "invalid_provider_stream",
        ] {
            let mut a = f.begin(&f.sid).await;
            seen(&mut m).await;
            a.until("error").await;
            a.end().await;
            assert_eq!(a.events.last().unwrap().1["code"], expected);
            assert!(!a.events.iter().any(|(n, _)| n == "done"));
            assert_eq!(f.state.chat_tasks.active_count(), 0);
            eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
            assert_eq!(
                f.state.ai.activity.snapshot()["active"]
                    .as_array()
                    .unwrap()
                    .len(),
                0
            );
        }
        assert!(f
            .messages(&f.sid)
            .iter()
            .all(|m| matches!(m, kde_assistant_lib::models::Message::User { .. })));
    });
}
#[test]
fn fragmented_success_persists_turn_identity() {
    if isolated("fragmented_success_persists_turn_identity") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::reply(complete("Español niño acción 🦀🙂"))]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let mut a = f.begin(&f.sid).await;
        seen(&mut m).await;
        a.until("done").await;
        a.end().await;
        let db = rusqlite::Connection::open(
            dirs::data_local_dir()
                .unwrap()
                .join("kde-assistant/sessions.db"),
        )
        .unwrap();
        let mut statement = db.prepare("SELECT turn_id FROM messages").unwrap();
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(ids, vec![a.id(), a.id()]);
        assert_eq!(
            db.query_row(
                "SELECT state FROM conversation_turns WHERE id=?1",
                [a.id()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "completed"
        );
        assert_eq!(f.messages(&f.sid).len(), 2);
    });
}
#[test]
fn cancelled_turn_survives_backend_restart_without_false_reply() {
    if isolated("cancelled_turn_survives_backend_restart_without_false_reply") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(token("partial"))]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let mut a = f.begin(&f.sid).await;
        let request = seen(&mut m).await;
        a.until("token").await;
        f.cancel(&f.sid, &a.id()).await;
        a.until("cancelled").await;
        a.end().await;
        eventually(|| request.closed.load(Ordering::SeqCst)).await;
        let sid = f.sid.clone();
        let id = a.id();
        let previous_db = Arc::downgrade(&f.backend.sessions);
        drop(f);
        // Aborting serve requests teardown; spawned HTTP connections also own state.
        // A restart begins only once every owner of the old database has gone.
        eventually(|| previous_db.upgrade().is_none()).await;
        // Weak::upgrade becomes None before SessionManager's destructor finishes
        // closing SQLite and its ownership file. Observe the actual lock release.
        let ownership = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(
                dirs::data_local_dir()
                    .unwrap()
                    .join("kde-assistant/sessions.db"),
            )
            .unwrap();
        eventually(|| {
            match rustix::fs::flock(
                &ownership,
                rustix::fs::FlockOperation::NonBlockingLockExclusive,
            ) {
                Ok(()) => true,
                Err(rustix::io::Errno::WOULDBLOCK) => false,
                Err(error) => panic!("unexpected ownership probe error: {error}"),
            }
        })
        .await;
        drop(ownership);
        let backend = Backend::new().await.unwrap();
        assert_eq!(
            backend
                .sessions
                .lock()
                .unwrap()
                .get_messages(&sid)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(backend.http_state().unwrap().chat_tasks.active_count(), 0);
        let db = rusqlite::Connection::open(
            dirs::data_local_dir()
                .unwrap()
                .join("kde-assistant/sessions.db"),
        )
        .unwrap();
        assert_eq!(
            db.query_row(
                "SELECT state FROM conversation_turns WHERE id=?1",
                [id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "cancelled"
        );
    });
}
#[test]
fn old_database_migration_preserves_rows_and_marks_abandoned_turns() {
    if isolated("old_database_migration_preserves_rows_and_marks_abandoned_turns") {
        return;
    }
    rt().block_on(async {
        let dir=dirs::data_local_dir().unwrap().join("kde-assistant");fs::create_dir_all(&dir).unwrap();let db=rusqlite::Connection::open(dir.join("sessions.db")).unwrap();
        db.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL); CREATE TABLE messages(id INTEGER PRIMARY KEY AUTOINCREMENT,session_id TEXT NOT NULL,role TEXT NOT NULL,content TEXT NOT NULL,tool_name TEXT,tool_call_id TEXT,tool_result TEXT,image_url TEXT,timestamp TEXT NOT NULL);INSERT INTO sessions VALUES ('old','old','then','then'); INSERT INTO messages(session_id,role,content,timestamp) VALUES ('old','user','legacy niño','then');").unwrap();
        let backend=Backend::new().await.unwrap();assert_eq!(backend.sessions.lock().unwrap().get_messages("old").unwrap().len(),1);
        let id:Option<String>=db.query_row("SELECT turn_id FROM messages",[],|r|r.get(0)).unwrap();assert_eq!(id,None);
        db.execute("INSERT INTO conversation_turns VALUES ('abandoned','old','cancelling')",[]).unwrap();drop(backend);
        let backend=Backend::new().await.unwrap();assert_eq!(db.query_row("SELECT state FROM conversation_turns WHERE id='abandoned'",[],|r|r.get::<_,String>(0)).unwrap(),"failed");assert_eq!(backend.sessions.lock().unwrap().get_messages("old").unwrap().len(),1);
    });
}
#[test]
fn complete_endpoint_returns_error_status_and_never_fake_success() {
    if isolated("complete_endpoint_returns_error_status_and_never_fake_success") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::reply(b"data: {bad}\n\n".to_vec())]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let response = f
            .post(
                "/api/chat/complete",
                json!({"session_id":f.sid,"message":"hello"}),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 502);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["state"], "failed");
        assert!(body.get("response").is_none());
        seen(&mut m).await;
        assert_eq!(f.messages(&f.sid).len(), 1);
    });
}

#[test]
fn interleaved_tools_execute_once_with_matching_local_ids() {
    if isolated("interleaved_tools_execute_once_with_matching_local_ids") {
        return;
    }
    rt().block_on(async {
        let root=dirs::data_local_dir().unwrap();let one=root.join("one.txt");let two=root.join("two.txt");fs::write(&one,"uno niño").unwrap();fs::write(&two,"dos acción").unwrap();
        let args1=json!({"path":one}).to_string();let args2=json!({"path":two}).to_string();
        let mid1=args1.len()/2;let mid2=args2.len()/2;
        let deltas=[json!({"tool_calls":[{"index":1,"id":"provider-b","function":{"name":"read_","arguments":&args2[..mid2]}},{"index":0,"id":"provider-a","function":{"name":"read_file","arguments":&args1[..mid1]}}]}),json!({"tool_calls":[{"index":0,"function":{"arguments":&args1[mid1..]}},{"index":1,"function":{"name":"file","arguments":&args2[mid2..]}}]})];
        let first=format!("{}data: [DONE]\n\n",deltas.iter().map(|d|format!("data: {}\n\n",json!({"choices":[{"delta":d}]}))).collect::<String>());
        let mut m=mock(vec![Plan::reply(first.into_bytes()),Plan::reply(complete("herramientas listas"))]).await;
        let f=Fixture::new(&m.url,Duration::from_secs(4)).await;
        {let mut cfg=f.backend.config.write().await;cfg.ai.enable_tool_calling=true;cfg.tools.read_file=true;cfg.tools.allowed_paths=vec![root.to_string_lossy().into()];}
        let mut a=f.begin(&f.sid).await;seen(&mut m).await;
        for count in 1..=2 {
            a.until_count("approval_needed",count).await;
            let approval=&a.events.iter().filter(|(name,_)|name=="approval_needed").nth(count-1).unwrap().1;
            let decision=json!({"tool_call_id":approval["tool_call_id"],"consent_id":approval["consent_id"],"turn_id":approval["turn_id"],"session_id":approval["session_id"],"approved":true});
            assert_eq!(f.post("/api/tools/approve",decision).send().await.unwrap().status(),200);
        }
        let second=seen(&mut m).await;a.until("done").await;a.end().await;
        let assistant=second.request["messages"].as_array().unwrap().iter().find(|m|m["role"]=="assistant").unwrap();
        let calls=assistant["tool_calls"].as_array().unwrap();assert_eq!(calls.len(),2);
        let outputs=second.request["messages"].as_array().unwrap().iter().filter(|m|m["role"]=="tool").collect::<Vec<_>>();assert_eq!(outputs.len(),2);
        for (call,output) in calls.iter().zip(&outputs) {assert_eq!(call["id"],output["tool_call_id"]);assert!(!call["id"].as_str().unwrap().starts_with("provider-"));}
        assert!(outputs[0]["content"].as_str().unwrap().contains("uno niño"));assert!(outputs[1]["content"].as_str().unwrap().contains("dos acción"));
        assert_eq!(f.backend.sessions.lock().unwrap().list_tool_audit(10).unwrap().len(),2);assert_eq!(f.messages(&f.sid).len(),5);
    });
}
#[test]
fn malformed_tool_schema_never_reaches_approval_or_executor() {
    if isolated("malformed_tool_schema_never_reaches_approval_or_executor") {
        return;
    }
    rt().block_on(async {
        let path = dirs::data_local_dir().unwrap().join("never.txt");
        let mut m = mock(vec![Plan::reply(tool(
            "create_file",
            &json!({"path":path}).to_string(),
        ))])
        .await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        {
            let mut cfg = f.backend.config.write().await;
            cfg.ai.enable_tool_calling = true;
            cfg.tools.create_file = true;
            cfg.tools.allowed_paths =
                vec![dirs::data_local_dir().unwrap().to_string_lossy().into()];
        }
        let mut a = f.begin(&f.sid).await;
        seen(&mut m).await;
        a.until("error").await;
        a.end().await;
        assert!(a.events.last().unwrap().1["message"]
            .as_str()
            .unwrap()
            .contains("malformed_tool"));
        assert!(!a
            .events
            .iter()
            .any(|(name, _)| name == "approval_needed" || name == "tool_result" || name == "done"));
        assert!(!path.exists());
        assert_eq!(f.state.approvals.pending_count(), 0);
        assert!(f
            .backend
            .sessions
            .lock()
            .unwrap()
            .list_tool_audit(10)
            .unwrap()
            .is_empty());
    });
}
#[test]
fn rate_limit_has_no_implicit_retry() {
    if isolated("rate_limit_has_no_implicit_retry") {
        return;
    }
    rt().block_on(async {
        let mut plan = Plan::reply(Vec::new());
        plan.status = 429;
        let mut m = mock(vec![plan]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let mut a = f.begin(&f.sid).await;
        seen(&mut m).await;
        a.until("error").await;
        a.end().await;
        assert!(a.events.last().unwrap().1["message"]
            .as_str()
            .unwrap()
            .contains("429"));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), m.seen.recv())
                .await
                .is_err()
        );
        assert_eq!(f.state.chat_tasks.active_count(), 0);
        eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
        assert_eq!(
            f.state.ai.activity.snapshot()["active"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    });
}

#[test]
fn cancel_before_headers_and_during_incomplete_utf8_or_event() {
    if isolated("cancel_before_headers_and_during_incomplete_utf8_or_event") {
        return;
    }
    rt().block_on(async {
        let mut headers = Plan::hold(Vec::new());
        headers.delay_headers = true;
        let mut m = mock(vec![
            headers,
            Plan::hold(b"data: {\"choices\":[{\"delta\":{\"content\":\"\xf0\x9f".to_vec()),
            Plan::hold(b"data: {\"choices\":[{".to_vec()),
        ])
        .await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        for _ in 0..3 {
            let mut a = f.begin(&f.sid).await;
            let request = seen(&mut m).await;
            f.cancel(&f.sid, &a.id()).await;
            a.until("cancelled").await;
            a.end().await;
            eventually(|| request.closed.load(Ordering::SeqCst)).await;
            eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
            assert!(!a
                .events
                .iter()
                .any(|(name, _)| name == "token" || name == "done"));
        }
        assert_eq!(f.messages(&f.sid).len(), 3);
    });
}
#[test]
fn complete_request_identity_query_and_cancellation() {
    if isolated("complete_request_identity_query_and_cancellation") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::hold(Vec::new())]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let request = f.post(
            "/api/chat/complete",
            json!({"session_id":f.sid,"message":"hello"}),
        );
        let client_task = tokio::spawn(async move { request.send().await.unwrap() });
        let provider = seen(&mut m).await;
        let identity: Value = f
            .client
            .get(format!("{}/api/chat/turn?session_id={}", f.base, f.sid))
            .header("Host", "127.0.0.1:8765")
            .bearer_auth(&f.state.local_token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = identity["turn_id"].as_str().unwrap();
        assert_eq!(identity["state"], "running");
        f.cancel(&f.sid, id).await;
        let response = client_task.await.unwrap();
        assert_eq!(response.status(), 409);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["state"], "cancelled");
        assert_eq!(body["turn_id"], id);
        eventually(|| provider.closed.load(Ordering::SeqCst)).await;
        eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
        assert_eq!(f.messages(&f.sid).len(), 1);
    });
}

#[test]
fn cancellation_during_supervised_summary_closes_its_http_request() {
    if isolated("cancellation_during_supervised_summary_closes_its_http_request") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![
            Plan::reply(complete("candidate response")),
            Plan::hold(Vec::new()),
        ])
        .await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        {
            let mut cfg = f.backend.config.write().await;
            cfg.memory.enabled = true;
            cfg.memory.auto_summarize = true;
        }
        {
            let sessions = f.backend.sessions.lock().unwrap();
            for _ in 0..30 {
                sessions
                    .add_message(
                        &f.sid,
                        &kde_assistant_lib::models::Message::user("old user"),
                    )
                    .unwrap();
                sessions
                    .add_message(
                        &f.sid,
                        &kde_assistant_lib::models::Message::assistant("old reply"),
                    )
                    .unwrap();
            }
        }
        let mut a = f.begin(&f.sid).await;
        seen(&mut m).await;
        let summary = seen(&mut m).await;
        assert_eq!(
            summary.request["messages"][0]["content"],
            "Eres un resumidor conciso. Solo hechos, decisiones y contexto útil."
        );
        f.cancel(&f.sid, &a.id()).await;
        a.until("cancelled").await;
        a.end().await;
        eventually(|| summary.closed.load(Ordering::SeqCst)).await;
        eventually(|| f.state.chat_tasks.running_tasks() == 0).await;
        assert_eq!(f.messages(&f.sid).len(), 61);
        assert!(!serde_json::to_string(&f.messages(&f.sid))
            .unwrap()
            .contains("candidate response"));
    });
}

#[test]
fn regeneration_rejects_completed_history_without_replaying_tools() {
    if isolated("regeneration_rejects_completed_history_without_replaying_tools") {
        return;
    }
    rt().block_on(async {
        let mut m = mock(vec![Plan::reply(complete("completed"))]).await;
        let f = Fixture::new(&m.url, Duration::from_secs(4)).await;
        let mut first = f.begin(&f.sid).await;
        seen(&mut m).await;
        first.until("done").await;
        first.end().await;
        let response = f
            .post("/api/chat/regenerate", json!({"session_id":f.sid}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let mut retry = Reader {
            stream: Box::pin(response.bytes_stream()),
            parser: kde_assistant_lib::backend::sse_parser::SseParser::default(),
            events: Vec::new(),
        };
        retry.until("error").await;
        retry.end().await;
        assert_eq!(retry.events[0].0, "turn");
        assert_ne!(retry.id(), first.id());
        assert!(retry.events.last().unwrap().1["message"]
            .as_str()
            .unwrap()
            .contains("regeneration_requires_unanswered_user"));
        assert_eq!(f.messages(&f.sid).len(), 2);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), m.seen.recv())
                .await
                .is_err()
        );
    });
}

#[test]
fn publication_failure_is_terminal_without_done_and_other_session_succeeds() {
    if isolated("publication_failure_is_terminal_without_done_and_other_session_succeeds") {
        return;
    }
    rt().block_on(async {
        let mut m=mock(vec![Plan::reply(complete("must rollback")),Plan::reply(complete("valid other"))]).await;
        let f=Fixture::new(&m.url,Duration::from_secs(4)).await;
        let conn=rusqlite::Connection::open(dirs::data_local_dir().unwrap().join("kde-assistant/sessions.db")).unwrap();
        conn.execute_batch(&format!("CREATE TRIGGER fail_publication BEFORE INSERT ON messages WHEN NEW.role='assistant' AND NEW.session_id='{}' BEGIN SELECT RAISE(ROLLBACK, 'injected publication failure'); END;", f.sid)).unwrap();
        let mut a=f.begin(&f.sid).await; seen(&mut m).await; a.until("error").await; a.end().await;
        assert_eq!(a.events.last().unwrap().1["state"],"failed");
        assert!(a.events.last().unwrap().1["message"].as_str().unwrap().contains("injected publication failure"));
        assert!(!a.events.iter().any(|(name,_)|name=="done"));
        assert_eq!(f.messages(&f.sid).len(),1);
        let state:String=conn.query_row("SELECT state FROM conversation_turns WHERE id=?1",[a.id()],|row|row.get(0)).unwrap(); assert_eq!(state,"failed");
        eventually(||f.state.chat_tasks.running_tasks()==0).await;
        let other=f.backend.sessions.lock().unwrap().create_session("other").unwrap().id;
        let mut b=f.begin(&other).await;seen(&mut m).await;b.until("done").await;b.end().await;
        assert_eq!(f.messages(&other).len(),2);
        assert_eq!(f.state.chat_tasks.active_count(),0);
        eventually(||f.state.chat_tasks.running_tasks()==0).await;
    });
}

async fn qml_review(mode: &str) {
    use axum::{
        response::sse::{Event, Sse},
        routing::{get, post},
        Json, Router,
    };
    use std::sync::atomic::AtomicUsize;
    let release = Arc::new(Notify::new());
    let cancelled = Arc::new(AtomicBool::new(false));
    let regenerations = Arc::new(AtomicUsize::new(0));
    let release_stream = release.clone();
    let cancel_seen = cancelled.clone();
    let release_endpoint = release.clone();
    let regenerate_count = regenerations.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8765")
        .await
        .expect("8765 libre para fixture QML");
    let stale = mode == "stale";
    let router = Router::new()
        .route("/api/sessions", get(move || async move {
        if stale { tokio::time::sleep(Duration::from_millis(700)).await; }
        Json(if stale {json!([{"id":"fixture-session","title":"obsolete","message_count":0}])} else {json!([])})
    })).route("/api/chat", post(move || {
            let release = release_stream.clone();
            async move {
                Sse::new(futures_util::stream::unfold(0usize, move |phase| {
                    let release = release.clone();
                    async move {
                        let (name, state, data) = match phase {
                            0 => ("turn", "running", json!({})),
                            1 => ("token", "running", json!({"content":"partial"})),
                            2 => { release.notified().await; ("cancelled", "cancelled", json!({"message":"Cancelado; efectos iniciados drenados"})) },
                            _ => return None,
                        };
                        let mut data = data;
                        data["turn_id"] = "fixture-turn".into();
                        data["session_id"] = "fixture-session".into();
                        data["sequence"] = (phase + 1).into();
                        data["state"] = state.into();
                        Some((Ok::<_, std::convert::Infallible>(Event::default().event(name).data(data.to_string())), phase + 1))
                    }
                }))
            }
        }))
        .route("/api/chat/cancel", post(move |Json(body): Json<Value>| {
            let seen = cancel_seen.clone();
            async move {
                assert_eq!(body["turn_id"], "fixture-turn");
                assert_eq!(body["session_id"], "fixture-session");
                seen.store(true, Ordering::SeqCst);
                (axum::http::StatusCode::ACCEPTED, Json(json!({"turn_id":"fixture-turn","session_id":"fixture-session","state":"cancelling","cancellation_requested":true,"cancellation_effective":false})))
            }
        }))
        .route("/release", get(move || { let release=release_endpoint.clone(); async move { release.notify_one(); "released" } }))
        .route("/api/chat/regenerate", post(move || { let count=regenerate_count.clone(); async move { count.fetch_add(1,Ordering::SeqCst); "unexpected replay" } }));
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let module =
        kde_assistant_lib::backend::auth::write_qml_auth_module(&"a".repeat(64), false).unwrap();
    let harness = include_str!("qml/F1B2Review.qml")
        .replace(
            "F1B2_QML_DIR",
            &format!("file://{}", repo.join("qml").display()),
        )
        .replace("F1B2_MODE", mode);
    let path = dirs::cache_dir().unwrap().join("F1B2Review.qml");
    fs::write(&path, harness).unwrap();
    let log_path = dirs::cache_dir().unwrap().join("qml-review.log");
    let log = fs::File::create(&log_path).unwrap();
    let child = Command::new("qml6")
        .args(["--verbose", "-apptype", "widget", "-I"])
        .arg(&repo)
        .arg("-I")
        .arg(&module)
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
    let status = tokio::time::timeout(Duration::from_secs(22), async {
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("QML timeout");
    let log = fs::read_to_string(log_path).unwrap();
    server.abort();
    let _ = server.await;
    assert!(
        status.success() && log.contains("F1B2_UI_PASS"),
        "{status}\n{log}"
    );
    assert_eq!(regenerations.load(Ordering::SeqCst), 0);
    assert_eq!(cancelled.load(Ordering::SeqCst), mode == "cancel");
}

#[test]
#[ignore = "requiere Qt/QML offscreen; fixture loopback"]
fn qml_regenerate_preserves_history_and_explains_restriction() {
    if isolated("qml_regenerate_preserves_history_and_explains_restriction") {
        return;
    }
    rt().block_on(qml_review("regenerate"));
}
#[test]
#[ignore = "requiere Qt/QML offscreen; fixture loopback"]
fn qml_cancel_202_remains_cancelling_until_terminal() {
    if isolated("qml_cancel_202_remains_cancelling_until_terminal") {
        return;
    }
    rt().block_on(qml_review("cancel"));
}

#[test]
#[ignore = "requiere Qt/QML offscreen; fixture loopback"]
fn qml_obsolete_session_list_cannot_replace_new_selection() {
    if isolated("qml_obsolete_session_list_cannot_replace_new_selection") {
        return;
    }
    rt().block_on(qml_review("stale"));
}
