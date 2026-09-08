//! Test de integracion con OpenRouter API real.
//! Solo se ejecuta si OPENROUTER_API_KEY esta definida en el entorno.
//!
//! Ejecutar:  OPENROUTER_API_KEY=sk-... cargo test --test agent_smoke -- --nocapture

use kde_assistant_lib::{
    backend::ai_service::AiService, backend::tool_executor::ToolExecutor,
    backend::tool_registry::all_tools, models::Config, models::Message, models::StreamEvent,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

#[tokio::test]
async fn chat_with_openrouter() {
    let key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("[skip] OPENROUTER_API_KEY no definida, saltando test de red");
            return;
        }
    };

    let mut cfg = Config::default();
    cfg.ai.api_key = key;
    cfg.ai.max_tokens = 256;
    cfg.ai.enable_tool_calling = true;

    let cfg_arc = Arc::new(RwLock::new(cfg));
    let svc = AiService::new(cfg_arc.clone()).await.expect("AiService");
    let tools_exec = Arc::new(ToolExecutor::new(cfg_arc.clone()));

    let messages = vec![
        Message::system("Responde en espanol en una sola oracion corta."),
        Message::user("Di 'hola mundo' y nada mas."),
    ];

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(64);
    let svc_arc = Arc::new(svc);

    // Re-crear el AiService dentro de un Arc<...> para spawn
    let svc_for_task = svc_arc.clone();
    let tools_exec_clone = tools_exec.clone();
    let task = tokio::spawn(async move {
        svc_for_task
            .run_agent(messages, all_tools(), tools_exec_clone, tx)
            .await
    });

    let mut tokens = 0;
    let mut tool_calls = 0;
    while let Some(ev) = rx.recv().await {
        match ev {
            StreamEvent::Token { content } => {
                tokens += 1;
                print!("{content}");
            }
            StreamEvent::ToolCall { tool } => {
                tool_calls += 1;
                eprintln!("\n[tool_call] {} {:?}", tool.name, tool.arguments);
            }
            StreamEvent::ToolResult {
                tool_call_id,
                content,
                image_url,
            } => {
                eprintln!("\n[tool_result] {tool_call_id} -> {content} {image_url:?}");
            }
            StreamEvent::Done { full_content } => {
                eprintln!("\n[done] len={}", full_content.len());
            }
            StreamEvent::Error { message } => {
                eprintln!("\n[error] {message}");
            }
        }
    }
    println!();

    let result = timeout(Duration::from_secs(60), task)
        .await
        .expect("timeout")
        .expect("task")
        .expect("run_agent");

    eprintln!("\nResultado final: {result}");
    assert!(tokens > 0, "debio recibir al menos un token");
}
