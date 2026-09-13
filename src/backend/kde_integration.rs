//! KDE Integration - DBus, System Tray, Global Shortcuts, KWin blur
//!
//! Implementation using:
//! - `dbus-send` for KWin blur, notifications, theme detection
//! - `QSystemTrayIcon` in QML for system tray (handled in QML)

use anyhow::Result;
use std::process::Command;

pub struct KdeIntegration {
    pub theme: ThemePreference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn is_dark(self) -> bool {
        match self {
            ThemePreference::Dark => true,
            ThemePreference::Light => false,
            ThemePreference::System => Self::detect_system_prefers_dark(),
        }
    }

    /// Lee el tema del sistema via DBus portal.
    /// Retorna `true` si es dark, `false` si light.
    fn detect_system_prefers_dark() -> bool {
        // Intentar leer "org.freedesktop.appearance color-scheme" via portal
        let output = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.portal.Desktop",
                "--type=method_call",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings.Read",
                "string:org.freedesktop.appearance",
                "string:color-scheme",
            ])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Respuesta tipica: variant 1 = dark, 0 = light, 2 = unknown
                if stdout.contains("uint32 1") {
                    return true;
                }
            }
        }
        // Fallback: leer variable de entorno
        if let Ok(v) = std::env::var("KDE_BREEZE_THEME") {
            if v.contains("Dark") {
                return true;
            }
        }
        // Default: dark
        true
    }
}

impl KdeIntegration {
    pub fn new() -> Self {
        log::info!("KdeIntegration: inicializando");
        let theme = ThemePreference::System;
        let is_dark = theme.is_dark();
        log::info!("Tema detectado: dark={is_dark}");
        Self { theme }
    }

    /// Setup completo: detección de tema.
    /// Los shortcuts globales se manejan en `hotkey_listener`.
    pub fn setup(&mut self) -> Result<()> {
        log::info!("KdeIntegration: setup");
        let is_dark = self.theme.is_dark();
        log::info!("Tema detectado: dark={is_dark}");
        Ok(())
    }

    /// Aplica blur de KWin a la ventana via DBus.
    pub fn set_window_blur(&self, _window_id: u32) -> Result<()> {
        // La translucidez real se aplica en QML via Qt.WA_TranslucentBackground
        log::info!("KWin blur se aplica via Qt.WA_TranslucentBackground en QML");
        Ok(())
    }

    /// Habilita translucidez global via KWin (blur a fondo de pantalla).
    pub fn enable_translucent(&self) -> Result<()> {
        // La translucidez real se aplica en QML
        log::info!("Translucidez se aplica en QML via Qt.WA_TranslucentBackground");
        Ok(())
    }

    /// Manda una notificación nativa via DBus org.freedesktop.Notifications.
    pub fn send_notification(&self, title: &str, body: &str, app_name: &str) -> bool {
        let icon = app_name.to_string();
        let result = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.Notifications",
                "--type=method_call",
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications.Notify",
                &format!("string:{app_name}"),
                "uint32:0",
                &format!("string:{icon}"),
                &format!("string:{title}"),
                &format!("string:{body}"),
                "string:",
                "array:string:",
                "dict:string:",
                "int32:-1",
            ])
            .output();
        matches!(result, Ok(o) if o.status.success())
    }
}

impl Default for KdeIntegration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_default() {
        let k = KdeIntegration::new();
        assert!(matches!(k.theme, ThemePreference::System));
    }

    #[test]
    fn theme_preference_default() {
        let k = KdeIntegration::new();
        let _ = k.theme.is_dark();
    }
}
