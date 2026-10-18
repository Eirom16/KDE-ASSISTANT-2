use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use kde_assistant_lib::{backend::provider_connection as connection, models::Config};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct Mock {
    status: u16,
    code: String,
    invalid: bool,
    delay: bool,
    incomplete: bool,
    limited: bool,
    requests: Vec<(String, HeaderMap, Value)>,
}
type Shared = Arc<Mutex<Mock>>;
async fn models(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let (status, code, invalid, delay) = {
        let mut m = state.lock().unwrap();
        m.requests.push(("models".into(), headers, json!(null)));
        (m.status, m.code.clone(), m.invalid, m.delay)
    };
    if delay {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if status != 200 {
        return (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({"error":{"code":code,"message":"DO_NOT_ECHO_SYNTHETIC_SECRET"}})),
        )
            .into_response();
    }
    if invalid {
        return "<html>bad</html>".into_response();
    }
    Json(json!({"data":[{"id":"fixture-chat"},{"id":"fixture-chat"},{"id":"whisper-test"},{"id":"inactive", "active":false}]})).into_response()
}
async fn chat(State(state): State<Shared>, headers: HeaderMap, Json(body): Json<Value>) -> String {
    let (incomplete, limited) = {
        let mut m = state.lock().unwrap();
        m.requests.push(("chat".into(), headers, body));
        (m.incomplete, m.limited)
    };
    format!(
        "data: {}\n\n{}",
        json!({"choices":[{"delta":{"content":"OK"},"finish_reason":if limited { "length" } else { "stop" }}]}),
        if incomplete { "" } else { "data: [DONE]\n\n" }
    )
}
async fn mock() -> (String, Shared, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(Mock {
        status: 200,
        ..Default::default()
    }));
    let app = Router::new()
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(chat))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (url, state, task)
}
#[tokio::test]
async fn groq_openrouter_custom_catalog_and_stream() {
    let (url, state, server) = mock().await;
    for provider in ["groq", "openrouter", "custom", "openai"] {
        let mut cfg = Config::first_run();
        cfg.ai.provider = provider.into();
        cfg.ai.base_url = url.clone();
        cfg.ai
            .provider_endpoint_approvals
            .insert(provider.into(), url.clone());
        cfg.ai.model = "fixture-chat".into();
        cfg.ai.api_key = "synthetic-loopback-only".into();
        assert_eq!(
            connection::models(&cfg.ai, Duration::from_secs(2))
                .await
                .unwrap(),
            ["fixture-chat"]
        );
        connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap();
        let requests = std::mem::take(&mut state.lock().unwrap().requests);
        assert_eq!(requests.len(), 3);
        for (_, headers, _) in &requests {
            assert_eq!(headers["authorization"], "Bearer synthetic-loopback-only");
            assert_eq!(
                headers.contains_key("http-referer"),
                provider == "openrouter"
            );
            assert_eq!(headers.contains_key("x-title"), provider == "openrouter");
        }
        let body = &requests[2].2;
        assert_eq!(body["model"], "fixture-chat");
        assert_eq!(body["stream"], true);
        assert!(body.get("tools").is_none());
        assert_eq!(
            body[if provider == "groq" {
                "max_completion_tokens"
            } else {
                "max_tokens"
            }],
            64
        );
        assert!(body
            .get(if provider == "groq" {
                "max_tokens"
            } else {
                "max_completion_tokens"
            })
            .is_none());
    }
    server.abort();
}
#[tokio::test]
async fn errors_are_classified_without_remote_text_or_retries() {
    let (url, state, server) = mock().await;
    let mut cfg = Config::first_run();
    cfg.ai.provider = "groq".into();
    cfg.ai
        .provider_endpoint_approvals
        .insert("groq".into(), url.clone());
    cfg.ai.base_url = url;
    cfg.ai.api_key = "synthetic-loopback-only".into();
    cfg.ai.model = "fixture-chat".into();
    for (status, code, expected) in [
        (401, "", "invalid_key"),
        (403, "", "forbidden"),
        (404, "model_not_found", "model_not_found"),
        (404, "", "not_found"),
        (429, "", "rate_limit"),
        (429, "insufficient_quota", "quota"),
        (402, "", "quota"),
        (500, "", "provider_unavailable"),
        (503, "", "provider_unavailable"),
        (400, "", "parameters"),
        (302, "", "redirect"),
    ] {
        {
            let mut m = state.lock().unwrap();
            m.status = status;
            m.code = code.into();
            m.requests.clear();
        }
        let err = connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err();
        assert_eq!(err.category, expected);
        assert_eq!(err.provider_status, Some(status));
        assert!(!serde_json::to_string(&err).unwrap().contains("DO_NOT_ECHO"));
        assert_eq!(state.lock().unwrap().requests.len(), 1);
    }
    {
        let mut m = state.lock().unwrap();
        m.status = 200;
        m.invalid = true;
    }
    assert_eq!(
        connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "protocol"
    );
    {
        let mut m = state.lock().unwrap();
        m.invalid = false;
        m.delay = true;
    }
    assert_eq!(
        connection::probe(&cfg.ai, Duration::from_millis(30))
            .await
            .unwrap_err()
            .category,
        "timeout"
    );
    {
        let mut m = state.lock().unwrap();
        m.delay = false;
    }
    cfg.ai.model = "not-listed".into();
    assert_eq!(
        connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "model_not_found"
    );
    cfg.ai.model = "fixture-chat".into();
    state.lock().unwrap().limited = true;
    assert!(
        connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap()
            .limited
    );
    state.lock().unwrap().incomplete = true;
    assert_eq!(
        connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "protocol"
    );
    server.abort();
    let _ = server.await;
    assert_eq!(
        connection::probe(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "network"
    );
}
#[test]
fn custom_never_uses_openrouter_environment_and_deletion_blocks_fallback() {
    let mut cfg = Config::first_run();
    cfg.ai.api_key = "legacy-only".into();
    cfg.ai.migrate_profiles();
    assert_eq!(cfg.ai.provider_keys["openrouter"], "legacy-only");
    assert!(!format!("{:?}", cfg).contains("legacy-only"));
    cfg.ai.provider = "custom".into();
    assert_eq!(cfg.ai.effective_api_key(), "");
    cfg.ai.provider_keys.insert("groq".into(), String::new());
    cfg.ai.provider = "groq".into();
    assert_eq!(cfg.ai.effective_api_key(), "");
    assert_eq!(
        kde_assistant_lib::models::AiConfig::provider_env_var("custom"),
        ""
    );
}

#[test]
fn environment_keys_are_scoped() {
    if std::env::var("KDA_F35_ENV_CHILD").as_deref() != Ok("yes") {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "environment_keys_are_scoped"])
            .env("KDA_F35_ENV_CHILD", "yes")
            .env("GROQ_API_KEY", "synthetic-env-groq")
            .env("OPENROUTER_API_KEY", "synthetic-env-or")
            .env("OPENAI_API_KEY", "synthetic-env-openai")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut cfg = Config::first_run();
    for (id, expected) in [
        ("groq", "synthetic-env-groq"),
        ("openrouter", "synthetic-env-or"),
        ("openai", "synthetic-env-openai"),
        ("custom", ""),
    ] {
        cfg.ai.provider = id.into();
        assert_eq!(cfg.ai.effective_api_key(), expected);
        cfg.ai.provider_keys.insert(id.into(), String::new());
        assert!(cfg.ai.effective_api_key().is_empty());
    }
    cfg.ai.provider = "unknown-provider".into();
    cfg.ai.migrate_profiles();
    assert!(cfg.validate().is_err());
}

#[tokio::test]
async fn f35_audit_known_provider_does_not_send_key_to_unapproved_origin() {
    let (url, state, server) = mock().await;
    let mut cfg = Config::first_run();
    cfg.ai.provider = "groq".into();
    cfg.ai.base_url = url;
    cfg.ai
        .provider_keys
        .insert("groq".into(), "synthetic-loopback-only".into());
    let outcome = connection::models(&cfg.ai, Duration::from_secs(2)).await;
    assert!(
        outcome.is_err(),
        "known provider must require explicit approval for a different destination"
    );
    assert!(
        state.lock().unwrap().requests.is_empty(),
        "no credential may cross before approval"
    );
    cfg.ai
        .provider_endpoint_approvals
        .insert("groq".into(), cfg.ai.base_url.clone());
    connection::models(&cfg.ai, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(state.lock().unwrap().requests.len(), 1);
    cfg.ai.base_url.push_str("/changed");
    assert_eq!(
        connection::models(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "untrusted_endpoint"
    );
    assert_eq!(state.lock().unwrap().requests.len(), 1);
    server.abort();
}

#[tokio::test]
async fn f35_audit_probe_rejects_malformed_choice_and_terminal_sequences() {
    for data in [
        r#"{"choices":[{"index":"not-an-index","delta":{"content":"OK"}}]}"#,
        r#"{"choices":[{"delta":{"content":"OK","tool_calls":"malformed"}}]}"#,
    ] {
        let app = Router::new()
            .route(
                "/v1/models",
                get(|| async { Json(json!({"data":[{"id":"fixture-chat"}]})) }),
            )
            .route(
                "/v1/chat/completions",
                post(move || async move { format!("data: {data}\n\ndata: [DONE]\n\n") }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut cfg = Config::first_run();
        cfg.ai.provider = "custom".into();
        cfg.ai.base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        cfg.ai.api_key = "synthetic-loopback-only".into();
        cfg.ai.model = "fixture-chat".into();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = connection::probe(&cfg.ai, Duration::from_secs(2)).await;
        assert!(
            result.is_err(),
            "malformed SSE choices are not a successful diagnostic"
        );
        server.abort();
    }
}

#[tokio::test]
async fn f35_audit_stt_does_not_log_remote_body_or_retry_billing() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let count = Arc::new(AtomicUsize::new(0));
    let requests = count.clone();
    let app = Router::new().route(
        "/v1/audio/transcriptions",
        post(move || {
            let requests = requests.clone();
            async move {
                requests.fetch_add(1, Ordering::SeqCst);
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error":{"message":"SYNTHETIC_STT_SECRET_ECHO"}})),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = kde_assistant_lib::backend::stt_api::SttApiClient::new(
        format!("http://{}/v1", listener.local_addr().unwrap()),
        "synthetic-stt-only".into(),
        "fixture-stt".into(),
    )
    .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let error = client.transcribe(&[0.0; 16], None).await.unwrap_err();
    assert!(!error.to_string().contains("SYNTHETIC_STT_SECRET_ECHO"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
}

#[test]
fn f35_audit_switch_does_not_borrow_another_profiles_flat_values() {
    let mut cfg = Config::first_run();
    cfg.ai.migrate_profiles();
    cfg.ai.provider = "groq".into();
    assert_eq!(
        cfg.ai.effective_base_url(),
        "https://api.groq.com/openai/v1"
    );
    assert!(cfg.ai.effective_model().is_empty());
}

#[test]
fn f35_audit_legacy_duplicate_cannot_cross_provider_or_revive_deletion() {
    let mut cfg = Config::first_run();
    cfg.ai.provider = "groq".into();
    cfg.ai.api_key = "synthetic-old-openrouter".into();
    cfg.ai
        .provider_keys
        .insert("openrouter".into(), "synthetic-old-openrouter".into());
    cfg.ai.provider_keys.insert("groq".into(), String::new());
    cfg.ai.migrate_profiles();
    assert!(
        cfg.ai.effective_api_key().is_empty(),
        "a legacy duplicate from another profile must never replace the tombstone"
    );
    assert_eq!(
        cfg.ai.provider_keys["openrouter"],
        "synthetic-old-openrouter"
    );
}

#[tokio::test]
async fn f35_audit_redirect_never_reaches_another_origin_and_response_is_bounded() {
    let (target, state, target_task) = mock().await;
    let destination = format!("{target}/models");
    let app =
        Router::new()
            .route(
                "/v1/models",
                get(move || async move {
                    (StatusCode::TEMPORARY_REDIRECT, [("location", destination)])
                }),
            )
            .route(
                "/large/models",
                get(|| async { "x".repeat(2 * 1024 * 1024 + 1) }),
            );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut cfg = Config::first_run();
    cfg.ai.provider = "custom".into();
    cfg.ai.api_key = "synthetic-loopback-redirect-only".into();
    cfg.ai.base_url = format!("{origin}/v1");
    assert_eq!(
        connection::models(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "redirect"
    );
    assert!(state.lock().unwrap().requests.is_empty());
    cfg.ai.base_url = format!("{origin}/large");
    assert_eq!(
        connection::models(&cfg.ai, Duration::from_secs(2))
            .await
            .unwrap_err()
            .category,
        "protocol"
    );
    task.abort();
    target_task.abort();
}
