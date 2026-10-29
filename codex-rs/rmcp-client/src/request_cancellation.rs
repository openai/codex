//! Cancel remote MCP work when its local response future is dropped.

use std::future::Future;

use rmcp::Peer;
use rmcp::RoleClient;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientRequest;
use rmcp::model::RequestId;
use rmcp::model::ServerResult;
use rmcp::service::PeerRequestOptions;
use rmcp::service::ServiceError;
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

tokio::task_local! {
    static REQUEST_SCOPE: RequestScope;
}

/// Tracks cancellation writes for one chat's requests on a shared stdio transport.
#[derive(Clone)]
pub struct RequestScope {
    runtime: Handle,
    cancellation: CancellationToken,
    pending: TaskTracker,
}

impl RequestScope {
    pub fn new(runtime: Handle, cancellation: CancellationToken) -> Self {
        Self {
            runtime,
            cancellation,
            pending: TaskTracker::new(),
        }
    }

    async fn send(
        &self,
        peer: Peer<RoleClient>,
        request: ClientRequest,
        options: PeerRequestOptions,
    ) -> Result<ServerResult, ServiceError> {
        let cancellation = self.cancellation.child_token();
        let _cancel_on_drop = cancellation.clone().drop_guard();
        // The transport must make progress even if the caller stops polling its future.
        // Register before the task checks cancellation so a racing release observes it.
        let send = async move {
            let handle = tokio::select! {
                biased;
                () = cancellation.cancelled() => return Err(ServiceError::Cancelled { reason: None }),
                result = peer.send_request_with_option(request, options) => result?,
            };
            let id = handle.id.clone();
            tokio::select! {
                biased;
                result = handle.await_response() => result,
                () = cancellation.cancelled() => {
                    notify_cancelled(peer, id).await;
                    Err(ServiceError::Cancelled { reason: None })
                }
            }
        };
        self.pending
            .spawn_on(send, &self.runtime)
            .await
            .map_err(|_| ServiceError::TransportClosed)?
    }

    /// Include every wire request made by this operation, including MCP continuations.
    pub async fn scope<F: Future>(&self, operation: F) -> F::Output {
        REQUEST_SCOPE.scope(self.clone(), operation).await
    }

    /// Stop admitting requests and wait for the transport to finish writing their cancellations.
    pub async fn cancel_and_wait(&self) {
        self.cancellation.cancel();
        self.pending.close();
        self.pending.wait().await;
    }
}

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
        if let Ok(scope) = REQUEST_SCOPE.try_with(Clone::clone) {
            return scope.send(peer.clone(), request, options).await;
        }
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
            runtime.spawn(notify_cancelled(peer, id));
        }
    }
}

async fn notify_cancelled(peer: Peer<RoleClient>, id: RequestId) {
    let _ = peer
        .notify_cancelled(CancelledNotificationParam::new(
            Some(id),
            Some("MCP request cancelled by caller".into()),
        ))
        .await;
}
