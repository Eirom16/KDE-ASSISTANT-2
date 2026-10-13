//! Fixtures XDG en procesos separados: nunca se modifica el entorno del runner.
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use kde_assistant_lib::{
    backend::{auth, http_server, Backend},
    models::Config,
    owned_process::OwnedChild,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;

#[path = "../src/backend/private_files.rs"]
mod bootstrap_files;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn isolated(name: &str) -> bool {
    if std::env::var("KDA_F1B1_CHILD").as_deref() == Ok(name) {
        return false;
    }
    let root = std::env::temp_dir().join(format!("kda-f1b1-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let fixture = Fixture(root);
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", name, "--nocapture", "--include-ignored"])
        .env("KDA_F1B1_CHILD", name);
    for (key, _) in std::env::vars() {
        if key.ends_with("_API_KEY") {
            cmd.env_remove(key);
        }
    }
    for (key, dir) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        let path = fixture.0.join(dir);
        fs::create_dir(&path).unwrap();
        cmd.env(key, path);
    }
    cmd.env("ORT_DYLIB_PATH", fixture.0.join("absent-onnx.so"));
    let status = cmd.status().unwrap();
    assert!(status.success(), "fixture {name}: {status}");
    true
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap()
}
fn req(c: &reqwest::Client, base: &str, path: &str, token: &str) -> reqwest::RequestBuilder {
    c.get(format!("{base}{path}"))
        .header("Host", "127.0.0.1:8765")
        .bearer_auth(token)
}
async fn server(state: http_server::AppState) -> (String, JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, http_server::router(state))
            .await
            .unwrap();
    });
    (base, handle)
}
async fn stop(handle: JoinHandle<()>) {
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
}
type Requests = Arc<Mutex<Vec<Value>>>;
async fn provider(State(requests): State<Requests>, Json(body): Json<Value>) -> String {
    let mut recorded = requests.lock().unwrap();
    recorded.push(body);
    let reply = if recorded.len() == 1 {
        "reply-one"
    } else {
        "reply-two"
    };
    format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"delta":{"content":reply}}]})
    )
}
async fn mock() -> (String, Requests, JoinHandle<()>) {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new()
        .route("/v1/chat/completions", post(provider))
        .with_state(recorded.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (base, recorded, task)
}
async fn persisted(c: &reqwest::Client, base: &str, token: &str, sid: &str, count: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let value: Value = req(c, base, &format!("/api/messages?session_id={sid}"), token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if value.as_array().is_some_and(|a| a.len() == count) {
            return value;
        }
        assert!(Instant::now() < deadline, "persistencia: {value}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[test]
fn first_run_without_piper_whisper_onnx_or_wake() {
    if isolated("first_run_without_piper_whisper_onnx_or_wake") {
        return;
    }
    runtime().block_on(async {
        let b = Backend::new().await.unwrap();
        let cfg = b.config.read().await;
        assert!(!cfg.ai.enable_tool_calling);
        let flags = serde_json::to_value(&cfg.tools).unwrap();
        for (name, value) in flags.as_object().unwrap() {
            if value.is_boolean() && name != "confirm_sensitive" {
                assert_eq!(value, false, "{name}");
            }
        }
        assert!(cfg.tools.allowed_paths.is_empty());
        assert!(
            !cfg.speech.wake_word_enabled
                && !cfg.speech.auto_listen
                && !cfg.speech.auto_speak
                && !cfg.speech.chimes_enabled
        );
        assert!(!cfg.memory.enabled && !cfg.memory.auto_summarize && !cfg.character.enabled);
        assert!(cfg.ai.api_key.is_empty());
        assert!(b.audio.read().await.is_none());
        assert!(!b.speech.tts_initialized() && !b.speech.stt_model_exists());
        assert!(!b.hotword.ml_active() && !b.hotword.is_running());
        let data = dirs::data_local_dir().unwrap().join("kde-assistant");
        assert!(!data.join("models").exists() && !data.join("lib").exists());
        assert!(b.http_state().is_ok());
    });
}

#[test]
fn existing_configuration_is_preserved() {
    if isolated("existing_configuration_is_preserved") {
        return;
    }
    runtime().block_on(async {
        let mut cfg = Config::first_run();
        cfg.ai.provider = "groq".into();
        cfg.ai.model = "llama-3.3-70b-versatile".into();
        cfg.ai.api_key = "fixture-secret".into();
        cfg.speech.wake_word_enabled = true;
        cfg.tools.read_file = true;
        cfg.save().await.unwrap();
        let path = Config::config_path().unwrap();
        let before = fs::read(&path).unwrap();
        let b = Backend::new().await.unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
        assert_eq!(b.config.read().await.ai.api_key, "fixture-secret");
        assert!(!b.speech.tts_initialized() && !b.hotword.ml_active());
        assert!(b.audio.read().await.is_none());
    });
}

#[test]
fn corrupt_configuration_is_recoverable_and_never_replaced() {
    if isolated("corrupt_configuration_is_recoverable_and_never_replaced") {
        return;
    }
    runtime().block_on(async {
        let path = Config::config_path().unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{broken").unwrap();
        let error = match Backend::new().await {
            Err(e) => e,
            Ok(_) => panic!("corrupt config accepted"),
        };
        assert!(format!("{error:#}").contains("configuración inválida"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
        assert!(!dirs::data_local_dir()
            .unwrap()
            .join("kde-assistant")
            .exists());
        fs::write(path, serde_json::to_string(&Config::first_run()).unwrap()).unwrap();
        assert!(Backend::new().await.is_ok());
    });
}

#[test]
fn missing_api_key_never_contacts_provider() {
    if isolated("missing_api_key_never_contacts_provider") {
        return;
    }
    runtime().block_on(async {
        let (url, recorded, mock_task) = mock().await;
        let mut cfg = Config::first_run();
        cfg.ai.provider = "custom".into();
        cfg.ai.base_url = url;
        cfg.save().await.unwrap();
        let b = Backend::new().await.unwrap();
        let state = b.http_state().unwrap();
        let token = state.local_token.clone();
        let (base, task) = server(state).await;
        let c = client();
        let config: Value = req(&c, &base, "/api/config", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(config["ai"]["has_api_key"], false);
        let response = c
            .post(format!("{base}/api/chat"))
            .header("Host", "localhost:8765")
            .bearer_auth(&token)
            .json(&json!({"message":"hello"}))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(response.contains("API key no configurada"), "{response}");
        assert!(recorded.lock().unwrap().is_empty());
        stop(task).await;
        stop(mock_task).await;
    });
}

#[test]
fn invalid_tokens_fail_closed() {
    if isolated("invalid_tokens_fail_closed") {
        return;
    }
    runtime().block_on(async {
        let b = Backend::new().await.unwrap();
        let mut state = b.http_state().unwrap();
        state.local_token.clear();
        let (base, task) = server(state.clone()).await;
        let c = client();
        let response = c
            .get(format!("{base}/api/config"))
            .header("Host", "localhost:8765")
            .header("Authorization", "Bearer ")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert!(http_server::serve(state, 0).await.is_err());
        stop(task).await;
        fs::write(auth::server_token_path().unwrap(), "").unwrap();
        assert!(b.http_state().is_err());
        assert_eq!(fs::read(auth::server_token_path().unwrap()).unwrap(), b"");
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Authorization", "Bearer ".parse().unwrap());
        assert!(!auth::valid_bearer(&headers, ""));
    });
}

#[test]
fn hostile_authorities_are_rejected_over_http() {
    if isolated("hostile_authorities_are_rejected_over_http") {
        return;
    }
    runtime().block_on(async {
        let b = Backend::new().await.unwrap();
        let state = b.http_state().unwrap();
        let token = state.local_token.clone();
        let (base, task) = server(state).await;
        let c = client();
        for host in [
            "localhost.evil:8765",
            "127.0.0.1.evil:8765",
            "localhost:87650",
            "localhost:99",
            "evil.localhost:8765",
            "[::1].evil:8765",
            "localhost",
            "user@localhost:8765",
        ] {
            assert_eq!(
                c.get(format!("{base}/api/config"))
                    .header("Host", host)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                403,
                "{host}"
            );
        }
        for origin in [
            "https://evil.test/localhost",
            "http://localhost.evil:8765",
            "http://127.0.0.1:8765.evil",
            "http://evil@localhost:8765",
            "http://localhost:8765/path",
            "http://localhost:9999",
            "file://evil.test",
            "qrc:evil",
            "https://localhost:8765",
        ] {
            assert_eq!(
                req(&c, &base, "/api/config", &token)
                    .header("Origin", origin)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                403,
                "{origin}"
            );
        }
        for origin in [
            "null",
            "http://localhost:8765",
            "http://127.0.0.1:8765",
            "http://[::1]:8765",
        ] {
            assert_eq!(
                req(&c, &base, "/api/config", &token)
                    .header("Origin", origin)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                200
            );
        }
        assert!(!auth::valid_host(&axum::http::HeaderMap::new()));
        stop(task).await;
    });
}

#[test]
fn private_files_redaction_and_failed_update_preserve_secrets() {
    if isolated("private_files_redaction_and_failed_update_preserve_secrets") {
        return;
    }
    runtime().block_on(async {
        let mut cfg = Config::first_run();
        cfg.ai.api_key = "fixture-secret".into();
        cfg.ai
            .provider_keys
            .insert("groq".into(), "fixture-groq".into());
        cfg.save().await.unwrap();
        cfg.save().await.unwrap();
        let b = Backend::new().await.unwrap();
        let state = b.http_state().unwrap();
        let token = state.local_token.clone();
        let module = auth::write_qml_auth_module(&token, false).unwrap();
        for path in [
            Config::config_path().unwrap(),
            Config::config_path().unwrap().with_extension("json.bak"),
            auth::server_token_path().unwrap(),
            module.join("qml/auth/AuthToken.qml"),
            module.join("qml/auth/qmldir"),
        ] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(
            fs::metadata(module.join("qml/auth"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let (base, task) = server(state).await;
        let c = client();
        let public = req(&c, &base, "/api/config", &token)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(!public.contains("fixture-secret") && !public.contains("fixture-groq"));
        let mut update: Value = serde_json::from_str(&public).unwrap();
        update["ai"]["model"] = json!("changed-model");
        let post_cfg = |value: Value| {
            c.post(format!("{base}/api/config"))
                .header("Host", "localhost:8765")
                .bearer_auth(&token)
                .json(&value)
        };
        assert_eq!(post_cfg(update.clone()).send().await.unwrap().status(), 200);
        assert_eq!(
            b.config.read().await.ai.provider_keys["groq"],
            "fixture-groq"
        );
        assert_eq!(
            b.config.read().await.ai.provider_keys["openrouter"],
            "fixture-secret"
        );
        let saved = fs::read(Config::config_path().unwrap()).unwrap();
        let backup = Config::config_path().unwrap().with_extension("json.bak");
        fs::remove_file(&backup).unwrap();
        let outside = backup.parent().unwrap().join("outside");
        fs::write(&outside, "untouched").unwrap();
        std::os::unix::fs::symlink(&outside, &backup).unwrap();
        update["ai"]["model"] = json!("must-not-publish");
        assert_eq!(post_cfg(update).send().await.unwrap().status(), 500);
        assert_eq!(b.config.read().await.ai.model, "changed-model");
        assert_eq!(fs::read(Config::config_path().unwrap()).unwrap(), saved);
        assert_eq!(fs::read_to_string(outside).unwrap(), "untouched");
        stop(task).await;
    });
}

#[test]
fn exclusive_bootstrap_only_tolerates_an_existing_winner() {
    if isolated("exclusive_bootstrap_only_tolerates_an_existing_winner") {
        return;
    }
    let path = dirs::config_dir().unwrap().join("bootstrap/value");
    bootstrap_files::write_private(&path, "winner", true).unwrap();
    bootstrap_files::allow_creation_race(bootstrap_files::write_private(&path, "loser", true))
        .unwrap();
    assert_eq!(bootstrap_files::read_private(&path).unwrap(), "winner");
    // Inyecta el resultado de sync fallido después de publicar: la existencia
    // del destino no debe convertir EIO en éxito. No simula una syscall real.
    let failure = std::io::Error::from_raw_os_error(5);
    assert!(bootstrap_files::allow_creation_race(Err(failure.into())).is_err());
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn redacted_updates_preserve_legacy_and_provider_credentials() {
    if isolated("redacted_updates_preserve_legacy_and_provider_credentials") {
        return;
    }
    runtime().block_on(async {
        for active in ["", "fixture-active-map"] {
            let mut cfg = Config::first_run();
            cfg.ai.api_key = "fixture-legacy".into();
            cfg.ai
                .provider_keys
                .insert("openrouter".into(), active.into());
            cfg.ai
                .provider_keys
                .insert("groq".into(), "fixture-groq".into());
            cfg.save().await.unwrap();
            let b = Backend::new().await.unwrap();
            let state = b.http_state().unwrap();
            let token = state.local_token.clone();
            let (base, task) = server(state).await;
            let c = client();
            for selected in ["openrouter", "groq"] {
                let mut public: Value = req(&c, &base, "/api/config", &token)
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                public["ai"]["provider"] = json!(selected);
                assert_eq!(
                    c.post(format!("{base}/api/config"))
                        .header("Host", "localhost:8765")
                        .bearer_auth(&token)
                        .json(&public)
                        .send()
                        .await
                        .unwrap()
                        .status(),
                    200
                );
                let saved = Config::load().await.unwrap();
                assert_eq!(saved.ai.api_key, "fixture-legacy");
                assert_eq!(saved.ai.provider_keys["groq"], "fixture-groq");
                assert_eq!(
                    saved.ai.provider_keys["openrouter"],
                    if active.is_empty() {
                        "fixture-legacy"
                    } else {
                        active
                    }
                );
                assert_eq!(
                    saved.ai.effective_api_key(),
                    if selected == "groq" {
                        "fixture-groq"
                    } else if active.is_empty() {
                        "fixture-legacy"
                    } else {
                        active
                    }
                );
            }
            let mut public: Value = req(&c, &base, "/api/config", &token)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            public["ai"]["api_key"] = json!("fixture-explicit-replacement");
            assert_eq!(
                c.post(format!("{base}/api/config"))
                    .header("Host", "localhost:8765")
                    .bearer_auth(&token)
                    .json(&public)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                200
            );
            let saved = Config::load().await.unwrap();
            assert_eq!(saved.ai.api_key, "fixture-explicit-replacement");
            assert_eq!(
                saved.ai.provider_keys["groq"],
                "fixture-explicit-replacement"
            );
            stop(task).await;
        }
    });
}

#[test]
fn model_catalog_uses_selected_provider_credentials_and_url() {
    if isolated("model_catalog_uses_selected_provider_credentials_and_url") {
        return;
    }
    runtime().block_on(async {
        async fn catalog() -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
            let keys = Arc::new(Mutex::new(Vec::new()));
            let recorded = keys.clone();
            let router = Router::new().route(
                "/v1/models",
                get(move |headers: axum::http::HeaderMap| {
                    let recorded = recorded.clone();
                    async move {
                        recorded
                            .lock()
                            .unwrap()
                            .push(headers["Authorization"].to_str().unwrap().to_string());
                        Json(json!({"data":[{"id":"fixture-model"}]}))
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/v1", listener.local_addr().unwrap());
            let task = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            (url, keys, task)
        }
        let (current_url, current_keys, current_task) = catalog().await;
        let (other_url, other_keys, other_task) = catalog().await;
        let mut cfg = Config::first_run();
        cfg.ai.base_url = current_url;
        cfg.ai
            .provider_keys
            .insert("openrouter".into(), "fixture-current-key".into());
        cfg.ai
            .provider_keys
            .insert("groq".into(), "fixture-other-key".into());
        cfg.ai.provider_base_urls.insert("groq".into(), other_url);
        cfg.save().await.unwrap();
        let b = Backend::new().await.unwrap();
        let state = b.http_state().unwrap();
        let token = state.local_token.clone();
        let (base, task) = server(state).await;
        let response: Value = client()
            .post(format!("{base}/api/ai-models"))
            .header("Host", "localhost:8765")
            .bearer_auth(&token)
            .json(&json!({"provider":"groq"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["models"], json!(["fixture-model"]));
        assert!(current_keys.lock().unwrap().is_empty());
        assert_eq!(
            *other_keys.lock().unwrap(),
            vec!["Bearer fixture-other-key"]
        );
        stop(task).await;
        stop(current_task).await;
        stop(other_task).await;
    });
}

#[test]
fn two_turns_with_mock_and_persistence_after_backend_restart() {
    if isolated("two_turns_with_mock_and_persistence_after_backend_restart") {
        return;
    }
    runtime().block_on(async {
        let (url, recorded, mock_task) = mock().await;
        let b = Backend::new().await.unwrap();
        let state = b.http_state().unwrap();
        let token = state.local_token.clone();
        let (base, task) = server(state).await;
        let c = client();
        assert_eq!(
            req(&c, &base, "/api/health", "")
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let mut public: Value = req(&c, &base, "/api/config", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        public["ai"]["provider"] = json!("custom");
        public["ai"]["base_url"] = json!(url);
        public["ai"]["model"] = json!("mock-text");
        public["ai"]["api_key"] = json!("fixture-key");
        assert_eq!(
            c.post(format!("{base}/api/config"))
                .header("Host", "localhost:8765")
                .bearer_auth(&token)
                .json(&public)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let session: Value = c
            .post(format!("{base}/api/session"))
            .header("Host", "localhost:8765")
            .bearer_auth(&token)
            .json(&json!({"title":"fixture"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let sid = session["id"].as_str().unwrap();
        for (i, message) in ["first", "second"].iter().enumerate() {
            let stream = c
                .post(format!("{base}/api/chat"))
                .header("Host", "localhost:8765")
                .bearer_auth(&token)
                .json(&json!({"message":message,"session_id":sid}))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(
                stream.contains("event: token") && stream.contains("event: done"),
                "{stream}"
            );
            assert!(stream.contains(if i == 0 { "reply-one" } else { "reply-two" }));
            persisted(&c, &base, &token, sid, (i + 1) * 2).await;
        }
        let before = persisted(&c, &base, &token, sid, 4).await;
        let previous_db = Arc::downgrade(&b.sessions);
        stop(task).await;
        drop(b);
        tokio::time::timeout(Duration::from_secs(3), async {
            while previous_db.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let restarted = Backend::new().await.unwrap();
        let state = restarted.http_state().unwrap();
        assert_eq!(state.local_token, token);
        let (base, task) = server(state).await;
        assert_eq!(persisted(&c, &base, &token, sid, 4).await, before);
        {
            let requests = recorded.lock().unwrap();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[1]["messages"][1]["content"], "first");
            assert_eq!(requests[1]["messages"][2]["content"], "reply-one");
            assert_eq!(requests[1]["messages"][3]["content"], "second");
            assert!(requests.iter().all(|r| r
                .get("tools")
                .is_none_or(|t| t.as_array().is_some_and(|t| t.is_empty()))));
        }
        stop(task).await;
        stop(mock_task).await;
    });
}

#[test]
fn startup_has_no_download_or_global_process_cleanup_calls() {
    let backend = include_str!("../src/backend/mod.rs");
    let main = include_str!("../src/main.rs");
    let snapshot = include_str!("../src/bin/snap_ui.rs");
    assert!(
        !backend.contains("ensure_models_available")
            && !backend.contains("downloader.download")
            && !backend.contains("ensure_onnx_runtime_lib")
    );
    assert!(
        !main.contains("pre-warm")
            && !main.contains("pkill")
            && !snapshot.contains("pkill")
            && !snapshot.contains(".X99-lock")
    );
}

fn wait_exit(child: &mut OwnedChild) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "child failed to stop");
        std::thread::sleep(Duration::from_millis(30));
    }
}
struct TestApp {
    child: OwnedChild,
    log: PathBuf,
}
impl std::ops::Deref for TestApp {
    type Target = OwnedChild;
    fn deref(&self) -> &OwnedChild {
        &self.child
    }
}
impl std::ops::DerefMut for TestApp {
    fn deref_mut(&mut self) -> &mut OwnedChild {
        &mut self.child
    }
}
impl Drop for TestApp {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = Command::new("kill")
                .args(["-INT", &self.child.id().to_string()])
                .status();
            let deadline = Instant::now() + Duration::from_secs(3);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(30));
            }
        }
    }
}
fn app(args: &[&str]) -> TestApp {
    app_with(args, |_| {})
}
fn app_with(args: &[&str], configure: impl FnOnce(&mut Command)) -> TestApp {
    let log = dirs::cache_dir()
        .unwrap()
        .join(format!("application-{}.log", uuid::Uuid::new_v4()));
    let output = fs::File::create(&log).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_kde-assistant"));
    command
        .args(args)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_ASSUME_STDERR_HAS_CONSOLE", "1")
        .stdout(output.try_clone().unwrap())
        .stderr(output);
    configure(&mut command);
    let child = OwnedChild::new(command.spawn().unwrap());
    TestApp { child, log }
}

#[test]
fn real_binary_lifecycle_and_critical_failure_leave_no_listener_or_foreign_victim() {
    if isolated("real_binary_lifecycle_and_critical_failure_leave_no_listener_or_foreign_victim") {
        return;
    }
    // Sólo este test usa el puerto de producción; si está ocupado no tocamos su dueño.
    let reserved = std::net::TcpListener::bind("127.0.0.1:8765")
        .expect("8765 debe estar libre para fixture real");
    let path = Config::config_path().unwrap();
    let mut foreign = OwnedChild::new(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let mut duplicate = app(&["--ui-off", "--no-shortcuts"]);
    assert!(!wait_exit(&mut duplicate).success());
    assert!(fs::read_to_string(&duplicate.log)
        .unwrap()
        .contains("Puerto 8765 ocupado"));
    assert!(!path.exists());
    assert!(foreign.try_wait().unwrap().is_none());
    drop(reserved);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{bad").unwrap();
    let mut broken = app(&["--ui-off", "--no-shortcuts"]);
    assert!(!wait_exit(&mut broken).success());
    assert!(fs::read_to_string(&broken.log)
        .unwrap()
        .contains("configuración inválida"));
    assert!(std::net::TcpListener::bind("127.0.0.1:8765").is_ok());
    fs::remove_file(&path).unwrap();
    let mut running = app(&["--ui-off", "--no-shortcuts"]);
    runtime().block_on(async {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if client()
                .get("http://127.0.0.1:8765/api/health")
                .send()
                .await
                .is_ok_and(|response| response.status() == 200)
            {
                break;
            }
            assert!(Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    });
    assert!(Command::new("kill")
        .args(["-INT", &running.id().to_string()])
        .status()
        .unwrap()
        .success());
    assert!(wait_exit(&mut running).success());
    assert!(std::net::TcpListener::bind("127.0.0.1:8765").is_ok());
    assert!(foreign.try_wait().unwrap().is_none());
    // Un fallo de token tampoco publica HTTP.
    fs::write(auth::server_token_path().unwrap(), "invalid").unwrap();
    let mut bad_token = app(&["--ui-off", "--no-shortcuts"]);
    assert!(!wait_exit(&mut bad_token).success());
    assert!(fs::read_to_string(&bad_token.log)
        .unwrap()
        .contains("Token local inválido"));
    assert!(std::net::TcpListener::bind("127.0.0.1:8765").is_ok());
    fs::remove_file(auth::server_token_path().unwrap()).unwrap();
    let empty_path = dirs::cache_dir().unwrap().join("empty-path");
    fs::create_dir(&empty_path).unwrap();
    let mut missing_qml = app_with(&["--no-shortcuts"], |command| {
        command.env("PATH", &empty_path);
    });
    assert!(!wait_exit(&mut missing_qml).success());
    assert!(fs::read_to_string(&missing_qml.log)
        .unwrap()
        .contains("lanzando qml6"));
    assert!(std::net::TcpListener::bind("127.0.0.1:8765").is_ok());
    assert!(foreign.try_wait().unwrap().is_none());
    let own_pid;
    {
        let own = OwnedChild::new(Command::new("/bin/sleep").arg("30").spawn().unwrap());
        own_pid = own.id();
    }
    assert!(!PathBuf::from(format!("/proc/{own_pid}")).exists());
    assert!(foreign.try_wait().unwrap().is_none());
}

#[test]
#[ignore = "requiere Qt/QML offscreen; no certifica Plasma Wayland"]
fn qml_offscreen_settings_and_two_visible_turns() {
    if isolated("qml_offscreen_settings_and_two_visible_turns") {
        return;
    }
    runtime().block_on(async {
        let (provider_url, recorded, mock_task) = mock().await;
        let b = Backend::new().await.unwrap();
        let state = b.http_state().unwrap();
        let token = state.local_token.clone();
        let module = auth::write_qml_auth_module(&token, false).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8765")
            .await
            .expect("puerto 8765 libre para UI");
        let base = "http://127.0.0.1:8765".to_string();
        let task = tokio::spawn(async move {
            axum::serve(listener, http_server::router(state))
                .await
                .unwrap();
        });
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let harness = include_str!("qml/F1B1Text.qml")
            .replace(
                "F1B1_QML_DIR",
                &format!("file://{}", repo.join("qml").display()),
            )
            .replace(
                "F1B1_COMPONENT_DIR",
                &format!("file://{}", repo.join("qml/components").display()),
            )
            .replace("F1B1_BACKEND", &base)
            .replace("F1B1_PROVIDER", &provider_url);
        let path = dirs::cache_dir().unwrap().join("F1B1Text.qml");
        fs::write(&path, harness).unwrap();
        let log_path = dirs::cache_dir().unwrap().join("qml.log");
        let log = fs::File::create(&log_path).unwrap();
        let mut child = OwnedChild::new(
            Command::new("qml6")
                .args(["--verbose", "-apptype", "widget", "-I"])
                .arg(&repo)
                .arg("-I")
                .arg(&module)
                .arg(path)
                .env("QT_QPA_PLATFORM", "offscreen")
                .env("QT_LOGGING_RULES", "*.warning=true;*.critical=true")
                .env("QT_ASSUME_STDERR_HAS_CONSOLE", "1")
                .env("QML_DISABLE_DISK_CACHE", "1")
                .env("QML_XHR_ALLOW_FILE_READ", "1")
                .stdout(log.try_clone().unwrap())
                .stderr(log)
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "QML timeout: {}",
                fs::read_to_string(&log_path).unwrap()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let log = fs::read_to_string(log_path).unwrap();
        assert!(
            status.success() && log.contains("F1B1_UI_PASS"),
            "{status}\n{log}"
        );
        assert!(!log.contains("F1B1_UI_FAIL"), "{log}");
        let sid = {
            let sessions = b.sessions.lock().unwrap();
            let list = sessions.list_sessions().unwrap();
            assert_eq!(list.len(), 1);
            list[0].id.clone()
        };
        persisted(&client(), &base, &token, &sid, 4).await;
        assert_eq!(recorded.lock().unwrap().len(), 2);
        assert!(!PathBuf::from(format!("/proc/{}", child.id())).exists());
        println!("F1B1_UI_PASS; QML terminado y recogido; cuatro mensajes persistidos");
        let previous_db = Arc::downgrade(&b.sessions);
        stop(task).await;
        drop(b);
        tokio::time::timeout(Duration::from_secs(3), async {
            while previous_db.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        // El ejecutable real también debe lanzar y recoger su propia UI.
        let foreign_qml = dirs::cache_dir().unwrap().join("Main.qml");
        fs::write(
            &foreign_qml,
            "import QtQuick\nWindow { visible: true; width: 100; height: 100 }\n",
        )
        .unwrap();
        let mut foreign = OwnedChild::new(
            Command::new("qml6")
                .arg(&foreign_qml)
                .env("QT_QPA_PLATFORM", "offscreen")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut application = app(&["--no-shortcuts"]);
        let children_path = PathBuf::from(format!(
            "/proc/{}/task/{}/children",
            application.id(),
            application.id()
        ));
        let deadline = Instant::now() + Duration::from_secs(8);
        let own_ui = loop {
            assert!(
                application.try_wait().unwrap().is_none(),
                "el binario terminó antes de lanzar UI"
            );
            if let Ok(children) = fs::read_to_string(&children_path) {
                if let Some(pid) = children.split_whitespace().find(|pid| {
                    fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|args| {
                        let argv = args.split(|byte| *byte == 0).collect::<Vec<_>>();
                        argv.first().is_some_and(|arg| arg.ends_with(b"qml6"))
                            && argv.contains(&b"qml/Main.qml".as_slice())
                    })
                }) {
                    break pid.to_owned();
                }
            }
            assert!(Instant::now() < deadline, "el binario no lanzó QML");
            tokio::time::sleep(Duration::from_millis(30)).await;
        };
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(application.try_wait().unwrap().is_none());
        assert!(foreign.try_wait().unwrap().is_none());
        assert!(Command::new("kill")
            .args(["-INT", &application.id().to_string()])
            .status()
            .unwrap()
            .success());
        assert!(wait_exit(&mut application).success());
        assert!(!PathBuf::from(format!("/proc/{own_ui}")).exists());
        assert!(foreign.try_wait().unwrap().is_none());
        assert!(std::net::TcpListener::bind("127.0.0.1:8765").is_ok());
        println!(
            "F1B1_BINARY_UI_PASS; hijo QML propio recogido; QML ajeno sigue vivo; puerto liberado"
        );
        let broken_ui_dir = dirs::cache_dir().unwrap().join("broken-ui");
        fs::create_dir_all(broken_ui_dir.join("qml")).unwrap();
        fs::write(broken_ui_dir.join("qml/Main.qml"), "import QtQuick\nWindow { invalid QML").unwrap();
        let mut broken_ui = app_with(&["--no-shortcuts"], |command| {
            command.current_dir(&broken_ui_dir);
        });
        assert!(!wait_exit(&mut broken_ui).success());
        assert!(fs::read_to_string(&broken_ui.log).unwrap().contains("interfaz QML terminó con error"));
        assert!(std::net::TcpListener::bind("127.0.0.1:8765").is_ok());
        assert!(foreign.try_wait().unwrap().is_none());
        println!("F1B1_QML_FAILURE_PASS; fallo real de carga QML visible; puerto liberado; QML ajeno vivo");
        stop(mock_task).await;
    });
}
