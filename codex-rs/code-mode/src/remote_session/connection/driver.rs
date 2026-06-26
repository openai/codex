use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::RequestId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(in crate::remote_session) use self::cleanup::SessionCleanup;
use self::delegate_runtime::DelegateRuntime;
use self::request_tracker::RequestTracker;
use self::session_registry::SessionRegistry;
pub(super) use self::types::DriverCommand;
pub(super) use self::types::DriverEvent;
pub(in crate::remote_session) use self::types::RemoteSession;

mod cell_ids;
mod cleanup;
mod commands;
mod delegate_runtime;
mod request_tracker;
mod responses;
mod session_registry;
mod types;

pub(super) struct DriverLifecycle {
    pub(super) alive: Arc<AtomicBool>,
    pub(super) failure: Arc<std::sync::Mutex<Option<String>>>,
    pub(super) cancellation: CancellationToken,
}

pub(super) struct ConnectionDriver {
    command_rx: mpsc::Receiver<DriverCommand>,
    event_rx: mpsc::Receiver<DriverEvent>,
    event_tx: mpsc::Sender<DriverEvent>,
    execute_claim_rx: mpsc::UnboundedReceiver<RequestId>,
    outgoing_tx: mpsc::Sender<EncodedFrame>,
    requests: RequestTracker,
    sessions: SessionRegistry,
    delegates: DelegateRuntime,
    alive: Arc<AtomicBool>,
    failure: Arc<std::sync::Mutex<Option<String>>>,
    cancellation: CancellationToken,
    failed: bool,
}

impl ConnectionDriver {
    pub(super) fn new(
        command_rx: mpsc::Receiver<DriverCommand>,
        event_rx: mpsc::Receiver<DriverEvent>,
        event_tx: mpsc::Sender<DriverEvent>,
        outgoing_tx: mpsc::Sender<EncodedFrame>,
        lifecycle: DriverLifecycle,
    ) -> (Self, mpsc::UnboundedSender<RequestId>) {
        let (execute_claim_tx, execute_claim_rx) = mpsc::unbounded_channel();
        (
            Self {
                command_rx,
                event_rx,
                event_tx: event_tx.clone(),
                execute_claim_rx,
                outgoing_tx,
                requests: RequestTracker::new(),
                sessions: SessionRegistry::new(),
                delegates: DelegateRuntime::new(event_tx),
                alive: lifecycle.alive,
                failure: lifecycle.failure,
                cancellation: lifecycle.cancellation,
                failed: false,
            },
            execute_claim_tx,
        )
    }

    pub(super) async fn run(mut self) {
        loop {
            tokio::select! {
                biased;
                _ = self.cancellation.cancelled() => {
                    self.fail("code-mode host connection closed".to_string());
                    return;
                }
                event = self.event_rx.recv() => {
                    let Some(event) = event else {
                        self.fail("code-mode host event stream closed".to_string());
                        return;
                    };
                    if !self.cancel_dropped_callers() || !self.handle_event(event) {
                        return;
                    }
                }
                claim = self.execute_claim_rx.recv() => {
                    let Some(request_id) = claim else {
                        self.fail("code-mode execute claim stream closed".to_string());
                        return;
                    };
                    self.requests.claim_execute(request_id);
                }
                command = self.command_rx.recv() => {
                    let Some(command) = command else {
                        self.fail("code-mode host command stream closed".to_string());
                        return;
                    };
                    if !self.cancel_dropped_callers() || !self.handle_command(command) {
                        return;
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, event: DriverEvent) -> bool {
        let keep_running = match event {
            DriverEvent::HostMessage(message) => self.handle_host_message(message),
            DriverEvent::DelegateCompleted { id, result } => self.complete_delegate(id, result),
            DriverEvent::RequestCancelled(id) => self.cancel_request(id),
            DriverEvent::Failed(reason) => {
                self.fail(reason);
                false
            }
        };
        if keep_running {
            self.flush_deferred_waits()
        } else {
            false
        }
    }

    fn queue_frame(&mut self, frame: EncodedFrame) -> bool {
        match self.outgoing_tx.try_send(frame) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.fail("code-mode host outgoing queue is full".to_string());
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.fail("code-mode host writer closed".to_string());
                false
            }
        }
    }

    fn fail(&mut self, reason: String) {
        if self.failed {
            return;
        }
        self.failed = true;
        self.alive.store(false, Ordering::Release);
        let reason = {
            let mut failure = self
                .failure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            failure.get_or_insert(reason).clone()
        };
        self.requests.fail_all(&reason);
        let failed_sessions = self.sessions.drain();
        self.delegates.fail_all(failed_sessions);
        self.cancellation.cancel();
    }
}

impl Drop for ConnectionDriver {
    fn drop(&mut self) {
        self.fail("code-mode connection driver stopped unexpectedly".to_string());
    }
}

fn notify_cell_closed(delegate: &Arc<dyn CodeModeSessionDelegate>, cell_id: &CellId) {
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| delegate.cell_closed(cell_id)));
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
