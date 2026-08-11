use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::DEFAULT_EXEC_YIELD_TIME_MS;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::grpc;
use tokio::sync::OwnedMutexGuard;
use tokio::sync::oneshot;
use tracing::debug;
use uuid::Uuid;

use super::SessionInner;
use super::conversion;
use super::deadline;

pub(super) struct WaitSlot {
    lock: Arc<tokio::sync::Mutex<()>>,
    active: AtomicBool,
}

struct ExecutionOwnership {
    session: Arc<SessionInner>,
    execution_id: String,
    armed: bool,
}

impl Drop for ExecutionOwnership {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let cell = self
            .session
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove_execution(&self.execution_id);
        let Some(cell_id) = cell else {
            return;
        };
        self.session.report_closed_cell(Some(cell_id.clone()));
        if self.session.stopped.is_cancelled() {
            return;
        }
        let session = Arc::clone(&self.session);
        self.session.runtime.spawn(async move {
            if let Err(error) = session.terminate(cell_id).await
                && !session.stopped.is_cancelled()
            {
                debug!("abandoned code-mode execution termination raced closure: {error}");
            }
        });
    }
}

impl SessionInner {
    pub(super) async fn execute(
        self: &Arc<Self>,
        request: ExecuteRequest,
    ) -> Result<StartedCell, String> {
        self.require_open()?;
        let execution_id = Uuid::new_v4().to_string();
        let request = conversion::execute_request(&self.id, execution_id.clone(), request)?;
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .begin_execution(&request)?;
        let ownership = ExecutionOwnership {
            session: Arc::clone(self),
            execution_id,
            armed: true,
        };
        let (started_tx, started_rx) = oneshot::channel();
        let inner = Arc::clone(self);
        self.stream_tasks.spawn(async move {
            inner.drive_execution(request, ownership, started_tx).await;
        });
        started_rx
            .await
            .map_err(|_| "gRPC code-mode execution driver ended unexpectedly".to_string())?
    }

    async fn drive_execution(
        self: Arc<Self>,
        request: grpc::ExecuteRequest,
        ownership: ExecutionOwnership,
        started_tx: oneshot::Sender<Result<StartedCell, String>>,
    ) {
        let runtime_timeout =
            Duration::from_millis(request.yield_time_ms.unwrap_or(DEFAULT_EXEC_YIELD_TIME_MS))
                .saturating_add(Duration::from_secs(1));
        let opening = async {
            let mut client = self.client();
            let mut stream =
                deadline::request(&self, "execution", Duration::ZERO, client.execute(request))
                    .await?
                    .into_inner();
            let first = deadline::request(
                &self,
                "execution starting event",
                Duration::ZERO,
                stream.message(),
            )
            .await?
            .ok_or_else(|| {
                "gRPC code-mode execution ended before its starting event".to_string()
            })?;
            let Some(grpc::execute_event::Event::Started(started)) = first.event else {
                return Err("gRPC code-mode execution omitted its starting event".to_string());
            };
            super::validate_identifier(&started.execution_id, "execution ID")?;
            if started.execution_id != ownership.execution_id {
                let error = format!(
                    "gRPC code-mode execution returned ID {} instead of {}",
                    started.execution_id, ownership.execution_id
                );
                self.fail(error.clone());
                return Err(error);
            }

            let admission = self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .admit_execution(&ownership.execution_id, &started.cell_id);
            if let Err(error) = admission {
                self.fail(error.clone());
                return Err(error);
            }

            Ok((CellId::new(started.cell_id), stream))
        }
        .await;
        let (cell_id, stream) = match opening {
            Ok(opening) => opening,
            Err(error) => {
                let _ = started_tx.send(Err(error));
                return;
            }
        };
        let (response_tx, response_rx) = oneshot::channel();
        let mut claim = ownership;
        let started = StartedCell::from_future(cell_id.clone(), async move {
            let closure = claim
                .session
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .mark_execution_ready(&claim.execution_id);
            match closure {
                Ok(cell) => claim.session.report_closed_cell(cell),
                Err(error) => {
                    claim.session.fail(error.clone());
                    return Err(error);
                }
            }
            let response = response_rx
                .await
                .map_err(|_| "exec runtime ended unexpectedly".to_string())??;
            claim.armed = false;
            drop(claim);
            Ok(response)
        });
        if started_tx.send(Ok(started)).is_err() {
            return;
        }
        self.drive_execution_outcome(cell_id, stream, response_tx, runtime_timeout)
            .await;
    }

    async fn drive_execution_outcome(
        self: Arc<Self>,
        cell_id: CellId,
        mut stream: tonic::Streaming<grpc::ExecuteEvent>,
        mut response_tx: oneshot::Sender<Result<RuntimeResponse, String>>,
        runtime_timeout: Duration,
    ) {
        let outcome = tokio::select! {
            biased;
            _ = response_tx.closed() => return,
            outcome = deadline::request(
                &self,
                "execution outcome",
                runtime_timeout,
                stream.message(),
            ) => match outcome {
                Ok(Some(grpc::ExecuteEvent {
                    event: Some(grpc::execute_event::Event::Outcome(outcome)),
                })) => conversion::runtime_response(outcome),
                Ok(Some(_)) => {
                    Err("gRPC code-mode execution returned an unexpected event".to_string())
                }
                Ok(None) => Err("gRPC code-mode execution omitted its initial outcome".to_string()),
                Err(error) => Err(error),
            },
        };
        let outcome = match outcome {
            Ok(response) if runtime_response_cell_id(&response) != &cell_id => {
                let error = format!(
                    "gRPC code-mode execution returned cell {} instead of {cell_id}",
                    runtime_response_cell_id(&response)
                );
                self.fail(error.clone());
                Err(error)
            }
            Ok(response) => {
                tokio::select! {
                    biased;
                    _ = response_tx.closed() => return,
                    _ = self.settle_notifications(&response) => {}
                }
                Ok(response)
            }
            Err(error) => Err(error),
        };
        let _ = response_tx.send(outcome);
    }

    pub(super) async fn wait(
        self: &Arc<Self>,
        request: WaitRequest,
    ) -> Result<WaitOutcome, String> {
        self.require_open()?;
        let slot = {
            let mut slots = self
                .wait_slots
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            slots.retain(|_, slot| slot.strong_count() != 0);
            let slot = match slots
                .get(&request.cell_id)
                .and_then(std::sync::Weak::upgrade)
            {
                Some(slot) => slot,
                None => {
                    let slot = Arc::new(WaitSlot {
                        lock: Arc::new(tokio::sync::Mutex::new(())),
                        active: AtomicBool::new(false),
                    });
                    slots.insert(request.cell_id.clone(), Arc::downgrade(&slot));
                    slot
                }
            };
            if slot.active.swap(true, Ordering::AcqRel) {
                return Err(format!(
                    "exec cell {} already has an active observer",
                    request.cell_id
                ));
            }
            slot
        };
        let lock = Arc::clone(&slot.lock);
        let mut cancellation = WaitCancellation {
            session: Arc::clone(self),
            slot: Some(slot),
            wait_id: None,
            permit: None,
        };
        let permit = lock.lock_owned().await;
        self.require_open()?;
        let wait_id = Uuid::new_v4().to_string();
        cancellation.wait_id = Some(wait_id.clone());
        cancellation.permit = Some(permit);
        let expected_cell_id = request.cell_id;
        let runtime_timeout =
            Duration::from_millis(request.yield_time_ms).saturating_add(Duration::from_secs(1));
        let request = grpc::WaitRequest {
            session_id: self.id.clone(),
            cell_id: expected_cell_id.as_str().to_string(),
            wait_id,
            yield_time_ms: request.yield_time_ms,
        };
        let mut client = self.client();
        let response = deadline::request(self, "wait", runtime_timeout, client.wait(request)).await;
        cancellation.disarm();
        self.prune_wait_slots();
        let outcome = conversion::wait_outcome(response?.into_inner())?;
        self.validate_wait_cell(&expected_cell_id, outcome).await
    }

    pub(super) async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, String> {
        self.require_open()?;
        let mut client = self.client();
        let response = deadline::request(
            self,
            "termination",
            Duration::ZERO,
            client.terminate(grpc::TerminateRequest {
                session_id: self.id.clone(),
                cell_id: cell_id.as_str().to_string(),
            }),
        )
        .await?
        .into_inner();
        let outcome = conversion::wait_outcome(response)?;
        self.validate_wait_cell(&cell_id, outcome).await
    }

    async fn validate_wait_cell(
        &self,
        expected_cell_id: &CellId,
        outcome: WaitOutcome,
    ) -> Result<WaitOutcome, String> {
        let response = match &outcome {
            WaitOutcome::LiveCell(response) | WaitOutcome::MissingCell(response) => response,
        };
        let actual_cell_id = runtime_response_cell_id(response);
        if actual_cell_id != expected_cell_id {
            let error = format!(
                "gRPC code-mode host returned cell {actual_cell_id} instead of {expected_cell_id}"
            );
            self.fail(error.clone());
            return Err(error);
        }
        self.settle_notifications(response).await;
        Ok(outcome)
    }

    async fn settle_notifications(&self, response: &RuntimeResponse) {
        match response {
            RuntimeResponse::Yielded { .. } => {}
            RuntimeResponse::Terminated { cell_id, .. } => self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel_notifications(cell_id),
            RuntimeResponse::Result { cell_id, .. } => {
                let cancellation = self
                    .state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .notification_cancellation(cell_id);
                if let Some(cancellation) = cancellation {
                    // CellClosed follows notifications on the lease stream and cancels this
                    // token only after every admitted notification has finished.
                    cancellation.cancelled().await;
                }
            }
        }
    }

    fn prune_wait_slots(&self) {
        self.wait_slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, slot| slot.strong_count() != 0);
    }
}

fn runtime_response_cell_id(response: &RuntimeResponse) -> &CellId {
    match response {
        RuntimeResponse::Yielded { cell_id, .. }
        | RuntimeResponse::Terminated { cell_id, .. }
        | RuntimeResponse::Result { cell_id, .. } => cell_id,
    }
}

struct WaitCancellation {
    session: Arc<SessionInner>,
    slot: Option<Arc<WaitSlot>>,
    wait_id: Option<String>,
    permit: Option<OwnedMutexGuard<()>>,
}

impl WaitCancellation {
    fn disarm(&mut self) {
        self.wait_id = None;
        if let Some(slot) = self.slot.take() {
            slot.active.store(false, Ordering::Release);
        }
        self.permit = None;
    }
}

impl Drop for WaitCancellation {
    fn drop(&mut self) {
        let slot = self.slot.take();
        if let Some(slot) = slot.as_ref() {
            slot.active.store(false, Ordering::Release);
        }
        let Some(wait_id) = self.wait_id.take() else {
            return;
        };
        let permit = self.permit.take();
        if self.session.stopped.is_cancelled() {
            return;
        }
        let session = Arc::clone(&self.session);
        self.session.runtime.spawn(async move {
            let mut client = session.client();
            let result = deadline::request(
                &session,
                "wait cancellation",
                Duration::ZERO,
                client.cancel_wait(grpc::CancelWaitRequest {
                    session_id: session.id.clone(),
                    wait_id,
                }),
            )
            .await;
            if let Err(error) = result
                && !session.stopped.is_cancelled()
            {
                session.fail(format!(
                    "failed to retire canceled gRPC code-mode wait: {error}"
                ));
            }
            drop(permit);
            drop(slot);
            session.prune_wait_slots();
        });
    }
}
