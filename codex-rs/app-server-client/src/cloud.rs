//! Channel-backed cloud client. Requests and retained replies stay on one owner;
//! events are drained independently so a foreground request cannot block its response.

use crate::AppServerEvent;
use crate::RequestResult;
use crate::TypedRequestError;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStartParams;
use std::io;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Operations the cloud worker must handle without forwarding to another backend.
/// Unsupported operations must return an explicit error; they must not claim success.
pub enum CloudOperation {
    Request(Box<ClientRequest>),
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

#[derive(Clone)]
pub struct CloudRequestHandle {
    commands: mpsc::Sender<CloudCommand>,
}

pub struct CloudClient {
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
        let (commands, receiver) = mpsc::channel(capacity.max(1));
        let (events, event_receiver) = mpsc::unbounded_channel();
        (
            Self {
                handle: CloudRequestHandle { commands },
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
        self.events.recv().await
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
