use crate::AppServerTarget;
use crate::RemoteAppServerEndpoint;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteConnectionStatus {
    pub(crate) address: String,
    pub(crate) version: String,
}

pub(crate) fn remote_connection_status_value(
    app_server_target: &AppServerTarget,
    server_version: Option<&str>,
) -> Option<RemoteConnectionStatus> {
    let endpoint = match app_server_target {
        AppServerTarget::Embedded => return None,
        AppServerTarget::Remote { endpoint } => endpoint,
    };
    let address = match endpoint {
        RemoteAppServerEndpoint::WebSocket { websocket_url, .. } => {
            sanitized_websocket_display_address(websocket_url)
                .unwrap_or_else(|| "<invalid websocket URL>".to_string())
        }
        RemoteAppServerEndpoint::UnixSocket { socket_path } => {
            format!("unix://{}", socket_path.display())
        }
    };
    let version = server_version
        .map(|version| format!("v{version}"))
        .unwrap_or_else(|| "unknown".to_string());
    Some(RemoteConnectionStatus { address, version })
}

fn sanitized_websocket_display_address(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;

    #[test]
    fn remote_connection_status_value_formats_display_value() -> color_eyre::Result<()> {
        assert_eq!(
            remote_connection_status_value(&AppServerTarget::Embedded, Some("1.2.3")),
            None
        );

        let websocket_target = AppServerTarget::Remote {
            endpoint: RemoteAppServerEndpoint::WebSocket {
                websocket_url: "ws://user:secret@127.0.0.1:4500/?token=abc#frag".to_string(),
                auth_token: Some("abc".to_string()),
            },
        };
        assert_eq!(
            remote_connection_status_value(&websocket_target, Some("1.2.3")),
            Some(RemoteConnectionStatus {
                address: "ws://127.0.0.1:4500/".to_string(),
                version: "v1.2.3".to_string(),
            })
        );

        let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
        let unix_socket_target = AppServerTarget::Remote {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: socket_path.clone(),
            },
        };
        assert_eq!(
            remote_connection_status_value(&unix_socket_target, /*server_version*/ None),
            Some(RemoteConnectionStatus {
                address: format!("unix://{}", socket_path.display()),
                version: "unknown".to_string(),
            })
        );
        Ok(())
    }
}
