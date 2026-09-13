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
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("KDE Assistant v2 starting...");

    let args: Vec<String> = std::env::args().collect();
    let ui_off = args.iter().any(|a| a == "--ui-off" || a == "--no-ui");
    let no_shortcuts = args.iter().any(|a| a == "--no-shortcuts");

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
    // Backend contiene AudioCapture (cpal), que no es Send/Sync en todas las
    // plataformas. El Arc no cruza threads: el servidor HTTP solo recibe el
    // subconjunto Send+Sync via http_state().
    #[allow(clippy::arc_with_non_send_sync)]
    let backend = runtime.block_on(async {
        let b = Backend::new().await?;
        log::info!("Backend inicializado correctamente");
        Ok::<_, anyhow::Error>(Arc::new(b))
    })?;

    // Pre-warm de whisper en background (el modelo de 147MB tarda
    // varios segundos en cargar; mejor al arrancar que en la primera voz)
    {
        let speech = backend.speech.clone();
        runtime.spawn(async move {
            if speech.stt_model_exists() {
                let whisper = speech.whisper.clone();
                match tokio::task::spawn_blocking(move || whisper.ensure_loaded()).await {
                    Ok(Ok(())) => log::info!("Whisper pre-cargado en background"),
                    Ok(Err(e)) => log::warn!("Pre-warm de whisper fallo: {e}"),
                    Err(e) => log::warn!("Task de pre-warm fallo: {e}"),
                }
            }
        });
    }

    // F6: disparar recordatorios pendientes de sesiones anteriores.
    {
        let sessions = backend.sessions.clone();
        runtime.spawn(async move {
            kde_assistant_lib::backend::tool_executor::fire_pending_reminders(sessions).await;
        });
    }

    // Iniciar servidor HTTP local (IPC con la UI QML)
    const HTTP_PORT: u16 = 8765;
    // Falla rápido si el puerto está ocupado (otra instancia vieja con OTRO
    // token: la UI lanzaría contra ese backend y todo daría 401 "No autorizado").
    if std::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], HTTP_PORT))).is_err()
    {
        eprintln!(
            "Puerto {HTTP_PORT} en uso. ¿Hay otra instancia de kde-assistant corriendo?\n\
             Ciérrala (o `pkill -f kde-assistant; pkill -f \"qml6.*Main.qml\"`) y vuelve a intentar."
        );
        std::process::exit(1);
    }
    // Token local F0-3 para que el QML se autentique en /api/*.
    // OJO: llamar http_state() una sola vez (contiene el mapa de tasks F0-7).
    let http_state = backend.http_state();
    let local_token = http_state.local_token.clone();
    {
        let http_state = http_state.clone();
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

    // Handler del wake word: saludo -> hilo conversacional (F3-2).
    const MAX_RECORDING_SECS: u64 = 8;
    const MAX_EXTRA_TURNS: u32 = 2;
    let wake_word_label = cfg.speech.wake_word.clone();
    let wake_greeting = cfg.speech.wake_greeting.clone();
    if let Some(mut rx) = voice_events {
        let vp = backend.voice.clone();
        let speech = backend.speech.clone();
        runtime.spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    kde_assistant_lib::backend::hotword::HotwordEvent::Detected => {
                        log::info!("Wake word detectado: '{wake_word_label}'");
                        // Saludo hablado ANTES de grabar (si no, el micro
                        // captaria nuestra propia voz y entraria en bucle)
                        if let Err(e) = speech.speak(&wake_greeting).await {
                            log::warn!("Saludo TTS fallo: {e}");
                        }
                        vp.start_listening();
                        let vp2 = vp.clone();
                        tokio::spawn(async move {
                            let turns = vp2
                                .converse_voice_driven(MAX_RECORDING_SECS, MAX_EXTRA_TURNS)
                                .await;
                            for (t, r) in &turns {
                                log::info!("Wake word: '{}' -> '{}'", t, truncate(r, 80));
                            }
                            if turns.is_empty() {
                                log::info!("Turno de voz vacío, listo para siguiente wake word");
                            } else {
                                log::info!(
                                    "Hilo de voz terminado ({} turno(s)), listo para siguiente wake word",
                                    turns.len()
                                );
                            }
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
        let _handle = kde_assistant_lib::backend::hotkey_listener::start_listener(
            action_tx,
            backend.config.clone(),
        );
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
                                    // F3-2: encadenar manos-libres si auto_listen.
                                    for (ct, cr) in vp.continue_conversation(8, 2).await {
                                        log::info!("PTT+: '{}' -> '{}'", ct, truncate(&cr, 80));
                                    }
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
        log::info!("Global hotkey listener: Super+Shift+A (toggle), Super+Shift+V (PTT mantener), Ctrl+Shift+K (new session), Super+Shift+M (menu)");
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

    // Autolimpieza: QMLs huespedes de ejecuciones previas. Si el backend
    // murio (p. ej. `pkill -f kde-assistant`) su qml6 queda vivo con icono
    // y menu propios en la bandeja, y cada arranque suma uno duplicado.
    // Solo cuando vamos a lanzar nuestra UI (con --ui-off no tocar nada).
    if !ui_off {
        cull_stale_qml();
    }

    // Lanzar UI QML como subproceso
    if !ui_off {
        log::info!("Lanzando UI QML (qml6)...");
        // -apptype widget es necesario para QApplication (SystemTrayIcon lo requiere)
        // QML_XHR_ALLOW_FILE_READ=1 permite al polling de hotkeys leer el
        // archivo de estado via file:// (deshabilitado por defecto en QML)
        // Módulo qml.auth: token + cacheDir sin depender de
        // Qt.platform.environment (nulo en algunas sesiones qml6).
        // KDE_ASSISTANT_TOKEN queda como respaldo.
        let auth_inc = kde_assistant_lib::backend::auth::write_qml_auth_module(&local_token)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|e| {
                log::warn!("No se pudo generar el módulo QML de auth: {e}");
                String::new()
            });
        let mut qml_cmd = Command::new("qml6");
        // El módulo generado va el ÚLTIMO: ante un módulo duplicado el motor
        // QML prefiere el último -I (verificado empíricamente; qmlimportscanner
        // dice lo contrario). Así eclipsa al fallback vacío de qml/auth.
        if !auth_inc.is_empty() {
            qml_cmd.args(["-apptype", "widget", "-I", ".", "-I", &auth_inc]);
        } else {
            qml_cmd.args(["-apptype", "widget", "-I", "."]);
        }
        // Plataforma por defecto XWayland (FIX-wayland: en nativo la ventana
        // a veces no mapea). El backend de render NO se fuerza: el software
        // cuelga el hilo de render en algunas GPUs (negro + "no responde").
        if std::env::var_os("QT_QPA_PLATFORM").is_none() {
            qml_cmd.env("QT_QPA_PLATFORM", "xcb");
            log::info!("UI: forzando XWayland (QT_QPA_PLATFORM=xcb) por compatibilidad");
        }
        let mut child = qml_cmd
            .arg("qml/Main.qml")
            // Sin caché de QML: evita arrancar con bytecode rancio tras actualizar.
            .env("QML_DISABLE_DISK_CACHE", "1")
            .env("QML_XHR_ALLOW_FILE_READ", "1")
            .env("KDE_ASSISTANT_TOKEN", &local_token)
            .spawn()
            .context(
                "lanzando qml6 (asegurate de tener Qt6 instalado: pacman -S qt6-declarative)",
            )?;
        log::info!("UI QML lanzada (pid {})", child.id());

        // === Proceso separado: Desktop Agent overlay (Fase 4) ===
        // Proceso qml6 independiente con layer-shell (Wayland). Independiente
        // de la ventana principal: si minimizas el chat, el personaje sigue.
        let overlay_child: Option<Child> = {
            let cfg_char_enabled =
                { runtime.block_on(async { backend.config.read().await.character.enabled }) };
            if cfg_char_enabled {
                let p = std::env::var_os("QT_QPA_PLATFORM")
                    .map(|v| v.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                            "wayland".to_string()
                        } else {
                            "xcb".to_string()
                        }
                    });
                if p == "wayland" {
                    let mut ocmd = Command::new("qml6");
                    ocmd.arg("-I")
                        .arg(".")
                        .arg("qml/agent/AgentOverlay.qml")
                        .env("QT_QPA_PLATFORM", "wayland")
                        .env("QML_DISABLE_DISK_CACHE", "1")
                        .env("QML_XHR_ALLOW_FILE_READ", "1")
                        .env("KDE_ASSISTANT_TOKEN", &local_token);
                    if !auth_inc.is_empty() {
                        ocmd.arg("-I").arg(&auth_inc);
                    }
                    match ocmd.spawn() {
                        Ok(c) => {
                            log::info!("Desktop Agent overlay lanzado (pid {})", c.id());
                            Some(c)
                        }
                        Err(e) => {
                            log::warn!("No se pudo lanzar el overlay del personaje: {e}");
                            None
                        }
                    }
                } else {
                    log::info!(
                        "Personaje en modo {}: overlay layer-shell omitido (solo Wayland)",
                        p
                    );
                    None
                }
            } else {
                None
            }
        };

        // Supervisar al hijo principal (Main.qml). Cuando termine, si el overlay
        // del personaje sigue vivo tambien lo matamos.
        let mut overlay_child = overlay_child;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    log::info!("UI QML termino con codigo {:?}", status.code());
                    break;
                }
                Ok(None) => {
                    if shutdown.load(Ordering::SeqCst) {
                        log::info!("Apagando UI QML (pid {})...", child.id());
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => {
                    log::warn!("Esperando a la UI QML: {e}");
                    break;
                }
            }
        }
        if let Some(mut oc) = overlay_child.take() {
            log::info!("Apagando Desktop Agent overlay (pid {})...", oc.id());
            let _ = oc.kill();
            let _ = oc.wait();
        }
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

/// Patron (ERE para pgrep/pkill -f) que identifica QMLs huespedes de
/// esta app: procesos `qml6 ... qml/Main.qml` de ejecuciones previas.
fn stale_qml_pattern() -> &'static str {
    "qml6.*Main\\.qml"
}

/// Mata QMLs huespedes de ejecuciones previas (best-effort). Sin esto,
/// cada arranque tras un backend muerto suma un icono y un menu
/// duplicados en la bandeja del sistema.
fn cull_stale_qml() {
    let pattern = stale_qml_pattern();
    // Listar primero para el log (pgrep se excluye solo).
    let listed = Command::new("pgrep")
        .args(["-af", pattern])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let victims: Vec<&str> = listed
        .lines()
        .filter(|l| !l.contains("pgrep") && !l.contains("pkill"))
        .collect();
    if victims.is_empty() {
        return;
    }
    log::warn!(
        "QMLs huespedes de ejecuciones previas ({}): los cierro para no duplicar la bandeja",
        victims.len()
    );
    for v in &victims {
        log::warn!("  huesped: {v}");
    }
    let _ = Command::new("pkill").args(["-f", pattern]).status();
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
