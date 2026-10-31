// KDE Assistant v2 - Entry Point
//
// Inicializa logging, runtime Tokio, backend, integración KDE,
// atajos mediante portal opt-in, y luego lanza la UI QML como subproceso de `qml6`.
//
// Flags:
//   --ui-off       No lanzar UI (solo backend + KDE integration)
//   --no-shortcuts No registrar global shortcuts
//   --enable-voice Legado: no inicia escucha; usar dictado explícito en la UI
//   --enable-shortcuts Solicitar el registro al portal (consentimiento del sistema)
//   --http-port <n> Puerto de perfil; 0 elige uno libre
//   --toggle       Alternar una instancia existente del mismo perfil

use anyhow::{Context, Result};
use kde_assistant_lib::backend::Backend;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<()> {
    let worker_args: Vec<String> = std::env::args().collect();
    if worker_args
        .get(1)
        .is_some_and(|a| a == "--voice-wake-worker" || a == "--voice-wake-stream")
    {
        anyhow::ensure!(worker_args.len() == 5, "Argumentos wake inválidos");
        let worker = if worker_args[1] == "--voice-wake-stream" {
            kde_assistant_lib::backend::wakeword::stream_worker
        } else {
            kde_assistant_lib::backend::wakeword::worker
        };
        return worker(
            std::path::Path::new(&worker_args[2]),
            std::path::Path::new(&worker_args[3]),
            worker_args[4].parse()?,
        );
    }
    if worker_args.get(1).is_some_and(|a| {
        matches!(
            a.as_str(),
            "--voice-piper-check"
                | "--voice-piper-worker"
                | "--voice-piper-stream"
                | "--voice-piper-warm-experiment"
                | "--voice-piper-managed"
        )
    }) {
        let check = worker_args[1] == "--voice-piper-check";
        anyhow::ensure!(
            worker_args.len() == if check { 4 } else { 6 },
            "Argumentos Piper inválidos"
        );
        let runtime = std::path::Path::new(&worker_args[2]);
        let model = std::path::Path::new(&worker_args[3]);
        if check {
            let error = kde_assistant_lib::backend::voice_tts::validate(runtime, model)
                .err()
                .map(|e| e.to_string());
            use std::io::Write;
            std::io::stdout().write_all(&serde_json::to_vec(&error)?)?;
            return Ok(());
        }
        let worker = if worker_args[1] == "--voice-piper-managed" {
            kde_assistant_lib::backend::voice_tts::managed_worker
        } else if worker_args[1] == "--voice-piper-warm-experiment" {
            kde_assistant_lib::backend::voice_tts::warm_experiment
        } else if worker_args[1] == "--voice-piper-stream" {
            kde_assistant_lib::backend::voice_tts::stream_worker
        } else {
            kde_assistant_lib::backend::voice_tts::worker
        };
        return worker(
            runtime,
            model,
            worker_args[4].parse()?,
            worker_args[5].parse()?,
        );
    }
    if worker_args
        .get(1)
        .is_some_and(|a| a == "--voice-vosk-check" || a == "--voice-vosk-worker")
    {
        anyhow::ensure!(worker_args.len() == 4, "Argumentos Vosk inválidos");
        let library = std::path::Path::new(&worker_args[2]);
        let model = std::path::Path::new(&worker_args[3]);
        if worker_args[1] == "--voice-vosk-worker" {
            return kde_assistant_lib::backend::vosk::worker(library, model);
        }
        let error = kde_assistant_lib::backend::vosk::validate_paths(library, model)
            .err()
            .map(|e| e.to_string());
        use std::io::Write;
        std::io::stdout().write_all(&serde_json::to_vec(&error)?)?;
        return Ok(());
    }
    if worker_args
        .get(1)
        .is_some_and(|a| a == "--voice-model-check")
    {
        anyhow::ensure!(worker_args.len() == 3, "Argumentos del validador inválidos");
        let error = kde_assistant_lib::backend::voice_coordinator::validate_model(
            std::path::Path::new(&worker_args[2]),
        )
        .err()
        .map(|e| e.to_string());
        use std::io::Write;
        std::io::stdout().write_all(&serde_json::to_vec(&error)?)?;
        return Ok(());
    }

    if worker_args
        .get(1)
        .is_some_and(|a| a == "--voice-stt-worker")
    {
        anyhow::ensure!(worker_args.len() == 4, "Argumentos del worker inválidos");
        return kde_assistant_lib::backend::voice_coordinator::stt_worker(
            worker_args[2].clone().into(),
            worker_args[3].clone(),
        );
    }

    anyhow::ensure!(
        !worker_args
            .iter()
            .skip(1)
            .any(|a| a.starts_with("--voice-")),
        "Modo interno de voz inválido"
    );

    // A misspelled diagnostic must fail before touching a desktop profile.
    validate_launch_args(&worker_args[1..])?;

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,zbus=warn,tracing=warn"),
    )
    .init();

    log::info!("KDE Assistant v2 starting...");

    let args: Vec<String> = std::env::args().collect();
    let ui_off = args.iter().any(|a| a == "--ui-off" || a == "--no-ui");
    let no_shortcuts = args.iter().any(|a| a == "--no-shortcuts");

    anyhow::ensure!(
        !args.iter().any(|a| a == "--send-msg"),
        "--send-msg retirado: DBus no acepta contenido del modelo ni comandos de herramientas"
    );
    let http_port: u16 = match args.iter().position(|a| a == "--http-port") {
        Some(i) => args
            .get(i + 1)
            .context("--http-port requiere un puerto (0 = automático)")?
            .parse()?,
        None => 8765,
    };
    let runtime = tokio::runtime::Runtime::new().context("creando Tokio runtime")?;
    let profile_dir = kde_assistant_lib::models::Config::config_path()?
        .parent()
        .context("sin config_dir")?
        .to_owned();
    let state = kde_assistant_lib::desktop::InstanceLock::acquire(&profile_dir)?;
    let bus = match runtime.block_on(kde_assistant_lib::desktop::session_bus()) {
        Ok(bus) => Some(bus),
        Err(_) => {
            log::warn!("DBus de sesión no disponible; integración y activación deshabilitadas");
            None
        }
    };
    let instance = match state {
        kde_assistant_lib::desktop::InstanceState::Existing(name) => {
            let bus = bus
                .as_ref()
                .context("Instancia existente; DBus no disponible para activarla")?;
            runtime.block_on(kde_assistant_lib::desktop::activate(bus, &name, args.iter().any(|a| a == "--toggle")))
                .context("Instancia existente: UI no disponible o todavía iniciando; no se inició un duplicado")?;
            log::info!("Instancia existente activada");
            return Ok(());
        }
        kde_assistant_lib::desktop::InstanceState::Primary(lock) => lock,
    };
    anyhow::ensure!(
        !args.iter().any(|a| a == "--toggle"),
        "No existe una instancia para --toggle"
    );

    // Señal de apagado limpio
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_signal.store(true, Ordering::SeqCst);
    })
    .context("setting up ctrl-c handler")?;

    // Mantener el listener reservado elimina la carrera y evita alterar bootstrap
    // de una instancia activa. No se publica HTTP hasta completar lo crítico.

    let listener = runtime.block_on(async {
        let socket = tokio::net::TcpSocket::new_v4()?;
        // Permit restart after our accepted connections enter TIME_WAIT. This
        // never enables SO_REUSEPORT or sharing an active listener.
        socket.set_reuseaddr(true)?;
        socket.bind(std::net::SocketAddr::from(([127, 0, 0, 1], http_port)))?;
        socket.listen(128)?.into_std()
    })
        .with_context(|| format!("Puerto {http_port} ocupado: otra instancia o servicio está activo; no se modificó ni terminó ningún proceso"))?;
    listener.set_nonblocking(true)?;

    let actual_port = listener.local_addr()?.port();
    // Init backend en runtime Tokio
    // Backend contiene AudioCapture (cpal), que no es Send/Sync en todas las
    // plataformas. El Arc no cruza threads: el servidor HTTP solo recibe el
    // subconjunto Send+Sync via http_state().
    #[allow(clippy::arc_with_non_send_sync)]
    let backend = runtime.block_on(async {
        let b = Backend::new().await?;
        log::info!("Backend inicializado correctamente");
        Ok::<_, anyhow::Error>(Arc::new(b))
    })?;

    // F2: los recordatorios legacy no tienen consentimiento verificable.
    // Se conservan en SQLite, pero no se disparan al arrancar.

    let mut http_state = backend.http_state()?;
    http_state.http_port = actual_port;
    let local_token = http_state.local_token.clone();
    // El módulo privado también es crítico para la interfaz autenticada.
    let cfg = runtime.block_on(async { backend.config.read().await.clone() });
    let auth_inc = if ui_off {
        None
    } else {
        Some(
            kde_assistant_lib::backend::auth::write_qml_auth_for_desktop(
                &local_token,
                false,
                &instance.bus_name,
                actual_port,
            )?,
        )
    };
    let (server_stop, server_stopped) = tokio::sync::oneshot::channel::<()>();
    let mut server = runtime.spawn({
        let http_state = http_state.clone();
        async move {
            let listener = tokio::net::TcpListener::from_std(listener)?;
            axum::serve(
                listener,
                kde_assistant_lib::backend::http_server::router(http_state),
            )
            .with_graceful_shutdown(async {
                let _ = server_stopped.await;
            })
            .await?;
            Ok::<_, anyhow::Error>(())
        }
    });
    log::info!("Núcleo de texto listo en http://127.0.0.1:{actual_port}; voz diferida, descargas desactivadas");

    // F4: no microphone/wake/prewarm at startup, including legacy --enable-voice.
    if args.iter().any(|a| a == "--enable-voice") {
        log::warn!("--enable-voice no inicia escucha; usa el botón de dictado. Hey Jarvis permanece OFF; se activa explícitamente desde el botón del robot.");
    }

    if let Some(bus) = &bus {
        runtime.block_on(kde_assistant_lib::desktop::register(
            bus,
            &instance.bus_name,
            backend.desktop.clone(),
        ))?;
        match runtime.block_on(
            kde_assistant_lib::backend::kde_integration::KdeIntegration::detect_theme(bus),
        ) {
            Ok(theme) => log::info!("Tema KDE: {theme:?}"),
            Err(_) => log::warn!("Tema del portal no disponible"),
        }
    }
    // Portal registration is explicit. --no-shortcuts always wins, including tests.
    let shortcut_task = if !no_shortcuts
        && args.iter().any(|a| a == "--enable-shortcuts")
        && !ui_off
    {
        if bus.is_some() {
            let desktop = backend.desktop.clone();
            let shortcut = cfg.shortcuts.toggle.clone();
            log::info!("GlobalShortcuts: se solicitará consentimiento del sistema para {shortcut}");
            Some(runtime.spawn(async move {
                // A dedicated connection owns requests/sessions. Aborting this
                // task closes any pending system dialog without touching others.
                let bus = match kde_assistant_lib::desktop::session_bus().await {
                    Ok(bus) => bus,
                    Err(_) => {
                        desktop.set_shortcut_status("NOT_SUPPORTED");
                        return None;
                    }
                };
                match kde_assistant_lib::backend::global_shortcuts::register(
                    &bus,
                    desktop.clone(),
                    &shortcut,
                )
                .await
                {
                    Ok(session) => Some(session),
                    Err(error) => {
                        desktop.set_shortcut_status("unavailable_or_denied");
                        log::warn!("GlobalShortcuts no registrado: {error}");
                        None
                    }
                }
            }))
        } else {
            backend.desktop.set_shortcut_status("NOT_SUPPORTED");
            None
        }
    } else {
        log::info!("Atajos globales deshabilitados; registro opt-in mediante --enable-shortcuts");
        None
    };

    let ui_result: Result<()> = (|| {
        // Lanzar UI QML como subproceso
        if !ui_off {
            let data_dir = kde_assistant_lib::app_paths::data_dir()?;
            kde_assistant_lib::app_paths::validate_resources(&data_dir)?;
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
                kde_assistant_lib::backend::auth::write_qml_auth_for_desktop(
                    &local_token,
                    true,
                    &instance.bus_name,
                    actual_port,
                )?;
            }
            let mut qml_cmd = Command::new("qml6");
            // El módulo generado va el ÚLTIMO: ante un módulo duplicado el motor
            // QML prefiere el último -I (verificado empíricamente; qmlimportscanner
            // dice lo contrario). Así eclipsa al fallback vacío de qml/auth.
            if !auth_inc.is_empty() {
                qml_cmd
                    .args(["-apptype", "widget", "-I"])
                    .arg(&data_dir)
                    .args(["-I", &auth_inc]);
            } else {
                qml_cmd.args(["-apptype", "widget", "-I"]).arg(&data_dir);
            }
            match platform.as_str() {
                "wayland" => log::info!("UI: usando Wayland nativo (QT_QPA_PLATFORM=wayland)"),
                "xcb" => {
                    log::info!("UI: forzando XWayland (QT_QPA_PLATFORM=xcb) por compatibilidad")
                }
                p => log::info!("UI: QT_QPA_PLATFORM ya definido = {p}"),
            }
            qml_cmd.env("QT_QPA_PLATFORM", &platform);

            kde_assistant_lib::owned_process::terminate_with_parent(&mut qml_cmd);
            let mut child = kde_assistant_lib::owned_process::OwnedChild::new(
            qml_cmd
                .arg(data_dir.join("qml/Main.qml"))
                // Sin caché de QML: evita arrancar con bytecode rancio tras actualizar.
                .env("QML_DISABLE_DISK_CACHE", "1")
                .env("QML_XHR_ALLOW_FILE_READ", "1")
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
            let overlay_child: Option<kde_assistant_lib::owned_process::OwnedChild> =
                if spawn_overlay {
                    let mut ocmd = Command::new("qml6");
                    // Las flags -I van ANTES del archivo QML: qml6 deja de parsear
                    // opciones al encontrar el primer argumento posicional y se
                    // tragaría "-I"/auth_inc como archivos (errores QQmlEngine).
                    ocmd.arg("-I").arg(&data_dir);
                    if !auth_inc.is_empty() {
                        ocmd.arg("-I").arg(&auth_inc);
                    }
                    ocmd.arg(data_dir.join("qml/agent/AgentOverlay.qml"))
                        .env("QT_QPA_PLATFORM", "wayland")
                        .env("QML_DISABLE_DISK_CACHE", "1")
                        .env("QML_XHR_ALLOW_FILE_READ", "1");
                    kde_assistant_lib::owned_process::terminate_with_parent(&mut ocmd);
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

        Ok(())
    })();
    backend.desktop.stop();
    runtime.block_on(backend.native_voice.shutdown());
    // Stop admitting HTTP work before cancelling and draining existing turns.
    let _ = server_stop.send(());
    let drained = runtime.block_on(backend.turns.shutdown());
    if !drained {
        log::warn!(
            "Apagado: operación aún incierta; journal conservado, sin repetición automática"
        );
    }
    runtime.block_on(async {
        if let Some(task) = shortcut_task {
            // Cancel pending portal dialogs by dropping this dedicated connection.
            if !task.is_finished() {
                task.abort();
            }
            // Completion may race with abort. Always collect a returned owner.
            if let Ok(Some(session)) = task.await {
                session.close().await;
            }
        }
        if tokio::time::timeout(std::time::Duration::from_secs(3), &mut server)
            .await
            .is_err()
        {
            server.abort();
            let _ = server.await;
        }
    });
    drop(bus);
    drop(backend);
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    log::info!("Apagando KDE Assistant v2...");
    ui_result
}

fn validate_launch_args(args: &[String]) -> Result<()> {
    let mut arguments = args.iter();
    let mut port_seen = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--ui-off" | "--no-ui" | "--no-shortcuts" | "--enable-shortcuts" | "--enable-voice"
            | "--toggle" => {}
            "--http-port" => {
                anyhow::ensure!(!port_seen, "--http-port duplicado");
                port_seen = true;
                arguments
                    .next()
                    .context("--http-port requiere un puerto")?
                    .parse::<u16>()
                    .context("Puerto inválido")?;
            }
            "--send-msg" => anyhow::bail!("--send-msg retirado: DBus no acepta mensajes"),
            _ => anyhow::bail!("Argumento desconocido: {argument}"),
        }
    }
    Ok(())
}
