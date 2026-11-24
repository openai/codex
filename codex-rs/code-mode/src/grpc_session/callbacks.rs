use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeToolResult;
use codex_code_mode_protocol::grpc;
use codex_protocol::protocol::W3cTraceContext;
use futures::FutureExt;
use opentelemetry::context::FutureExt as _;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::SessionInner;
use super::completion;
use super::conversion;
use super::deadline;
use super::state::CallbackAdmission;

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
                        inner.fail(format!("gRPC code-mode {stream_name} closed unexpectedly"));
                        return;
                    }
                    Err(error) => {
                        inner.fail(deadline::failure(stream_name, error));
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
            grpc::session_event::Event::NotificationCancelled(_)
            | grpc::session_event::Event::Heartbeat(_) => Ok(()),
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

    fn handle_tool_call(self: &Arc<Self>, mut call: grpc::ToolCall) -> Result<(), String> {
        if call.session_id != self.id {
            return Err(format!(
                "gRPC code-mode tool invocation belongs to session {} instead of {}",
                call.session_id, self.id
            ));
        }
        if call.heartbeat.take().is_some() {
            // A heartbeat carries only its marker and the verified session ID.
            call.session_id.clear();
            if call != grpc::ToolCall::default() {
                return Err("gRPC code-mode heartbeat carried a tool invocation".to_string());
            }
            return Ok(());
        }
        let admission = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admit_invocation(&call)?;
        let cancellation = match admission {
            CallbackAdmission::Active(cancellation, delegate, trace) => {
                Ok((cancellation, delegate, trace))
            }
            CallbackAdmission::Cancelled => return Ok(()),
            CallbackAdmission::Closed => Err(format!("code-mode cell {} is closed", call.cell_id)),
            CallbackAdmission::Rejected(error) => Err(error),
        };
        let (cancellation, delegate, trace) = match cancellation {
            Ok(cancellation) => cancellation,
            Err(error) => {
                let inner = Arc::clone(self);
                let invocation_id = call.invocation_id;
                tokio::spawn(async move {
                    inner
                        .complete_tool_call(invocation_id, CancellationToken::new(), Err(error))
                        .await;
                });
                return Ok(());
            }
        };
        // A traced host supplies the per-tool parent. A local host without an
        // exporter still shares the originating execution's context.
        let trace = call
            .traceparent
            .take()
            .map(|traceparent| W3cTraceContext {
                traceparent: Some(traceparent),
                tracestate: None,
            })
            .or(trace);
        let parent = trace
            .as_ref()
            .and_then(codex_otel::context_from_w3c_trace_context)
            .unwrap_or_default();
        tokio::spawn(
            Arc::clone(self)
                .invoke_tool(call, cancellation, delegate)
                .with_context(parent),
        );
        Ok(())
    }

    #[tracing::instrument(
        name = "code_mode.grpc.callback",
        skip_all,
        fields(
            session.id = %call.session_id,
            execution.id = %call.execution_id,
            cell.id = %call.cell_id,
            invocation.id = %call.invocation_id,
        )
    )]
    async fn invoke_tool(
        self: Arc<Self>,
        mut call: grpc::ToolCall,
        cancellation: CancellationToken,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) {
        let invocation_id = std::mem::take(&mut call.invocation_id);
        let result = match conversion::tool_call(call) {
            Ok(invocation) => {
                let callback = AssertUnwindSafe(async {
                    delegate
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
        self.complete_tool_call(invocation_id, cancellation, result)
            .await;
    }

    async fn complete_tool_call(
        &self,
        invocation_id: String,
        cancellation: CancellationToken,
        result: Result<CodeModeToolResult, String>,
    ) {
        let request = completion::request(&self.id, &invocation_id, result);
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {}
            // NotFound can mean an invocation in a live session was canceled.
            // Check cancellation before treating a completion error as fatal.
            result = async {
                let _permit = self.completion_permits.acquire().await
                    .map_err(|_| "code-mode completion queue is closed".to_string())?;
                let mut client = self.client()?;
                deadline::request(
                    self,
                    "tool invocation completion",
                    Duration::ZERO,
                    client.complete_tool_call(self.request(request)),
                ).await
            } => {
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
        notification: grpc::Notification,
    ) -> Result<(), String> {
        let admission = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admit_notification(&notification)?;
        let (cancellation, delegate, trace) = match admission {
            CallbackAdmission::Active(cancellation, delegate, trace) => {
                (cancellation, delegate, trace)
            }
            CallbackAdmission::Cancelled | CallbackAdmission::Closed => return Ok(()),
            CallbackAdmission::Rejected(error) => {
                warn!("code-mode notification was dropped: {error}");
                return Ok(());
            }
        };
        let parent = trace
            .as_ref()
            .and_then(codex_otel::context_from_w3c_trace_context)
            .unwrap_or_default();
        // Delegate callbacks stay outside the tracked session tasks so shutdown can cancel
        // them without waiting for arbitrary delegate work to complete.
        tokio::spawn(
            Arc::clone(self)
                .deliver_notification(notification, cancellation, delegate)
                .with_context(parent),
        );
        Ok(())
    }

    #[tracing::instrument(
        name = "code_mode.grpc.notify",
        skip_all,
        fields(
            session.id = %self.id,
            execution.id = %notification.execution_id,
            cell.id = %notification.cell_id,
        )
    )]
    async fn deliver_notification(
        self: Arc<Self>,
        notification: grpc::Notification,
        cancellation: CancellationToken,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) {
        let callback = AssertUnwindSafe(async {
            delegate
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
            _ = self.stopped.cancelled() => return,
            result = callback => result,
        };
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => warn!("code-mode notification delegate failed: {error}"),
            Err(_) => warn!("code-mode notification delegate panicked"),
        }
        let cell = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .finish_notification(&notification.execution_id);
        self.report_closed_cell(cell);
    }
}

#[cfg(test)]
#[path = "callbacks_tests.rs"]
mod tests;
