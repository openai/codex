use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::grpc;
use futures::FutureExt;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::SessionInner;
use super::completion;
use super::conversion;
use super::deadline;
use super::state::CallbackAdmission;

const MAX_NOTIFICATION_BYTES: usize = 1_024;
const TRUNCATED_NOTIFICATION_SUFFIX: &str = "... [truncated]";

impl SessionInner {
    pub(super) fn spawn_session_events(
        self: &Arc<Self>,
        events: tonic::Streaming<grpc::SessionEvent>,
    ) {
        self.spawn_stream(events, "session lease", Self::handle_session_event);
    }

    pub(super) fn spawn_tool_subscription(
        self: &Arc<Self>,
        calls: tonic::Streaming<grpc::ToolCall>,
    ) {
        self.spawn_stream(calls, "tool subscription", Self::handle_tool_call);
    }

    fn spawn_stream<T: Send + 'static>(
        self: &Arc<Self>,
        mut stream: tonic::Streaming<T>,
        stream_name: &'static str,
        handle: fn(&Arc<Self>, T) -> Result<(), String>,
    ) {
        let inner = Arc::clone(self);
        self.stream_tasks.spawn(async move {
            loop {
                let message = tokio::select! {
                    biased;
                    _ = inner.stopped.cancelled() => return,
                    message = stream.message() => message,
                };
                match message {
                    Ok(Some(message)) => {
                        if let Err(error) = handle(&inner, message) {
                            inner.fail(error);
                            return;
                        }
                    }
                    Ok(None) => {
                        if !inner.shutdown_requested.load(Ordering::Acquire) {
                            inner.fail(format!("gRPC code-mode {stream_name} closed unexpectedly"));
                        }
                        return;
                    }
                    Err(error) => {
                        if !inner.shutdown_requested.load(Ordering::Acquire) {
                            inner.fail(deadline::failure(stream_name, error));
                        }
                        return;
                    }
                }
            }
        });
    }

    fn handle_session_event(self: &Arc<Self>, event: grpc::SessionEvent) -> Result<(), String> {
        match event
            .event
            .ok_or_else(|| "gRPC code-mode host sent an empty session event".to_string())?
        {
            grpc::session_event::Event::Opened(_) => {
                Err("gRPC code-mode host repeated the session opening event".to_string())
            }
            grpc::session_event::Event::ToolCallCancelled(cancelled) => {
                self.state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .cancel_invocation(&cancelled.invocation_id)?;
                Ok(())
            }
            grpc::session_event::Event::Notification(notification) => {
                self.handle_notification(notification)
            }
            grpc::session_event::Event::NotificationCancelled(_) => Ok(()),
            grpc::session_event::Event::CellClosed(closed) => {
                let cell = self
                    .state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .close_cell(closed)?;
                self.report_closed_cell(cell);
                Ok(())
            }
        }
    }

    fn handle_tool_call(self: &Arc<Self>, call: grpc::ToolCall) -> Result<(), String> {
        if call.session_id != self.id {
            return Err(format!(
                "gRPC code-mode tool invocation belongs to session {} instead of {}",
                call.session_id, self.id
            ));
        }
        let admission = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admit_invocation(&call)?;
        let invocation_id = call.invocation_id.clone();
        let cancellation = match admission {
            CallbackAdmission::Active(cancellation) => Ok(cancellation),
            CallbackAdmission::Cancelled => return Ok(()),
            CallbackAdmission::Closed => Err(format!("code-mode cell {} is closed", call.cell_id)),
            CallbackAdmission::Rejected(error) => Err(error),
        };
        let cancellation = match cancellation {
            Ok(cancellation) => cancellation,
            Err(error) => {
                let inner = Arc::clone(self);
                tokio::spawn(async move {
                    inner
                        .complete_tool_call(invocation_id, CancellationToken::new(), Err(error))
                        .await;
                });
                return Ok(());
            }
        };
        let invocation = conversion::tool_call(call);
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            let result = match invocation {
                Ok(invocation) => {
                    let callback = AssertUnwindSafe(async {
                        inner
                            .delegate
                            .invoke_tool(invocation, cancellation.child_token())
                            .await
                    })
                    .catch_unwind();
                    tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => return,
                        result = callback => match result {
                            Ok(result) => result,
                            Err(_) => Err("code-mode tool delegate panicked".to_string()),
                        },
                    }
                }
                Err(error) => Err(error),
            };
            inner
                .complete_tool_call(invocation_id, cancellation, result)
                .await;
        });
        Ok(())
    }

    async fn complete_tool_call(
        &self,
        invocation_id: String,
        cancellation: CancellationToken,
        result: Result<serde_json::Value, String>,
    ) {
        let request = completion::request(&self.id, &invocation_id, result);
        let mut client = self.client();
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {}
            result = deadline::request(
                self,
                "tool invocation completion",
                Duration::ZERO,
                client.complete_tool_call(request),
            ) => {
                if let Err(error) = result
                    && !cancellation.is_cancelled()
                    && !self.stopped.is_cancelled()
                {
                    self.fail(error);
                }
            }
        }
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .finish_invocation(&invocation_id);
    }

    fn handle_notification(
        self: &Arc<Self>,
        mut notification: grpc::Notification,
    ) -> Result<(), String> {
        if notification.text.len() > MAX_NOTIFICATION_BYTES {
            let boundary = notification
                .text
                .floor_char_boundary(MAX_NOTIFICATION_BYTES - TRUNCATED_NOTIFICATION_SUFFIX.len());
            notification.text.truncate(boundary);
            notification.text.push_str(TRUNCATED_NOTIFICATION_SUFFIX);
        }
        let admission = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admit_notification(&notification)?;
        let cancellation = match admission {
            CallbackAdmission::Active(cancellation) => cancellation,
            CallbackAdmission::Cancelled | CallbackAdmission::Closed => return Ok(()),
            CallbackAdmission::Rejected(error) => {
                warn!("code-mode notification was dropped: {error}");
                return Ok(());
            }
        };
        let execution_id = notification.execution_id;
        let inner = Arc::clone(self);
        // Delegate callbacks stay outside the tracked session tasks so shutdown can cancel
        // them without waiting for arbitrary delegate work to complete.
        tokio::spawn(async move {
            let callback = AssertUnwindSafe(async {
                inner
                    .delegate
                    .notify(
                        notification.call_id,
                        CellId::new(notification.cell_id),
                        notification.text,
                        cancellation,
                    )
                    .await
            })
            .catch_unwind();
            let result = tokio::select! {
                biased;
                _ = inner.stopped.cancelled() => return,
                result = callback => result,
            };
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => warn!("code-mode notification delegate failed: {error}"),
                Err(_) => warn!("code-mode notification delegate panicked"),
            }
            let cell = inner
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .finish_notification(&execution_id);
            inner.report_closed_cell(cell);
        });
        Ok(())
    }
}
