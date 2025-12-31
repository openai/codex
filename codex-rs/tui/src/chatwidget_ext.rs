//! Extension module for chatwidget.rs to minimize upstream merge conflicts.
//!
//! Contains helper functions for ExtEvent handling and plan mode.
//! Note: The actual event handlers remain in chatwidget.rs because they require
//! &mut self access. This module provides supporting utilities.

use crate::bottom_pane::ApprovalRequest;
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
