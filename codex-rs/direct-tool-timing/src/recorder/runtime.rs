//! Records runtime execution-lock handoffs.
//!
//! Admission guards must finish before their execution lock is released, so the
//! next admitted invocation can inherit the completed dependency path.

use super::CompletedWork;
use super::ToolTimingGuard;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Copy)]
pub enum ToolAdmission {
    Parallel,
    Exclusive,
}

#[derive(Clone, Default)]
pub struct ToolGateTiming(Arc<Mutex<GateState>>);

#[derive(Default)]
struct GateState {
    latest: Option<CompletedWork>,
    writer: Option<CompletedWork>,
}

pub struct AdmittedToolTiming {
    work: Arc<ToolTimingGuard>,
    gate: ToolGateTiming,
    admission: ToolAdmission,
}

impl ToolGateTiming {
    pub fn admit(
        &self,
        work: Arc<ToolTimingGuard>,
        admission: ToolAdmission,
        started_at: Instant,
    ) -> AdmittedToolTiming {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dependency = match admission {
            ToolAdmission::Parallel => &state.writer,
            ToolAdmission::Exclusive => &state.latest,
        };
        if let Some(dependency) = dependency
            && dependency.ended_at > started_at
        {
            work.timings.join(
                &work.call_id,
                started_at,
                Instant::now(),
                dependency.path.clone(),
            );
        }
        AdmittedToolTiming {
            work,
            gate: self.clone(),
            admission,
        }
    }
}

impl Drop for AdmittedToolTiming {
    fn drop(&mut self) {
        if let Some(completed) = self.work.finish() {
            let mut state = self
                .gate
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if matches!(self.admission, ToolAdmission::Exclusive) {
                state.writer = Some(completed.clone());
            }
            if state
                .latest
                .as_ref()
                .is_none_or(|previous| previous.ended_at < completed.ended_at)
            {
                state.latest = Some(completed);
            }
        }
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
