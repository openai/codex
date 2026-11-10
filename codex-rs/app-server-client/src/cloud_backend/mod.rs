//! Adapts native Cloud requests and the currently open conversation’s reply stream.
//! A replacement becomes active only after its native attachment opens successfully.
//! Loaded-thread queries report reply attachments, not retained resume metadata.

mod auth;
mod events;
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
    active: Option<Attachment>,
    stream_losses: CloudStreamLosses,
    // Keep the last observed status through EOF until this conversation is unloaded.
    latest_status: Option<(String, cloud::ThreadStatus)>,
}

impl Replies {
    async fn forward_next(&mut self, output: &mpsc::UnboundedSender<AppServerEvent>) {
        let Some(active) = &mut self.active else {
            return std::future::pending().await;
        };
        match active.stream.next().await {
            Some(Ok(event)) => forward(&active.thread_id, event, output, &mut self.latest_status),
            Some(Err(_)) | None => {
                let error = cloud_error(
                    &active.thread_id,
                    "Cloud stream ended. Reopen the conversation to reconnect.",
                );
                self.active = None;
                self.stream_losses.record(&error.thread_id);
                let _ = output.send(AppServerEvent::ThreadStreamDisconnected(Box::new(error)));
            }
        }
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
            Some(None | Some(Err(_))) => bail!(
                "Cloud conversation activated but its stream did not connect. Reopen it before sending."
            ),
            Some(Some(Ok(event))) => Some(event),
            None => None,
        };
        self.active = Some(Attachment {
            thread_id: id.to_owned(),
            stream,
        });
        self.stream_losses.supersede(id);
        self.latest_status = None;
        if let Some(event) = first_event {
            forward(id, event, output, &mut self.latest_status);
        }
        Ok(())
    }

    fn thread_status(&self, id: &str) -> Option<&cloud::ThreadStatus> {
        self.latest_status
            .as_ref()
            .filter(|(thread_id, _)| thread_id == id)
            .map(|(_, status)| status)
    }

    fn detach(&mut self, id: &str) {
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
fn start_worker(client: cloud::Client, account: protocol::GetAccountResponse) -> AppServerClient {
    let (app_client, commands, output) =
        CloudClient::channel_with_catalog(/*capacity*/ 32, Some(client.clone()));
    let mut requests = requests::Requests {
        client,
        account,
        loaded: None,
        warned_partial_history: None,
    };
    let mut replies = Replies {
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
        let command = tokio::select! {
            _ = output.closed() => return,
            () = replies.forward_next(output) => continue,
            command = commands.recv() => match command { Some(command) => command, None => return },
        };
        let user_message_submission =
            matches!(&command.operation, CloudOperation::SubmitUserMessage(_));
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
            Ok((request, mode, project)) => tokio::select! {
                _ = output.closed() => return,
                result = handle_request(request, mode, project, requests, replies, output) => result,
            },
            Err(error) => Err(error),
        };
        let result = result.map_err(|error| protocol::JSONRPCErrorError {
            code: if error
                .downcast_ref::<requests::UnsupportedOperation>()
                .is_some()
            {
                -32601
            } else if error.is::<PreflightRejection>()
                || user_message_submission && error.is::<MissingReplyAttachment>()
            {
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
    if let protocol::ClientRequest::TurnStart { params, .. } = &request {
        replies.admit_turn(&params.thread_id, output)?;
    }
    match replies
        .while_listening(requests.handle(request, mode, project, output), output)
        .await?
    {
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
) {
    if let Some(status) = event.thread_status_for(id) {
        *latest_status = Some((id.to_owned(), status));
        return;
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
