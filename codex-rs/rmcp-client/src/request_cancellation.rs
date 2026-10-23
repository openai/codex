//! Cancel remote MCP work when its local response future is dropped.

use rmcp::Peer;
use rmcp::RoleClient;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientRequest;
use rmcp::model::RequestId;
use rmcp::model::ServerResult;
use rmcp::service::PeerRequestOptions;
use rmcp::service::ServiceError;

/// Whether dropping a caller's future should cancel its request on the peer.
#[derive(Clone, Copy)]
pub(crate) enum RequestCancellation {
    Disabled,
    OnDrop,
}

impl RequestCancellation {
    /// Send a request and tie its remote lifetime to the caller when requested.
    pub(crate) async fn send(
        self,
        peer: &Peer<RoleClient>,
        request: ClientRequest,
        options: PeerRequestOptions,
    ) -> Result<ServerResult, ServiceError> {
        let handle = peer.send_request_with_option(request, options).await?;
        let mut cancellation = CancelOnDrop(match self {
            Self::Disabled => None,
            Self::OnDrop => Some((peer.clone(), handle.id.clone())),
        });
        let result = handle.await_response().await;
        cancellation.0 = None;
        result
    }
}

struct CancelOnDrop(Option<(Peer<RoleClient>, RequestId)>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some((peer, id)) = self.0.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = peer
                    .notify_cancelled(CancelledNotificationParam::new(
                        Some(id),
                        Some("MCP response future dropped".into()),
                    ))
                    .await;
            });
        }
    }
}
