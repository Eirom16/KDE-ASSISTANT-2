//! Internal KDE adapters. None is connected to the agent dispatcher.
use anyhow::{Context, Result};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

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
        self != Self::Light
    }
}
impl Default for KdeIntegration {
    fn default() -> Self {
        Self::new()
    }
}
impl KdeIntegration {
    // Construction must not contact DBus (including during unit tests).
    pub fn new() -> Self {
        Self {
            theme: ThemePreference::System,
        }
    }
    pub async fn detect_theme(bus: &zbus::Connection) -> Result<ThemePreference> {
        let proxy = zbus::Proxy::new(
            bus,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .await?;
        let value: OwnedValue = tokio::time::timeout(
            crate::desktop::DBUS_TIMEOUT,
            proxy.call("Read", &("org.freedesktop.appearance", "color-scheme")),
        )
        .await
        .context("theme_dbus_timeout")??;
        parse_theme(&value)
    }
    pub fn set_window_blur(&self, _window_id: u32) -> Result<()> {
        anyhow::bail!("NOT_SUPPORTED: no existe integración de blur nativo; transparencia Qt no garantiza blur")
    }
    pub fn enable_translucent(&self) -> Result<()> {
        self.set_window_blur(0)
    }
}

fn parse_theme(value: &OwnedValue) -> Result<ThemePreference> {
    // Settings.Read historically wraps its result in an extra variant.
    Ok(match value.downcast_ref::<u32>()? {
        1 => ThemePreference::Dark,
        2 => ThemePreference::Light,
        _ => ThemePreference::System,
    })
}

pub fn validate_notification(title: &str, body: &str) -> Result<()> {
    anyhow::ensure!(
        !title.trim().is_empty() && title.len() <= 160 && body.len() <= 2048,
        "notification_size_invalid"
    );
    anyhow::ensure!(
        !title.chars().any(char::is_control) && !body.chars().any(|c| c.is_control() && c != '\n'),
        "notification_control_character"
    );
    Ok(())
}
/// Internal application notices only. No HTTP/QML/LLM entry point accepts this action.
/// Return the server-assigned ID; a timeout means outcome unknown, never retry.
pub async fn send_notification(bus: &zbus::Connection, title: &str, body: &str) -> Result<u32> {
    validate_notification(title, body)?;
    let body = body
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let proxy = zbus::Proxy::new(
        bus,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await?;
    let hints: HashMap<String, OwnedValue> = HashMap::new();
    let id: u32 = tokio::time::timeout(
        crate::desktop::DBUS_TIMEOUT,
        proxy.call(
            "Notify",
            &(
                "KDE Assistant",
                0u32,
                "",
                title,
                body,
                Vec::<String>::new(),
                hints,
                5000i32,
            ),
        ),
    )
    .await
    .context("notification_timeout_outcome_unknown")??;
    anyhow::ensure!(id != 0, "notification_invalid_id_outcome_unknown");
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_creates_default() {
        assert_eq!(KdeIntegration::new().theme, ThemePreference::System);
    }
    #[test]
    fn theme_preference_default() {
        assert!(ThemePreference::Dark.is_dark());
        assert!(!ThemePreference::Light.is_dark());
    }
    #[test]
    fn theme_accepts_portal_nested_variant_but_rejects_wrong_type() {
        let nested = OwnedValue::try_from(zbus::zvariant::Value::Value(Box::new(
            zbus::zvariant::Value::U32(1),
        )))
        .unwrap();
        assert_eq!(parse_theme(&nested).unwrap(), ThemePreference::Dark);
        assert_eq!(
            parse_theme(&OwnedValue::from(2u32)).unwrap(),
            ThemePreference::Light
        );
        assert_eq!(
            parse_theme(&OwnedValue::from(0u32)).unwrap(),
            ThemePreference::System
        );
        assert!(parse_theme(&OwnedValue::from(zbus::zvariant::Str::from("invalid"))).is_err());
    }
}
