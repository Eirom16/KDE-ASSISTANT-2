//! Global Hotkey Listener - Cross-platform global shortcut listener
//!
//! Usa `rdev` para escuchar teclas globalmente. Funciona en Linux (X11),
//! Windows y macOS sin necesidad de KGlobalAccel u otros frameworks del SO.
//!
//! Para el envío de acciones a la UI QML, emite señales DBus al servicio
//! `org.kde.assistant` que el QML escucha.

use anyhow::Result;
use rdev::{listen, Event, EventType, Key};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use crate::models::Config;

const DBUS_DEST: &str = "org.kde.assistant";
const DBUS_PATH: &str = "/Chat";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    ToggleWindow,
    PushToTalkStart,
    PushToTalkEnd,
    NewSession,
}

impl HotkeyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            HotkeyAction::ToggleWindow => "toggle_window",
            HotkeyAction::PushToTalkStart => "push_to_talk_start",
            HotkeyAction::PushToTalkEnd => "push_to_talk_end",
            HotkeyAction::NewSession => "new_session",
        }
    }
}

// Estado de modificadores (compartido entre el callback de rdev y el main thread)
static MOD_SUPER: AtomicBool = AtomicBool::new(false);
static MOD_SHIFT: AtomicBool = AtomicBool::new(false);
static MOD_CTRL: AtomicBool = AtomicBool::new(false);
static MOD_ALT: AtomicBool = AtomicBool::new(false);
static PTT_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn start_listener(action_tx: mpsc::Sender<HotkeyAction>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        log::info!("Global hotkey listener iniciado");
        let tx = action_tx.clone();
        let callback = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => on_key(key, true, &tx),
            EventType::KeyRelease(key) => on_key(key, false, &tx),
            _ => {}
        };
        // listen() bloquea hasta error
        if let Err(e) = listen(callback) {
            log::warn!("rdev listen() finalizo con error: {e:?}");
        }
    })
}

fn on_key(key: Key, pressed: bool, tx: &mpsc::Sender<HotkeyAction>) {
    // Push-to-talk por release: si se suelta V mientras PTT estaba activo,
    // terminar aunque los modificadores ya se hayan soltado.
    if !pressed && matches!(key, Key::KeyV) && PTT_ACTIVE.load(Ordering::Relaxed) {
        PTT_ACTIVE.store(false, Ordering::Relaxed);
        emit_action(HotkeyAction::PushToTalkEnd, tx);
        return;
    }

    // Actualizar estado de modificadores
    match key {
        Key::MetaLeft | Key::MetaRight => MOD_SUPER.store(pressed, Ordering::Relaxed),
        Key::ShiftLeft | Key::ShiftRight => MOD_SHIFT.store(pressed, Ordering::Relaxed),
        Key::ControlLeft | Key::ControlRight => MOD_CTRL.store(pressed, Ordering::Relaxed),
        Key::Alt | Key::AltGr => MOD_ALT.store(pressed, Ordering::Relaxed),
        _ => {}
    }

    if !pressed {
        return;
    }

    let super_ = MOD_SUPER.load(Ordering::Relaxed);
    let shift = MOD_SHIFT.load(Ordering::Relaxed);
    let ctrl = MOD_CTRL.load(Ordering::Relaxed);
    let _alt = MOD_ALT.load(Ordering::Relaxed);

    // Push-to-talk por press: Super+Shift+V mantenido inicia la grabacion.
    if matches!(key, Key::KeyV) && super_ && shift {
        if !PTT_ACTIVE.load(Ordering::Relaxed) {
            PTT_ACTIVE.store(true, Ordering::Relaxed);
            emit_action(HotkeyAction::PushToTalkStart, tx);
        }
        return;
    }

    let action = match key {
        Key::KeyA if super_ && shift => Some(HotkeyAction::ToggleWindow),
        Key::KeyK if ctrl && shift => Some(HotkeyAction::NewSession),
        _ => None,
    };

    if let Some(action) = action {
        emit_action(action, tx);
    }
}

fn emit_action(action: HotkeyAction, tx: &mpsc::Sender<HotkeyAction>) {
    log::info!("Global hotkey detectado: {action:?}");
    // Emitir via DBus a la UI QML
    let result = Command::new("dbus-send")
        .args([
            "--session",
            "--type=signal",
            &format!("--dest={DBUS_DEST}"),
            DBUS_PATH,
            "org.kde.assistant.Chat.HotkeyTriggered",
            &format!("string:{}", action.as_str()),
        ])
        .status();
    if let Ok(s) = result {
        if !s.success() {
            log::debug!("No se pudo emitir HotkeyTriggered (UI no escuchando?)");
        }
    }
    // Tambien escribir a un archivo de estado (para que QML lo lea)
    write_hotkey_state(action);
    // Tambien emitir al canal local
    let _ = tx.try_send(action);
}

/// Escribe la accion de hotkey a un archivo de estado que QML puede leer.
/// Path: ~/.cache/kde-assistant/hotkey.flag
fn write_hotkey_state(action: HotkeyAction) {
    let cache_dir = match dirs::cache_dir() {
        Some(d) => d.join("kde-assistant"),
        None => return,
    };
    let _ = std::fs::create_dir_all(&cache_dir);
    let path = cache_dir.join("hotkey.state");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let content = format!("{}|{}", action.as_str(), timestamp);
    let _ = std::fs::write(&path, content);
}

pub struct GlobalHotkeyListener;

impl GlobalHotkeyListener {
    /// Crea una instancia placeholder. La escucha real se hace con
    /// `start_listener()` en un thread separado.
    pub async fn new(_config: Arc<RwLock<Config>>) -> Result<Self> {
        Ok(Self)
    }
}
