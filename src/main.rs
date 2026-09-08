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
    let backend = runtime.block_on(async {
        let b = Backend::new().await?;
        log::info!("Backend inicializado correctamente");
        Ok::<_, anyhow::Error>(Arc::new(b))
    })?;

    // Iniciar servidor HTTP local (IPC con la UI QML)
    const HTTP_PORT: u16 = 8765;
    {
        let http_state = backend.http_state();
        runtime.spawn(async move {
            if let Err(e) =
                kde_assistant_lib::backend::http_server::serve(http_state, HTTP_PORT).await
            {
                log::error!("Servidor HTTP fallo: {e}");
            }
        });
        log::info!("Servidor HTTP local en http://127.0.0.1:{HTTP_PORT}");
    }

    // Iniciar pipeline de voz (captura de audio + deteccion de wake word)
    // Solo si esta habilitado en config (por defecto desactivado por falsos positivos)
    let cfg = runtime.block_on(async { backend.config.read().await.clone() });
    let voice_events = if cfg.speech.wake_word_enabled {
        runtime.block_on(async {
            match backend.start_voice_pipeline().await {
                Ok(rx) => Some(rx),
                Err(e) => {
                    log::warn!("No se pudo iniciar el pipeline de voz: {e}");
                    log::warn!("(verifica que tienes microfono y permisos de audio)");
                    None
                }
            }
        })
    } else {
        log::info!(
            "Wake word desactivado por configuracion. Usa el InputBar para escribir mensajes."
        );
        None
    };

    // Handler del wake word: deteccion -> grabacion -> procesamiento
    const MAX_RECORDING_SECS: u64 = 8;
    let wake_word_label = cfg.speech.wake_word.clone();
    if let Some(mut rx) = voice_events {
        let vp = backend.voice.clone();
        runtime.spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    kde_assistant_lib::backend::hotword::HotwordEvent::Detected => {
                        log::info!("Wake word detectado: '{wake_word_label}'");
                        vp.start_listening();
                        // Auto-stop tras MAX_RECORDING_SECS y procesar
                        let vp2 = vp.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(MAX_RECORDING_SECS))
                                .await;
                            match vp2.stop_and_process().await {
                                Ok((t, r)) => {
                                    if !t.is_empty() {
                                        log::info!("Wake word: '{}' -> '{}'", t, truncate(&r, 80));
                                    }
                                }
                                Err(e) => log::warn!("Procesamiento de voz fallo: {e}"),
                            }
                            log::info!("Listo para siguiente wake word");
                        });
                    }
                }
            }
        });
        log::info!(
            "Voice pipeline de wake word activo (max {}s de grabacion)",
            MAX_RECORDING_SECS
        );
    }

    // Setup KDE integration (theme detection, notifications)
    let mut kde = kde_assistant_lib::backend::kde_integration::KdeIntegration::new();
    if let Err(e) = kde.setup() {
        log::warn!("KDE setup fallo: {e}");
    }
    log::info!("KDE integration: theme dark={}", kde.theme.is_dark());

    // Start global hotkey listener (rdev, en thread separado)
    if !no_shortcuts {
        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel::<
            kde_assistant_lib::backend::hotkey_listener::HotkeyAction,
        >(32);
        let _handle = kde_assistant_lib::backend::hotkey_listener::start_listener(action_tx);
        // Consumir acciones: PTT start/stop conectan con la grabacion real
        let vp = backend.voice.clone();
        runtime.spawn(async move {
            use kde_assistant_lib::backend::hotkey_listener::HotkeyAction;
            while let Some(action) = action_rx.recv().await {
                match action {
                    HotkeyAction::PushToTalkStart => {
                        log::info!("PTT: inicio de grabacion");
                        vp.start_listening();
                    }
                    HotkeyAction::PushToTalkEnd => {
                        log::info!("PTT: fin de grabacion, procesando...");
                        match vp.stop_and_process().await {
                            Ok((t, r)) => {
                                if !t.is_empty() {
                                    log::info!("PTT: '{}' -> '{}'", t, truncate(&r, 80));
                                } else {
                                    log::info!("PTT: grabacion vacia");
                                }
                            }
                            Err(e) => log::warn!("PTT: procesamiento fallo: {e}"),
                        }
                    }
                    _ => {}
                }
            }
        });
        log::info!("Global hotkey listener: Super+Shift+A (toggle), Super+Shift+V (PTT mantener), Ctrl+Shift+K (new session)");
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
        // -apptype widget es necesario para QApplication (SystemTrayIcon lo requiere)
        // QML_XHR_ALLOW_FILE_READ=1 permite al polling de hotkeys leer el
        // archivo de estado via file:// (deshabilitado por defecto en QML)
        let qml_status = Command::new("qml6")
            .args(["-apptype", "widget", "-I", ".", "qml/Main.qml"])
            .env("QML_XHR_ALLOW_FILE_READ", "1")
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push_str("...");
        t
    }
}
