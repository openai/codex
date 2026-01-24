//! Extension module for chatwidget.rs to minimize upstream merge conflicts.
//!
//! Contains helper functions for ExtEvent handling and plan mode.
//! Uses impl ChatWidget blocks to move event handler logic out of the main file.

use crate::bottom_pane::ApprovalRequest;
use crate::chatwidget::ChatWidget;
use codex_protocol::protocol_ext::ExtEventMsg;
use codex_protocol::protocol_ext::PlanModeEntryRequestEvent;
use codex_protocol::protocol_ext::PlanModeExitRequestEvent;
use codex_protocol::protocol_ext::UserQuestionRequestEvent;

/// Build ApprovalRequest for plan mode entry.
pub fn build_plan_mode_entry_request() -> ApprovalRequest {
    ApprovalRequest::EnterPlanMode
}

/// Build ApprovalRequest for plan mode exit (plan approval).
pub fn build_plan_mode_exit_request(ev: &PlanModeExitRequestEvent) -> ApprovalRequest {
    ApprovalRequest::Plan {
        plan_content: ev.plan_content.clone(),
        plan_file_path: ev.plan_file_path.clone(),
    }
}

/// Build ApprovalRequest for user question.
pub fn build_user_question_request(ev: &UserQuestionRequestEvent) -> ApprovalRequest {
    ApprovalRequest::UserQuestion {
        tool_call_id: ev.tool_call_id.clone(),
        questions: ev.questions.clone(),
    }
}

// =============================================================================
// impl ChatWidget - Event handlers moved from chatwidget.rs
// =============================================================================

impl ChatWidget {
    /// Handle extension events (plan mode, user questions, etc.).
    /// Moved from chatwidget.rs to minimize upstream merge conflicts.
    pub(crate) fn on_ext_event(&mut self, ext_msg: ExtEventMsg) {
        match ext_msg {
            ExtEventMsg::PlanModeEntryRequest(ev) => {
                self.on_plan_mode_entry_request(ev);
            }
            ExtEventMsg::PlanModeExitRequest(ev) => {
                self.on_plan_mode_exit_request(ev);
            }
            ExtEventMsg::UserQuestionRequest(ev) => {
                self.on_user_question_request(ev);
            }
            ExtEventMsg::PlanModeEntered(_) => {
                self.set_plan_mode_and_redraw(true);
            }
            ExtEventMsg::PlanModeExited(_) => {
                self.set_plan_mode_and_redraw(false);
            }
            // Other extension events (compact, subagent activity) are handled elsewhere or ignored
            _ => {}
        }
    }

    fn on_plan_mode_entry_request(&mut self, _ev: PlanModeEntryRequestEvent) {
        let request = build_plan_mode_entry_request();
        self.push_approval_request_and_redraw(request);
    }

    fn on_plan_mode_exit_request(&mut self, ev: PlanModeExitRequestEvent) {
        let request = build_plan_mode_exit_request(&ev);
        self.push_approval_request_and_redraw(request);
    }

    fn on_user_question_request(&mut self, ev: UserQuestionRequestEvent) {
        let request = build_user_question_request(&ev);
        self.push_approval_request_and_redraw(request);
    }
}
