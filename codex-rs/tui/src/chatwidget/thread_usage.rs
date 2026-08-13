//! Demand-driven estimated-cost state for the currently visible enterprise thread.

use super::AppEvent;
use super::ChatWidget;
use super::PlanType;
use super::ThreadId;
use crate::status::StatusHistoryHandle;
use codex_app_server_protocol::ThreadUsage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ThreadUsageOutcome {
    Available(ThreadUsage),
    Disabled,
}

#[derive(Debug, Default)]
pub(super) struct ThreadUsageState {
    thread_id: Option<ThreadId>,
    estimate: Option<ThreadUsage>,
    pending_request_id: Option<u64>,
    next_request_id: u64,
    feature_disabled: bool,
    status_history_handles: Vec<StatusHistoryHandle>,
}

impl ChatWidget {
    pub(super) fn clear_thread_usage_state(&mut self) {
        let next_request_id = self.thread_usage.next_request_id;
        self.thread_usage = ThreadUsageState {
            next_request_id,
            ..ThreadUsageState::default()
        };
    }

    pub(super) fn request_thread_usage_for_status(&mut self, handle: StatusHistoryHandle) {
        if !self.thread_usage_is_available() {
            return;
        }

        let Some(thread_id) = self.thread_id else {
            return;
        };
        if self.thread_usage.thread_id != Some(thread_id) {
            self.clear_thread_usage_state();
            self.thread_usage.thread_id = Some(thread_id);
        }

        self.thread_usage.status_history_handles.push(handle);
        self.request_thread_usage();
    }

    pub(crate) fn finish_thread_usage_refresh(
        &mut self,
        thread_id: ThreadId,
        request_id: u64,
        result: Result<ThreadUsageOutcome, String>,
    ) -> bool {
        if self.thread_id != Some(thread_id)
            || self.thread_usage.thread_id != Some(thread_id)
            || self.thread_usage.pending_request_id != Some(request_id)
        {
            return false;
        }

        self.thread_usage.pending_request_id = None;
        let mut usage_updated = false;
        match result {
            Ok(ThreadUsageOutcome::Disabled) => {
                self.thread_usage.feature_disabled = true;
                self.thread_usage.estimate = None;
                usage_updated = true;
            }
            Ok(ThreadUsageOutcome::Available(usage))
                if usage.thread_id != thread_id.to_string() =>
            {
                tracing::warn!(
                    requested_thread_id = %thread_id,
                    returned_thread_id = %usage.thread_id,
                    "thread usage response referred to another thread"
                );
            }
            Ok(ThreadUsageOutcome::Available(usage)) => {
                self.thread_usage.estimate = Some(usage);
                usage_updated = true;
            }
            Err(err) => {
                tracing::debug!(error = %err, "failed to fetch estimated thread usage");
            }
        }
        if usage_updated && !self.thread_usage.status_history_handles.is_empty() {
            for handle in self.thread_usage.status_history_handles.drain(..) {
                handle.set_thread_usage(self.thread_usage.estimate.clone());
            }
        }
        self.request_redraw();
        true
    }

    pub(super) fn estimated_thread_usage(&self) -> Option<&ThreadUsage> {
        self.thread_usage.estimate.as_ref()
    }

    fn request_thread_usage(&mut self) {
        let Some(thread_id) = self.thread_id else {
            return;
        };
        if !self.thread_usage_is_available() || self.thread_usage.pending_request_id.is_some() {
            return;
        }

        let request_id = self.thread_usage.next_request_id;
        self.thread_usage.next_request_id =
            self.thread_usage.next_request_id.wrapping_add(/*rhs*/ 1);
        self.thread_usage.pending_request_id = Some(request_id);
        self.thread_usage.thread_id = Some(thread_id);
        self.app_event_tx.send(AppEvent::RefreshThreadUsage {
            thread_id,
            request_id,
        });
    }

    pub(super) fn thread_usage_is_available(&self) -> bool {
        self.has_codex_backend_auth
            && !self.thread_usage.feature_disabled
            && self.thread_id.is_some()
            && matches!(
                self.plan_type,
                Some(
                    PlanType::Business
                        | PlanType::EnterpriseCbpUsageBased
                        | PlanType::EnterpriseCbpAutomation
                )
            )
    }
}
