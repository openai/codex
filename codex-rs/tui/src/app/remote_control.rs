//! Remote control configures the entire local daemon. Device pairing is a separate action,
//! and pairing codes are redacted from event diagnostics.

use super::App;
use super::backend_sessions::BackendSessions;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::backend_id::BackendId;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_features::Feature;
use std::time::Duration;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const ADMIN_UPDATE_GUIDANCE: &str =
    "Contact your administrator to update the background server for persistent remote control.";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Action {
    Menu,
    Enable,
    Disable,
    AddDevice,
}

pub(crate) enum Outcome {
    Menu(RemoteControlConnectionStatus, Option<bool>),
    Unavailable(String),
    Pairing(String),
    Info(String),
    Error(String),
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Menu(status, saved) => f.debug_tuple("Menu").field(status).field(saved).finish(),
            Self::Unavailable(reason) => f.debug_tuple("Unavailable").field(reason).finish(),
            Self::Pairing(_) => f.write_str("Pairing([redacted])"),
            Self::Info(_) => f.write_str("Info([redacted])"),
            Self::Error(_) => f.write_str("Error([redacted])"),
        }
    }
}

fn pairing(code: String) -> SelectionViewParams {
    SelectionViewParams {
        title: Some("Add device".into()),
        subtitle: Some(format!(
            "Enter this pairing code on your other device: {code}. If it expires, select Add device again."
        )),
        allow_input_when_disconnected: true,
        items: vec![SelectionItem {
            name: "Done".into(),
            dismiss_on_select: true,
            ..Default::default()
        }],
        ..SelectionViewParams::picker()
    }
}

fn menu(
    connection: Option<(BackendId, RemoteControlConnectionStatus, Option<bool>)>,
    unavailable: &str,
) -> SelectionViewParams {
    let status = connection.map(|(_, status, _)| status);
    let enabled = status.is_some_and(|status| status != RemoteControlConnectionStatus::Disabled);
    let retry_disable = !enabled && connection.is_some_and(|(_, _, saved)| saved != Some(false));
    let (name, action) = if enabled || retry_disable {
        ("Disable", Action::Disable)
    } else if status.is_none() {
        ("Enable (disabled)", Action::Enable)
    } else {
        ("Enable", Action::Enable)
    };
    SelectionViewParams {
        title: Some("Remote control".into()),
        allow_input_when_disconnected: true,
        subtitle: Some(match status {
            None => unavailable.into(),
            Some(_) if retry_disable => "Remote control is off now, but may return after restart. Choose Disable to save the preference.".into(),
            Some(_) => "Applies to local sessions; cloud sessions are unaffected.".into(),
        }),
        items: [
            (name, action, None),
            (
                "Add device",
                Action::AddDevice,
                (!enabled).then(|| "Enable remote control first".into()),
            ),
        ]
        .into_iter()
        .take(if status.is_some() { 2 } else { 1 })
        .map(|(name, action, disabled_reason)| SelectionItem {
            name: name.into(),
            disabled_reason,
            is_disabled: status.is_none(),
            actions: vec![Box::new(move |tx| {
                if let Some((backend_id, _, _)) = connection {
                    tx.send(AppEvent::RemoteControlForBackend { backend_id, action });
                }
            })],
            dismiss_on_select: true,
            ..Default::default()
        })
        .collect(),
        ..SelectionViewParams::picker()
    }
}

fn local_daemon<'a>(
    app: &App,
    backends: &'a BackendSessions,
) -> Option<(&'a AppServerSession, std::path::PathBuf)> {
    let (owner, target) = match &app.app_server_target {
        target @ crate::AppServerTarget::LocalDaemon { .. } => (backends.displayed_owner(), target),
        crate::AppServerTarget::Cloud => {
            backends.settings.iter().find_map(|(&owner, settings)| {
                matches!(settings.target, crate::AppServerTarget::LocalDaemon { .. })
                    .then_some((owner, &settings.target))
            })?
        }
        _ => return None,
    };
    let crate::AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket { socket_path },
        ..
    } = target
    else {
        return None;
    };
    backends
        .session(owner)
        .map(|session| (session, socket_path.to_path_buf()))
}

pub(super) fn handle(app: &mut App, backends: &BackendSessions, event: AppEvent) {
    let daemon = local_daemon(app, backends);
    let (action, requested_owner) = match event {
        AppEvent::RemoteControl(action) => (action, None),
        AppEvent::RemoteControlForBackend { backend_id, action } => (action, Some(backend_id)),
        AppEvent::RemoteControlFinished { backend_id, result } => {
            app.remote_control_pending = false;
            if daemon
                .as_ref()
                .is_some_and(|(session, _)| session.backend_id() == backend_id)
            {
                match result {
                    Outcome::Menu(status, saved) => app
                        .chat_widget
                        .show_selection_view(menu(Some((backend_id, status, saved)), "")),
                    Outcome::Unavailable(reason) => {
                        app.chat_widget
                            .show_selection_view(menu(/*connection*/ None, &reason))
                    }
                    Outcome::Pairing(code) => app.chat_widget.show_selection_view(pairing(code)),
                    Outcome::Info(message) => {
                        app.chat_widget.add_info_message(message, /*hint*/ None)
                    }
                    Outcome::Error(message) => app.chat_widget.add_error_message(message),
                }
            }
            return;
        }
        _ => return,
    };
    let Some((daemon, endpoint)) = daemon
        .filter(|(session, _)| requested_owner.is_none_or(|owner| session.backend_id() == owner))
    else {
        if matches!(action, Action::Menu) {
            app.chat_widget.show_selection_view(menu(
                /*connection*/ None,
                "Remote control requires the local background server",
            ));
        } else {
            app.chat_widget
                .add_error_message("Remote control requires the local background server.".into());
        }
        return;
    };
    if app.remote_control_pending {
        app.chat_widget.add_error_message(
            "A remote control operation is already in progress. Try again when it finishes.".into(),
        );
        return;
    }
    app.remote_control_pending = true;
    let backend_id = daemon.backend_id();
    let tx = app.app_event_tx.app_event_tx.clone();
    let updates_allowed = app.config.features.enabled(Feature::DaemonUpdates);
    tokio::spawn(async move {
        let operation = async {
            match verified_daemon_session(endpoint.as_path(), backend_id, updates_allowed).await {
                Ok((session, settings_file)) => {
                    let mut result = run(session, action).await;
                    let expected = match (&action, &result) {
                        (Action::Enable, Outcome::Menu(..)) => Some(true),
                        (Action::Disable, Outcome::Info(_)) => Some(false),
                        _ => None,
                    };
                    if expected.is_some() || matches!(result, Outcome::Menu(..)) {
                        let saved = match tokio::fs::read(&settings_file).await {
                            Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
                                .ok()
                                .and_then(|value| {
                                    value
                                        .get("remoteControlEnabled")
                                        .map_or(Some(false), serde_json::Value::as_bool)
                                }),
                            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Some(false),
                            Err(_) => None,
                        };
                        if let Outcome::Menu(_, preference) = &mut result {
                            *preference = saved;
                        }
                        if let Some(expected) = expected
                            && saved != Some(expected)
                        {
                            return Outcome::Error("Remote control changed for the running daemon, but the preference could not be saved. It may revert after restart.".into());
                        }
                    }
                    result
                }
                Err(reason) if matches!(action, Action::Menu) => Outcome::Unavailable(reason),
                Err(reason) => Outcome::Error(reason),
            }
        };
        let result = bounded(action, operation).await;
        let _ = tx.send(AppEvent::RemoteControlFinished { backend_id, result });
    });
}

async fn bounded(action: Action, operation: impl std::future::Future<Output = Outcome>) -> Outcome {
    tokio::time::timeout(OPERATION_TIMEOUT, operation)
        .await
        .unwrap_or_else(|_| match action {
            Action::Menu => Outcome::Error("Remote control status timed out. Try again.".into()),
            Action::Enable | Action::Disable | Action::AddDevice => Outcome::Error(
                "The remote control operation timed out. Its outcome is unknown; reopen /rc to check the state.".into(),
            ),
        })
}

async fn verified_daemon_session(
    endpoint: &std::path::Path,
    backend_id: BackendId,
    updates_allowed: bool,
) -> Result<(AppServerSession, std::path::PathBuf), String> {
    let status = codex_app_server_daemon::run(codex_app_server_daemon::LifecycleCommand::Version)
        .await
        .map_err(|_| {
            "Codex could not verify that it manages the local background server.".to_string()
        })?;
    let Some(protected) =
        codex_app_server_client::verified_local_daemon_socket(&status.socket_path)
            .ok()
            .flatten()
    else {
        return Err("Codex could not verify that it manages the local background server.".into());
    };
    if status.backend.is_none() || endpoint != status.socket_path && endpoint != protected.as_path()
    {
        return Err("Codex could not verify that it manages the local background server.".into());
    }
    let target = crate::AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: protected,
        },
        allow_embedded_fallback: false,
    };
    let client = crate::app_server_connection::connect(&target)
        .await
        .map_err(|err| {
            format!("Could not connect to the verified local background server: {err}")
        })?;
    let session = AppServerSession::new(
        backend_id,
        client,
        crate::app_server_session::ThreadParamsMode::Embedded,
    );
    let supported = session
        .server_version()
        .and_then(|v| semver::Version::parse(v).ok())
        .is_some_and(|v| {
            v == semver::Version::new(0, 0, 0)
                || semver::Version::parse("0.163.0-alpha.2").is_ok_and(|minimum| v >= minimum)
        });
    if !supported {
        return Err(if updates_allowed {
            "Update the background server with /daemon to use persistent remote control."
        } else {
            ADMIN_UPDATE_GUIDANCE
        }
        .into());
    }
    let home = status
        .socket_path
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or_else(|| "Could not locate background server settings.".to_string())?;
    Ok((session, home.join("app-server-daemon/settings.json")))
}

async fn run(mut session: AppServerSession, action: Action) -> Outcome {
    let request_id = session.next_request_id();
    let request = match action {
        Action::Menu => ClientRequest::RemoteControlStatusRead {
            request_id,
            params: None,
        },
        Action::Enable => ClientRequest::RemoteControlEnable {
            request_id,
            params: None,
        },
        Action::Disable => ClientRequest::RemoteControlDisable {
            request_id,
            params: None,
        },
        Action::AddDevice => ClientRequest::RemoteControlPairingStart {
            request_id,
            params: RemoteControlPairingStartParams { manual_code: true },
        },
    };
    let handle = session.request_handle();
    if matches!(action, Action::AddDevice) {
        return match handle
            .request_typed::<RemoteControlPairingStartResponse>(request)
            .await
        {
            Ok(response) => response.manual_pairing_code.map_or_else(
                || Outcome::Error("No manual pairing code was returned.".into()),
                Outcome::Pairing,
            ),
            Err(_) => Outcome::Error("Could not add device. Try again.".into()),
        };
    }
    #[derive(serde::Deserialize)]
    struct Status {
        status: RemoteControlConnectionStatus,
    }
    let status = handle.request_typed::<Status>(request).await;
    match (action, status) {
        (Action::Menu | Action::Enable, Ok(response)) => Outcome::Menu(response.status, None),
        (Action::Disable, Ok(_)) => Outcome::Info(
            "Remote control is disabled for local sessions. Cloud sessions are unaffected.".into(),
        ),
        (Action::Menu, Err(codex_app_server_client::TypedRequestError::Server { source, .. }))
            if source.message == "remote control is disabled by managed requirements" =>
        {
            Outcome::Unavailable("Remote control is disabled by managed requirements.".into())
        }
        (Action::Menu, Err(_)) => Outcome::Error("Could not read remote control status.".into()),
        (Action::Enable, Err(err)) => Outcome::Error(mutation_error("enabling", err)),
        (Action::Disable, Err(err)) => Outcome::Error(mutation_error("disabling", err)),
        (Action::AddDevice, _) => unreachable!(),
    }
}

fn mutation_error(operation: &str, err: codex_app_server_client::TypedRequestError) -> String {
    if let codex_app_server_client::TypedRequestError::Server { source, .. } = err {
        match source.message.as_str() {
            "remote control requires ChatGPT authentication"
            | "remote control requires ChatGPT authentication; API key auth is not supported" => {
                return "Sign in with ChatGPT to manage remote control.".into();
            }
            "remote control is disabled by managed requirements" => {
                return "Remote control is disabled by managed requirements.".into();
            }
            _ => {}
        }
    }
    format!(
        "Could not complete {operation} remote control. The running daemon may already have changed; reopen /rc to check its state."
    )
}

#[cfg(test)]
#[path = "remote_control_tests.rs"]
mod tests;
