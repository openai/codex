use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireCellId;
use codex_code_mode_protocol::host::WireResult;
use codex_code_mode_protocol::host::WireWaitRequest;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use self::cell_ids::public_cell_id;
use self::cell_ids::public_cell_id_from_protocol;
use self::cell_ids::public_runtime_response;
use self::cell_ids::public_wait_outcome;
use self::cell_ids::remote_cell_id;
use self::cell_ids::remote_wait_request;
use self::cell_ids::runtime_response_cell_id;
use self::cell_ids::wait_outcome_cell_id;
use self::types::CancellableRequest;
use self::types::DeferredWait;
use self::types::DelegateCall;
use self::types::DelegateTask;
use self::types::DeliveredExecute;
pub(super) use self::types::DriverCommand;
pub(super) use self::types::DriverEvent;
use self::types::InitialResponse;
use self::types::PendingRequest;
pub(in crate::remote_session) use self::types::RemoteSession;
use self::types::SessionPhase;
use self::types::SessionRecord;
use self::types::UnclaimedExecute;

mod cell_ids;
mod types;

const MAX_RECENT_DELEGATE_REQUEST_IDS: usize = 4096;

pub(super) struct ConnectionDriver {
    command_rx: mpsc::Receiver<DriverCommand>,
    event_rx: mpsc::Receiver<DriverEvent>,
    event_tx: mpsc::Sender<DriverEvent>,
    execute_claim_rx: mpsc::UnboundedReceiver<RequestId>,
    outgoing_tx: mpsc::Sender<EncodedFrame>,
    pending: HashMap<RequestId, PendingRequest>,
    unclaimed_executes: HashMap<RequestId, UnclaimedExecute>,
    initial_responses: HashMap<RequestId, InitialResponse>,
    sessions: HashMap<SessionId, SessionRecord>,
    delegate_requests: HashMap<DelegateRequestId, DelegateCall>,
    seen_delegate_requests: HashSet<DelegateRequestId>,
    delegate_request_order: VecDeque<DelegateRequestId>,
    deferred_waits: VecDeque<DeferredWait>,
    next_request_id: i64,
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
        alive: Arc<AtomicBool>,
        failure: Arc<std::sync::Mutex<Option<String>>>,
        cancellation: CancellationToken,
    ) -> (Self, mpsc::UnboundedSender<RequestId>) {
        let (execute_claim_tx, execute_claim_rx) = mpsc::unbounded_channel();
        (
            Self {
                command_rx,
                event_rx,
                event_tx,
                execute_claim_rx,
                outgoing_tx,
                pending: HashMap::new(),
                unclaimed_executes: HashMap::new(),
                initial_responses: HashMap::new(),
                sessions: HashMap::new(),
                delegate_requests: HashMap::new(),
                seen_delegate_requests: HashSet::new(),
                delegate_request_order: VecDeque::new(),
                deferred_waits: VecDeque::new(),
                next_request_id: 1,
                alive,
                failure,
                cancellation,
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
                    self.unclaimed_executes.remove(&request_id);
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

    fn handle_command(&mut self, command: DriverCommand) -> bool {
        match command {
            DriverCommand::OpenSession {
                session,
                delegate,
                caller_cancellation,
                response_tx,
            } => self.open_session(session, delegate, caller_cancellation, response_tx),
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
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<(), String>>,
    ) -> bool {
        if self.sessions.contains_key(&session.id)
            || self.pending.values().any(|pending| {
                matches!(
                    pending,
                    PendingRequest::OpenSession {
                        session: pending_session,
                        ..
                    } if pending_session.id == session.id
                )
            })
        {
            let _ = response_tx.send(Err(format!(
                "code-mode session {} is already open",
                session.id
            )));
            return true;
        }
        let request_id = match self.request_id() {
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
        cancellation.spawn_watcher(request_id, self.event_tx.clone());
        self.pending.insert(
            request_id,
            PendingRequest::OpenSession {
                session,
                delegate,
                cancellation,
                response_tx,
            },
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
        if let Err(err) = self.require_ready_session(&session) {
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
        let request_id = match self.request_id() {
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
        cancellation.spawn_watcher(request_id, self.event_tx.clone());
        self.pending.insert(
            request_id,
            PendingRequest::Execute {
                session,
                response_tx,
                initial_response_tx,
                initial_response_rx,
                cancellation,
            },
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
        if let Err(err) = self.require_ready_session(&session) {
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
        if self.has_cancelled_wait(&session, &request.cell_id) {
            self.deferred_waits.push_back(DeferredWait {
                session,
                request,
                caller_cancellation,
                response_tx,
            });
            return true;
        }
        self.start_wait(session, request, caller_cancellation, response_tx)
    }

    fn start_wait(
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

    fn has_cancelled_wait(&self, session: &RemoteSession, cell_id: &WireCellId) -> bool {
        self.pending.values().any(|pending| {
            matches!(
                pending,
                PendingRequest::Wait {
                    session: pending_session,
                    cell_id: pending_cell_id,
                    cancellation,
                    ..
                } if pending_session == session
                    && pending_cell_id == cell_id
                    && cancellation.is_cancelled()
            )
        })
    }

    fn terminate(
        &mut self,
        session: RemoteSession,
        cell_id: CellId,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    ) -> bool {
        if let Err(err) = self.require_ready_session(&session) {
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
        let Some(record) = self.sessions.get_mut(&session.id) else {
            let _ = response_tx.send(Err(format!("unknown code-mode session {}", session.id)));
            return true;
        };
        if record.remote != session {
            let _ = response_tx.send(Err("stale code-mode session generation".to_string()));
            return true;
        }
        if record.phase == SessionPhase::Closing {
            let _ = response_tx.send(Err("code-mode session is already closing".to_string()));
            return true;
        }
        record.phase = SessionPhase::Closing;
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

    fn send_request(&mut self, request: HostRequest, pending: PendingRequest) -> bool {
        let request_id = match self.request_id() {
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
        self.pending.insert(request_id, pending);
        let event_tx = self.event_tx.clone();
        if let Some(cancellation) = self
            .pending
            .get_mut(&request_id)
            .and_then(PendingRequest::cancellation_mut)
        {
            cancellation.spawn_watcher(request_id, event_tx);
        }
        self.queue_frame(frame)
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

    fn flush_deferred_waits(&mut self) -> bool {
        let mut deferred = std::mem::take(&mut self.deferred_waits);
        while let Some(wait) = deferred.pop_front() {
            if wait.caller_cancellation.is_cancelled() {
                let _ = wait
                    .response_tx
                    .send(Err("code-mode request cancelled".to_string()));
                continue;
            }
            if self.has_cancelled_wait(&wait.session, &wait.request.cell_id) {
                self.deferred_waits.push_back(wait);
                continue;
            }
            if !self.start_wait(
                wait.session,
                wait.request,
                wait.caller_cancellation,
                wait.response_tx,
            ) {
                for wait in deferred {
                    let _ = wait
                        .response_tx
                        .send(Err("code-mode host connection closed".to_string()));
                }
                return false;
            }
        }
        true
    }

    fn handle_host_message(&mut self, message: HostToClient) -> bool {
        match message {
            HostToClient::Response { id, result } => {
                self.complete_request(id, result.into_result())
            }
            HostToClient::InitialResponse { id, result } => {
                self.complete_initial_response(id, result.into_result())
            }
            HostToClient::DelegateRequest {
                id,
                session_id,
                request,
            } => self.start_delegate(id, session_id, request),
            HostToClient::CancelDelegateRequest { id } => {
                if let Some(call) = self.delegate_requests.remove(&id) {
                    call.cancellation.cancel();
                }
                true
            }
            HostToClient::CellClosed {
                session_id,
                cell_id,
            } => self.close_cell(session_id, cell_id),
            HostToClient::HostHello(_) | HostToClient::HandshakeRejected { .. } => {
                self.fail("code-mode host sent a second handshake response".to_string());
                false
            }
        }
    }

    fn complete_request(&mut self, id: RequestId, result: Result<HostResponse, String>) -> bool {
        let Some(pending) = self.pending.remove(&id) else {
            self.fail(format!("code-mode host returned unknown request ID {id:?}"));
            return false;
        };
        match pending {
            PendingRequest::OpenSession {
                session,
                delegate,
                cancellation,
                response_tx,
            } => match result {
                Ok(HostResponse::SessionReady { session_id }) if session_id == session.id => {
                    let abandoned = cancellation.is_cancelled() || response_tx.is_closed();
                    self.sessions.insert(
                        session.id.clone(),
                        SessionRecord {
                            remote: session.clone(),
                            delegate,
                            phase: SessionPhase::Ready,
                            cells: HashMap::new(),
                        },
                    );
                    if abandoned || response_tx.send(Ok(())).is_err() {
                        return self.shutdown_abandoned_session(session);
                    }
                }
                Ok(_) => {
                    let reason =
                        "code-mode host returned an invalid open-session response".to_string();
                    let _ = response_tx.send(Err(reason.clone()));
                    self.fail(reason);
                    return false;
                }
                Err(err) => {
                    let _ = response_tx.send(Err(err));
                }
            },
            PendingRequest::Execute {
                session,
                response_tx,
                initial_response_tx,
                initial_response_rx,
                cancellation,
            } => {
                let Some(record) = self.sessions.get_mut(&session.id) else {
                    let _ = response_tx
                        .send(Err("code-mode session closed during execute".to_string()));
                    return true;
                };
                match result {
                    Ok(HostResponse::ExecutionStarted { cell_id }) => {
                        // The host owns a checked, never-reused ID sequence. Retain only live
                        // IDs so client memory scales with concurrency, not session lifetime.
                        if record.cells.contains_key(&cell_id) {
                            let reason = format!(
                                "code-mode host reused live cell {} in session {}",
                                cell_id.as_str(),
                                session.id
                            );
                            let _ = response_tx.send(Err(reason.clone()));
                            self.fail(reason);
                            return false;
                        }
                        let public_id = public_cell_id(session.generation, &cell_id);
                        let remote_cell_id = cell_id.clone();
                        record.cells.insert(cell_id.clone(), public_id.clone());
                        self.initial_responses.insert(
                            id,
                            InitialResponse {
                                generation: session.generation,
                                cell_id,
                                response_tx: initial_response_tx,
                            },
                        );
                        let started =
                            StartedCell::from_result_receiver(public_id, initial_response_rx);
                        if cancellation.is_cancelled() || response_tx.is_closed() {
                            return self.terminate_abandoned_cell(session, remote_cell_id);
                        }
                        let delivered = DeliveredExecute {
                            request_id: id,
                            started,
                        };
                        if response_tx.send(Ok(delivered)).is_err() {
                            return self.terminate_abandoned_cell(session, remote_cell_id);
                        }
                        self.unclaimed_executes.insert(
                            id,
                            UnclaimedExecute {
                                session,
                                cell_id: remote_cell_id,
                                cancellation,
                            },
                        );
                    }
                    Ok(_) => {
                        let reason =
                            "code-mode host returned an invalid execute response".to_string();
                        let _ = response_tx.send(Err(reason.clone()));
                        self.fail(reason);
                        return false;
                    }
                    Err(err) => {
                        let _ = response_tx.send(Err(err));
                    }
                }
            }
            PendingRequest::Wait {
                session,
                cell_id,
                cancellation: _,
                response_tx,
            }
            | PendingRequest::Terminate {
                session,
                cell_id,
                response_tx,
            } => {
                let result = match result {
                    Ok(HostResponse::WaitCompleted { outcome }) => {
                        if wait_outcome_cell_id(&outcome) != &cell_id {
                            let reason = format!(
                                "code-mode host returned cell {} for request targeting {}",
                                wait_outcome_cell_id(&outcome).as_str(),
                                cell_id.as_str()
                            );
                            let _ = response_tx.send(Err(reason.clone()));
                            self.fail(reason);
                            return false;
                        }
                        Ok(public_wait_outcome(session.generation, outcome.into()))
                    }
                    Ok(_) => {
                        let reason = "code-mode host returned an invalid cell response".to_string();
                        let _ = response_tx.send(Err(reason.clone()));
                        self.fail(reason);
                        return false;
                    }
                    Err(err) => Err(err),
                };
                let _ = response_tx.send(result);
            }
            PendingRequest::ShutdownSession {
                session,
                response_tx,
            } => {
                let result = match result {
                    Ok(HostResponse::SessionClosed { session_id }) if session_id == session.id => {
                        self.close_session_locally(&session.id);
                        Ok(())
                    }
                    Ok(_) => {
                        Err("code-mode host returned an invalid shutdown response".to_string())
                    }
                    Err(err) => Err(err),
                };
                match result {
                    Ok(()) => {
                        let _ = response_tx.send(Ok(()));
                    }
                    Err(err) => {
                        let _ = response_tx.send(Err(err.clone()));
                        self.fail(err);
                        return false;
                    }
                }
            }
        }
        true
    }

    fn cancel_dropped_callers(&mut self) -> bool {
        let cancelled = self
            .pending
            .iter_mut()
            .filter_map(|(id, pending)| {
                let cancellation = pending.cancellation_mut()?;
                (cancellation.is_cancelled() && cancellation.mark_reported()).then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in cancelled {
            if !self.send_cancel_request(id) {
                return false;
            }
        }
        let cancelled_executes = self
            .unclaimed_executes
            .iter_mut()
            .filter_map(|(id, execute)| {
                (execute.cancellation.is_cancelled() && execute.cancellation.mark_reported())
                    .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in cancelled_executes {
            if !self.cancel_unclaimed_execute(id) {
                return false;
            }
        }
        true
    }

    fn cancel_request(&mut self, id: RequestId) -> bool {
        if let Some(cancellation) = self
            .pending
            .get_mut(&id)
            .and_then(PendingRequest::cancellation_mut)
        {
            return !cancellation.mark_reported() || self.send_cancel_request(id);
        }
        let Some(execute) = self.unclaimed_executes.get_mut(&id) else {
            return true;
        };
        if !execute.cancellation.mark_reported() {
            return true;
        }
        self.cancel_unclaimed_execute(id)
    }

    fn cancel_unclaimed_execute(&mut self, id: RequestId) -> bool {
        let Some(execute) = self.unclaimed_executes.remove(&id) else {
            return true;
        };
        if !self.send_cancel_request(id) {
            return false;
        }
        self.terminate_abandoned_cell(execute.session, execute.cell_id)
    }

    fn send_cancel_request(&mut self, id: RequestId) -> bool {
        let frame = match EncodedFrame::encode(&ClientToHost::CancelRequest { id }) {
            Ok(frame) => frame,
            Err(err) => {
                self.fail(format!(
                    "failed to encode code-mode cancellation request: {err}"
                ));
                return false;
            }
        };
        self.queue_frame(frame)
    }

    fn shutdown_abandoned_session(&mut self, session: RemoteSession) -> bool {
        let Some(record) = self.sessions.get_mut(&session.id) else {
            self.fail(format!(
                "code-mode host committed abandoned session {} without local state",
                session.id
            ));
            return false;
        };
        if record.phase == SessionPhase::Closing {
            return true;
        }
        record.phase = SessionPhase::Closing;
        let (response_tx, response_rx) = oneshot::channel();
        drop(response_rx);
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

    fn terminate_abandoned_cell(&mut self, session: RemoteSession, cell_id: WireCellId) -> bool {
        let Some(record) = self.sessions.get(&session.id) else {
            self.fail(format!(
                "code-mode host admitted an abandoned cell in unknown session {}",
                session.id
            ));
            return false;
        };
        if record.phase == SessionPhase::Closing {
            return true;
        }
        let (response_tx, response_rx) = oneshot::channel();
        drop(response_rx);
        self.send_request(
            HostRequest::Terminate {
                session_id: session.id.clone(),
                cell_id: cell_id.clone(),
            },
            PendingRequest::Terminate {
                session,
                cell_id,
                response_tx,
            },
        )
    }

    fn complete_initial_response(
        &mut self,
        id: RequestId,
        result: Result<codex_code_mode_protocol::host::WireRuntimeResponse, String>,
    ) -> bool {
        let Some(initial) = self.initial_responses.remove(&id) else {
            self.fail(format!(
                "code-mode host returned initial response for unknown request ID {id:?}"
            ));
            return false;
        };
        let response = match result {
            Ok(response) if runtime_response_cell_id(&response) == &initial.cell_id => {
                Ok(public_runtime_response(initial.generation, response.into()))
            }
            Ok(response) => {
                let reason = format!(
                    "code-mode host returned initial response for cell {} instead of {}",
                    runtime_response_cell_id(&response).as_str(),
                    initial.cell_id.as_str()
                );
                let _ = initial.response_tx.send(Err(reason.clone()));
                self.fail(reason);
                return false;
            }
            Err(err) => Err(err),
        };
        let _ = initial.response_tx.send(response);
        true
    }

    fn start_delegate(
        &mut self,
        id: DelegateRequestId,
        session_id: SessionId,
        request: DelegateRequest,
    ) -> bool {
        if self.delegate_requests.contains_key(&id) || self.seen_delegate_requests.contains(&id) {
            self.fail(format!("duplicate code-mode delegate request ID {id:?}"));
            return false;
        }
        self.seen_delegate_requests.insert(id);
        self.delegate_request_order.push_back(id);
        while self.delegate_request_order.len() > MAX_RECENT_DELEGATE_REQUEST_IDS {
            if let Some(expired) = self.delegate_request_order.pop_front() {
                self.seen_delegate_requests.remove(&expired);
            }
        }
        let Some(session) = self.sessions.get(&session_id) else {
            self.fail(format!(
                "code-mode host delegated for unknown session {session_id}"
            ));
            return false;
        };
        let generation = session.remote.generation;
        let wire_cell_id = match &request {
            DelegateRequest::InvokeTool { invocation } => &invocation.cell_id,
            DelegateRequest::Notify { cell_id, .. } => cell_id,
        };
        if !session.cells.contains_key(wire_cell_id) {
            self.fail(format!(
                "code-mode host delegated for unknown cell {} in session {session_id}",
                wire_cell_id.as_str()
            ));
            return false;
        }
        let delegate = Arc::clone(&session.delegate);
        let cancellation = CancellationToken::new();
        let (cell_id, task_request) = match request {
            DelegateRequest::InvokeTool { invocation } => {
                let mut invocation: CodeModeNestedToolCall = invocation.into();
                invocation.cell_id = public_cell_id_from_protocol(generation, &invocation.cell_id);
                let cell_id = invocation.cell_id.clone();
                (cell_id, DelegateTask::InvokeTool(invocation))
            }
            DelegateRequest::Notify {
                call_id,
                cell_id,
                text,
            } => {
                let cell_id = public_cell_id(generation, &cell_id);
                (
                    cell_id.clone(),
                    DelegateTask::Notify {
                        call_id,
                        cell_id,
                        text,
                    },
                )
            }
        };
        self.delegate_requests.insert(
            id,
            DelegateCall {
                session_id,
                cell_id,
                cancellation: cancellation.clone(),
            },
        );
        let delegate_task = tokio::spawn(async move {
            match task_request {
                DelegateTask::InvokeTool(invocation) => delegate
                    .invoke_tool(invocation, cancellation)
                    .await
                    .map(|result| DelegateResponse::ToolResult { result }),
                DelegateTask::Notify {
                    call_id,
                    cell_id,
                    text,
                } => delegate
                    .notify(call_id, cell_id, text, cancellation)
                    .await
                    .map(|()| DelegateResponse::NotificationDelivered),
            }
        });
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let result = match delegate_task.await {
                Ok(result) => result,
                Err(err) => Err(format!("code-mode delegate task failed: {err}")),
            };
            let _ = event_tx
                .send(DriverEvent::DelegateCompleted { id, result })
                .await;
        });
        true
    }

    fn complete_delegate(
        &mut self,
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    ) -> bool {
        if self.delegate_requests.remove(&id).is_none() {
            return true;
        }
        self.send_delegate_response(id, result)
    }

    fn send_delegate_response(
        &mut self,
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    ) -> bool {
        let message = ClientToHost::DelegateResponse {
            id,
            result: WireResult::from_result(result),
        };
        let frame = match EncodedFrame::encode(&message) {
            Ok(frame) => frame,
            Err(err) => {
                let fallback = ClientToHost::DelegateResponse {
                    id,
                    result: WireResult::Err {
                        message: format!(
                            "code-mode delegate response exceeds the IPC frame limit: {err}"
                        ),
                    },
                };
                match EncodedFrame::encode(&fallback) {
                    Ok(frame) => frame,
                    Err(fallback_err) => {
                        self.fail(format!(
                            "failed to encode code-mode delegate error response: {fallback_err}"
                        ));
                        return false;
                    }
                }
            }
        };
        self.queue_frame(frame)
    }

    fn close_cell(&mut self, session_id: SessionId, cell_id: WireCellId) -> bool {
        let closed = {
            let Some(session) = self.sessions.get_mut(&session_id) else {
                self.fail(format!(
                    "code-mode host closed cell {} in unknown session {session_id}",
                    cell_id.as_str()
                ));
                return false;
            };
            let public_id = public_cell_id(session.remote.generation, &cell_id);
            session
                .cells
                .remove(&cell_id)
                .map(|_| (Arc::clone(&session.delegate), public_id))
        };
        let Some((delegate, public_id)) = closed else {
            self.fail(format!(
                "code-mode host closed unknown cell in session {session_id}"
            ));
            return false;
        };
        self.cancel_delegate_calls(&session_id, Some(&public_id));
        notify_cell_closed(&delegate, &public_id);
        true
    }

    fn close_session_locally(&mut self, session_id: &SessionId) {
        let Some(session) = self.sessions.remove(session_id) else {
            return;
        };
        self.cancel_delegate_calls(session_id, /*cell_id*/ None);
        self.unclaimed_executes
            .retain(|_, execute| &execute.session.id != session_id);
        for cell_id in session.cells.into_values() {
            notify_cell_closed(&session.delegate, &cell_id);
        }
    }

    fn cancel_delegate_calls(&mut self, session_id: &SessionId, cell_id: Option<&CellId>) {
        self.delegate_requests.retain(|_, call| {
            let matches = &call.session_id == session_id
                && cell_id.is_none_or(|cell_id| &call.cell_id == cell_id);
            if matches {
                call.cancellation.cancel();
            }
            !matches
        });
    }

    fn require_ready_session(&self, session: &RemoteSession) -> Result<(), String> {
        let record = self
            .sessions
            .get(&session.id)
            .ok_or_else(|| format!("unknown code-mode session {}", session.id))?;
        if record.remote != *session {
            return Err("stale code-mode session generation".to_string());
        }
        if record.phase != SessionPhase::Ready {
            return Err("code-mode session is shutting down".to_string());
        }
        Ok(())
    }

    fn request_id(&mut self) -> Result<RequestId, String> {
        let id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or_else(|| "code-mode host request ID space exhausted".to_string())?;
        Ok(RequestId::new(id))
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
        for (_, pending) in self.pending.drain() {
            pending.fail(reason.clone());
        }
        self.unclaimed_executes.clear();
        for (_, initial) in self.initial_responses.drain() {
            let _ = initial.response_tx.send(Err(reason.clone()));
        }
        for (_, call) in self.delegate_requests.drain() {
            call.cancellation.cancel();
        }
        for wait in self.deferred_waits.drain(..) {
            let _ = wait.response_tx.send(Err(reason.clone()));
        }
        let sessions = std::mem::take(&mut self.sessions);
        for (_, session) in sessions {
            for cell_id in session.cells.into_values() {
                notify_cell_closed(&session.delegate, &cell_id);
            }
        }
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
