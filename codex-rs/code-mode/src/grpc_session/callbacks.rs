use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::grpc;

use super::SessionInner;

impl SessionInner {
    pub(super) fn spawn_session_events(
        self: &Arc<Self>,
        mut events: tonic::Streaming<grpc::SessionEvent>,
    ) {
        let inner = Arc::clone(self);
        self.stream_tasks.spawn(async move {
            loop {
                let event = tokio::select! {
                    biased;
                    _ = inner.stopped.cancelled() => return,
                    event = events.message() => event,
                };
                match event {
                    Ok(Some(event)) => {
                        if let Err(error) = inner.handle_session_event(event) {
                            inner.fail(error);
                            return;
                        }
                    }
                    Ok(None) => {
                        if !inner.shutdown_requested.load(Ordering::Acquire) {
                            inner.fail(
                                "gRPC code-mode session lease closed unexpectedly".to_string(),
                            );
                        }
                        return;
                    }
                    Err(error) => {
                        if !inner.shutdown_requested.load(Ordering::Acquire) {
                            inner.fail(super::deadline::failure("session lease", error));
                        }
                        return;
                    }
                }
            }
        });
    }

    fn handle_session_event(&self, event: grpc::SessionEvent) -> Result<(), String> {
        match event
            .event
            .ok_or_else(|| "gRPC code-mode host sent an empty session event".to_string())?
        {
            grpc::session_event::Event::Opened(_) => {
                Err("gRPC code-mode host repeated the session opening event".to_string())
            }
            grpc::session_event::Event::ToolCallCancelled(_)
            | grpc::session_event::Event::Notification(_)
            | grpc::session_event::Event::NotificationCancelled(_) => Ok(()),
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
}
