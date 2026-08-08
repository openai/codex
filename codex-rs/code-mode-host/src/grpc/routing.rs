use std::sync::Arc;
use std::sync::PoisonError;

use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use prost::Message;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::Status;
use uuid::Uuid;

use super::GrpcStream;
use super::conversions;
use super::session::GrpcSession;
use super::session::PendingInvocation;
use super::session::ToolSubscription;
use super::validation;
use crate::OUTGOING_CHANNEL_CAPACITY;

const MAX_SUBSCRIPTIONS: usize = OUTGOING_CHANNEL_CAPACITY;

impl GrpcSession {
    pub(super) fn subscribe(
        self: &Arc<Self>,
        filters: Vec<proto::ToolName>,
    ) -> Result<GrpcStream<proto::ToolCall>, Status> {
        validation::tool_filters(&filters)?;
        let id = Uuid::new_v4();
        let (sender, receiver) = mpsc::channel(OUTGOING_CHANNEL_CAPACITY);
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if self.closed.is_cancelled() {
            return Err(Status::cancelled("code-mode session is closed"));
        }
        if state.subscriptions.len() >= MAX_SUBSCRIPTIONS {
            return Err(Status::resource_exhausted(
                "code-mode session has too many tool subscriptions",
            ));
        }
        state.subscriptions.push(ToolSubscription {
            id,
            filters,
            sender: sender.clone(),
        });
        drop(state);

        let session = Arc::downgrade(self);
        let closed = self.closed.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = sender.closed() => {}
                _ = closed.cancelled() => return,
            }
            if let Some(session) = session.upgrade() {
                let abandoned = {
                    let mut state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
                    state
                        .subscriptions
                        .retain(|subscription| subscription.id != id);
                    state
                        .pending_invocations
                        .extract_if(|_, invocation| invocation.subscription_id == id)
                        .map(|(_, invocation)| invocation)
                        .collect::<Vec<_>>()
                };
                if !abandoned.is_empty() {
                    // A buffered call may never have reached the client. Closing
                    // the lease avoids replaying a possibly delivered invocation
                    // or leaving an unfillable execution-sequence gap.
                    session.closed.cancel();
                }
                for invocation in abandoned {
                    let _ = invocation.response.send(Err(
                        "code-mode tool subscription closed before returning tool output"
                            .to_string(),
                    ));
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(receiver)))
    }

    pub(super) async fn dispatch_tool(
        &self,
        invocation: CodeModeNestedToolCall,
        execution_id: String,
        invocation_id: Uuid,
        input_json: Option<Vec<u8>>,
        response: oneshot::Sender<Result<JsonValue, String>>,
        cancellation: &CancellationToken,
    ) -> Result<(), String> {
        let cell_id = invocation.cell_id.to_string();
        let tool_name = proto::ToolName {
            name: invocation.tool_name.name,
            namespace: invocation.tool_name.namespace,
        };
        let (sequence, subscriptions) = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(execution) = state.cells.get(&cell_id) else {
                return Err("code-mode cell closed before dispatching its tool call".to_string());
            };
            let sequence = execution.tool_call_sequence.checked_add(1).ok_or_else(|| {
                "code-mode execution tool-call sequence was exhausted".to_string()
            })?;
            let count = state.subscriptions.len();
            let subscriptions = (0..count)
                .map(|offset| &state.subscriptions[(state.next_subscription + offset) % count])
                .filter(|subscription| {
                    subscription.filters.is_empty()
                        || subscription.filters.iter().any(|filter| {
                            filter.name == tool_name.name && filter.namespace == tool_name.namespace
                        })
                })
                .map(|subscription| (subscription.id, subscription.sender.clone()))
                .collect::<Vec<_>>();
            (sequence, subscriptions)
        };
        if subscriptions.is_empty() {
            return Err("no code-mode tool subscription matches the requested tool".to_string());
        }
        let mut message = proto::ToolCall {
            session_id: self.id.to_string(),
            execution_id,
            cell_id: cell_id.clone(),
            invocation_id: invocation_id.to_string(),
            runtime_tool_call_id: invocation.runtime_tool_call_id,
            tool_name: Some(tool_name.clone()),
            tool_kind: conversions::tool_kind(invocation.tool_kind),
            input_json,
            sequence,
        };
        if message.encoded_len() > MAX_FRAME_BYTES {
            return Err(format!(
                "code-mode tool invocation exceeds the {MAX_FRAME_BYTES}-byte gRPC message limit"
            ));
        }

        let mut reservations =
            subscriptions
                .into_iter()
                .map(|(id, sender)| async move {
                    sender.reserve_owned().await.map(|permit| (id, permit))
                })
                .collect::<FuturesUnordered<_>>();
        loop {
            let (subscription_id, permit) = tokio::select! {
                biased;
                _ = cancellation.cancelled() => {
                    return Err("code mode delegate request cancelled".to_string());
                }
                _ = self.closed.cancelled() => {
                    return Err("code-mode session closed before dispatching its tool call".to_string());
                }
                reservation = reservations.next() => match reservation {
                    Some(Ok(reservation)) => reservation,
                    Some(Err(_)) => continue,
                    None => {
                        return Err("matching code-mode tool subscriptions are unavailable".to_string());
                    }
                },
            };

            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(subscription_index) = state.subscriptions.iter().position(|subscription| {
                subscription.id == subscription_id && !subscription.sender.is_closed()
            }) else {
                continue;
            };
            let Some(execution) = state.cells.get_mut(&cell_id) else {
                return Err("code-mode cell closed before dispatching its tool call".to_string());
            };
            let sequence = execution.tool_call_sequence.checked_add(1).ok_or_else(|| {
                "code-mode execution tool-call sequence was exhausted".to_string()
            })?;
            message.sequence = sequence;
            if message.encoded_len() > MAX_FRAME_BYTES {
                return Err(format!(
                    "code-mode tool invocation exceeds the {MAX_FRAME_BYTES}-byte gRPC message limit"
                ));
            }
            execution.tool_call_sequence = sequence;
            state.pending_invocations.insert(
                invocation_id,
                PendingInvocation {
                    subscription_id,
                    response,
                },
            );
            state.seen_invocations.remember(invocation_id);
            state.next_subscription = (subscription_index + 1) % state.subscriptions.len();
            permit.send(Ok(message));
            return Ok(());
        }
    }

    pub(super) fn complete_invocation(
        &self,
        invocation_id: Uuid,
        result: Result<JsonValue, String>,
    ) -> Result<(), Status> {
        let response = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match state.pending_invocations.remove(&invocation_id) {
                Some(invocation) => Some(invocation.response),
                None if state.seen_invocations.contains(&invocation_id) => None,
                None => {
                    return Err(Status::not_found(format!(
                        "unknown code-mode tool invocation {invocation_id}"
                    )));
                }
            }
        };
        if let Some(response) = response {
            let _ = response.send(result);
        }
        Ok(())
    }

    pub(super) fn cancel_invocation(&self, invocation_id: Uuid) {
        let pending = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending_invocations
            .remove(&invocation_id);
        if pending.is_some() {
            let _ = self.send_event_now(
                proto::session_event::Event::ToolCallCancelled(proto::ToolCallCancelled {
                    invocation_id: invocation_id.to_string(),
                }),
                /*cell_permit*/ None,
            );
        }
    }

    pub(super) async fn begin_notification(
        &self,
        notification_id: Uuid,
        notification: proto::Notification,
        response: oneshot::Sender<()>,
        cancellation: &CancellationToken,
    ) -> Result<(), String> {
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state
                .pending_notifications
                .insert(notification_id, response);
            state.seen_notifications.remember(notification_id);
        }
        if let Err(error) = self
            .send_event(
                proto::session_event::Event::Notification(notification),
                cancellation,
            )
            .await
        {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pending_notifications
                .remove(&notification_id);
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn acknowledge_notification(&self, notification_id: Uuid) -> Result<(), Status> {
        let response = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match state.pending_notifications.remove(&notification_id) {
                Some(response) => Some(response),
                None if state.seen_notifications.contains(&notification_id) => None,
                None => {
                    return Err(Status::not_found(format!(
                        "unknown code-mode notification {notification_id}"
                    )));
                }
            }
        };
        if let Some(response) = response {
            let _ = response.send(());
        }
        Ok(())
    }

    pub(super) fn cancel_notification(&self, notification_id: Uuid) {
        let pending = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending_notifications
            .remove(&notification_id);
        if pending.is_some() {
            let _ = self.send_event_now(
                proto::session_event::Event::NotificationCancelled(proto::NotificationCancelled {
                    notification_id: notification_id.to_string(),
                }),
                /*cell_permit*/ None,
            );
        }
    }
}
