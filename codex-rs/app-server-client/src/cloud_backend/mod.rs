//! Adapts native Cloud requests and the currently open conversation’s reply stream.
//! A replacement becomes active only after its native attachment opens successfully.
//! Loaded-thread queries report reply attachments, not retained resume metadata.

mod auth;
mod events;
mod idle;
mod requests;
mod thread_title;

use crate::AppServerClient;
use crate::AppServerEvent;
use crate::CLOUD_RECOVERY_NETWORK_BLOCKED;
use crate::CloudClient;
use crate::CloudOperation;
use crate::cloud::CloudStreamLosses;
use codex_app_server_protocol as protocol;
use codex_cloud_client as cloud;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;

use color_eyre::eyre::eyre;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub use auth::connect;
pub(super) use thread_title::CloudTitleClient;

type ReplyStream = BoxStream<'static, Result<cloud::Event, cloud::Error>>;
type OpenReplyStream = (ReplyStream, Option<cloud::Event>);

/// Separate an immediate EOF from a quiet, successfully attached stream.
fn check_reply_stream(
    stream: &mut ReplyStream,
) -> Result<Option<cloud::Event>, Option<cloud::Error>> {
    match tokio::task::unconstrained(stream.next()).now_or_never() {
        Some(None) => Err(None),
        Some(Some(Err(error))) => Err(Some(error)),
        Some(Some(Ok(event))) => Ok(Some(event)),
        None => Ok(None),
    }
}

/// Rejected locally before an admission request can be sent.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct PreflightRejection(&'static str);

#[derive(Debug, thiserror::Error)]
#[error("Cloud stream is not connected. Reopen the conversation before sending.")]
struct MissingReplyAttachment;

/// Local rejection when recovery history no longer belongs to the bound sign-in.
/// This carries no credentials and never enters the app-server wire envelope.
#[derive(Debug, thiserror::Error)]
#[error("{}", idle::SIGN_IN_AGAIN)]
pub struct CloudRecoverySignInChanged;

struct Attachment {
    thread_id: String,
    stream: ReplyStream,
}

#[derive(Default)]
struct Replies {
    idle: Option<idle::Recovery>,
    active: Option<Attachment>,
    generation: u64,
    stream_losses: CloudStreamLosses,
    // Keep the last observed status through EOF until this conversation is unloaded.
    latest_status: Option<(String, cloud::ThreadStatus)>,
}

impl Replies {
    async fn next_event(&mut self, output: &mpsc::UnboundedSender<AppServerEvent>) {
        if let Some(recovery) = &mut self.idle
            && let Some((root, _, open)) = &mut recovery.held
        {
            let opened = open.await;
            let root = std::mem::take(root);
            recovery.held = None;
            if let Some(opened) = opened {
                if let Err(reason) = recovery.check_permit() {
                    let _ = opened.ack.send(Err(reason));
                    return;
                }
                let Ok((stream, first)) = check_prepared_stream((opened.stream, opened.first))
                else {
                    let _ = opened
                        .ack
                        .send(Err("Cloud reply stream disconnected again."));
                    return;
                };
                // Commit through the acknowledgement. If the caller timed out
                // and closed it first, leave the stream held and drop it here.
                if opened.ack.send(Ok(())).is_err() {
                    return;
                }
                recovery.attempts += 1;
                recovery.installed = Some(tokio::time::Instant::now());
                recovery.observed_idle = None;
                self.stream_losses.supersede(&root);
                self.install(&root, stream, first, output);
            }
        } else {
            self.forward_next(output).await;
        }
    }

    async fn forward_next(&mut self, output: &mpsc::UnboundedSender<AppServerEvent>) {
        if self.active.is_none()
            && let Some(recovery) = &mut self.idle
            && let Some((root, open)) = &mut recovery.pending
        {
            let opened = open.await;
            let root = Arc::clone(root);
            recovery.pending = None;
            match opened.and_then(|attachment| {
                recovery.login.permit.check().map_err(|_| None)?;
                Ok(attachment)
            }) {
                Ok((stream, first)) => {
                    recovery.installed = Some(tokio::time::Instant::now());
                    // A same-thread Attach can stay quiet. Keep the last observation until an
                    // event or a dispatched mutation changes recovery eligibility.
                    self.install(&root, stream, first, output);
                    if let Some(recovery) = &self.idle
                        && recovery.observed_idle != Some(false)
                    {
                        recovery.notify_connected(&root, output);
                    }
                }
                Err(reason) => {
                    recovery
                        .state
                        .send_replace(crate::cloud::IdleReconnect::Inactive);
                    self.report_loss(&root, reason, output);
                }
            }
            return;
        }
        let Some(active) = &mut self.active else {
            return std::future::pending().await;
        };
        match active.stream.next().await {
            Some(Ok(event)) => forward(
                &active.thread_id,
                event,
                output,
                &mut self.latest_status,
                self.idle.as_mut(),
            ),
            ended => {
                let id = active.thread_id.clone();
                let error = ended.and_then(Result::err);
                self.active = None;
                if let Some(recovery) = &mut self.idle {
                    recovery
                        .state
                        .send_replace(crate::cloud::IdleReconnect::Inactive);
                    let stable = recovery
                        .installed
                        .is_none_or(|start| start.elapsed() >= std::time::Duration::from_secs(15));
                    if stable {
                        recovery.attempts = 0;
                    }
                    if recovery.observed_idle == Some(true)
                        && stable
                        && idle::transient(error.as_ref())
                    {
                        recovery.offer(&id);
                    }
                }
                self.report_loss(&id, error.as_ref().and_then(idle::blocked_recovery), output);
            }
        }
    }

    fn report_loss(
        &self,
        id: &str,
        reason: Option<&str>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) {
        self.stream_losses.record(id);
        let _ = output.send(AppServerEvent::ThreadStreamDisconnected(Box::new(
            cloud_error(
                id,
                reason.unwrap_or("Cloud stream ended. Reopen the conversation to reconnect."),
            ),
        )));
    }

    async fn while_listening<T>(
        &mut self,
        future: impl Future<Output = T>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) -> T {
        tokio::pin!(future);
        loop {
            tokio::select! {
                biased;
                result = &mut future => return result,
                () = self.next_event(output) => {}
            }
        }
    }

    async fn replace(
        &mut self,
        id: &str,
        open: impl Future<Output = Result<ReplyStream, cloud::Error>>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) -> Result<()> {
        if let Some(opened) = self.open_attachment(id, open, output).await? {
            self.install_replacement(id, opened, output);
        }
        Ok(())
    }

    async fn open_attachment(
        &mut self,
        id: &str,
        open: impl Future<Output = Result<ReplyStream, cloud::Error>>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) -> Result<Option<OpenReplyStream>> {
        if self.admit_turn(id, output).is_ok() {
            return Ok(None);
        }
        let mut stream = self.while_listening(open, output).await?;
        let first = probe_reply_stream(&mut stream)?;
        Ok(Some((stream, first)))
    }

    fn install_replacement(
        &mut self,
        id: &str,
        (stream, first): OpenReplyStream,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) {
        if let Some(idle) = &mut self.idle {
            idle.reset();
        }
        self.stream_losses.supersede(id);
        self.latest_status = None;
        self.install(id, stream, first, output);
    }

    fn install(
        &mut self,
        id: &str,
        stream: ReplyStream,
        first_event: Option<cloud::Event>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) {
        self.generation = self.generation.wrapping_add(1);
        self.active = Some(Attachment {
            thread_id: id.to_owned(),
            stream,
        });
        if let Some(event) = first_event {
            forward(
                id,
                event,
                output,
                &mut self.latest_status,
                self.idle.as_mut(),
            );
        }
    }

    fn thread_status(&self, id: &str) -> Option<&cloud::ThreadStatus> {
        self.latest_status
            .as_ref()
            .filter(|(thread_id, _)| thread_id == id)
            .map(|(_, status)| status)
    }

    fn detach(&mut self, id: &str) {
        if let Some(idle) = &mut self.idle
            && (idle.held.as_ref().is_some_and(|(root, _, _)| root == id)
                || idle
                    .pending
                    .as_ref()
                    .is_some_and(|(root, _)| root.as_ref() == id)
                || self
                    .active
                    .as_ref()
                    .is_some_and(|active| active.thread_id == id))
        {
            idle.reset();
        }
        if self.thread_status(id).is_some() {
            self.latest_status = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.thread_id == id)
        {
            self.active = None;
        }
    }

    fn admit_turn(
        &mut self,
        id: &str,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) -> Result<(), MissingReplyAttachment> {
        // Notice a ready disconnect before sending. A continuous stream of events
        // must still allow commands to make progress.
        let _ = tokio::task::unconstrained(self.forward_next(output)).now_or_never();
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.thread_id != id)
        {
            return Err(MissingReplyAttachment);
        }
        Ok(())
    }
}

fn probe_reply_stream(stream: &mut ReplyStream) -> Result<Option<cloud::Event>> {
    check_reply_stream(stream).map_err(|_| {
        eyre!(
            "Cloud conversation activated but its stream did not connect. Reopen it before sending."
        )
    })
}

/// Opens an app-server client backed by the authenticated native Cloud client.
fn start_worker(
    client: cloud::Client,
    login: idle::Login,
    account: protocol::GetAccountResponse,
    title_client: Option<CloudTitleClient>,
) -> AppServerClient {
    let (app_client, commands, output) =
        CloudClient::channel_with_native(/*capacity*/ 32, Some(client.clone()), title_client);
    let client = Arc::new(client);
    let mut requests = requests::Requests {
        client: Arc::clone(&client),
        account,
        loaded: None,
        warned_partial_history: None,
    };
    let mut replies = Replies {
        idle: Some(idle::Recovery::new(
            app_client.request_handle().reconnect,
            client,
            login,
        )),
        stream_losses: app_client.stream_losses.clone(),
        ..Default::default()
    };
    tokio::spawn(async move {
        run_worker(commands, &mut requests, &mut replies, &output).await;
    });
    AppServerClient::Cloud(app_client)
}

async fn run_worker(
    commands: mpsc::Receiver<crate::CloudCommand>,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
) {
    run_worker_with_prepared(commands, requests, replies, output, /*prepared*/ None).await;
}

async fn run_worker_with_prepared(
    mut commands: mpsc::Receiver<crate::CloudCommand>,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
    mut prepared: Option<PreparedResume>,
) {
    loop {
        let mut command = tokio::select! {
            _ = output.closed() => return,
            () = replies.next_event(output) => continue,
            command = commands.recv() => match command { Some(command) => command, None => return },
            command = async {
                match &mut prepared {
                    Some(candidate) => candidate.history.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                let Some(candidate) = prepared.take() else {
                    continue;
                };
                prepared = advance_prepared_resume(candidate, command, requests, replies, output).await;
                continue;
            }
        };
        let (is_thread_start, selects_attachment) = match &command.operation {
            CloudOperation::Request(request) => match request.as_ref() {
                protocol::ClientRequest::ThreadStart { .. } => (true, true),
                protocol::ClientRequest::ThreadResume { .. } => (false, true),
                _ => (false, false),
            },
            CloudOperation::StartThreadWithEnvironment { .. } => (true, true),
            CloudOperation::PrepareThreadResume { .. } if command.response.is_closed() => continue,
            CloudOperation::PrepareThreadResume { .. } => (false, true),
            _ => (false, false),
        };
        // A queued start that nobody owns must leave the existing recovery alone.
        if is_thread_start && command.response.is_closed() {
            continue;
        }
        // A deliberate selection owns the next attachment, even if its handshake fails.
        let canceled = if selects_attachment {
            let previous = prepared
                .take()
                .and_then(|candidate| candidate.canceled_recovery);
            replies
                .idle
                .as_mut()
                .and_then(idle::Recovery::cancel)
                .or(previous)
        } else {
            None
        };
        let request = match command.operation {
            CloudOperation::Shutdown => {
                let _ = command.response.send(Ok(Ok(Value::Null)));
                return;
            }
            CloudOperation::ThreadNameSaved(params) => {
                requests.thread_name_saved(params, output);
                let _ = command.response.send(Ok(Ok(Value::Null)));
                continue;
            }
            CloudOperation::Request(request) => {
                Ok((*request, cloud::SubmissionMode::StartIfIdle, None))
            }
            CloudOperation::PrepareThreadResume {
                request_id,
                params,
                history,
            } => {
                prepared = tokio::select! {
                    _ = output.closed() => return,
                    candidate = prepare_thread_resume(
                        protocol::ClientRequest::ThreadResume { request_id, params: *params },
                        history, command.response, requests, replies, output,
                    ) => candidate,
                };
                if let Some(candidate) = &mut prepared {
                    candidate.canceled_recovery = canceled;
                } else if let Some(root) = canceled {
                    replies.report_loss(&root, /*reason*/ None, output);
                }
                continue;
            }
            CloudOperation::CommitPreparedResume => Err(eyre!(
                "Cloud resume commit requires its scoped history handle."
            )),
            CloudOperation::StartThreadWithEnvironment {
                request_id,
                params,
                environment,
            } => Ok((
                protocol::ClientRequest::ThreadStart {
                    request_id,
                    params: *params,
                },
                cloud::SubmissionMode::StartIfIdle,
                Some(environment),
            )),
            CloudOperation::SubmitUserMessage(params) => Ok((
                protocol::ClientRequest::TurnStart {
                    // Native admission is keyed by the caller's stable message ID.
                    request_id: protocol::RequestId::Integer(0),
                    params: *params,
                },
                cloud::SubmissionMode::Steer,
                None,
            )),
            CloudOperation::ResumeDisconnectedThread {
                thread_id,
                require_history,
                deadline,
                release,
            } => {
                // Retire canceled holds, but commit a ready handoff through its normal ACK.
                if replies
                    .idle
                    .as_ref()
                    .is_some_and(|recovery| recovery.held.is_some())
                {
                    let _ = replies.next_event(output).now_or_never();
                }
                if let Some(recovery) = &mut replies.idle
                    && let Some(cached) = requests
                        .loaded
                        .as_ref()
                        .filter(|settings| settings.thread.id == thread_id)
                    && replies.active.is_none()
                    && recovery.held.is_none()
                {
                    // Start a fresh warning epoch before any recovery history is read.
                    requests.warned_partial_history = None;
                    recovery.start(
                        thread_id,
                        cached.clone(),
                        require_history,
                        deadline,
                        release,
                        command.response,
                    );
                } else {
                    let _ = command.response.send(Ok(Err(idle::recovery_error(
                        "This cloud conversation no longer has a reply attachment to restore.",
                    ))));
                }
                continue;
            }
            CloudOperation::Notify(_) => {
                Err(eyre!("Cloud client notifications are not supported yet."))
            }
            CloudOperation::Resolve { .. } | CloudOperation::Reject { .. } => Err(eyre!(
                "Answer this cloud approval in another client; TUI approval replies are not supported yet."
            )),
        };
        let result = match request {
            Ok((request, mode, environment)) => {
                let unloads = prepared.as_ref().is_some_and(|candidate| {
                    let id = match &request {
                        protocol::ClientRequest::ThreadArchive { params, .. } => &params.thread_id,
                        protocol::ClientRequest::ThreadUnsubscribe { params, .. } => {
                            &params.thread_id
                        }
                        _ => return false,
                    };
                    let loaded = requests.loaded.as_deref();
                    candidate.settings.thread.id == *id
                        || loaded.is_some_and(|loaded| loaded.thread.id == *id)
                });
                // Inventory and history reads have no conversation state to commit.
                let history_thread = match &request {
                    protocol::ClientRequest::ThreadTurnsList { params, .. } => {
                        Some(&params.thread_id)
                    }
                    protocol::ClientRequest::ThreadItemsList { params, .. } => {
                        Some(&params.thread_id)
                    }
                    _ => None,
                };
                let is_read = history_thread.is_some()
                    || matches!(&request, protocol::ClientRequest::ThreadList { .. });
                let login = history_thread.and_then(|id| replies.idle.as_ref()?.history_login(id));
                // When closure and the handshake are both ready, cancel before
                // the handshake can replace the active attachment.
                let result = tokio::select! {
                    biased;
                    _ = output.closed() => return,
                    _ = command.response.closed(), if is_read || is_thread_start => {
                        if let Some(root) = canceled {
                            replies.report_loss(&root, /*reason*/ None, output);
                        }
                        continue;
                    }
                    result = async {
                        let read = handle_request(request, mode, environment, requests, replies, output);
                        let Some((login, deadline)) = login else {
                            return read.await;
                        };
                        tokio::time::timeout_at(deadline, login.permit.run(async {
                            if !login.current().await {
                                return Err(eyre!(CloudRecoverySignInChanged));
                            }
                            let result = read.await;
                            if !login.current().await {
                                return Err(eyre!(CloudRecoverySignInChanged));
                            }
                            login.permit.check().map_err(|_| eyre!(CLOUD_RECOVERY_NETWORK_BLOCKED))?;
                            result
                        })).await.map_err(|_| eyre!(idle::RECOVERY_TIMEOUT))?
                            .map_err(|_| eyre!(CLOUD_RECOVERY_NETWORK_BLOCKED))?
                    } => result,
                };
                if unloads
                    && result.is_ok()
                    && let Some(root) = prepared
                        .take()
                        .and_then(|candidate| candidate.canceled_recovery)
                    && requests.loaded.is_some()
                {
                    replies.report_loss(&root, /*reason*/ None, output);
                }
                result
            }
            Err(error) => Err(error),
        };
        if result.is_err()
            && let Some(root) = canceled
        {
            replies.report_loss(&root, /*reason*/ None, output);
        }
        let response = match result {
            Err(error) if error.is::<CloudRecoverySignInChanged>() => {
                Err(std::io::Error::other(CloudRecoverySignInChanged))
            }
            result => Ok(request_result(result)),
        };
        let _ = command.response.send(response);
    }
}

fn request_result(result: Result<Value>) -> crate::RequestResult {
    result.map_err(|error| protocol::JSONRPCErrorError {
        code: if error
            .downcast_ref::<requests::UnsupportedOperation>()
            .is_some()
        {
            -32601
        } else if error.is::<PreflightRejection>() || error.is::<MissingReplyAttachment>() {
            -32602
        } else {
            -32000
        },
        message: error.to_string(),
        data: None,
    })
}

struct PreparedResume {
    settings: Box<protocol::ThreadResumeResponse>,
    opened: Option<OpenReplyStream>,
    history: mpsc::Receiver<crate::CloudCommand>,
    generation: u64,
    canceled_recovery: Option<Arc<str>>,
    warning: Option<AppServerEvent>,
}

const PREPARED_EVENT_LIMIT: usize = 8;

fn check_prepared_stream((mut stream, first): OpenReplyStream) -> Result<OpenReplyStream> {
    let mut prefix = Vec::new();
    // The regular stream path forwards the prefix and unread tail in order.
    for _ in usize::from(first.is_some())..PREPARED_EVENT_LIMIT {
        let Some(event) = probe_reply_stream(&mut stream)? else {
            break;
        };
        prefix.push(Ok(event));
    }
    Ok((futures::stream::iter(prefix).chain(stream).boxed(), first))
}

async fn prepare_thread_resume(
    request: protocol::ClientRequest,
    history: mpsc::Receiver<crate::CloudCommand>,
    mut ready: oneshot::Sender<std::io::Result<crate::RequestResult>>,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
) -> Option<PreparedResume> {
    let prepared = tokio::select! {
        biased;
        _ = ready.closed() => return None,
        prepared = async {
            let response = replies.while_listening(
                requests.handle(request, cloud::SubmissionMode::StartIfIdle, /*environment*/ None, output),
                output,
            ).await?;
            let requests::Response::Attach(settings) = response else {
                bail!("Cloud resume did not return attachment settings.");
            };
            let value = serde_json::to_value(&settings)?;
            let id = &settings.thread.id;
            let opened = replies.open_attachment(id, requests.client.attach(id), output).await?;
            Ok((settings, value, opened))
        } => prepared,
    };
    let (settings, value, opened) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = ready.send(Ok(request_result(Err(error))));
            return None;
        }
    };
    ready.send(Ok(Ok(value))).ok()?;
    Some(PreparedResume {
        settings,
        opened,
        history,
        generation: replies.generation,
        canceled_recovery: None,
        warning: None,
    })
}

async fn advance_prepared_resume(
    mut candidate: PreparedResume,
    command: Option<crate::CloudCommand>,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
) -> Option<PreparedResume> {
    if let Some(crate::CloudCommand {
        operation,
        mut response,
    }) = command.filter(|command| !command.response.is_closed())
    {
        let id = &candidate.settings.thread.id;
        match operation {
            CloudOperation::CommitPreparedResume => {
                let ready = match candidate.opened {
                    Some(opened) => check_prepared_stream(opened).map(Some),
                    None if replies.admit_turn(id, output).is_ok()
                        && replies.generation == candidate.generation =>
                    {
                        Ok(None)
                    }
                    None => Err(MissingReplyAttachment.into()),
                };
                match ready {
                    Ok(opened) => {
                        // No await between accepting the handoff and replacing the state.
                        if response.send(Ok(Ok(Value::Null))).is_ok() {
                            if let Some(opened) = opened {
                                replies.install_replacement(id, opened, output);
                                let status = &candidate.settings.thread.status;
                                if let Some(recovery) = &mut replies.idle {
                                    let initial_idle = *status == protocol::ThreadStatus::Idle;
                                    recovery.observed_idle.get_or_insert(initial_idle);
                                }
                            }
                            requests.warned_partial_history =
                                candidate.warning.as_ref().map(|_| id.clone());
                            requests.loaded = Some(candidate.settings);
                            if let Some(warning) = candidate.warning {
                                let _ = output.send(warning);
                            }
                            return None;
                        }
                    }
                    Err(error) => {
                        let _ = response.send(Ok(request_result(Err(error))));
                    }
                }
            }
            CloudOperation::Request(request)
                if match request.as_ref() {
                    protocol::ClientRequest::ThreadTurnsList { params, .. } => {
                        params.thread_id == *id
                    }
                    protocol::ClientRequest::ThreadItemsList { params, .. } => {
                        params.thread_id == *id
                    }
                    _ => false,
                } =>
            {
                // Ordinary requests keep their own warning marker between scoped pages.
                let previous = requests.warned_partial_history.take();
                let (warning_output, mut warnings) = mpsc::unbounded_channel();
                let page = tokio::select! {
                    biased;
                    _ = response.closed() => None,
                    _ = output.closed() => None,
                    page = replies.while_listening(
                        requests.handle(*request, cloud::SubmissionMode::StartIfIdle, /*environment*/ None, &warning_output),
                        output,
                    ) => Some(page),
                };
                requests.warned_partial_history = previous;
                if let Some(page) = page {
                    let page = page.and_then(|response| match response {
                        requests::Response::Value(value) => Ok(value),
                        _ => Err(eyre!("Expected a Cloud history page.")),
                    });
                    let ok = page.is_ok();
                    if response.send(Ok(request_result(page))).is_ok() && ok {
                        candidate.warning = candidate.warning.or_else(|| warnings.try_recv().ok());
                        return Some(candidate);
                    }
                }
            }
            _ => {
                let _ = response.send(Ok(request_result(Err(eyre!(
                    "Only the prepared Cloud conversation's history may be read here."
                )))));
            }
        }
    }
    if let Some(root) = candidate.canceled_recovery {
        replies.report_loss(&root, /*reason*/ None, output);
    }
    None
}

async fn handle_request(
    request: protocol::ClientRequest,
    mode: cloud::SubmissionMode,
    environment: Option<cloud::EnvironmentSelection>,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
) -> Result<Value> {
    if let protocol::ClientRequest::ThreadLoadedList { .. } = &request {
        // The archive RPC can fail after closing Attach. Observe a ready EOF even
        // when the RPC result won the worker's select before the stream did.
        let _ = tokio::task::unconstrained(replies.forward_next(output)).now_or_never();
        return Ok(serde_json::json!({
            "data": replies.active.iter().map(|active| &active.thread_id).collect::<Vec<_>>(),
            "nextCursor": null,
        }));
    }
    if let protocol::ClientRequest::ThreadRead { params, .. } = &request {
        return requests.read_thread(params, replies.thread_status(&params.thread_id));
    }
    if let protocol::ClientRequest::ThreadUnsubscribe { params, .. } = &request {
        let status = if requests
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.thread.id == params.thread_id)
        {
            replies.detach(&params.thread_id);
            requests.loaded = None;
            protocol::ThreadUnsubscribeStatus::Unsubscribed
        } else {
            protocol::ThreadUnsubscribeStatus::NotSubscribed
        };
        return Ok(serde_json::json!({ "status": status }));
    }
    let selected_mutation = match &request {
        protocol::ClientRequest::TurnStart { params, .. } => {
            replies.admit_turn(&params.thread_id, output)?;
            true
        }
        protocol::ClientRequest::TurnInterrupt { params, .. } => replies
            .active
            .as_ref()
            .is_some_and(|attachment| attachment.thread_id == params.thread_id),
        _ => false,
    };
    let response = {
        let handling = requests.handle(request, mode, environment, output);
        tokio::pin!(handling);
        let immediate = if selected_mutation && let Some(recovery) = &mut replies.idle {
            // Local admission validation finishes before the RPC yields. Poll without
            // listening to the stream so a pending mutation cannot inherit stale idle.
            match handling.as_mut().now_or_never() {
                Some(Err(error)) if error.is::<PreflightRejection>() => return Err(error),
                result => {
                    recovery.observed_idle = None;
                    result
                }
            }
        } else {
            None
        };
        match immediate {
            Some(result) => result,
            None => replies.while_listening(handling, output).await,
        }
    };
    match response? {
        requests::Response::Value(value) => Ok(value),
        requests::Response::Archived(thread_id) => {
            replies.detach(&thread_id);
            replies.stream_losses.supersede(&thread_id);
            if requests
                .loaded
                .as_ref()
                .is_some_and(|loaded| loaded.thread.id == thread_id)
            {
                requests.loaded = None;
            }
            send_notification(
                output,
                protocol::ServerNotification::ThreadArchived(
                    protocol::ThreadArchivedNotification { thread_id },
                ),
            );
            Ok(serde_json::to_value(protocol::ThreadArchiveResponse {})?)
        }
        requests::Response::Attach(settings) => {
            let id = &settings.thread.id;
            let result = serde_json::to_value(&settings)?;
            replies
                .replace(id, requests.client.attach(id), output)
                .await?;
            if let Some(idle) = &mut replies.idle {
                idle.observed_idle.get_or_insert(matches!(
                    settings.thread.status,
                    protocol::ThreadStatus::Idle
                ));
            }
            requests.loaded = Some(settings);
            requests.warned_partial_history = None;
            Ok(result)
        }
    }
}

fn forward(
    id: &str,
    event: cloud::Event,
    output: &mpsc::UnboundedSender<AppServerEvent>,
    latest_status: &mut Option<(String, cloud::ThreadStatus)>,
    recovery: Option<&mut idle::Recovery>,
) {
    if let Some(status) = event.thread_status_for(id) {
        if let Some(recovery) = recovery {
            if matches!(status, cloud::ThreadStatus::Idle) {
                recovery.observed_idle = Some(true);
                recovery.notify_connected(id, output);
            } else {
                recovery.observe_busy();
            }
        }
        *latest_status = Some((id.to_owned(), status));
        return;
    }
    if let Some(recovery) = recovery
        && let cloud::Event::Transcript(item) = &event
        && item.thread_id == id
    {
        match &item.update {
            cloud::TranscriptUpdate::TurnStarted(_) => recovery.observe_busy(),
            // The ordinary StartIfIdle admission still decides if the next turn can start.
            cloud::TranscriptUpdate::TurnCompleted(_) => recovery.notify_connected(id, output),
            _ => {}
        }
    }
    let notification = match event {
        cloud::Event::Transcript(event) => match events::transcript(event.thread_id, event.update) {
            Ok(Some(notification)) => notification,
            Ok(None) => return,
            Err(_) => protocol::ServerNotification::Error(cloud_error(
                id,
                "The cloud sent an unsupported transcript event.",
            )),
        },
        cloud::Event::ServerRequest(_) => protocol::ServerNotification::Warning(protocol::WarningNotification {
            thread_id: Some(id.to_owned()),
            message: "This cloud turn requested an unsupported interaction. Open it in another client to continue.".into(),
        }),
        cloud::Event::Notification(_) => return,
    };
    send_notification(output, notification);
}

fn send_notification(
    output: &mpsc::UnboundedSender<AppServerEvent>,
    notification: protocol::ServerNotification,
) {
    let _ = output.send(AppServerEvent::ServerNotification(Box::new(notification)));
}

fn cloud_error(id: &str, message: &str) -> protocol::ErrorNotification {
    protocol::ErrorNotification {
        thread_id: id.to_owned(),
        turn_id: String::new(),
        will_retry: false,
        error: protocol::TurnError {
            message: message.to_owned(),
            additional_details: None,
            codex_error_info: None,
            misalignment: None,
        },
    }
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "submission_tests.rs"]
mod submission_tests;
