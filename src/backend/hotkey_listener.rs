//! Global Hotkey Listener - Cross-platform global shortcut listener
//!
//! Usa `rdev` para escuchar teclas globalmente. Funciona en Linux (X11),
//! Windows y macOS sin necesidad de KGlobalAccel u otros frameworks del SO.
//!
//! Los atajos salen de `config.shortcuts` (editables en Ajustes, F1-4) con
//! formato `"Super+Shift+A"`. Si un atajo no parsea, se usa el default.
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

use crate::models::{Config, ShortcutsConfig};

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

/// Atajo parseado: modificadores + tecla principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub super_: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub key: Key,
}

/// Parsea `"Super+Shift+A"`, `"Ctrl+Shift+K"`, `"Alt+F4"`, etc.
/// Modificadores: Super/Meta/Win/Cmd, Shift, Ctrl/Control, Alt.
/// Tecla: letra, dígito, F1-F12, Space/Enter/Tab/Escape.
pub fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let mut super_ = false;
    let mut shift = false;
    let mut ctrl = false;
    let mut alt = false;
    let mut key: Option<Key> = None;
    for part in s.split('+') {
        let p = part.trim().to_lowercase();
        match p.as_str() {
            "super" | "meta" | "win" | "windows" | "cmd" | "command" => super_ = true,
            "shift" => shift = true,
            "ctrl" | "control" => ctrl = true,
            "alt" | "altgr" => alt = true,
            "" => return None,
            _ => {
                if key.is_some() {
                    return None; // dos teclas principales
                }
                key = Some(parse_key(&p)?);
            }
        }
    }
    Some(Shortcut {
        super_,
        shift,
        ctrl,
        alt,
        key: key?,
    })
}

fn parse_key(p: &str) -> Option<Key> {
    if p.len() == 1 {
        let c = p.chars().next()?.to_ascii_lowercase();
        return match c {
            'a' => Some(Key::KeyA),
            'b' => Some(Key::KeyB),
            'c' => Some(Key::KeyC),
            'd' => Some(Key::KeyD),
            'e' => Some(Key::KeyE),
            'f' => Some(Key::KeyF),
            'g' => Some(Key::KeyG),
            'h' => Some(Key::KeyH),
            'i' => Some(Key::KeyI),
            'j' => Some(Key::KeyJ),
            'k' => Some(Key::KeyK),
            'l' => Some(Key::KeyL),
            'm' => Some(Key::KeyM),
            'n' => Some(Key::KeyN),
            'o' => Some(Key::KeyO),
            'p' => Some(Key::KeyP),
            'q' => Some(Key::KeyQ),
            'r' => Some(Key::KeyR),
            's' => Some(Key::KeyS),
            't' => Some(Key::KeyT),
            'u' => Some(Key::KeyU),
            'v' => Some(Key::KeyV),
            'w' => Some(Key::KeyW),
            'x' => Some(Key::KeyX),
            'y' => Some(Key::KeyY),
            'z' => Some(Key::KeyZ),
            '0' => Some(Key::Num0),
            '1' => Some(Key::Num1),
            '2' => Some(Key::Num2),
            '3' => Some(Key::Num3),
            '4' => Some(Key::Num4),
            '5' => Some(Key::Num5),
            '6' => Some(Key::Num6),
            '7' => Some(Key::Num7),
            '8' => Some(Key::Num8),
            '9' => Some(Key::Num9),
            _ => None,
        };
    }
    match p {
        "space" | "espacio" => Some(Key::Space),
        "enter" | "return" => Some(Key::Return),
        "tab" => Some(Key::Tab),
        "escape" | "esc" => Some(Key::Escape),
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        _ => None,
    }
}

impl Shortcut {
    fn matches(&self, key: Key, super_: bool, shift: bool, ctrl: bool, alt: bool) -> bool {
        self.key == key
            && self.super_ == super_
            && self.shift == shift
            && self.ctrl == ctrl
            && self.alt == alt
    }
}

fn default_shortcuts() -> ShortcutsConfig {
    ShortcutsConfig {
        toggle: "Super+Shift+A".to_string(),
        push_to_talk: "Super+Shift+V".to_string(),
        new_session: "Ctrl+Shift+K".to_string(),
    }
}

// Estado de modificadores (compartido entre el callback de rdev y el main thread)
static MOD_SUPER: AtomicBool = AtomicBool::new(false);
static MOD_SHIFT: AtomicBool = AtomicBool::new(false);
static MOD_CTRL: AtomicBool = AtomicBool::new(false);
static MOD_ALT: AtomicBool = AtomicBool::new(false);
static PTT_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn start_listener(
    action_tx: mpsc::Sender<HotkeyAction>,
    config: Arc<RwLock<Config>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        log::info!("Global hotkey listener iniciado");
        let tx = action_tx.clone();
        let callback = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => on_key(key, true, &tx, &config),
            EventType::KeyRelease(key) => on_key(key, false, &tx, &config),
            _ => {}
        };
        // listen() bloquea hasta error
        if let Err(e) = listen(callback) {
            log::warn!("rdev listen() finalizo con error: {e:?}");
        }
    })
}

fn current_shortcuts(config: &Arc<RwLock<Config>>) -> ShortcutsConfig {
    config
        .try_read()
        .map(|c| c.shortcuts.clone())
        .unwrap_or_else(|_| default_shortcuts())
}

fn on_key(key: Key, pressed: bool, tx: &mpsc::Sender<HotkeyAction>, config: &Arc<RwLock<Config>>) {
    let sc = current_shortcuts(config);
    let toggle = parse_shortcut(&sc.toggle).unwrap_or(Shortcut {
        super_: true,
        shift: true,
        ctrl: false,
        alt: false,
        key: Key::KeyA,
    });
    let ptt = parse_shortcut(&sc.push_to_talk).unwrap_or(Shortcut {
        super_: true,
        shift: true,
        ctrl: false,
        alt: false,
        key: Key::KeyV,
    });
    let new_session = parse_shortcut(&sc.new_session).unwrap_or(Shortcut {
        super_: false,
        shift: true,
        ctrl: true,
        alt: false,
        key: Key::KeyK,
    });

    // Push-to-talk por release: si se suelta la tecla PTT mientras estaba activo,
    // terminar aunque los modificadores ya se hayan soltado.
    if !pressed && PTT_ACTIVE.load(Ordering::Relaxed) && key == ptt.key {
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
    let alt = MOD_ALT.load(Ordering::Relaxed);

    // Push-to-talk por press (mantener para dictar).
    if ptt.matches(key, super_, shift, ctrl, alt) {
        if !PTT_ACTIVE.load(Ordering::Relaxed) {
            PTT_ACTIVE.store(true, Ordering::Relaxed);
            emit_action(HotkeyAction::PushToTalkStart, tx);
        }
        return;
    }

    if toggle.matches(key, super_, shift, ctrl, alt) {
        emit_action(HotkeyAction::ToggleWindow, tx);
    } else if new_session.matches(key, super_, shift, ctrl, alt) {
        emit_action(HotkeyAction::NewSession, tx);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults() {
        let t = parse_shortcut("Super+Shift+A").unwrap();
        assert!(t.super_ && t.shift && !t.ctrl && t.key == Key::KeyA);
        let p = parse_shortcut("Super+Shift+V").unwrap();
        assert!(p.key == Key::KeyV);
        let n = parse_shortcut("Ctrl+Shift+K").unwrap();
        assert!(n.ctrl && n.shift && n.key == Key::KeyK);
    }

    #[test]
    fn parse_case_and_aliases() {
        assert!(parse_shortcut("ctrl+shift+k").is_some());
        assert!(parse_shortcut("Meta+Shift+A").is_some());
        assert!(parse_shortcut("Alt+F4").is_some());
        assert!(parse_shortcut("").is_none());
        assert!(parse_shortcut("Super+FooBar").is_none());
        assert!(parse_shortcut("A+B").is_none());
    }

    #[test]
    fn matches_modifiers_exactly() {
        let t = parse_shortcut("Super+Shift+A").unwrap();
        assert!(t.matches(Key::KeyA, true, true, false, false));
        assert!(!t.matches(Key::KeyA, true, false, false, false));
        assert!(!t.matches(Key::KeyB, true, true, false, false));
    }
}
