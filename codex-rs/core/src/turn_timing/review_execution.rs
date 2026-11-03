//! Attaches submitted review execution to its child turn before measurement starts.

use super::TurnTimingState;
use crate::session::session::Session;
use codex_utils_critical_path::ReviewExecution;
use codex_utils_critical_path::ReviewExecutionAttempt;
use std::sync::Arc;

pub(crate) struct PendingReviewExecution(pub(crate) ReviewExecution);

/// Keeps a submission unresolved until completion and removes only its pending observer.
pub(crate) struct ReviewExecutionObservation {
    session: Arc<Session>,
    execution: Option<ReviewExecution>,
    attempt: Option<ReviewExecutionAttempt>,
}

impl ReviewExecutionObservation {
    pub(crate) fn install(session: Arc<Session>, execution: Option<ReviewExecution>) -> Self {
        let attempt = execution.as_ref().map(ReviewExecution::start_attempt);
        if let Some(execution) = &execution {
            session
                .services
                .thread_extension_data
                .insert(PendingReviewExecution(execution.clone()));
        }
        Self {
            session,
            execution,
            attempt,
        }
    }

    pub(crate) fn finish(&mut self, completed: bool) {
        if let Some(attempt) = self.attempt.take() {
            attempt.finish(completed);
        }
    }
}

impl Drop for ReviewExecutionObservation {
    fn drop(&mut self) {
        if let Some(execution) = &self.execution
            && self
                .session
                .services
                .thread_extension_data
                .get::<PendingReviewExecution>()
                .is_some_and(|pending| pending.0 == *execution)
        {
            self.session
                .services
                .thread_extension_data
                .remove::<PendingReviewExecution>();
        }
    }
}

pub(crate) async fn observe_pending_review_execution(session: &Session, timing: &TurnTimingState) {
    if let Some(execution) = session
        .services
        .thread_extension_data
        .remove::<PendingReviewExecution>()
    {
        timing.observe_review_execution(execution.0.clone()).await;
    }
}
