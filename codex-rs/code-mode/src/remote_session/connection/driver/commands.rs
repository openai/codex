use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::WireWaitRequest;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::ConnectionDriver;
use super::cell_ids::remote_cell_id;
use super::cell_ids::remote_wait_request;
use super::types::CancellableRequest;
use super::types::DeferredWait;
use super::types::DeliveredExecute;
use super::types::DriverCommand;
use super::types::PendingRequest;
use super::types::RemoteSession;

impl ConnectionDriver {
    pub(super) fn handle_command(&mut self, command: DriverCommand) -> bool {
        match command {
            DriverCommand::OpenSession {
                session,
                delegate,
                cleanup,
                caller_cancellation,
                response_tx,
            } => self.open_session(session, delegate, cleanup, caller_cancellation, response_tx),
            DriverCommand::Execute {
                session,
                request,
                caller_cancellation,
                response_tx,
            } => self.execute(session, request, caller_cancellation, response_tx),
            DriverCommand::Wait {
                session,
                request,
                caller_cancellation,
                response_tx,
            } => self.wait(session, request, caller_cancellation, response_tx),
            DriverCommand::Terminate {
                session,
                cell_id,
                response_tx,
            } => self.terminate(session, cell_id, response_tx),
            DriverCommand::ShutdownSession {
                session,
                response_tx,
            } => self.shutdown_session(session, response_tx),
        }
    }

    fn open_session(
        &mut self,
        session: RemoteSession,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        cleanup: super::cleanup::SessionCleanup,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<(), String>>,
    ) -> bool {
        if self.sessions.contains(&session.id) || self.requests.contains_pending_open(&session) {
            let _ = response_tx.send(Err(format!(
                "code-mode session {} is already open",
                session.id
            )));
            return true;
        }
        let request_id = match self.requests.allocate_id() {
            Ok(id) => id,
            Err(err) => {
                let _ = response_tx.send(Err(err));
                return false;
            }
        };
        let message = ClientToHost::Request {
            id: request_id,
            request: HostRequest::OpenSession {
                session_id: session.id.clone(),
            },
        };
        let frame = match EncodedFrame::encode(&message) {
            Ok(frame) => frame,
            Err(err) => {
                let _ = response_tx.send(Err(format!(
                    "failed to encode code-mode open-session request: {err}"
                )));
                return true;
            }
        };
        let cancellation = CancellableRequest::new(caller_cancellation);
        self.requests.insert_pending(
            request_id,
            PendingRequest::OpenSession {
                session,
                delegate,
                cleanup,
                cancellation,
                response_tx,
            },
            &self.event_tx,
        );
        self.queue_frame(frame)
    }

    fn execute(
        &mut self,
        session: RemoteSession,
        request: ExecuteRequest,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<DeliveredExecute, String>>,
    ) -> bool {
        if let Err(err) = self.sessions.require_ready(&session) {
            let _ = response_tx.send(Err(err));
            return true;
        }
        let request = match request.try_into() {
            Ok(request) => request,
            Err(err) => {
                let _ = response_tx.send(Err(format!(
                    "failed to encode code-mode execute request: {err}"
                )));
                return true;
            }
        };
        let request_id = match self.requests.allocate_id() {
            Ok(id) => id,
            Err(err) => {
                let _ = response_tx.send(Err(err));
                return false;
            }
        };
        let message = ClientToHost::Request {
            id: request_id,
            request: HostRequest::Execute {
                session_id: session.id.clone(),
                request,
            },
        };
        let frame = match EncodedFrame::encode(&message) {
            Ok(frame) => frame,
            Err(err) => {
                let _ = response_tx.send(Err(format!(
                    "code-mode execute request exceeds the IPC frame limit: {err}"
                )));
                return true;
            }
        };
        let (initial_response_tx, initial_response_rx) = oneshot::channel();
        let cancellation = CancellableRequest::new(caller_cancellation);
        self.requests.insert_pending(
            request_id,
            PendingRequest::Execute {
                session,
                response_tx,
                initial_response_tx,
                initial_response_rx,
                cancellation,
            },
            &self.event_tx,
        );
        self.queue_frame(frame)
    }

    fn wait(
        &mut self,
        session: RemoteSession,
        request: WaitRequest,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    ) -> bool {
        if let Err(err) = self.sessions.require_ready(&session) {
            let _ = response_tx.send(Err(err));
            return true;
        }
        let request = match remote_wait_request(&session, request) {
            Ok(request) => request,
            Err(err) => {
                let _ = response_tx.send(Err(err));
                return true;
            }
        };
        if self.requests.has_cancelled_wait(&session, &request.cell_id) {
            self.requests.push_deferred_wait(DeferredWait {
                session,
                request,
                caller_cancellation,
                response_tx,
            });
            return true;
        }
        self.start_wait(session, request, caller_cancellation, response_tx)
    }

    pub(super) fn start_wait(
        &mut self,
        session: RemoteSession,
        request: WireWaitRequest,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    ) -> bool {
        let cell_id = request.cell_id.clone();
        self.send_request(
            HostRequest::Wait {
                session_id: session.id.clone(),
                request,
            },
            PendingRequest::Wait {
                session,
                cell_id,
                cancellation: CancellableRequest::new(caller_cancellation),
                response_tx,
            },
        )
    }

    fn terminate(
        &mut self,
        session: RemoteSession,
        cell_id: CellId,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    ) -> bool {
        if let Err(err) = self.sessions.require_ready(&session) {
            let _ = response_tx.send(Err(err));
            return true;
        }
        let cell_id = match remote_cell_id(&session, &cell_id) {
            Ok(cell_id) => cell_id,
            Err(err) => {
                let _ = response_tx.send(Err(err));
                return true;
            }
        };
        let pending_cell_id = cell_id.clone();
        self.send_request(
            HostRequest::Terminate {
                session_id: session.id.clone(),
                cell_id,
            },
            PendingRequest::Terminate {
                session,
                cell_id: pending_cell_id,
                response_tx,
            },
        )
    }

    fn shutdown_session(
        &mut self,
        session: RemoteSession,
        response_tx: oneshot::Sender<Result<(), String>>,
    ) -> bool {
        if let Err(err) = self.sessions.begin_shutdown(&session) {
            let _ = response_tx.send(Err(err));
            return true;
        }
        self.send_request(
            HostRequest::ShutdownSession {
                session_id: session.id.clone(),
            },
            PendingRequest::ShutdownSession {
                session,
                response_tx,
            },
        )
    }

    pub(super) fn send_request(&mut self, request: HostRequest, pending: PendingRequest) -> bool {
        let request_id = match self.requests.allocate_id() {
            Ok(id) => id,
            Err(err) => {
                pending.fail(err);
                return false;
            }
        };
        let message = ClientToHost::Request {
            id: request_id,
            request,
        };
        let frame = match EncodedFrame::encode(&message) {
            Ok(frame) => frame,
            Err(err) => {
                pending.fail(format!(
                    "code-mode request exceeds the IPC frame limit: {err}"
                ));
                return true;
            }
        };
        self.requests
            .insert_pending(request_id, pending, &self.event_tx);
        self.queue_frame(frame)
    }
}
