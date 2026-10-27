//! Applies the selected server's socket policy on initial connections and reconnects.

use crate::AppServerTarget;
#[cfg(windows)]
use crate::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use crate::connect_remote_app_server;
use codex_app_server_client::AppServerClient;
#[cfg(windows)]
use codex_app_server_client::RemoteAppServerClient;
#[cfg(windows)]
use codex_app_server_client::RemoteAppServerConnectArgs;
#[cfg(windows)]
use codex_app_server_client::RemoteAppServerEndpoint;
#[cfg(windows)]
use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) async fn connect(target: &AppServerTarget) -> color_eyre::Result<AppServerClient> {
    match target {
        AppServerTarget::Embedded => {
            color_eyre::eyre::bail!("embedded sessions have no remote connection")
        }
        #[cfg(windows)]
        AppServerTarget::LocalDaemon {
            endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
            ..
        } => {
            // Revalidate at the real connection, not just the earlier discovery probe.
            let (socket_path, _directory) =
                codex_uds::validate_private_socket_path(socket_path.as_path())?;
            let app_server =
                RemoteAppServerClient::connect_local_daemon(RemoteAppServerConnectArgs {
                    endpoint: RemoteAppServerEndpoint::UnixSocket {
                        socket_path: AbsolutePathBuf::from_absolute_path_checked(socket_path)?,
                    },
                    client_name: "codex-tui".to_string(),
                    client_version: env!("CARGO_PKG_VERSION").to_string(),
                    experimental_api: true,
                    mcp_server_openai_form_elicitation: false,
                    opt_out_notification_methods: Vec::new(),
                    channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
                })
                .await?;
            Ok(AppServerClient::Remote(app_server))
        }
        AppServerTarget::LocalDaemon { endpoint, .. } | AppServerTarget::Remote { endpoint } => {
            connect_remote_app_server(endpoint.clone()).await
        }
    }
}
