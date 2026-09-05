// KDE Assistant v2 - Entry Point
//
// Inicializa logging, runtime Tokio, backend, y luego lanza la UI QML
// como subproceso de `qml6`. La integracion Rust<->QML se hara via
// DBus/socket en una fase posterior (cuando llegue Fase 6).
//
// Uso: cargo run [-- --ui-off]

use anyhow::{Context, Result};
use kde_assistant_lib::backend::Backend;
use std::process::Command;
use std::sync::Arc;

fn main() -> Result<()> {
    // Init logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("KDE Assistant v2 starting...");

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let ui_off = args.iter().any(|a| a == "--ui-off" || a == "--no-ui");

    // Init backend en runtime Tokio
    let runtime = tokio::runtime::Runtime::new().context("creando Tokio runtime")?;
    let _backend = runtime.block_on(async {
        let b = Backend::new().await?;
        log::info!("Backend inicializado correctamente");
        Ok::<_, anyhow::Error>(Arc::new(b))
    })?;

    // Lanzar UI QML como subproceso
    if !ui_off {
        log::info!("Lanzando UI QML (qml6)...");
        // -I . le dice a qml6 donde buscar modulos (qml/, qml/components/)
        let qml_status = Command::new("qml6")
            .args(["-I", ".", "qml/Main.qml"])
            .status()
            .context(
                "lanzando qml6 (asegurate de tener Qt6 instalado: pacman -S qt6-declarative)",
            )?;

        log::info!("UI QML termino con codigo {:?}", qml_status.code());
    } else {
        log::info!("UI desactivada por flag --ui-off");
        log::info!("Presiona Ctrl+C para salir.");
        runtime.block_on(async {
            tokio::signal::ctrl_c().await.ok();
        });
    }

    log::info!("Apagando KDE Assistant v2...");
    Ok(())
}
