//! Synthetic F3 tests. DBus services run on an owned private session bus.
use kde_assistant_lib::{
    backend::{auth, global_shortcuts, kde_integration},
    desktop::*,
    owned_process::OwnedChild,
};
use std::{
    fs,
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("kda-f3-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn instance_lock_is_owned_released_and_stale_pid_is_not_authority() {
    let f = Fixture::new();
    fs::write(f.0.join("desktop.lock"), "1 stale-untrusted-pid").unwrap();
    fs::set_permissions(f.0.join("desktop.lock"), fs::Permissions::from_mode(0o600)).unwrap();
    let InstanceState::Primary(lock) = InstanceLock::acquire(&f.0).unwrap() else {
        panic!()
    };
    assert!(matches!(
        InstanceLock::acquire(&f.0).unwrap(),
        InstanceState::Existing(_)
    ));
    assert!(!lock.bus_name.contains(f.0.to_str().unwrap()));
    drop(lock);
    assert!(matches!(
        InstanceLock::acquire(&f.0).unwrap(),
        InstanceState::Primary(_)
    ));
    assert_eq!(
        fs::read_to_string(f.0.join("desktop.lock")).unwrap(),
        "1 stale-untrusted-pid"
    );
}

#[test]
fn profiles_do_not_share_instance_authority() {
    let a = Fixture::new();
    let b = Fixture::new();
    let InstanceState::Primary(a) = InstanceLock::acquire(&a.0).unwrap() else {
        panic!()
    };
    let InstanceState::Primary(b) = InstanceLock::acquire(&b.0).unwrap() else {
        panic!()
    };
    assert_ne!(a.bus_name, b.bus_name);
}

#[test]
fn lock_rejects_symlink_hardlink_and_public_file() {
    let f = Fixture::new();
    let lock = f.0.join("desktop.lock");
    let outside = f.0.join("outside");
    fs::write(&outside, "do not touch").unwrap();
    std::os::unix::fs::symlink(&outside, &lock).unwrap();
    assert!(InstanceLock::acquire(&f.0).is_err());
    fs::remove_file(&lock).unwrap();
    fs::hard_link(&outside, &lock).unwrap();
    assert!(InstanceLock::acquire(&f.0).is_err());
    fs::remove_file(&lock).unwrap();
    fs::write(&lock, "public").unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(InstanceLock::acquire(&f.0).is_err());
    assert_eq!(fs::read_to_string(outside).unwrap(), "do not touch");
}

#[test]
fn ui_channel_is_bounded_no_replay_and_fail_closed_on_stop() {
    let desktop = Desktop::default();
    assert!(desktop.send(UiAction::Show).is_err());
    let mut rx = desktop.subscribe();
    desktop.send(UiAction::Show).unwrap();
    assert_eq!(rx.try_recv().unwrap(), UiAction::Show);
    for _ in 0..40 {
        desktop.send(UiAction::Toggle).unwrap();
    }
    assert!(matches!(
        rx.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
    ));
    let mut fresh = desktop.subscribe();
    assert!(fresh.try_recv().is_err());
    desktop.stop();
    assert!(desktop.send(UiAction::Show).is_err());
}

#[test]
fn shortcut_parser_is_explicit_and_never_defaults_on_error() {
    assert_eq!(
        global_shortcuts::preferred_trigger("Super+Shift+A").unwrap(),
        "LOGO+SHIFT+a"
    );
    for invalid in ["A", "", "Ctrl+Ctrl+A", "Super+A+B", "Ctrl+;", "Ctrl+Space"] {
        assert!(
            global_shortcuts::preferred_trigger(invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn notifications_and_blur_reject_false_success() {
    assert!(kde_integration::validate_notification("", "body").is_err());
    assert!(kde_integration::validate_notification("a", &"x".repeat(2049)).is_err());
    assert!(kde_integration::validate_notification("bad\n", "body").is_err());
    assert!(kde_integration::KdeIntegration::new()
        .set_window_blur(1)
        .is_err());
}

#[test]
fn profile_http_authority_remains_exact() {
    use axum::http::{HeaderMap, HeaderValue};
    let mut h = HeaderMap::new();
    h.insert("Host", HeaderValue::from_static("127.0.0.1:18999"));
    assert!(auth::valid_host_for_port(&h, 18999));
    assert!(!auth::valid_host_for_port(&h, 8765));
    h.insert("Origin", HeaderValue::from_static("http://localhost:18999"));
    assert!(auth::valid_origin_for_port(&h, 18999));
    assert!(!auth::valid_origin_for_port(&h, 8765));
    h.insert(
        "Origin",
        HeaderValue::from_static("http://localhost:18999/path"),
    );
    assert!(!auth::valid_origin_for_port(&h, 18999));
}

struct PrivateBus {
    _child: OwnedChild,
    _fixture: Fixture,
}
fn private_bus() -> (PrivateBus, String) {
    let fixture = Fixture::new();
    let config = fixture.0.join("bus.conf");
    fs::write(&config, format!("<busconfig><type>session</type><listen>unix:abstract=kda_f3_{}</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>", uuid::Uuid::new_v4().simple())).unwrap();
    let mut child = OwnedChild::new(
        Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    (
        PrivateBus {
            _child: child,
            _fixture: fixture,
        },
        address.trim().into(),
    )
}
async fn connect(address: &str) -> zbus::Connection {
    zbus::connection::Builder::address(address)
        .unwrap()
        .method_timeout(DBUS_TIMEOUT)
        .max_queued(32)
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn dbus_activation_acknowledges_only_connected_own_ui() {
    let (_daemon, address) = private_bus();
    let server = connect(&address).await;
    let client = connect(&address).await;
    let desktop = std::sync::Arc::new(Desktop::default());
    let name = "org.kde.assistant.f3test";
    register(&server, name, desktop.clone()).await.unwrap();
    assert!(activate(&client, name, false).await.is_err());
    let mut rx = desktop.subscribe();
    activate(&client, name, false).await.unwrap();
    assert_eq!(rx.recv().await.unwrap(), UiAction::Show);
    activate(&client, name, true).await.unwrap();
    assert_eq!(rx.recv().await.unwrap(), UiAction::Toggle);
    let proxy = zbus::Proxy::new(&client, name, DBUS_PATH, DBUS_INTERFACE)
        .await
        .unwrap();
    assert!(proxy
        .call::<_, _, ()>("ExecuteTool", &("notify", "untrusted"))
        .await
        .is_err());
    assert!(register(&client, name, desktop.clone()).await.is_err());
    desktop.stop();
    assert!(activate(&client, name, false).await.is_err());
}

struct Notifications;
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app: &str,
        replaces: u32,
        icon: &str,
        title: &str,
        body: &str,
        actions: Vec<String>,
        hints: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        expires: i32,
    ) -> zbus::fdo::Result<u32> {
        assert_eq!(app, "KDE Assistant");
        assert_eq!(replaces, 0);
        assert_eq!(icon, "");
        assert!(actions.is_empty() && hints.is_empty());
        assert_eq!(expires, 5000);
        if title == "error" {
            return Err(zbus::fdo::Error::Failed("synthetic".into()));
        }
        if title == "timeout" {
            tokio::time::sleep(Duration::from_secs(4)).await;
        }
        if title == "zero" {
            return Ok(0);
        }
        if title == "markup" {
            assert_eq!(body, "&lt;b&gt;&amp;&lt;/b&gt;");
        }
        Ok(77)
    }
}

#[tokio::test]
async fn notifications_use_real_reply_error_timeout_and_no_retry_on_private_bus() {
    let (_daemon, address) = private_bus();
    let server = connect(&address).await;
    let client = connect(&address).await;
    server
        .object_server()
        .at("/org/freedesktop/Notifications", Notifications)
        .await
        .unwrap();
    server
        .request_name("org.freedesktop.Notifications")
        .await
        .unwrap();
    assert_eq!(
        kde_integration::send_notification(&client, "markup", "<b>&</b>")
            .await
            .unwrap(),
        77
    );
    assert!(
        kde_integration::send_notification(&client, "error", "synthetic")
            .await
            .is_err()
    );
    let before = std::time::Instant::now();
    assert!(
        kde_integration::send_notification(&client, "zero", "synthetic")
            .await
            .is_err()
    );
    assert!(
        kde_integration::send_notification(&client, "timeout", "synthetic")
            .await
            .is_err()
    );
    assert!(before.elapsed() < Duration::from_secs(4));
}

#[tokio::test]
async fn missing_global_shortcuts_stays_disabled() {
    let (_daemon, address) = private_bus();
    let client = connect(&address).await;
    let desktop = std::sync::Arc::new(Desktop::default());
    assert!(
        global_shortcuts::register(&client, desktop, "Super+Shift+A")
            .await
            .is_err()
    );
}

#[test]
fn owned_child_drop_reaps_only_its_child() {
    let mut unrelated = OwnedChild::new(Command::new("sleep").arg("30").spawn().unwrap());
    let own = OwnedChild::new(Command::new("sleep").arg("30").spawn().unwrap());
    let pid = own.id();
    drop(own);
    assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
    assert!(unrelated.try_wait().unwrap().is_none());
}

type Dict = std::collections::HashMap<String, zbus::zvariant::OwnedValue>;
fn value(s: impl Into<String>) -> zbus::zvariant::OwnedValue {
    zbus::zvariant::OwnedValue::from(zbus::zvariant::Str::from(s.into()))
}
struct PortalSession(std::sync::Arc<std::sync::atomic::AtomicBool>);
#[zbus::interface(name = "org.freedesktop.portal.Session")]
impl PortalSession {
    fn close(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AuditMode {
    EarlyActivation,
    EarlyClosed,
    WrongSession,
    WrongBound,
    DropOwner,
    ServiceLost,
}
struct Portal {
    audit: Option<AuditMode>,
    path: std::sync::Arc<std::sync::Mutex<Option<zbus::zvariant::OwnedObjectPath>>>,
    denied: bool,
    existing: bool,
    unknown: bool,
    oversized: bool,
    closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
async fn respond(
    bus: &zbus::Connection,
    header: zbus::message::Header<'_>,
    opts: Dict,
    code: u32,
    results: Dict,
) -> zbus::zvariant::OwnedObjectPath {
    let sender = header
        .sender()
        .unwrap()
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let token = <&str>::try_from(opts.get("handle_token").unwrap()).unwrap();
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    // Deliberately respond before the method returns to cover the subscription race.
    bus.emit_signal(
        None::<&str>,
        path.as_str(),
        "org.freedesktop.portal.Request",
        "Response",
        &(code, results),
    )
    .await
    .unwrap();
    zbus::zvariant::OwnedObjectPath::try_from(path).unwrap()
}
#[zbus::interface(name = "org.freedesktop.portal.GlobalShortcuts")]
impl Portal {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
    async fn create_session(
        &self,
        options: Dict,
        #[zbus(connection)] bus: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::zvariant::OwnedObjectPath {
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("session_handle_token").unwrap()).unwrap();
        let path = if self.audit == Some(AuditMode::WrongSession) {
            "/f3/not_our_session".to_owned()
        } else {
            format!("/org/freedesktop/portal/desktop/session/{sender}/{token}")
        };
        let path = zbus::zvariant::OwnedObjectPath::try_from(path).unwrap();
        *self.path.lock().unwrap() = Some(path.clone());
        bus.object_server()
            .at(path.clone(), PortalSession(self.closed.clone()))
            .await
            .unwrap();
        respond(
            bus,
            header,
            options,
            0,
            Dict::from([("session_handle".into(), value(path.as_str()))]),
        )
        .await
    }
    async fn list_shortcuts(
        &self,
        session: zbus::zvariant::OwnedObjectPath,
        options: Dict,
        #[zbus(connection)] bus: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::zvariant::OwnedObjectPath {
        assert_eq!(Some(session.clone()), *self.path.lock().unwrap());
        let list: Vec<(String, Dict)> = if self.existing {
            vec![(
                if self.unknown { "unknown" } else { "toggle" }.into(),
                Dict::new(),
            )]
        } else {
            Vec::new()
        };
        respond(bus, header, options, 0, {
            let mut result = Dict::from([(
                "shortcuts".into(),
                zbus::zvariant::Value::from(list).try_into().unwrap(),
            )]);
            if self.oversized {
                result.insert("padding".into(), value("x".repeat(65536)));
            }
            result
        })
        .await
    }
    async fn bind_shortcuts(
        &self,
        session: zbus::zvariant::OwnedObjectPath,
        mut shortcuts: Vec<(String, Dict)>,
        parent: &str,
        options: Dict,
        #[zbus(connection)] bus: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::zvariant::OwnedObjectPath {
        assert_eq!(Some(session.clone()), *self.path.lock().unwrap());
        assert_eq!(parent, "");
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].0, "toggle");
        if self.existing {
            assert!(!shortcuts[0].1.contains_key("preferred_trigger"));
        } else {
            assert_eq!(
                <&str>::try_from(shortcuts[0].1.get("preferred_trigger").unwrap()).unwrap(),
                "LOGO+SHIFT+a"
            );
        }
        if self.audit == Some(AuditMode::WrongBound) {
            shortcuts.push(("unexpected".into(), Dict::new()));
        }
        if self.audit == Some(AuditMode::EarlyActivation) {
            bus.emit_signal(
                None::<&str>,
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                "Activated",
                &(session.clone(), "toggle", 1u64, Dict::new()),
            )
            .await
            .unwrap();
        }
        if self.audit == Some(AuditMode::EarlyClosed) {
            bus.emit_signal(
                None::<&str>,
                session.as_str(),
                "org.freedesktop.portal.Session",
                "Closed",
                &(Dict::new(),),
            )
            .await
            .unwrap();
        }
        respond(
            bus,
            header,
            options,
            if self.denied { 1 } else { 0 },
            Dict::from([(
                "shortcuts".into(),
                zbus::zvariant::Value::from(shortcuts).try_into().unwrap(),
            )]),
        )
        .await
    }
}

#[tokio::test]
async fn portal_session_activation_release_close_and_denial_are_owned() {
    for (denied, existing, unknown, oversized) in [
        (false, false, false, false),
        (false, true, false, false),
        (true, false, false, false),
        (false, true, true, false),
        (false, false, false, true),
    ] {
        let (_daemon, address) = private_bus();
        let server = connect(&address).await;
        let client = connect(&address).await;
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let path = std::sync::Arc::new(std::sync::Mutex::new(None));
        server
            .object_server()
            .at(
                "/org/freedesktop/portal/desktop",
                Portal {
                    audit: None,
                    path: path.clone(),
                    denied,
                    existing,
                    unknown,
                    oversized,
                    closed: closed.clone(),
                },
            )
            .await
            .unwrap();
        server
            .request_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        let desktop = std::sync::Arc::new(Desktop::default());
        let mut rx = desktop.subscribe();
        let result = global_shortcuts::register(&client, desktop.clone(), "Super+Shift+A").await;
        if denied || unknown || oversized {
            assert!(result.is_err());
            assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
            continue;
        }
        let session = result.unwrap();
        // Status events are metadata, never tool actions.
        while rx.try_recv().is_ok() {}
        let path = path.lock().unwrap().clone().unwrap();
        server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                "Activated",
                &(
                    path.clone(),
                    "toggle",
                    1u64,
                    Dict::from([("activation_token".into(), value("synthetic-activation"))]),
                ),
            )
            .await
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .unwrap()
                .unwrap(),
            UiAction::PortalToggle("synthetic-activation".into())
        );
        server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                "Deactivated",
                &(path, "toggle", 2u64, Dict::new()),
            )
            .await
            .unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .is_err());
        session.close().await;
        assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[test]
fn qml_profile_cannot_escape_generated_module_or_use_unresolved_port() {
    let token = "a".repeat(64);
    assert!(auth::write_qml_auth_for_desktop(&token, false, "../escape", 8765).is_err());
    let profile = profile_bus_name(std::path::Path::new("/synthetic/config"));
    assert!(auth::write_qml_auth_for_desktop(&token, false, &profile, 0).is_err());
}

async fn audit_portal(mode: AuditMode) {
    let (_daemon, address) = private_bus();
    let server = connect(&address).await;
    let client = connect(&address).await;
    let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let session_path = std::sync::Arc::new(std::sync::Mutex::new(None));
    server
        .object_server()
        .at(
            "/org/freedesktop/portal/desktop",
            Portal {
                audit: Some(mode),
                path: session_path.clone(),
                closed: closed.clone(),
                denied: false,
                existing: false,
                unknown: false,
                oversized: false,
            },
        )
        .await
        .unwrap();
    server
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    let desktop = std::sync::Arc::new(Desktop::default());
    let mut rx = desktop.subscribe();
    let result = global_shortcuts::register(&client, desktop.clone(), "Super+Shift+A").await;
    if mode == AuditMode::ServiceLost {
        let session = result.unwrap();
        server
            .release_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        let unavailable = tokio::time::timeout(Duration::from_millis(300), async {
            while desktop.shortcut_status() == "registered_portal" {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        session.close().await;
        assert!(
            unavailable.is_ok(),
            "vanished portal still reported a live shortcut session"
        );
    } else if mode == AuditMode::DropOwner {
        let session = result.unwrap();
        while rx.try_recv().is_ok() {}
        let path = session_path.lock().unwrap().clone().unwrap();
        drop(session);
        server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                "Activated",
                &(path, "toggle", 1u64, Dict::new()),
            )
            .await
            .unwrap();
        let activation = tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                if let Ok(UiAction::PortalToggle(_)) = rx.recv().await {
                    break;
                }
            }
        })
        .await;
        assert!(
            activation.is_err(),
            "discarded ShortcutSession left its activation listener alive"
        );
    } else if mode == AuditMode::EarlyActivation {
        let session = result.unwrap();
        let activation = tokio::time::timeout(Duration::from_millis(300), async {
            loop {
                if let Ok(UiAction::PortalToggle(token)) = rx.recv().await {
                    break token;
                }
            }
        })
        .await;
        session.close().await;
        assert!(
            activation.is_ok(),
            "activation sent before BindShortcuts reply was lost"
        );
    } else if let Ok(session) = result {
        session.close().await;
        panic!("portal accepted a closed session, unowned handle or unexpected bound action");
    }
}
#[tokio::test]
async fn portal_audit_early_activation_is_not_lost() {
    audit_portal(AuditMode::EarlyActivation).await
}
#[tokio::test]
async fn portal_audit_early_session_close_is_not_success() {
    audit_portal(AuditMode::EarlyClosed).await
}
#[tokio::test]
async fn portal_audit_session_handle_is_owned() {
    audit_portal(AuditMode::WrongSession).await
}
#[tokio::test]
async fn portal_audit_bound_actions_are_exact() {
    audit_portal(AuditMode::WrongBound).await
}

#[tokio::test]
async fn portal_audit_discarded_owner_stops_listener() {
    audit_portal(AuditMode::DropOwner).await
}

#[test]
fn concurrent_startup_has_exactly_one_lock_owner() {
    let fixture = Fixture::new();
    let barrier = std::sync::Barrier::new(8);
    let owners = std::sync::Mutex::new(Vec::new());
    let existing = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                barrier.wait();
                match InstanceLock::acquire(&fixture.0).unwrap() {
                    InstanceState::Primary(owner) => owners.lock().unwrap().push(owner),
                    InstanceState::Existing(_) => {
                        existing.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            });
        }
    });
    assert_eq!(owners.lock().unwrap().len(), 1);
    assert_eq!(existing.load(std::sync::atomic::Ordering::SeqCst), 7);
}

struct PendingPortal(std::sync::Arc<std::sync::atomic::AtomicBool>);
#[zbus::interface(name = "org.freedesktop.portal.GlobalShortcuts")]
impl PendingPortal {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
    async fn create_session(&self, _options: Dict) -> zbus::zvariant::OwnedObjectPath {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        std::future::pending().await
    }
}
#[tokio::test]
async fn portal_startup_cancellation_releases_dedicated_connection() {
    let (_daemon, address) = private_bus();
    let server = connect(&address).await;
    let entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    server
        .object_server()
        .at(
            "/org/freedesktop/portal/desktop",
            PendingPortal(entered.clone()),
        )
        .await
        .unwrap();
    server
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    let (send_name, receive_name) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let dedicated = connect(&address).await;
        send_name
            .send(dedicated.unique_name().unwrap().to_string())
            .unwrap();
        global_shortcuts::register(
            &dedicated,
            std::sync::Arc::new(Desktop::default()),
            "Super+Shift+A",
        )
        .await
    });
    let name = receive_name.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !entered.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    let dbus = zbus::Proxy::new(
        &server,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while dbus
            .call::<_, _, bool>("NameHasOwner", &(name.as_str(),))
            .await
            .unwrap()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn portal_audit_service_loss_is_not_registered() {
    audit_portal(AuditMode::ServiceLost).await
}
