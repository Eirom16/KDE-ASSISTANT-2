// KDE Assistant v2 - Entry Point
//
// Inicializa logging, runtime Tokio, backend, integración KDE,
// global hotkeys (rdev), y luego lanza la UI QML como subproceso de `qml6`.
//
// Flags:
//   --ui-off       No lanzar UI (solo backend + KDE integration)
//   --no-shortcuts No registrar global shortcuts
//   --send-msg <s> Emitir senal DBus SendMessage con texto y salir
//   --toggle       Emitir senal ShowWindow (toggle) y salir

use anyhow::{Context, Result};
use kde_assistant_lib::backend::Backend;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("KDE Assistant v2 starting...");

    let args: Vec<String> = std::env::args().collect();
    let ui_off = args.iter().any(|a| a == "--ui-off" || a == "--no-ui");
    let no_shortcuts = args
        .iter()
        .any(|a| a == "--no-shortcuts" || a == "--no-shortcuts");

    // Modo comando: enviar senal y salir
    if let Some(idx) = args.iter().position(|a| a == "--send-msg") {
        if let Some(msg) = args.get(idx + 1) {
            let status = Command::new("dbus-send")
                .args([
                    "--session",
                    "--type=signal",
                    "--dest=org.kde.assistant",
                    "/Chat",
                    "org.kde.assistant.Chat.SendMessage",
                    &format!("string:{msg}"),
                ])
                .status();
            return match status {
                Ok(s) if s.success() => Ok(()),
                _ => {
                    eprintln!("No se pudo enviar senal. ¿Esta corriendo kde-assistant?");
                    std::process::exit(1);
                }
            };
        }
    }
    if args.iter().any(|a| a == "--toggle") {
        let status = Command::new("dbus-send")
            .args([
                "--session",
                "--type=signal",
                "--dest=org.kde.assistant",
                "/Chat",
                "org.kde.assistant.Chat.ShowWindow",
            ])
            .status();
        return match status {
            Ok(s) if s.success() => Ok(()),
            _ => {
                eprintln!("No se pudo enviar senal. ¿Esta corriendo kde-assistant?");
                std::process::exit(1);
            }
        };
    }

    // Init backend en runtime Tokio
    let runtime = tokio::runtime::Runtime::new().context("creando Tokio runtime")?;
    runtime.block_on(async {
        let _backend = Backend::new().await?;
        log::info!("Backend inicializado correctamente");
        Ok::<_, anyhow::Error>(())
    })?;

    // Setup KDE integration (theme detection, notifications)
    let mut kde = kde_assistant_lib::backend::kde_integration::KdeIntegration::new();
    if let Err(e) = kde.setup() {
        log::warn!("KDE setup fallo: {e}");
    }
    log::info!("KDE integration: theme dark={}", kde.theme.is_dark());

    // Start global hotkey listener (rdev, en thread separado)
    if !no_shortcuts {
        let (action_tx, _action_rx) = tokio::sync::mpsc::channel::<
            kde_assistant_lib::backend::hotkey_listener::HotkeyAction,
        >(32);
        let _handle = kde_assistant_lib::backend::hotkey_listener::start_listener(action_tx);
        log::info!("Global hotkey listener: Super+Shift+A (toggle), Super+Shift+V (PTT), Ctrl+Shift+K (new session)");
    } else {
        log::info!("Global shortcuts deshabilitados (--no-shortcuts)");
    }

    // Señal de apagado limpio
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_signal.store(true, Ordering::SeqCst);
    })
    .context("setting up ctrl-c handler")?;

    // Lanzar UI QML como subproceso
    if !ui_off {
        log::info!("Lanzando UI QML (qml6)...");
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
