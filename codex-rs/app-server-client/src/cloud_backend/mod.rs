//! Adapts native Cloud requests and the currently open conversation’s reply stream.
//! A replacement becomes active only after its native attachment opens successfully.
//! Loaded-thread queries report reply attachments, not retained resume metadata.

mod auth;
mod events;
mod idle;
mod requests;

use crate::AppServerClient;
use crate::AppServerEvent;
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

pub use auth::connect;

type ReplyStream = BoxStream<'static, Result<cloud::Event, cloud::Error>>;

/// Rejected locally before an admission request can be sent.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct PreflightRejection(&'static str);

#[derive(Debug, thiserror::Error)]
#[error("Cloud stream is not connected. Reopen the conversation before sending.")]
struct MissingReplyAttachment;

struct Attachment {
    thread_id: String,
    stream: ReplyStream,
}

#[derive(Default)]
struct Replies {
    idle: Option<idle::Recovery>,
    active: Option<Attachment>,
    stream_losses: CloudStreamLosses,
    // Keep the last observed status through EOF until this conversation is unloaded.
    latest_status: Option<(String, cloud::ThreadStatus)>,
}

impl Replies {
    async fn forward_next(&mut self, output: &mpsc::UnboundedSender<AppServerEvent>) {
        if self.active.is_none()
            && let Some(recovery) = &mut self.idle
            && let Some((root, open)) = &mut recovery.pending
        {
            let opened = open.await;
            let root = Arc::clone(root);
            recovery.pending = None;
            if let Some((stream, first)) = opened.filter(|_| recovery.login.permit.check().is_ok())
            {
                recovery.installed = Some(tokio::time::Instant::now());
                // A same-thread Attach can stay quiet. Keep the last observation until an
                // event or a dispatched mutation changes recovery eligibility.
                self.install(&root, stream, first, output);
                if let Some(recovery) = &self.idle
                    && recovery.observed_idle != Some(false)
                {
                    recovery.notify_connected(&root, output);
                }
            } else {
                recovery
                    .state
                    .send_replace(crate::cloud::IdleReconnect::Inactive);
                self.report_loss(&root, output);
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
                self.active = None;
                if let Some(recovery) = &mut self.idle {
                    recovery
                        .state
                        .send_replace(crate::cloud::IdleReconnect::Inactive);
                    let stable = recovery
                        .installed
                        .is_none_or(|start| start.elapsed() >= std::time::Duration::from_secs(15));
                    if recovery.observed_idle == Some(true)
                        && stable
                        && idle::transient(ended.and_then(Result::err).as_ref())
                    {
                        recovery.offer(&id);
                    }
                }
                self.report_loss(&id, output);
            }
        }
    }

    fn report_loss(&self, id: &str, output: &mpsc::UnboundedSender<AppServerEvent>) {
        self.stream_losses.record(id);
        let _ = output.send(AppServerEvent::ThreadStreamDisconnected(Box::new(
            cloud_error(
                id,
                "Cloud stream ended. Reopen the conversation to reconnect.",
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
                () = self.forward_next(output) => {}
            }
        }
    }

    async fn replace(
        &mut self,
        id: &str,
        open: impl Future<Output = Result<ReplyStream, cloud::Error>>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) -> Result<()> {
        let mut stream = self.while_listening(open, output).await?;
        // A server can close Attach immediately after returning successful headers.
        // A healthy idle stream needs no event to prove that it is ready.
        let first_event = match tokio::task::unconstrained(stream.next()).now_or_never() {
            Some(None | Some(Err(_))) => {
                bail!(
                    "Cloud conversation activated but its stream did not connect. Reopen it before sending."
                );
            }
            Some(Some(Ok(event))) => Some(event),
            None => None,
        };
        if let Some(idle) = &mut self.idle {
            idle.reset();
        }
        self.stream_losses.supersede(id);
        self.latest_status = None;
        self.install(id, stream, first_event, output);
        Ok(())
    }

    fn install(
        &mut self,
        id: &str,
        stream: ReplyStream,
        first_event: Option<cloud::Event>,
        output: &mpsc::UnboundedSender<AppServerEvent>,
    ) {
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
            && (idle
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

/// Opens an app-server client backed by the authenticated native Cloud client.
fn start_worker(
    client: cloud::Client,
    login: idle::Login,
    account: protocol::GetAccountResponse,
) -> AppServerClient {
    let (app_client, commands, output) =
        CloudClient::channel_with_catalog(/*capacity*/ 32, Some(client.clone()));
    let client = Arc::new(client);
    let mut requests = requests::Requests {
        client: Arc::clone(&client),
        account,
        loaded: None,
        warned_partial_history: None,
    };
    let mut replies = Replies {
        idle: Some(idle::Recovery {
            state: app_client.request_handle().reconnect,
            pending: None,
            installed: None,
            observed_idle: None,
            client,
            login: Arc::new(login),
        }),
        stream_losses: app_client.stream_losses.clone(),
        ..Default::default()
    };
    tokio::spawn(async move {
        run_worker(commands, &mut requests, &mut replies, &output).await;
    });
    AppServerClient::Cloud(app_client)
}

async fn run_worker(
    mut commands: mpsc::Receiver<crate::CloudCommand>,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
) {
    loop {
        let mut command = tokio::select! {
            _ = output.closed() => return,
            () = replies.forward_next(output) => continue,
            command = commands.recv() => match command { Some(command) => command, None => return },
        };
        // A deliberate selection owns the next attachment, even if its handshake fails.
        let canceled = match &command.operation {
            CloudOperation::Request(request)
                if matches!(
                    request.as_ref(),
                    protocol::ClientRequest::ThreadStart { .. }
                        | protocol::ClientRequest::ThreadResume { .. }
                ) =>
            {
                replies.idle.as_mut().and_then(idle::Recovery::cancel)
            }
            CloudOperation::StartProjectThread { .. } => {
                replies.idle.as_mut().and_then(idle::Recovery::cancel)
            }
            _ => None,
        };
        let request = match command.operation {
            CloudOperation::Shutdown => {
                let _ = command.response.send(Ok(Ok(Value::Null)));
                return;
            }
            CloudOperation::Request(request) => {
                Ok((*request, cloud::SubmissionMode::StartIfIdle, None))
            }
            CloudOperation::StartProjectThread {
                request_id,
                params,
                project,
            } => Ok((
                protocol::ClientRequest::ThreadStart {
                    request_id,
                    params: *params,
                },
                cloud::SubmissionMode::StartIfIdle,
                Some(project),
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
            CloudOperation::Notify(_) => {
                Err(eyre!("Cloud client notifications are not supported yet."))
            }
            CloudOperation::Resolve { .. } | CloudOperation::Reject { .. } => Err(eyre!(
                "Answer this cloud approval in another client; TUI approval replies are not supported yet."
            )),
        };
        let result = match request {
            Ok((request, mode, project)) => {
                // Inventory is a read with no conversation state to commit.
                let is_thread_list = matches!(&request, protocol::ClientRequest::ThreadList { .. });
                if is_thread_list && command.response.is_closed() {
                    continue;
                }
                tokio::select! {
                    _ = output.closed() => return,
                    _ = command.response.closed(), if is_thread_list => continue,
                    result = handle_request(request, mode, project, requests, replies, output) => result,
                }
            }
            Err(error) => Err(error),
        };
        if result.is_err()
            && let Some(root) = canceled
        {
            replies.report_loss(&root, output);
        }
        let result = result.map_err(|error| protocol::JSONRPCErrorError {
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
        });
        let _ = command.response.send(Ok(result));
    }
}

async fn handle_request(
    request: protocol::ClientRequest,
    mode: cloud::SubmissionMode,
    project: Option<cloud::ProjectEnvironment>,
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
        let handling = requests.handle(request, mode, project, output);
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
