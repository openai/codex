//! Channel-backed cloud client. Requests and retained replies stay on one owner;
//! events are drained independently so a foreground request cannot block its response.

use crate::AppServerEvent;
use crate::RequestResult;
use crate::TypedRequestError;
use crate::cloud_backend::CloudTitleClient;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStartParams;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Rejection from the original network policy during Cloud recovery.
pub const CLOUD_RECOVERY_NETWORK_BLOCKED: &str =
    "The current network policy blocks cloud reconnection.";

pub(super) const MAX_RECOVERY_BACKOFF: Duration = Duration::from_secs(30);

/// Operations the cloud worker must handle without forwarding to another backend.
/// Unsupported operations must return an explicit error; they must not claim success.
pub enum CloudOperation {
    Request(Box<ClientRequest>),
    /// Opens a provisional attachment and serves its initial history on a scoped channel.
    PrepareThreadResume {
        request_id: RequestId,
        params: Box<codex_app_server_protocol::ThreadResumeParams>,
        history: mpsc::Receiver<CloudCommand>,
    },
    /// Sent on the scoped channel after the caller has prepared the replacement view.
    CommitPreparedResume,
    /// Environment selection stays on the private transport, outside the app-server wire.
    StartThreadWithEnvironment {
        request_id: RequestId,
        params: Box<codex_app_server_protocol::ThreadStartParams>,
        environment: codex_cloud_client::EnvironmentSelection,
    },
    SubmitUserMessage(Box<TurnStartParams>),
    /// Apply the result of a name write that already succeeded outside the command queue.
    ThreadNameSaved(ThreadSetNameParams),
    ResumeDisconnectedThread {
        thread_id: String,
        require_history: bool,
        timeout: Duration,
        dispatched: oneshot::Sender<()>,
        release: oneshot::Receiver<oneshot::Sender<io::Result<()>>>,
    },
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

/// Allows the worker to install a held stream after the caller reads its history.
/// Dropping the token cancels that replacement.
pub struct CloudRecoveryRelease {
    release: oneshot::Sender<oneshot::Sender<io::Result<()>>>,
    timeout: Duration,
}

impl CloudRecoveryRelease {
    pub async fn install(self) -> io::Result<()> {
        let (ack, mut installed) = oneshot::channel();
        let closed = || io::Error::new(io::ErrorKind::BrokenPipe, "Cloud reconnection expired.");
        self.release.send(ack).map_err(|_| closed())?;
        let response = match tokio::time::timeout(self.timeout, &mut installed).await {
            Ok(result) => return result.map_err(|_| closed())?,
            Err(_) => {
                installed.close();
                installed.try_recv().ok()
            }
        };
        response.unwrap_or_else(|| Err(io::ErrorKind::TimedOut.into()))
    }
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
    pub(super) attached: Arc<AtomicBool>,
    commands: mpsc::Sender<CloudCommand>,
    native: Option<Arc<codex_cloud_client::Client>>,
    title_client: Option<Arc<CloudTitleClient>>,
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
        Self::channel_with_native(capacity, /*native*/ None, /*title_client*/ None)
    }

    pub(super) fn channel_with_native(
        capacity: usize,
        native: Option<codex_cloud_client::Client>,
        title_client: Option<CloudTitleClient>,
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
                    attached: Default::default(),
                    native: native.map(Arc::new),
                    title_client: title_client.map(Arc::new),
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
    /// Whether the worker has an installed reply stream that can deliver inventory updates.
    pub fn has_attachment(&self) -> bool {
        self.attached.load(Ordering::Relaxed) && !self.commands.is_closed()
    }

    /// Generate metadata with Cloud's own account, outside its conversation command queue.
    pub async fn generate_thread_title(
        &self,
        model: &str,
        effort: Option<codex_protocol::openai_models::ReasoningEffort>,
        prompt: String,
        schema: serde_json::Value,
    ) -> color_eyre::Result<String> {
        let client = self
            .title_client
            .as_ref()
            .ok_or_else(|| io::Error::other("Cloud title generation is unavailable"))?;
        client.generate(model, effort, prompt, schema).await
    }

    /// Read the saved title even when another Cloud conversation owns the attachment.
    pub async fn read_native_thread_name(
        &self,
        thread_id: &str,
    ) -> Option<Result<Option<String>, codex_cloud_client::Error>> {
        Some(self.native.as_ref()?.read_thread_name(thread_id).await)
    }

    /// Save an automatic name without occupying the conversation command queue.
    pub async fn save_native_thread_name(
        &self,
        params: ThreadSetNameParams,
    ) -> Option<color_eyre::Result<()>> {
        let client = self.native.as_ref()?;
        Some(
            async {
                client
                    .set_thread_name(&params.thread_id, &params.name)
                    .await?;
                self.unit(CloudOperation::ThreadNameSaved(params)).await?;
                Ok(())
            }
            .await,
        )
    }

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

    pub async fn resume_disconnected_thread(
        &self,
        thread_id: String,
        require_history: bool,
        timeout: Duration,
    ) -> Result<(Option<ThreadResumeResponse>, CloudRecoveryRelease), TypedRequestError> {
        let (release, held) = oneshot::channel();
        let (dispatched, dispatch) = oneshot::channel();
        let request = self.send(CloudOperation::ResumeDisconnectedThread {
            thread_id,
            require_history,
            timeout,
            dispatched,
            release: held,
        });
        // Bound time in the queue separately so it cannot eat into the
        // worker's backoff and Attach budget. Prefer a ready response on expiry.
        let watchdog = async {
            if tokio::time::timeout(timeout, dispatch).await.is_ok() {
                tokio::time::sleep(timeout.saturating_add(MAX_RECOVERY_BACKOFF)).await;
            }
        };
        let response = tokio::select! {
            biased;
            result = request => result,
            () = watchdog => Err(io::ErrorKind::TimedOut.into()),
        };
        let settings: Option<ThreadResumeResponse> =
            crate::decode_typed_response("thread/resume", response)?;
        Ok((settings, CloudRecoveryRelease { release, timeout }))
    }

    /// Whether a completed model read still belongs to the bound Cloud account.
    pub fn model_catalog_is_current(&self) -> bool {
        self.native
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

    /// Discover saved environments using this Cloud connection's authenticated account.
    pub async fn list_saved_environments(
        &self,
    ) -> Result<Vec<codex_cloud_client::SavedEnvironment>, codex_cloud_client::Error> {
        let catalog = self
            .native
            .as_ref()
            .ok_or(codex_cloud_client::Error::InvalidCredentials)?;
        let environments = catalog.list_saved_environments().await?;
        if !catalog.model_catalog_is_current() {
            return Err(codex_cloud_client::Error::InvalidCredentials);
        }
        Ok(environments)
    }

    pub(crate) async fn start_thread_with_environment(
        &self,
        request_id: RequestId,
        params: codex_app_server_protocol::ThreadStartParams,
        environment: codex_cloud_client::EnvironmentSelection,
    ) -> io::Result<RequestResult> {
        self.send(CloudOperation::StartThreadWithEnvironment {
            request_id,
            params: Box::new(params),
            environment,
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

    /// Returns the activation snapshot and a handle restricted to this resume's history.
    /// Dropping the scoped handle without committing discards the new attachment.
    pub async fn prepare_thread_resume(
        &self,
        request_id: RequestId,
        params: codex_app_server_protocol::ThreadResumeParams,
    ) -> Result<(codex_app_server_protocol::ThreadResumeResponse, Self), TypedRequestError> {
        let (commands, history) = mpsc::channel(/*capacity*/ 1);
        let response = self
            .send(CloudOperation::PrepareThreadResume {
                request_id,
                params: Box::new(params),
                history,
            })
            .await;
        let snapshot = crate::decode_typed_response("thread/resume", response)?;
        Ok((
            snapshot,
            Self {
                commands,
                reconnect: self.reconnect.clone(),
                attached: self.attached.clone(),
                native: None,
                title_client: None,
            },
        ))
    }

    /// Installs the prepared attachment on the worker before its next ordinary command.
    pub async fn commit_prepared_resume(&self) -> Result<(), TypedRequestError> {
        let response = self.send(CloudOperation::CommitPreparedResume).await;
        crate::decode_typed_response("thread/resume", response)
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
        if let (Some(catalog), ClientRequest::ModelList { params, .. }) = (&self.native, &request) {
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
