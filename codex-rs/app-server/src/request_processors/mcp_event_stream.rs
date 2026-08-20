use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerEventNotification;
use codex_app_server_protocol::McpServerEventStreamNotification;
use codex_app_server_protocol::McpServerEventStreamStartParams;
use codex_app_server_protocol::ServerNotification;
use codex_core::CodexThread;
use codex_login::AuthManager;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpEventStream;
use codex_protocol::ThreadId;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::mcp_processor::McpRequestProcessor;
use crate::error_code::OVERLOADED_ERROR_CODE;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;

const MAX_MCP_EVENT_STREAMS_PER_CONNECTION: usize = 64;
const MAX_MCP_EVENT_STREAM_RECONNECT_ATTEMPTS: u32 = 3;
const MCP_EVENT_STREAM_RECONNECT_DELAY: Duration = Duration::from_secs(/*secs*/ 1);
const MCP_EVENT_STREAM_STARTUP_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 90);

pub(crate) type McpEventStreamReady = oneshot::Receiver<Result<(), JSONRPCErrorError>>;

#[derive(Debug)]
struct McpEventStreamTask {
    thread_id: ThreadId,
    task: JoinHandle<()>,
}

#[derive(Debug, Default)]
pub(crate) struct McpEventStreams {
    tasks: Mutex<HashMap<String, McpEventStreamTask>>,
}

impl McpEventStreams {
    pub(crate) async fn start(
        &self,
        connection_id: ConnectionId,
        params: McpServerEventStreamStartParams,
        processor: McpRequestProcessor,
    ) -> Result<McpEventStreamReady, JSONRPCErrorError> {
        if params.server != CODEX_APPS_MCP_SERVER_NAME {
            return Err(invalid_request(
                "MCP event subscriptions are only supported for hosted apps",
            ));
        }
        let thread_id = ThreadId::from_string(&params.thread_id)
            .map_err(|error| invalid_request(format!("invalid thread id: {error}")))?;
        let subscription_id = params.subscription_id.clone();
        let (ready_tx, ready_rx) = oneshot::channel();
        {
            let mut tasks = self.tasks.lock().await;
            tasks.retain(|_, task| !task.task.is_finished());
            if tasks.contains_key(&subscription_id) {
                return Err(invalid_request(format!(
                    "MCP event subscription '{subscription_id}' already exists"
                )));
            }
            if tasks.len() >= MAX_MCP_EVENT_STREAMS_PER_CONNECTION {
                return Err(JSONRPCErrorError {
                    code: OVERLOADED_ERROR_CODE,
                    message: format!(
                        "MCP event subscription limit of {MAX_MCP_EVENT_STREAMS_PER_CONNECTION} reached"
                    ),
                    data: None,
                });
            }
            let task = tokio::spawn(async move {
                let mut auth_changes =
                    McpEventStreamAuthChanges::new(Arc::clone(&processor.auth_manager));
                let opened = tokio::select! {
                    () = auth_changes.changed() => Err(internal_error(
                        "MCP event subscription authentication changed during startup",
                    )),
                    result = async {
                        if !processor
                            .thread_state_manager
                            .subscribed_connection_ids(thread_id)
                            .await
                            .contains(&connection_id)
                        {
                            return Err(invalid_request(format!(
                                "connection is not subscribed to thread '{thread_id}'"
                            )));
                        }
                        let (_, thread) = processor.load_thread(&params.thread_id).await?;
                        let stream = thread
                            .start_mcp_event_stream(
                                &params.name,
                                params.arguments.clone(),
                                params.meta.clone(),
                            )
                            .await
                            .map_err(|error| internal_error(format!(
                                "failed to start MCP event stream for '{}': {error:#}",
                                params.server
                            )))?;
                        Ok((stream, thread))
                    } => result,
                };
                match opened {
                    Ok((stream, thread)) => {
                        forward_events(
                            connection_id,
                            &params,
                            processor.outgoing.as_ref(),
                            thread,
                            stream,
                            auth_changes,
                            ready_tx,
                        )
                        .await;
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            });
            tasks.insert(subscription_id, McpEventStreamTask { thread_id, task });
        }
        Ok(ready_rx)
    }

    pub(crate) async fn wait_for_activation(
        &self,
        subscription_id: &str,
        ready: McpEventStreamReady,
    ) -> Result<(), JSONRPCErrorError> {
        match tokio::time::timeout(MCP_EVENT_STREAM_STARTUP_TIMEOUT, ready).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(internal_error(
                "MCP event stream ended before becoming active",
            )),
            Err(_) => {
                self.stop(subscription_id).await;
                Err(internal_error("MCP event stream startup timed out"))
            }
        }
    }

    pub(crate) async fn stop(&self, subscription_id: &str) {
        let task = self.tasks.lock().await.remove(subscription_id);
        if let Some(task) = task {
            task.task.abort();
            let _ = task.task.await;
        }
    }

    pub(crate) async fn stop_thread(&self, thread_id: ThreadId) {
        self.stop_matching(Some(thread_id)).await;
    }

    pub(crate) async fn clear(&self) {
        self.stop_matching(/*thread_id*/ None).await;
    }

    async fn stop_matching(&self, thread_id: Option<ThreadId>) {
        let tasks = self
            .tasks
            .lock()
            .await
            .extract_if(|_, task| thread_id.is_none_or(|id| task.thread_id == id))
            .map(|(_, task)| task)
            .collect::<Vec<_>>();
        for task in tasks {
            task.task.abort();
            let _ = task.task.await;
        }
    }
}

async fn forward_events(
    connection_id: ConnectionId,
    params: &McpServerEventStreamStartParams,
    outgoing: &OutgoingMessageSender,
    thread: Arc<CodexThread>,
    mut stream: McpEventStream,
    mut auth_changes: McpEventStreamAuthChanges,
    ready: oneshot::Sender<Result<(), JSONRPCErrorError>>,
) {
    let mut ready = Some(ready);
    let mut reconnect_attempts = 0;
    let mut reconnect_deadline = None;
    let send = |notification| {
        outgoing.send_server_notification_to_connection_and_wait(
            connection_id,
            ServerNotification::McpServerEventStream(McpServerEventStreamNotification {
                subscription_id: params.subscription_id.clone(),
                notification,
            }),
        )
    };
    loop {
        let notification = tokio::select! {
            biased;
            () = auth_changes.changed() => break,
            () = thread.wait_until_terminated() => break,
            result = stream.recv() => match result {
                Ok(Some(notification)) => notification,
                Ok(None)
                    if ready.is_none()
                        && reconnect_attempts < MAX_MCP_EVENT_STREAM_RECONNECT_ATTEMPTS =>
                {
                    reconnect_attempts += 1;
                    let deadline = *reconnect_deadline.get_or_insert_with(|| {
                        tokio::time::Instant::now() + MCP_EVENT_STREAM_STARTUP_TIMEOUT
                    });
                    tokio::select! {
                        biased;
                        () = auth_changes.changed() => break,
                        () = thread.wait_until_terminated() => break,
                        result = tokio::time::timeout_at(deadline, async {
                            tokio::time::sleep(
                                MCP_EVENT_STREAM_RECONNECT_DELAY
                                    * (1 << (reconnect_attempts - 1)),
                            )
                            .await;
                            let mut stream = thread
                                .start_mcp_event_stream(
                                    &params.name,
                                    params.arguments.clone(),
                                    params.meta.clone(),
                                )
                                .await?;
                            let notification = stream.recv().await?;
                            Ok::<_, anyhow::Error>((stream, notification))
                        }) => match result {
                            Ok(Ok((reconnected, Some(notification)))) => {
                                stream = reconnected;
                                notification
                            }
                            Ok(Ok((_, None)) | Err(_)) => continue,
                            Err(_) => break,
                        },
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    if let Some(ready) = ready.take() {
                        let _ = ready.send(Err(internal_error(format!("{error:#}"))));
                    }
                    break;
                }
            },
        };
        let active = notification.method == "notifications/events/active";
        let terminated = notification.method == "notifications/events/terminated";
        if !send(McpServerEventNotification {
            method: notification.method,
            params: notification
                .params
                .unwrap_or_else(|| Value::Object(Default::default())),
        })
        .await
        {
            break;
        }
        if active && let Some(ready) = ready.take() {
            let _ = ready.send(Ok(()));
        }
        if terminated {
            return;
        }
        if !active {
            reconnect_attempts = 0;
            reconnect_deadline = None;
        }
    }
    if let Some(ready) = ready {
        let _ = ready.send(Err(internal_error(
            "MCP event stream ended before becoming active",
        )));
    } else {
        send(McpServerEventNotification {
            method: "notifications/events/terminated".to_string(),
            params: Value::Object(Default::default()),
        })
        .await;
    }
}

struct McpEventStreamAuthChanges {
    manager: Arc<AuthManager>,
    receiver: watch::Receiver<u64>,
    revision: u64,
    principal: Option<(Option<String>, Option<String>)>,
}

impl McpEventStreamAuthChanges {
    fn new(manager: Arc<AuthManager>) -> Self {
        let receiver = manager.auth_change_receiver();
        let revision = *receiver.borrow();
        let principal = manager
            .auth_cached()
            .map(|auth| (auth.get_account_id(), auth.get_chatgpt_user_id()));
        Self {
            manager,
            receiver,
            revision,
            principal,
        }
    }

    async fn changed(&mut self) {
        loop {
            if self
                .manager
                .auth_cached()
                .map(|auth| (auth.get_account_id(), auth.get_chatgpt_user_id()))
                != self.principal
            {
                return;
            }
            if self.receiver.changed().await.is_err() || self.principal == Some((None, None)) {
                return;
            }
            let revision = *self.receiver.borrow_and_update();
            if revision.wrapping_sub(self.revision) > 1 {
                return;
            }
            self.revision = revision;
        }
    }
}
