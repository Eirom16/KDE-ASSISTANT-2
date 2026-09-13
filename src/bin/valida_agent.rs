// valida_agent.rs - Smoke test offscreen del Assistant Character (qml/agent/).
// Hermano de valida_qml.rs: carga el harness DevPreview (que instancia
// AssistantCharacter + ExpressionController + AgentAnimationController +
// CharacterController) y falla si qml6 no lo levanta.
//
// Uso: cargo run --bin valida_agent

use anyhow::Result;
use std::process::Command;

fn main() -> Result<()> {
    let child = Command::new("qml6")
        .args(["-I", ".", "qml/agent/DevPreview.qml"])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_ASSUME_STDERR_HAS_CONSOLE", "1")
        .env("QML_DISABLE_DISK_CACHE", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut child = child;
    std::thread::sleep(std::time::Duration::from_secs(3));

    match child.try_wait()? {
        Some(status) => {
            if !status.success() {
                eprintln!("qml6 fallo con {:?}", status);
                std::process::exit(1);
            }
            // DevPreview debe seguir vivo (ventana); un exit 0 inmediato
            // tambien es sospechoso: avisar pero no fallar.
            eprintln!("aviso: qml6 termino por si solo (revisar DevPreview)");
        }
        None => {
            child.kill()?;
            child.wait()?;
        }
    }

    println!("qml6 cargo qml/agent/DevPreview.qml sin errores criticos");
    Ok(())
}
