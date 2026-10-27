//! Tracks client runtime changes and turn starts admitted before a shutdown drain.

use codex_app_server_protocol::JSONRPCErrorError;
use codex_extension_api::TurnStartAdmission;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::watch;

use crate::error_code::invalid_request;
use crate::error_code::server_draining_error;

#[derive(Debug, Default)]
struct AdmissionState {
    closed: bool,
    paused: bool,
    active: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct TurnAdmission {
    state: Arc<Mutex<AdmissionState>>,
    active_tx: watch::Sender<usize>,
}

impl Default for TurnAdmission {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(AdmissionState::default())),
            active_tx: watch::channel(0).0,
        }
    }
}

pub(crate) struct TurnPermit(TurnAdmission);
pub(crate) struct TurnAdmissionPause(TurnAdmission);

impl TurnAdmission {
    pub(crate) async fn pause(&self) -> Result<TurnAdmissionPause, JSONRPCErrorError> {
        let mut active = self.subscribe_active();
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed || state.paused {
                return Err(invalid_request("Runtime changes are already paused"));
            }
            state.paused = true;
        }
        let pause = TurnAdmissionPause(self.clone());
        tokio::time::timeout(
            std::time::Duration::from_secs(/*secs*/ 10),
            active.wait_for(|active| *active == 0),
        )
        .await
        .map_err(|_| invalid_request("Work is still starting; retry the workspace switch"))?
        .map_err(|_| invalid_request("Turn admission is unavailable"))?;
        Ok(pause)
    }

    pub(crate) fn begin_drain(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
    }

    pub(crate) fn subscribe_active(&self) -> watch::Receiver<usize> {
        self.active_tx.subscribe()
    }

    // Admit and close take the same short lock. The permit keeps shutdown from
    // finishing while an earlier request is still preparing or submitting work.
    pub(crate) fn admit(&self) -> Result<TurnPermit, JSONRPCErrorError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(server_draining_error());
        }
        if state.paused {
            return Err(invalid_request(
                "Workspace change in progress; retry when it completes",
            ));
        }
        state.active += 1;
        self.active_tx.send_replace(state.active);
        Ok(TurnPermit(self.clone()))
    }
}

impl Drop for TurnAdmissionPause {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .paused = false;
    }
}

impl TurnStartAdmission for TurnAdmission {
    fn admit_turn_start(&self) -> Option<Box<dyn Send>> {
        self.admit()
            .ok()
            .map(|permit| Box::new(permit) as Box<dyn Send>)
    }
}

impl Drop for TurnPermit {
    fn drop(&mut self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active -= 1;
        self.0.active_tx.send_replace(state.active);
    }
}

#[cfg(test)]
#[path = "turn_admission_tests.rs"]
mod tests;
