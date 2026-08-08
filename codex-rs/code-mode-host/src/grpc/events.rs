use std::sync::Arc;

use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use prost::Message;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::TryAcquireError;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::Status;

use crate::MAX_ACTIVE_CELLS;

const MAX_BUFFERED_CONTROL_EVENTS: usize = MAX_PENDING_DELEGATE_CALLS * 2 + MAX_ACTIVE_CELLS;

#[derive(Clone)]
pub(super) struct EventSender {
    sender: mpsc::UnboundedSender<QueuedEvent>,
    permits: Arc<Semaphore>,
    closed: CancellationToken,
    writer: TaskTracker,
}

struct QueuedEvent {
    message: proto::SessionEvent,
    _queue_permit: OwnedSemaphorePermit,
    _cell_permit: Option<OwnedSemaphorePermit>,
}

impl EventSender {
    pub(super) fn new(
        output: mpsc::Sender<Result<proto::SessionEvent, Status>>,
        closed: CancellationToken,
    ) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<QueuedEvent>();
        let writer_closed = closed.clone();
        let writer = TaskTracker::new();
        writer.spawn(async move {
            loop {
                let event = tokio::select! {
                    _ = writer_closed.cancelled() => return,
                    event = receiver.recv() => match event {
                        Some(event) => event,
                        None => return,
                    },
                };
                let QueuedEvent {
                    message,
                    _queue_permit,
                    _cell_permit,
                } = event;
                tokio::select! {
                    _ = writer_closed.cancelled() => return,
                    result = output.send(Ok(message)) => {
                        if result.is_err() {
                            writer_closed.cancel();
                            return;
                        }
                    }
                }
                drop(_queue_permit);
                drop(_cell_permit);
            }
        });
        writer.close();
        Self {
            sender,
            permits: Arc::new(Semaphore::new(MAX_BUFFERED_CONTROL_EVENTS)),
            closed,
            writer,
        }
    }

    pub(super) async fn shutdown(&self) {
        self.closed.cancel();
        self.writer.wait().await;
    }

    pub(super) async fn send(
        &self,
        event: proto::session_event::Event,
        cancellation: &CancellationToken,
    ) -> Result<(), String> {
        let message = validate_event(event)?;
        let permit = tokio::select! {
            biased;
            _ = self.closed.cancelled() => {
                return Err("code-mode session event stream is closed".to_string());
            }
            _ = cancellation.cancelled() => {
                return Err("code-mode session event was cancelled".to_string());
            }
            permit = Arc::clone(&self.permits).acquire_owned() => permit
                .map_err(|_| "code-mode session event queue is closed".to_string())?,
        };
        self.enqueue(message, permit, /*cell_permit*/ None)
    }

    pub(super) fn send_now(
        &self,
        event: proto::session_event::Event,
        cell_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<(), String> {
        let message = validate_event(event)?;
        if self.closed.is_cancelled() {
            return Err("code-mode session event stream is closed".to_string());
        }
        match Arc::clone(&self.permits).try_acquire_owned() {
            Ok(permit) => self.enqueue(message, permit, cell_permit),
            Err(TryAcquireError::NoPermits) => {
                self.closed.cancel();
                Err("code-mode session event queue is full".to_string())
            }
            Err(TryAcquireError::Closed) => {
                self.closed.cancel();
                Err("code-mode session event queue is closed".to_string())
            }
        }
    }

    fn enqueue(
        &self,
        message: proto::SessionEvent,
        permit: OwnedSemaphorePermit,
        cell_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<(), String> {
        self.sender
            .send(QueuedEvent {
                message,
                _queue_permit: permit,
                _cell_permit: cell_permit,
            })
            .map_err(|_| {
                self.closed.cancel();
                "code-mode session event stream is closed".to_string()
            })
    }
}

fn validate_event(event: proto::session_event::Event) -> Result<proto::SessionEvent, String> {
    let message = proto::SessionEvent { event: Some(event) };
    if message.encoded_len() > MAX_FRAME_BYTES {
        return Err(format!(
            "code-mode session event exceeds the {MAX_FRAME_BYTES}-byte gRPC message limit"
        ));
    }
    Ok(message)
}
