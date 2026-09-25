use std::sync::Arc;
use std::sync::PoisonError;

use tokio_util::sync::CancellationToken;
use tonic::Status;

use super::session::GrpcSession;
use super::validation;

pub(super) struct ActiveWait {
    pub(super) cancellation: CancellationToken,
    pub(super) yield_signal: CancellationToken,
    retired: CancellationToken,
}

pub(super) struct WaitRegistration {
    session: Arc<GrpcSession>,
    id: String,
    cancellation: CancellationToken,
    yield_signal: CancellationToken,
    retired: CancellationToken,
}

impl WaitRegistration {
    pub(super) fn new(session: Arc<GrpcSession>, id: String) -> Result<Self, Status> {
        validation::identifier(&id, "wait ID")?;
        let cancellation = CancellationToken::new();
        let yield_signal = CancellationToken::new();
        let retired = CancellationToken::new();
        let mut state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
        if session.closed.is_cancelled() {
            return Err(Status::cancelled("code-mode session is closed"));
        }
        if state.waits.contains_key(&id) || !state.seen_waits.remember(id.clone()) {
            return Err(Status::already_exists(format!(
                "code-mode wait ID `{id}` was reused"
            )));
        }
        if state.cancelled_waits.remove(&id) {
            state.yielded_waits.remove(&id);
            return Err(Status::cancelled("code-mode wait was cancelled"));
        }
        if state.yielded_waits.remove(&id) {
            yield_signal.cancel();
        }
        state.waits.insert(
            id.clone(),
            ActiveWait {
                cancellation: cancellation.clone(),
                yield_signal: yield_signal.clone(),
                retired: retired.clone(),
            },
        );
        drop(state);
        Ok(Self {
            session,
            id,
            cancellation,
            yield_signal,
            retired,
        })
    }

    pub(super) fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    pub(super) fn yield_signal(&self) -> CancellationToken {
        self.yield_signal.clone()
    }
}

impl Drop for WaitRegistration {
    fn drop(&mut self) {
        self.session
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waits
            .remove(&self.id);
        self.retired.cancel();
    }
}

impl GrpcSession {
    pub(super) fn yield_execution(&self, id: &str) -> Result<(), Status> {
        validation::identifier(id, "execution ID")?;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(signal) = state.execution_yields.get(id) {
            signal.cancel();
        } else if !state.seen_executions.contains(id) {
            state.yielded_executions.remember(id.to_string());
        }
        Ok(())
    }

    pub(super) fn yield_wait(&self, id: &str) -> Result<(), Status> {
        validation::identifier(id, "wait ID")?;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(wait) = state.waits.get(id) {
            wait.yield_signal.cancel();
        } else if !state.seen_waits.contains(id) {
            state.yielded_waits.remember(id.to_string());
        }
        Ok(())
    }

    pub(super) fn retire_execution_observation(&self, id: &str) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .execution_yields
            .remove(id);
    }

    pub(super) async fn cancel_wait(&self, id: &str) -> Result<(), Status> {
        validation::identifier(id, "wait ID")?;
        let active = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(wait) = state.waits.get(id) {
                Some((wait.cancellation.clone(), wait.retired.clone()))
            } else {
                if !state.seen_waits.contains(id) {
                    state.cancelled_waits.remember(id.to_string());
                }
                None
            }
        };
        if let Some((cancellation, retired)) = active {
            cancellation.cancel();
            retired.cancelled().await;
        }
        Ok(())
    }
}
