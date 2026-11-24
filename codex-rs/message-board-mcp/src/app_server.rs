//! Optional local app-server connection for best-effort board notifications.
//! Requests share the client's reply routing without holding the connection-cache lock.
//! Disconnected or cancelled transports are replaced for new notices, never replayed.

use anyhow::Context;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::MESSAGE_BOARD_CLIENT_NAME;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_remote_client::AppServerEvent;
use codex_app_server_remote_client::RemoteAppServerClient;
use codex_app_server_remote_client::RemoteAppServerConnectArgs;
use codex_app_server_remote_client::RemoteAppServerEndpoint;
use codex_app_server_remote_client::RemoteAppServerRequestHandle;
use codex_message_board::NotificationDelivery;
use codex_message_board::PostPreview;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::InterAgentCommunication;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use url::Url;

#[cfg(test)]
#[path = "app_server_tests.rs"]
mod tests;

struct Connection {
    requests: RemoteAppServerRequestHandle,
    closed: CancellationToken,
    _events: AbortOnDropHandle<()>,
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared local app-server connection for bounded, queue-only board notifications.
/// The caller must validate the recipient's membership before delivering a notice.
pub struct AppServerNotifications {
    url: Url,
    connection: Mutex<Arc<OnceCell<Connection>>>,
}

impl AppServerNotifications {
    /// Connects to a loopback WebSocket endpoint using the shared app-server client.
    pub async fn connect(url: Url) -> anyhow::Result<Self> {
        anyhow::ensure!(
            url.scheme() == "ws"
                && match url.host() {
                    Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                    Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                    Some(url::Host::Domain(host)) => host == "localhost",
                    None => false,
                }
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "--app-server requires a loopback ws:// URL without credentials"
        );
        let connection = timeout(REQUEST_TIMEOUT, connect(&url))
            .await
            .context("app-server initialization timed out")??;
        Ok(Self {
            url,
            connection: Mutex::new(Arc::new(OnceCell::from(connection))),
        })
    }

    /// Delivers the existing post preview through `thread/inject_items`.
    /// Notices retain the 1,024-byte budget and are never replayed after failure.
    pub async fn notify(
        &self,
        recipient: ThreadId,
        path: AgentPath,
        post: PostPreview,
    ) -> codex_protocol::error::Result<NotificationDelivery> {
        let notice = InterAgentCommunication::new(
            post.metadata.author.clone(),
            path,
            Vec::new(),
            post.notification_text(),
            /*trigger_turn*/ false,
        );
        self.inject(recipient, notice)
            .await
            .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?;
        Ok(NotificationDelivery::Accepted)
    }

    async fn inject(
        &self,
        recipient: ThreadId,
        notice: InterAgentCommunication,
    ) -> anyhow::Result<()> {
        // Match the existing remote-board limit, including both routing paths.
        anyhow::ensure!(
            notice.content.len() + notice.author.as_str().len() + notice.recipient.as_str().len()
                <= 1024,
            "remote board notice exceeds the context budget"
        );
        timeout(REQUEST_TIMEOUT, async {
            let slot = {
                let mut slot = self
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if slot
                    .get()
                    .is_some_and(|connection| connection.closed.is_cancelled())
                {
                    *slot = Arc::new(OnceCell::new());
                }
                Arc::clone(&slot)
            };
            // Coalesce reconnects without serializing requests on a healthy client.
            let connection = slot.get_or_try_init(|| connect(&self.url)).await?;
            let failed = connection.closed.clone().drop_guard();
            let response = connection
                .requests
                .request(ClientRequest::ThreadInjectItems {
                    request_id: RequestId::String(uuid::Uuid::new_v4().to_string()),
                    params: ThreadInjectItemsParams {
                        thread_id: recipient.to_string(),
                        items: vec![serde_json::to_value(notice.to_model_input_item())?],
                    },
                })
                .await
                .map_err(|_| anyhow::anyhow!("app-server notification connection failed"))?;
            // A remote rejection still has a matched reply and a usable transport.
            failed.disarm();
            response.map_err(|error| {
                anyhow::anyhow!("app-server rejected request (code {})", error.code)
            })?;
            Ok(())
        })
        .await
        .context("app-server notification timed out")?
    }
}

async fn connect(url: &Url) -> anyhow::Result<Connection> {
    let mut client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        required_human_approval: false,
        endpoint: RemoteAppServerEndpoint::WebSocket {
            websocket_url: url.to_string(),
            auth_token: None,
        },
        client_name: MESSAGE_BOARD_CLIENT_NAME.into(),
        client_version: env!("CARGO_PKG_VERSION").into(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: 1,
    })
    .await
    .map_err(|_| anyhow::anyhow!("failed to connect to the local app-server"))?;
    let requests = client.request_handle();
    let closed = CancellationToken::new();
    let disconnected = closed.clone();
    // Keep the shared client's event queue drained even between notices. The
    // connection owns this task; dropping it also releases the client handle.
    let events = AbortOnDropHandle::new(tokio::spawn(async move {
        while let Some(event) = tokio::select! {
            _ = disconnected.cancelled() => None,
            event = client.next_event() => event,
        } {
            if matches!(
                event,
                AppServerEvent::Disconnected { .. } | AppServerEvent::ServerRequest(_)
            ) {
                break;
            }
        }
        disconnected.cancel();
        let _ = client.shutdown().await;
    }));
    Ok(Connection {
        requests,
        closed,
        _events: events,
    })
}
