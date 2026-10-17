//! XDG GlobalShortcuts only. Explicit opt-in; the portal arbitrates conflicts.
use crate::desktop::{Desktop, UiAction};
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use std::{collections::HashMap, sync::Arc, time::Duration};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Str};

const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
type Dict = HashMap<String, OwnedValue>;
type Shortcuts = Vec<(String, Dict)>;
fn string(value: impl Into<String>) -> OwnedValue {
    OwnedValue::from(Str::from(value.into()))
}

/// Reject malformed or unmodified keys rather than quietly using a default.
pub fn preferred_trigger(shortcut: &str) -> Result<String> {
    anyhow::ensure!(shortcut.len() <= 80, "invalid_shortcut");
    let parts: Vec<_> = shortcut.split('+').collect();
    anyhow::ensure!(
        parts.len() >= 2 && parts.len() <= 5,
        "shortcut_requires_modifier"
    );
    let mut mods = Vec::new();
    for part in &parts[..parts.len() - 1] {
        let modifier = match part.to_ascii_lowercase().as_str() {
            "super" | "meta" => "LOGO",
            "ctrl" | "control" => "CTRL",
            "shift" => "SHIFT",
            "alt" => "ALT",
            _ => bail!("invalid_shortcut_modifier"),
        };
        anyhow::ensure!(!mods.contains(&modifier), "duplicate_shortcut_modifier");
        mods.push(modifier);
    }
    let key = parts[parts.len() - 1];
    anyhow::ensure!(
        key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric(),
        "unsupported_shortcut_key"
    );
    Ok(format!("{}+{}", mods.join("+"), key.to_ascii_lowercase()))
}

async fn request<B: serde::Serialize + zbus::zvariant::DynamicType>(
    bus: &zbus::Connection,
    proxy: &zbus::Proxy<'_>,
    member: &str,
    body: impl FnOnce(String) -> B,
) -> Result<Dict> {
    let token = format!("kda{}", uuid::Uuid::new_v4().simple());
    let sender = bus
        .unique_name()
        .context("portal_requires_bus_name")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let req = zbus::Proxy::new(bus, DEST, path.as_str(), "org.freedesktop.portal.Request").await?;
    // Subscribe before invoking: a fast Response must not be lost.
    let mut responses = req.receive_signal("Response").await?;
    let returned: OwnedObjectPath = proxy.call(member, &body(token)).await?;
    anyhow::ensure!(
        returned.as_str() == path,
        "portal_unexpected_request_handle"
    );
    let response = tokio::time::timeout(Duration::from_secs(120), responses.next()).await;
    match response {
        Ok(Some(message)) => {
            anyhow::ensure!(
                message.body().len() <= 64 * 1024,
                "portal_response_bytes_limit"
            );
            let (code, result): (u32, Dict) = message.body().deserialize()?;
            anyhow::ensure!(code == 0, "portal_request_denied_or_cancelled:{code}");
            anyhow::ensure!(result.len() <= 16, "portal_result_too_large");
            Ok(result)
        }
        _ => {
            let _ = req.call::<_, _, ()>("Close", &()).await;
            bail!("portal_response_timeout");
        }
    }
}

pub struct ShortcutSession {
    bus: zbus::Connection,
    path: OwnedObjectPath,
    task: tokio::task::JoinHandle<()>,
    desktop: Arc<Desktop>,
}
impl Drop for ShortcutSession {
    fn drop(&mut self) {
        // Dropping a JoinHandle would detach the listener and retain its bus.
        self.task.abort();
        self.desktop.set_shortcut_status("portal_session_closed");
    }
}
impl ShortcutSession {
    pub async fn close(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
        if let Ok(proxy) = zbus::Proxy::new(
            &self.bus,
            DEST,
            self.path.as_str(),
            "org.freedesktop.portal.Session",
        )
        .await
        {
            let _ = proxy.call::<_, _, ()>("Close", &()).await;
        }
    }
}

pub async fn register(
    bus: &zbus::Connection,
    desktop: Arc<Desktop>,
    shortcut: &str,
) -> Result<ShortcutSession> {
    let trigger = preferred_trigger(shortcut)?;
    let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(bus)
        .destination(DEST)?
        .path(PATH)?
        .interface(IFACE)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let version: u32 =
        tokio::time::timeout(crate::desktop::DBUS_TIMEOUT, proxy.get_property("version"))
            .await
            .context("global_shortcuts_probe_timeout")?
            .context("global_shortcuts_not_supported")?;
    anyhow::ensure!(version >= 1, "global_shortcuts_not_supported");
    let mut owner_changes = proxy.receive_owner_changed().await?;
    desktop.set_shortcut_status("awaiting_system_consent");
    let session_token = format!("kda{}", uuid::Uuid::new_v4().simple());
    let sender = bus
        .unique_name()
        .context("portal_requires_bus_name")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = OwnedObjectPath::try_from(format!(
        "/org/freedesktop/portal/desktop/session/{sender}/{session_token}"
    ))?;
    let session =
        zbus::Proxy::new(bus, DEST, path.as_str(), "org.freedesktop.portal.Session").await?;
    // Subscribe before creation/binding: a portal may close or activate the
    // session before its method reply. A grant is still required before delivery.
    let mut closed = session.receive_signal("Closed").await?;
    let mut activated = proxy.receive_signal("Activated").await?;
    let mut deactivated = proxy.receive_signal("Deactivated").await?;
    let handshake = async {
        let created = request(bus, &proxy, "CreateSession", |token| {
            let options = Dict::from([
                ("handle_token".into(), string(token)),
                ("session_handle_token".into(), string(session_token)),
            ]);
            (options,)
        })
        .await?;
        let returned = created
            .get("session_handle")
            .context("portal_missing_session")?;
        anyhow::ensure!(
            <&str>::try_from(returned)? == path.as_str(),
            "portal_unowned_session_handle"
        );
        let listed = request(bus, &proxy, "ListShortcuts", |token| {
            (
                path.clone(),
                Dict::from([("handle_token".into(), string(token))]),
            )
        })
        .await?;
        let existing: Shortcuts = match listed.get("shortcuts") {
            Some(value) => Shortcuts::try_from(value.try_clone()?)?,
            None => Vec::new(),
        };
        anyhow::ensure!(existing.len() <= 32, "portal_shortcut_limit");
        anyhow::ensure!(
            existing.iter().all(|(id, _)| id == "toggle"),
            "portal_existing_actions_require_review"
        );
        let mut properties = Dict::from([(
            "description".into(),
            string("Mostrar u ocultar KDE Assistant"),
        )]);
        if existing.is_empty() {
            properties.insert("preferred_trigger".into(), string(trigger));
        }
        // Bind only our action. No direct changes to KGlobalAccel or personal config.
        // Omit the hint for an existing binding, so the user's assignment is
        // retained by the portal rather than proposed again as a new default.
        let desired = vec![("toggle".to_owned(), properties)];
        let bound = request(bus, &proxy, "BindShortcuts", |token| {
            (
                path.clone(),
                desired,
                "",
                Dict::from([("handle_token".into(), string(token))]),
            )
        })
        .await?;
        let shortcuts: Shortcuts = Shortcuts::try_from(
            bound
                .get("shortcuts")
                .context("portal_missing_shortcuts")?
                .try_clone()?,
        )?;
        anyhow::ensure!(
            shortcuts.len() == 1 && shortcuts[0].0 == "toggle",
            "shortcut_not_bound"
        );
        Ok::<_, anyhow::Error>(())
    };
    let result = tokio::select! {
        biased;
        _ = closed.next() => Err(anyhow::anyhow!("portal_session_closed_during_registration")),
        _ = owner_changes.next() => Err(anyhow::anyhow!("portal_owner_changed_during_registration")),
        result = handshake => result,
    };
    if let Err(error) = result {
        if let Ok(session) =
            zbus::Proxy::new(bus, DEST, path.as_str(), "org.freedesktop.portal.Session").await
        {
            let _ = session.call::<_, _, ()>("Close", &()).await;
        }
        return Err(error);
    }
    let our_path = path.clone();
    desktop.set_shortcut_status("registered_portal");
    let owner = desktop.clone();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = closed.next() => break,
                _ = owner_changes.next() => break,
                message = activated.next() => {
                    let Some(message) = message else { break; };
                    if message.body().len() > 8192 { continue; }
                    if let Ok((session, id, _, options)) = message.body().deserialize::<(OwnedObjectPath, String, u64, Dict)>() {
                        if session == our_path && id == "toggle" {
                            let token = options.get("activation_token").and_then(|v| <&str>::try_from(v).ok()).unwrap_or("");
                            if token.len() <= 4096 && !token.chars().any(char::is_control) {
                                let _ = desktop.send(UiAction::PortalToggle(token.to_owned()));
                            }
                        }
                    }
                }
                message = deactivated.next() => {
                    if message.is_none() { break; }
                    // Release is a portal event, never synthesized into PTT/voice.
                }
            }
        }
        desktop.set_shortcut_status("portal_session_closed");
    });
    Ok(ShortcutSession {
        bus: bus.clone(),
        path,
        task,
        desktop: owner,
    })
}
