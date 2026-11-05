//! Adapts native Cloud requests and the currently open conversation’s reply stream.
//! A replacement becomes active only after its native attachment opens successfully.

mod events;
mod requests;

use crate::AppServerClient;
use crate::AppServerEvent;
use crate::CloudClient;
use crate::CloudOperation;
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

type ReplyStream = BoxStream<'static, Result<cloud::Event, cloud::Error>>;

struct Attachment {
    thread_id: String,
    stream: ReplyStream,
}

#[derive(Default)]
struct Replies {
    active: Option<Attachment>,
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
                send_notification(
                    output,
                    cloud_error(
                        &active.thread_id,
                        "Cloud stream ended. Reopen the conversation to reconnect.",
                    ),
                );
                self.active = None;
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
        self.latest_status = None;
        if let Some(event) = first_event {
            forward(id, event, output, &mut self.latest_status);
        }
        self.active = Some(Attachment {
            thread_id: id.to_owned(),
            stream,
        });
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
    ) -> Result<()> {
        // Notice a ready disconnect before sending. A continuous stream of events
        // must still allow commands to make progress.
        let _ = tokio::task::unconstrained(self.forward_next(output)).now_or_never();
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.thread_id != id)
        {
            bail!("Cloud stream is not connected. Reopen the conversation before sending.");
        }
        Ok(())
    }
}

/// Opens an app-server client backed by the authenticated native Cloud client.
pub fn connect(client: cloud::Client, account: protocol::GetAccountResponse) -> AppServerClient {
    let (app_client, mut commands, output) = CloudClient::channel(/*capacity*/ 32);
    let mut requests = requests::Requests {
        client,
        account,
        loaded: None,
        warned_partial_history: None,
    };
    tokio::spawn(async move {
        let mut replies = Replies::default();
        loop {
            let command = tokio::select! {
                _ = output.closed() => break,
                () = replies.forward_next(&output) => continue,
                command = commands.recv() => match command { Some(command) => command, None => break },
            };
            let result = match command.operation {
                CloudOperation::Shutdown => {
                    let _ = command.response.send(Ok(Ok(Value::Null)));
                    break;
                }
                CloudOperation::Request(request) => tokio::select! {
                    _ = output.closed() => break,
                    result = handle_request(*request, &mut requests, &mut replies, &output) => result,
                },
                CloudOperation::Notify(_) => {
                    Err(eyre!("Cloud client notifications are not supported yet."))
                }
                CloudOperation::Resolve { .. } | CloudOperation::Reject { .. } => Err(eyre!(
                    "Answer this cloud approval in another client; TUI approval replies are not supported yet."
                )),
            };
            let result = result.map_err(|error| protocol::JSONRPCErrorError {
                code: if error
                    .downcast_ref::<requests::UnsupportedOperation>()
                    .is_some()
                {
                    -32601
                } else {
                    -32000
                },
                message: error.to_string(),
                data: None,
            });
            let _ = command.response.send(Ok(result));
        }
    });
    AppServerClient::Cloud(app_client)
}

async fn handle_request(
    request: protocol::ClientRequest,
    requests: &mut requests::Requests,
    replies: &mut Replies,
    output: &mpsc::UnboundedSender<AppServerEvent>,
) -> Result<Value> {
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
        .while_listening(requests.handle(request, output), output)
        .await?
    {
        requests::Response::Value(value) => Ok(value),
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
            Err(_) => cloud_error(id, "The cloud sent an unsupported transcript event."),
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

fn cloud_error(id: &str, message: &str) -> protocol::ServerNotification {
    protocol::ServerNotification::Error(protocol::ErrorNotification {
        thread_id: id.to_owned(),
        turn_id: String::new(),
        will_retry: false,
        error: protocol::TurnError {
            message: message.to_owned(),
            additional_details: None,
            codex_error_info: None,
            misalignment: None,
        },
    })
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
