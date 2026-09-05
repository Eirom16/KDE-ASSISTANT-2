// KDE Assistant v2 - Entry Point
//
// Fase 1 (Scaffold): Inicializa logging, runtime Tokio, y la estructura
// base del backend. La UI de Qt/QML se conectara en Fase 3 via cxx-qt.

use anyhow::Result;
use std::sync::Arc;

mod backend;
mod models;

use backend::Backend;

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("KDE Assistant v2 starting...");

    // Init backend
    let backend = Arc::new(Backend::new().await?);
    log::info!("Backend inicializado correctamente");

    // TODO(Fase 3): Inicializar Qt Engine y cargar Main.qml
    // TODO(Fase 3): Crear QSystemTrayIcon
    // TODO(Fase 3): Registrar Global Shortcuts (KGlobalAccel)

    // Por ahora solo bloqueamos para mantener el proceso vivo
    log::info!("Scaffold en ejecucion. Presiona Ctrl+C para salir.");

    // Esperar senal de terminacion
    tokio::signal::ctrl_c().await?;
    log::info!("Apagando KDE Assistant v2...");

    Ok(())
}
