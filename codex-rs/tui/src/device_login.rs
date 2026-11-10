//! Handles gateway device login during TUI startup.
use crate::app_server_session::AppServerSession;
use crate::app_server_session::is_method_not_found;
use crate::legacy_core::config::Config;
use crate::resume_picker::SessionSelection;
use codex_app_server_client::AppServerEvent;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GatewayOAuthCancelResponse;
use codex_app_server_protocol::GatewayOAuthLoginParams;
use codex_app_server_protocol::GatewayOAuthLoginResponse;
use codex_app_server_protocol::GatewayOAuthPrompt;
use codex_app_server_protocol::GatewayOAuthReadParams;
use codex_app_server_protocol::GatewayOAuthReadResponse;
use codex_app_server_protocol::GatewayOAuthStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadStatus;
use std::io;

pub(crate) async fn prefer_device_login() -> bool {
    if crate::clipboard_copy::is_ssh_session()
        || cfg!(target_os = "linux")
            && !["DISPLAY", "WAYLAND_DISPLAY"]
                .iter()
                .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
    {
        return true;
    }
    #[cfg(not(windows))]
    let browser_available = webbrowser::Browser::is_available;
    #[cfg(windows)]
    let browser_available = has_http_handler;
    !tokio::task::spawn_blocking(browser_available)
        .await
        .unwrap_or(false)
}

#[cfg(windows)]
fn has_http_handler() -> bool {
    use windows_sys::Win32::Foundation::S_FALSE;
    use windows_sys::Win32::UI::Shell::ASSOCF_IS_PROTOCOL;
    use windows_sys::Win32::UI::Shell::ASSOCSTR_COMMAND;
    use windows_sys::Win32::UI::Shell::AssocQueryStringW;

    let mut chars = 0;
    // A null output buffer asks Windows for the size of the registered HTTP command.
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_COMMAND,
            windows_sys::w!("http"),
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut chars,
        )
    };
    result == S_FALSE && chars > 1
}

pub(crate) async fn needs_device_login(
    state: &GatewayOAuthReadResponse,
    prefer_device: impl Future<Output = bool>,
) -> bool {
    state.required
        && state.supports_device_auth
        && matches!(
            state.status,
            Some(GatewayOAuthStatus::NotReady | GatewayOAuthStatus::Failed)
        )
        && prefer_device.await
}

/// Starts device-code login if this session will use a gateway that needs sign-in.
///
/// `server` reports whether its configured gateway supports device code and checks
/// for a usable credential. `config` provides the model provider settings for this
/// launch. When `selection` resumes a thread, we also check that thread's provider.
///
/// Returns the URL and one-time code to display, or `None` when startup can continue
/// without a prompt, including when the server refreshes a saved credential.
pub(crate) async fn start_if_needed(
    server: &mut AppServerSession,
    selection: &SessionSelection,
    config: &Config,
    prefer_device: impl Future<Output = bool>,
) -> io::Result<Option<GatewayOAuthPrompt>> {
    let mut selected_provider = server.explicit_model_provider(config);
    if let SessionSelection::Resume(target) = selection {
        let thread = server
            .thread_read(target.thread_id, /*include_turns*/ false)
            .await
            .map_err(io::Error::other)?;
        // Resuming a thread still in memory reattaches to it with its current provider.
        // An override can take effect when the server loads the thread from disk.
        if selected_provider.is_none() || thread.status != ThreadStatus::NotLoaded {
            selected_provider = Some(thread.model_provider);
        }
    }
    let handle = server.request_handle();
    let state: GatewayOAuthReadResponse = match handle
        .request_typed(ClientRequest::GatewayOAuthRead {
            request_id: server.next_request_id(),
            params: selected_provider.as_ref().map(|id| GatewayOAuthReadParams {
                model_provider: Some(id.clone()),
            }),
        })
        .await
    {
        Ok(state) => state,
        Err(err) if is_method_not_found(&err) => return Ok(None),
        Err(error) => {
            if let codex_app_server_client::TypedRequestError::Server { source, .. } = &error
                && selected_provider.is_some()
                && source.code == crate::app_server_session::JSONRPC_INVALID_REQUEST
                && source.data.is_none()
            {
                // Servers with device login always include supportsDeviceAuth, even when false.
                // Ask without a provider to recognize older servers that reject named reads.
                let probe = handle
                    .request_typed::<serde_json::Value>(ClientRequest::GatewayOAuthRead {
                        request_id: server.next_request_id(),
                        params: None,
                    })
                    .await;
                if let Ok(serde_json::Value::Object(state)) = probe
                    && state.contains_key("providerId")
                    && !state.contains_key("supportsDeviceAuth")
                {
                    return Ok(None);
                }
            }
            return Err(io::Error::other(error));
        }
    };
    if selected_provider.is_some_and(|id| id != state.provider_id)
        || !needs_device_login(&state, prefer_device).await
    {
        return Ok(None);
    }
    let response: GatewayOAuthLoginResponse = handle
        .request_typed(ClientRequest::GatewayOAuthLogin {
            request_id: server.next_request_id(),
            params: Some(GatewayOAuthLoginParams {
                device_auth: true,
                model_provider: Some(state.provider_id),
            }),
        })
        .await
        .map_err(io::Error::other)?;
    Ok(response.prompt)
}

pub(crate) async fn cancel(server: &mut AppServerSession) {
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        server
            .request_handle()
            .request_typed::<GatewayOAuthCancelResponse>(ClientRequest::GatewayOAuthCancel {
                request_id: server.next_request_id(),
                params: None,
            }),
    )
    .await;
}

/// Handles one app-server `event` while the device code is on screen.
///
/// `prompt.login_id` identifies the attempt we're waiting for. Returns `Ok(true)`
/// when that attempt succeeds, `Ok(false)` to keep waiting, or an error if sign-in
/// fails or we lose the result. We use the `origin` request handle to reject
/// incoming requests the sign-in screen cannot answer.
pub(crate) async fn handle_event_during_device_login(
    prompt: &GatewayOAuthPrompt,
    event: AppServerEvent,
    origin: &AppServerRequestHandle,
) -> io::Result<bool> {
    if let AppServerEvent::ServerNotification(notification) = &event
        && let ServerNotification::GatewayOAuthChanged(change) = notification.as_ref()
        && change.login_id.as_deref() == Some(prompt.login_id.as_str())
    {
        return match change.status {
            GatewayOAuthStatus::Succeeded => Ok(true),
            GatewayOAuthStatus::Failed | GatewayOAuthStatus::NotReady => Err(io::Error::other(
                change.error.as_deref().unwrap_or("Gateway sign-in failed"),
            )),
            GatewayOAuthStatus::Started => Ok(false),
        };
    }
    match event {
        AppServerEvent::Disconnected { message } => Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            color_eyre::eyre::eyre!(message).wrap_err(crate::onboarding::SETUP_CONNECTION_LOST),
        )),
        AppServerEvent::Lagged { .. } => Err(io::Error::other(
            "Lost the gateway sign-in result. Restart Codex to check the connection.",
        )),
        AppServerEvent::ServerRequest(request) => {
            origin
                .server_request_reply_handle(request.id().clone())
                .reject(codex_app_server_protocol::JSONRPCErrorError {
                    code: -32000,
                    message: "Codex cannot handle this request during gateway sign-in".into(),
                    data: None,
                })
                .await?;
            Ok(false)
        }
        AppServerEvent::ServerNotification(_) | AppServerEvent::ThreadStreamDisconnected(_) => {
            Ok(false)
        }
    }
}

#[cfg(test)]
#[path = "device_login_tests.rs"]
mod tests;
