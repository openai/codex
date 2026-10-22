//! Cancel remote tool work when its local response future is dropped.

use rmcp::Peer;
use rmcp::RoleClient;
use rmcp::model::CallToolRequest;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientRequest;
use rmcp::model::RequestId;
use rmcp::model::ServerResult;
use rmcp::service::PeerRequestOptions;
use rmcp::service::ServiceError;

pub(crate) async fn call_tool(
    peer: &Peer<RoleClient>,
    params: CallToolRequestParams,
    options: PeerRequestOptions,
) -> Result<(RequestId, ServerResult), ServiceError> {
    let handle = peer
        .send_request_with_option(
            ClientRequest::CallToolRequest(CallToolRequest::new(params)),
            options,
        )
        .await?;
    let id = handle.id.clone();
    let mut cancellation = CancelOnDrop(Some((peer.clone(), id.clone())));
    let result = handle.await_response().await;
    cancellation.0 = None;
    result.map(|result| (id, result))
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
                        Some("tool call response future dropped".into()),
                    ))
                    .await;
            });
        }
    }
}
