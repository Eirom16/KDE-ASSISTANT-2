// KDE Assistant v2 - Entry Point
//
// Inicializa logging, runtime Tokio, backend, integración KDE,
// global hotkeys (rdev), y luego lanza la UI QML como subproceso de `qml6`.
//
// Flags:
//   --ui-off       No lanzar UI (solo backend + KDE integration)
//   --no-shortcuts No registrar global shortcuts
//   --enable-voice Activar voz existente con dependencias locales instaladas
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

    // Señal de apagado limpio
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_signal.store(true, Ordering::SeqCst);
    })
    .context("setting up ctrl-c handler")?;

    // Mantener el listener reservado elimina la carrera y evita alterar bootstrap
    // de una instancia activa. No se publica HTTP hasta completar lo crítico.
    const HTTP_PORT: u16 = 8765;
    let listener = std::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], HTTP_PORT)))
        .context("Puerto 8765 ocupado: otra instancia o servicio está activo; no se modificó ni terminó ningún proceso")?;
    listener.set_nonblocking(true)?;

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

    // F6: disparar recordatorios pendientes de sesiones anteriores.
    {
        let sessions = backend.sessions.clone();
        runtime.spawn(async move {
            kde_assistant_lib::backend::tool_executor::fire_pending_reminders(sessions).await;
        });
    }

    let http_state = backend.http_state()?;
    let local_token = http_state.local_token.clone();
    // El módulo privado también es crítico para la interfaz autenticada.
    let cfg = runtime.block_on(async { backend.config.read().await.clone() });
    let auth_inc = if ui_off {
        None
    } else {
        Some(kde_assistant_lib::backend::auth::write_qml_auth_module(
            &local_token,
            false,
        )?)
    };
    let server = runtime.spawn({
        let http_state = http_state.clone();
        async move {
            let listener = tokio::net::TcpListener::from_std(listener)?;
            axum::serve(
                listener,
                kde_assistant_lib::backend::http_server::router(http_state),
            )
            .await?;
            Ok::<_, anyhow::Error>(())
        }
    });
    log::info!("Núcleo de texto listo en http://127.0.0.1:{HTTP_PORT}; voz diferida, descargas desactivadas");

    // Iniciar pipeline de voz (captura de audio + deteccion de wake word)
    // Solo si esta habilitado en config (por defecto desactivado por falsos positivos)
    // Rust es la única autoridad de conversación: conserva una sola sesión,
    // permisos, streaming y TTS. El sidecar experimental tenía otro LLM y
    // rompía la conversación bidireccional al no compartir contexto.
    let mut sidecar_child: Option<Child> = None;
    let voice_events = if args.iter().any(|a| a == "--enable-voice") && cfg.speech.wake_word_enabled
    {
        runtime.block_on(async {
            match backend.start_voice_pipeline().await {
                Ok(rx) => {
                    http_state.voice_enabled.store(true, Ordering::Relaxed);
                    if cfg.speech.chimes_enabled {
                        if let Err(e) = backend.chimes.initialize().await {
                            log::warn!("Chimes no disponibles: {e}");
                        }
                    }
                    Ok::<_, anyhow::Error>(Some(rx))
                }
                Err(e) => {
                    log::warn!("No se pudo iniciar el pipeline de voz: {e}");
                    log::warn!("(verifica que tienes microfono y permisos de audio)");
                    Ok(None)
                }
            }
        })?
    } else {
        log::info!(
            "Voz diferida; requiere --enable-voice y dependencias locales. Usa el InputBar para escribir mensajes."
        );
        None
    };

    // Handler del wake word: saludo -> hilo conversacional (F3-2).
    const MAX_RECORDING_SECS: u64 = 8;
    const MAX_EXTRA_TURNS: u32 = 2;
    let wake_word_label = cfg.speech.wake_word.clone();
    if let Some(mut rx) = voice_events {
        let vp = backend.voice.clone();
        runtime.spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    kde_assistant_lib::backend::hotword::HotwordEvent::Detected => {
                        // El dictado al chat tiene el micro: ignorar el wake
                        // word entero (ni saludo TTS ni escucha).
                        if vp.is_dictating() {
                            log::info!("[VOICE] wake word ignorado: dictado al chat en curso");
                            continue;
                        }
                        // Si el pipeline esta ocupado (escuchando, pensando o
                        // hablando), NO re-abrir: un "hey jarvis" repetido
                        // pisaba el turno en curso y perdia el comando del
                        // usuario. El barge-in durante TTS es aparte
                        // (por amplitud, en push_audio).
                        if !vp.wake_gate_open() {
                            log::info!("[VOICE] wake word ignorado: pipeline ocupado");
                            continue;
                        }
                        log::info!("[VOICE] Wake detected: '{wake_word_label}'");
                        vp.emit_state(
                            kde_assistant_lib::backend::voice_state::VoiceState::WakeDetected,
                        );
                        let vp2 = vp.clone();
                        let greeting = {
                            let cfg = vp2.config.read().await;
                            cfg.speech.wake_greeting.trim().to_string()
                        };
                        tokio::spawn(async move {
                            // Dar una señal audible de que Jarvis está atento
                            // evita que el usuario empiece a hablar antes de
                            // que el micrófono haya cambiado de estado. La
                            // grabación comienza solo después del saludo para
                            // que Whisper no lo confunda con el comando.
                            if !greeting.is_empty() {
                                vp2.emit_state(
                                    kde_assistant_lib::backend::voice_state::VoiceState::Attentive,
                                );
                                if let Err(e) = vp2.speech.speak(&greeting).await {
                                    log::warn!("Saludo de wake word fallo: {e}");
                                }
                            }
                            vp2.start_listening();
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
        let voice_enabled = http_state.voice_enabled.clone();
        // Consumir acciones: PTT start/stop conectan con la grabacion real
        let vp = backend.voice.clone();
        runtime.spawn(async move {
            use kde_assistant_lib::backend::hotkey_listener::HotkeyAction;
            while let Some(action) = action_rx.recv().await {
                match action {
                    HotkeyAction::PushToTalkStart => {
                        if !voice_enabled.load(Ordering::Relaxed) {
                            log::warn!("PTT no disponible: voz diferida; utiliza el chat escrito");
                            continue;
                        }
                        log::info!("PTT: inicio de grabacion");
                        vp.start_listening();
                    }
                    HotkeyAction::PushToTalkEnd => {
                        if !voice_enabled.load(Ordering::Relaxed) {
                            continue;
                        }
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

    // Lanzar UI QML como subproceso
    if !ui_off {
        log::info!("Lanzando UI QML (qml6)...");
        // -apptype widget es necesario para QApplication (SystemTrayIcon lo requiere)
        // QML_XHR_ALLOW_FILE_READ=1 permite al polling de hotkeys leer el
        // archivo de estado via file:// (deshabilitado por defecto en QML)
        //
        // Plataforma: Wayland nativo si está disponible y QT_QPA_PLATFORM no está forzado a xcb.
        // FIX-wayland: en nativo la ventana a veces no mapea; XWayland es fallback seguro.
        // Respetar QT_QPA_PLATFORM si ya está seteado (wayland/xcb/offscreen).
        let platform = std::env::var_os("QT_QPA_PLATFORM")
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                if std::env::var_os("WAYLAND_DISPLAY").is_some()
                    || std::env::var_os("XDG_SESSION_TYPE")
                        .map(|v| v == "wayland")
                        .unwrap_or(false)
                {
                    "wayland".to_string()
                } else {
                    "xcb".to_string()
                }
            });

        // Si el overlay del personaje va a correr (Wayland + enabled), el
        // AgentWindowStandalone de Main.qml se desactiva: dos personajes a la
        // vez seria un duplicado (el flag se hornea en el módulo QML generado,
        // porque Qt.platform.environment es null en muchas sesiones qml6).
        let character_enabled =
            runtime.block_on(async { backend.config.read().await.character.enabled });
        let spawn_overlay = character_enabled && platform == "wayland";

        // Módulo qml.auth: token + cacheDir + flag de overlay, sin depender de
        // Qt.platform.environment (nulo en varias sesiones qml6).
        // KDE_ASSISTANT_TOKEN / KDE_ASSISTANT_AGENT_OVERLAY quedan como respaldo.
        let auth_inc = auth_inc
            .as_ref()
            .context("Falta módulo QML privado")?
            .to_string_lossy()
            .to_string();
        if spawn_overlay {
            kde_assistant_lib::backend::auth::write_qml_auth_module(&local_token, true)?;
        }
        let mut qml_cmd = Command::new("qml6");
        // El módulo generado va el ÚLTIMO: ante un módulo duplicado el motor
        // QML prefiere el último -I (verificado empíricamente; qmlimportscanner
        // dice lo contrario). Así eclipsa al fallback vacío de qml/auth.
        if !auth_inc.is_empty() {
            qml_cmd.args(["-apptype", "widget", "-I", ".", "-I", &auth_inc]);
        } else {
            qml_cmd.args(["-apptype", "widget", "-I", "."]);
        }
        match platform.as_str() {
            "wayland" => log::info!("UI: usando Wayland nativo (QT_QPA_PLATFORM=wayland)"),
            "xcb" => log::info!("UI: forzando XWayland (QT_QPA_PLATFORM=xcb) por compatibilidad"),
            p => log::info!("UI: QT_QPA_PLATFORM ya definido = {p}"),
        }
        qml_cmd.env("QT_QPA_PLATFORM", &platform);

        let mut child = kde_assistant_lib::owned_process::OwnedChild::new(
            qml_cmd
                .arg("qml/Main.qml")
                // Sin caché de QML: evita arrancar con bytecode rancio tras actualizar.
                .env("QML_DISABLE_DISK_CACHE", "1")
                .env("QML_XHR_ALLOW_FILE_READ", "1")
                .env("KDE_ASSISTANT_TOKEN", &local_token)
                .env(
                    "KDE_ASSISTANT_AGENT_OVERLAY",
                    if spawn_overlay { "1" } else { "0" },
                )
                .spawn()
                .context(
                    "lanzando qml6 (asegurate de tener Qt6 instalado: pacman -S qt6-declarative)",
                )?,
        );
        log::info!("UI QML lanzada (pid {})", child.id());

        // === Proceso separado: Desktop Agent overlay (Fase 4) ===
        // Proceso qml6 independiente con layer-shell (Wayland). Independiente
        // de la ventana principal: si minimizas el chat, el personaje sigue.
        let overlay_child: Option<kde_assistant_lib::owned_process::OwnedChild> = if spawn_overlay {
            let mut ocmd = Command::new("qml6");
            // Las flags -I van ANTES del archivo QML: qml6 deja de parsear
            // opciones al encontrar el primer argumento posicional y se
            // tragaría "-I"/auth_inc como archivos (errores QQmlEngine).
            ocmd.arg("-I").arg(".");
            if !auth_inc.is_empty() {
                ocmd.arg("-I").arg(&auth_inc);
            }
            ocmd.arg("qml/agent/AgentOverlay.qml")
                .env("QT_QPA_PLATFORM", "wayland")
                .env("QML_DISABLE_DISK_CACHE", "1")
                .env("QML_XHR_ALLOW_FILE_READ", "1")
                .env("KDE_ASSISTANT_TOKEN", &local_token);
            match ocmd.spawn() {
                Ok(c) => {
                    log::info!("Desktop Agent overlay lanzado (pid {})", c.id());
                    Some(kde_assistant_lib::owned_process::OwnedChild::new(c))
                }
                Err(e) => {
                    log::warn!("No se pudo lanzar el overlay del personaje: {e}");
                    None
                }
            }
        } else {
            if character_enabled {
                log::info!(
                    "Personaje: overlay omitido (solo Wayland); lo cubre la ventana standalone"
                );
            }
            None
        };

        // Supervisar al hijo principal (Main.qml). Cuando termine, si el overlay
        // del personaje sigue vivo tambien lo matamos.
        let mut overlay_child = overlay_child;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    anyhow::ensure!(
                        status.success(),
                        "La interfaz QML terminó con error: {status}"
                    );
                    log::info!("UI QML termino con codigo {:?}", status.code());
                    break;
                }
                Ok(None) => {
                    anyhow::ensure!(
                        !server.is_finished(),
                        "Servidor HTTP terminó inesperadamente"
                    );
                    if shutdown.load(Ordering::SeqCst) {
                        log::info!("Apagando UI QML (pid {})...", child.id());
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => return Err(e).context("Esperando a la UI QML"),
            }
        }
        if let Some(mut oc) = overlay_child.take() {
            log::info!("Apagando Desktop Agent overlay (pid {})...", oc.id());
            let _ = oc.kill();
            let _ = oc.wait();
        }
        if let Some(mut sc) = sidecar_child.take() {
            log::info!("Apagando sidecar de voz (pid {})...", sc.id());
            let _ = sc.kill();
            let _ = sc.wait();
        }
    } else {
        log::info!("UI desactivada por flag --ui-off");
        log::info!("Presiona Ctrl+C para salir.");
        while !shutdown.load(Ordering::SeqCst) {
            anyhow::ensure!(
                !server.is_finished(),
                "Servidor HTTP terminó inesperadamente"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    server.abort();
    let _ = runtime.block_on(server);
    runtime.block_on(backend.stop_voice_pipeline());
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
