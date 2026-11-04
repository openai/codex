//! Observe the local graphical session's confirmed lock state through system logind.
//!
//! Resolve a real session object, subscribe before reading its lock state, and retain individual
//! property-change payloads so a quick lock/unlock cannot erase the lock. Unsupported environments
//! leave voice available; idle hints and requests to lock are not confirmed locks.

use std::collections::HashMap;

use futures::StreamExt;
use zbus::zvariant::OwnedObjectPath;
use zbus::zvariant::OwnedValue;

use super::Activation;

const SERVICE: &str = "org.freedesktop.login1";
const SESSION: &str = "org.freedesktop.login1.Session";

pub(super) async fn observe() -> anyhow::Result<Activation> {
    let setup = async {
        // SAFETY: geteuid has no preconditions and does not mutate process state.
        let current_uid = unsafe { libc::geteuid() };
        let connection = zbus::Connection::system().await?;
        let manager = zbus::Proxy::new(
            &connection,
            SERVICE,
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await?;
        let path: Option<OwnedObjectPath> = match manager
            .call::<_, _, OwnedObjectPath>("GetSessionByPID", &std::process::id())
            .await
        {
            Ok(path) => {
                let candidate = session_proxy(&connection, path.clone()).await?;
                let class: String = candidate.get_property("Class").await?;
                if class == "manager" || class == "manager-early" {
                    None
                } else {
                    Some(path)
                }
            }
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str() == "org.freedesktop.login1.NoSessionForPID" =>
            {
                None
            }
            Err(error) => return Err(error.into()),
        };
        let path = if let Some(path) = path {
            path
        } else {
            // Graphical terminals may be launched by the per-user service manager, outside
            // the login scope. Prefer the client environment's explicit session association.
            if std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some()
            {
                anyhow::bail!("remote terminal has no graphical session");
            }
            match std::env::var("XDG_SESSION_ID") {
                Ok(session_id) => {
                    anyhow::ensure!(
                        !session_id.is_empty() && !matches!(session_id.as_str(), "self" | "auto"),
                        "no explicit graphical session"
                    );
                    manager.call("GetSession", &session_id).await?
                }
                Err(std::env::VarError::NotPresent) => {
                    unique_graphical_session(&connection, &manager, current_uid).await?
                }
                Err(error) => return Err(error.into()),
            }
        };
        let session = session_proxy(&connection, path.clone()).await?;
        let remote: bool = session.get_property("Remote").await?;
        let kind: String = session.get_property("Type").await?;
        let class: String = session.get_property("Class").await?;
        let (uid, _): (u32, OwnedObjectPath) = session.get_property("User").await?;
        let (seat, _): (String, OwnedObjectPath) = session.get_property("Seat").await?;
        anyhow::ensure!(
            !remote
                && is_graphical_user_session(&kind, &class)
                && uid == current_uid
                && !seat.is_empty(),
            "process is not in a local graphical user session"
        );
        let properties = zbus::Proxy::new(
            &connection,
            SERVICE,
            path,
            "org.freedesktop.DBus.Properties",
        )
        .await?;
        // Do not use receive_property_changed: zbus coalesces its cached updates, potentially
        // replacing a queued true with false before the TUI can observe it.
        let mut changes = properties
            .receive_signal_with_args("PropertiesChanged", &[(0, SESSION)])
            .await?;
        let initially_locked: bool = session.get_property("LockedHint").await?;
        Ok::<Activation, anyhow::Error>(Box::pin(async move {
            // Keep the connection and session alive until this explicit voice call ends.
            let _connection = connection;
            if initially_locked {
                return Ok(());
            }
            while let Some(signal) = changes.next().await {
                let (_, changed, invalidated): (String, HashMap<String, OwnedValue>, Vec<String>) =
                    signal.body().deserialize()?;
                if changed
                    .get("LockedHint")
                    .and_then(|value| bool::try_from(value).ok())
                    == Some(true)
                    || (invalidated.iter().any(|name| name == "LockedHint")
                        && session.get_property::<bool>("LockedHint").await?)
                {
                    return Ok(());
                }
            }
            anyhow::bail!("desktop lock observation ended")
        }))
    };
    // Missing logind, unsupported sessions, and unavailable properties must not disable voice.
    match tokio::time::timeout(std::time::Duration::from_secs(3), setup).await {
        Ok(Ok(activation)) => Ok(activation),
        Ok(Err(_)) | Err(_) => Ok(Box::pin(std::future::pending())),
    }
}

async fn unique_graphical_session(
    connection: &zbus::Connection,
    manager: &zbus::Proxy<'_>,
    current_uid: u32,
) -> anyhow::Result<OwnedObjectPath> {
    // GNOME terminal servers may forward DISPLAY/WAYLAND_DISPLAY without a
    // session ID. Resolve only an unambiguous graphical login for this user.
    let display = std::env::var("DISPLAY").ok();
    let local_display = display.as_deref().and_then(local_x_display);
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty());
    anyhow::ensure!(
        local_display.is_some() || wayland,
        "no local graphical client environment"
    );
    let sessions: Vec<(String, u32, String, String, OwnedObjectPath)> =
        manager.call("ListSessions", &()).await?;
    let mut selected = None;
    for (_, uid, _, _, path) in sessions {
        if uid != current_uid {
            continue;
        }
        let candidate = session_proxy(connection, path.clone()).await?;
        let kind: String = candidate.get_property("Type").await?;
        let class: String = candidate.get_property("Class").await?;
        if is_graphical_user_session(&kind, &class) {
            // Remote/seatless graphical logins also make association ambiguous.
            anyhow::ensure!(selected.is_none(), "ambiguous graphical session");
            if kind == "x11"
                && let Some(client_display) = local_display
            {
                let display: String = candidate.get_property("Display").await?;
                anyhow::ensure!(
                    display.is_empty() || local_x_display(&display) == Some(client_display),
                    "graphical session display does not match client"
                );
            }
            selected = Some(path);
        }
    }
    selected.ok_or_else(|| anyhow::anyhow!("no graphical user session"))
}

fn is_graphical_user_session(kind: &str, class: &str) -> bool {
    matches!(kind, "x11" | "wayland" | "mir")
        && matches!(
            class,
            "user" | "user-early" | "user-light" | "user-early-light"
        )
}

async fn session_proxy(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
) -> zbus::Result<zbus::Proxy<'static>> {
    zbus::proxy::Builder::new(connection)
        .destination(SERVICE)?
        .path(path)?
        .interface(SESSION)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

fn local_x_display(display: &str) -> Option<u32> {
    let number = display
        .strip_prefix(':')
        .or_else(|| display.strip_prefix("unix:"))
        .or_else(|| display.strip_prefix("unix/:"))?;
    let number = if let Some((number, screen)) = number.split_once('.') {
        screen.parse::<u32>().ok()?;
        number
    } else {
        number
    };
    number.parse().ok()
}
