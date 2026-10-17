//! Legacy shortcut syntax parser. No keyboard capture or execution authority.
use rdev::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    ToggleWindow,
    PushToTalkStart,
    PushToTalkEnd,
    NewSession,
    /// Abre el menu del tray (popup propio junto al icono).
    OpenMenu,
    /// Abre el diálogo de configuración de la ventana principal.
    OpenSettings,
}

impl HotkeyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            HotkeyAction::ToggleWindow => "toggle_window",
            HotkeyAction::PushToTalkStart => "push_to_talk_start",
            HotkeyAction::PushToTalkEnd => "push_to_talk_end",
            HotkeyAction::NewSession => "new_session",
            HotkeyAction::OpenMenu => "open_menu",
            HotkeyAction::OpenSettings => "open_settings",
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
    pub fn matches(&self, key: Key, super_: bool, shift: bool, ctrl: bool, alt: bool) -> bool {
        self.key == key
            && self.super_ == super_
            && self.shift == shift
            && self.ctrl == ctrl
            && self.alt == alt
    }
}
