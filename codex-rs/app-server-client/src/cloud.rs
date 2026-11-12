//! Channel-backed cloud client. Requests and retained replies stay on one owner;
//! events are drained independently so a foreground request cannot block its response.

use crate::AppServerEvent;
use crate::RequestResult;
use crate::TypedRequestError;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStartParams;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Operations the cloud worker must handle without forwarding to another backend.
/// Unsupported operations must return an explicit error; they must not claim success.
pub enum CloudOperation {
    Request(Box<ClientRequest>),
    /// Host project metadata stays on the private transport, outside the app-server wire.
    StartProjectThread {
        request_id: RequestId,
        params: Box<codex_app_server_protocol::ThreadStartParams>,
        project: codex_cloud_client::ProjectEnvironment,
    },
    SubmitUserMessage(Box<TurnStartParams>),
    Notify(ClientNotification),
    Resolve {
        request_id: RequestId,
        result: JsonRpcResult,
    },
    Reject {
        request_id: RequestId,
        error: JSONRPCErrorError,
    },
    Shutdown,
}

/// One server-decided admission, not completion. A duplicate carries identity
/// only; read history or observe the reply stream before inferring its status.
#[derive(serde::Serialize, serde::Deserialize)]
pub enum CloudUserMessageReceipt {
    Started(Turn),
    Steered(Turn),
    Duplicate { turn_id: Option<String> },
}

/// An operation and its single response. Dropping a caller does not undo admission;
/// cloud workers must not retry mutations when the receiver has gone away.
pub struct CloudCommand {
    pub operation: CloudOperation,
    pub response: oneshot::Sender<io::Result<RequestResult>>,
}

#[derive(Clone, Default)]
pub(super) enum IdleReconnect {
    #[default]
    Inactive,
    Offered(Arc<str>),
    Opening(Arc<str>),
    Connected(Arc<str>),
}

#[derive(Clone)]
pub struct CloudRequestHandle {
    pub(super) reconnect: tokio::sync::watch::Sender<IdleReconnect>,
    commands: mpsc::Sender<CloudCommand>,
    catalog: Option<Arc<codex_cloud_client::Client>>,
}

/// Tracks only losses queued for delivery. The worker remains the sole owner
/// of the active attachment. A successful replacement supersedes earlier losses
/// for that conversation, even if the replacement itself later ends.
#[derive(Clone, Default)]
pub(super) struct CloudStreamLosses(Arc<Mutex<HashMap<String, PendingLosses>>>);

#[derive(Default)]
struct PendingLosses {
    remaining: usize,
    superseded: usize,
}

impl CloudStreamLosses {
    fn pending(&self) -> MutexGuard<'_, HashMap<String, PendingLosses>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn record(&self, thread_id: &str) {
        self.pending()
            .entry(thread_id.to_owned())
            .or_default()
            .remaining += 1;
    }

    pub(super) fn supersede(&self, thread_id: &str) {
        if let Some(losses) = self.pending().get_mut(thread_id) {
            losses.superseded = losses.remaining;
        }
    }

    fn should_deliver(&self, thread_id: &str) -> bool {
        let mut pending = self.pending();
        let Some(losses) = pending.get_mut(thread_id) else {
            return true;
        };
        // A single worker sends losses in the same order as it records them.
        let deliver = losses.superseded == 0;
        losses.superseded = losses.superseded.saturating_sub(1);
        losses.remaining -= 1;
        if losses.remaining == 0 {
            pending.remove(thread_id);
        }
        deliver
    }
}

pub struct CloudClient {
    pub(super) stream_losses: CloudStreamLosses,
    handle: CloudRequestHandle,
    events: mpsc::UnboundedReceiver<AppServerEvent>,
}

impl CloudClient {
    /// Creates a cloud transport. The owner must drain commands and stop when
    /// its receiver closes. Event delivery is independent of request completion,
    /// matching conventional transports; cloud workers must bound remote payloads.
    pub fn channel(
        capacity: usize,
    ) -> (
        Self,
        mpsc::Receiver<CloudCommand>,
        mpsc::UnboundedSender<AppServerEvent>,
    ) {
        Self::channel_with_catalog(capacity, /*catalog*/ None)
    }

    pub(super) fn channel_with_catalog(
        capacity: usize,
        catalog: Option<codex_cloud_client::Client>,
    ) -> (
        Self,
        mpsc::Receiver<CloudCommand>,
        mpsc::UnboundedSender<AppServerEvent>,
    ) {
        let (commands, receiver) = mpsc::channel(capacity.max(1));
        let (events, event_receiver) = mpsc::unbounded_channel();
        (
            Self {
                stream_losses: CloudStreamLosses::default(),
                handle: CloudRequestHandle {
                    commands,
                    reconnect: Default::default(),
                    catalog: catalog.map(Arc::new),
                },
                events: event_receiver,
            },
            receiver,
            events,
        )
    }

    pub(crate) fn request_handle(&self) -> CloudRequestHandle {
        self.handle.clone()
    }

    pub(crate) async fn next_event(&mut self) -> Option<AppServerEvent> {
        while let Some(event) = self.events.recv().await {
            if let AppServerEvent::ThreadStreamDisconnected(error) = &event
                && !self.stream_losses.should_deliver(&error.thread_id)
            {
                continue;
            }
            if let AppServerEvent::ThreadStreamReconnected(id) = &event
                && !self.handle.reconnect.send_if_modified(|state| {
                    if matches!(state, IdleReconnect::Connected(root) if root.as_ref() == id) {
                        *state = IdleReconnect::Inactive;
                        return true;
                    }
                    false
                })
            {
                continue;
            }
            return Some(event);
        }
        None
    }

    pub(crate) async fn request(&self, request: ClientRequest) -> io::Result<RequestResult> {
        self.handle.request(request).await
    }

    pub(crate) async fn notify(&self, notification: ClientNotification) -> io::Result<()> {
        self.handle.unit(CloudOperation::Notify(notification)).await
    }

    pub(crate) async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: JsonRpcResult,
    ) -> io::Result<()> {
        self.handle.resolve_server_request(request_id, result).await
    }

    pub(crate) async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> io::Result<()> {
        self.handle.reject_server_request(request_id, error).await
    }

    pub(crate) async fn shutdown(self) -> io::Result<()> {
        tokio::time::timeout(
            crate::SHUTDOWN_TIMEOUT,
            self.handle.unit(CloudOperation::Shutdown),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "cloud backend shutdown timed out"))?
    }
}

impl CloudRequestHandle {
    /// Accept a quiet Attach only for the conversation displayed by the caller.
    pub fn reconnect_idle_thread(&self, thread_id: &str) -> bool {
        self.reconnect.send_if_modified(|state| {
            if let IdleReconnect::Offered(root) = state
                && root.as_ref() == thread_id
            {
                *state = IdleReconnect::Opening(Arc::clone(root));
                return true;
            }
            false
        })
    }

    /// Whether a completed model read still belongs to the bound Cloud account.
    pub fn model_catalog_is_current(&self) -> bool {
        self.catalog
            .as_ref()
            .is_none_or(|catalog| catalog.model_catalog_is_current())
    }

    async fn model_list(
        catalog: &codex_cloud_client::Client,
        params: &ModelListParams,
    ) -> RequestResult {
        if params.model_provider.is_some() {
            return Err(JSONRPCErrorError {
                code: -32602,
                message: "Cloud model discovery uses the server's model provider.".into(),
                data: None,
            });
        }
        let result = async {
            let body = catalog
                .list_models(
                    params.cursor.as_deref(),
                    params.limit,
                    params.include_hidden,
                )
                .await
                .map_err(|error| error.to_string())?;
            if !catalog.model_catalog_is_current() {
                return Err("Cloud model catalog account changed.".into());
            }
            serde_json::from_slice(&body)
                .map_err(|_| "Invalid Cloud model catalog response.".into())
        }
        .await;
        result.map_err(|message| JSONRPCErrorError {
            code: -32000,
            message,
            data: None,
        })
    }

    pub(crate) async fn start_project_thread(
        &self,
        request_id: RequestId,
        params: codex_app_server_protocol::ThreadStartParams,
        project: codex_cloud_client::ProjectEnvironment,
    ) -> io::Result<RequestResult> {
        self.send(CloudOperation::StartProjectThread {
            request_id,
            params: Box::new(params),
            project,
        })
        .await
    }

    /// Submit once with a stable message ID and saved thread settings. The server
    /// starts or steers; an error can mean admission is unknown. Never auto-resend.
    pub async fn submit_user_message(
        &self,
        params: TurnStartParams,
    ) -> Result<CloudUserMessageReceipt, TypedRequestError> {
        let response = self
            .send(CloudOperation::SubmitUserMessage(Box::new(params)))
            .await;
        crate::decode_typed_response("cloud/submitUserMessage", response)
    }

    async fn send(&self, operation: CloudOperation) -> io::Result<RequestResult> {
        let (response, result) = oneshot::channel();
        self.commands
            .send(CloudCommand {
                operation,
                response,
            })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "cloud backend is closed"))?;
        result.await.map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "cloud backend dropped its response",
            )
        })?
    }

    async fn unit(&self, operation: CloudOperation) -> io::Result<()> {
        self.send(operation)
            .await?
            .map(|_| ())
            .map_err(|error| io::Error::other(error.message))
    }

    pub(crate) async fn request(&self, request: ClientRequest) -> io::Result<RequestResult> {
        // Catalog reads must not occupy the serialized send and attachment lane.
        if let (Some(catalog), ClientRequest::ModelList { params, .. }) = (&self.catalog, &request)
        {
            // Keep the catalog HTTP state out of the shared request future.
            return Ok(Box::pin(Self::model_list(catalog, params)).await);
        }
        self.send(CloudOperation::Request(Box::new(request))).await
    }

    pub(crate) async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: JsonRpcResult,
    ) -> io::Result<()> {
        self.unit(CloudOperation::Resolve { request_id, result })
            .await
    }

    pub(crate) async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> io::Result<()> {
        self.unit(CloudOperation::Reject { request_id, error })
            .await
    }
}

#[cfg(test)]
#[path = "cloud_tests.rs"]
mod tests;
