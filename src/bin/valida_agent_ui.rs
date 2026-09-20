// Smoke test offscreen para los flujos UI agregados al agente:
// selector de modelos, dashboard y sincronizacion de escenas por tool call.

use anyhow::{bail, Result};
use std::process::{Command, Stdio};

fn main() -> Result<()> {
    let output = Command::new("qml6")
        .args([
            "-I",
            ".",
            "-apptype",
            "widget",
            "tests/qml/AgentUiHarness.qml",
        ])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_ASSUME_STDERR_HAS_CONSOLE", "1")
        .env("QML_DISABLE_DISK_CACHE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        bail!(
            "AgentUiHarness fallo con {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("qml6 valido tests/qml/AgentUiHarness.qml");
    Ok(())
}
