//! Exit Plan Mode Tool Handler
//!
//! Requests to exit Plan Mode and triggers user approval flow.
//! Reads the plan file and sends PlanModeExitRequest event.

use crate::function_tool::FunctionCallError;
use crate::plan_mode::plan_file_exists;
use crate::plan_mode::read_plan_file;
use crate::subagent::expect_session_state;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol_ext::ExtEventMsg;
use codex_protocol::protocol_ext::PlanModeExitRequestEvent;
use codex_protocol::protocol_ext::PlanModeExitedEvent;

/// Exit Plan Mode Tool Handler
///
/// This tool:
/// 1. Validates we are in Plan Mode
/// 2. Checks plan file exists and reads it
/// 3. Sends PlanModeExitRequest event for user approval
/// 4. Returns a message indicating approval is pending
pub struct ExitPlanModeHandler;

#[async_trait]
impl ToolHandler for ExitPlanModeHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // Maximum plan file size (10MB)
        const MAX_PLAN_SIZE: usize = 10 * 1024 * 1024;

        // 1. Get stores and check plan mode state
        let stores = expect_session_state(&invocation.session.conversation_id);
        let plan_mode_state = stores.get_plan_mode_state().map_err(|e| {
            tracing::error!("failed to get plan mode state: {e}");
            FunctionCallError::RespondToModel(
                "Failed to get plan mode state. Please try again.".to_string(),
            )
        })?;

        if !plan_mode_state.is_active {
            return Err(FunctionCallError::RespondToModel(
                "Not in plan mode. Cannot exit.".to_string(),
            ));
        }

        // 2. Get plan file path
        let plan_file_path = match &plan_mode_state.plan_file_path {
            Some(path) => path.clone(),
            None => {
                return Err(FunctionCallError::RespondToModel(
                    "No plan file path set. Enter plan mode first.".to_string(),
                ));
            }
        };

        // 3. Check plan file exists
        if !plan_file_exists(&plan_file_path) {
            return Err(FunctionCallError::RespondToModel(format!(
                "Plan file not found at {}. Please write your plan to this file before exiting.",
                plan_file_path.display()
            )));
        }

        // 4. Read plan content
        let plan_content = read_plan_file(&plan_file_path).ok_or_else(|| {
            FunctionCallError::RespondToModel(format!(
                "Failed to read plan file at {}",
                plan_file_path.display()
            ))
        })?;

        // 5. Validate plan content
        if plan_content.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "Plan file is empty. Please write your plan before exiting.".to_string(),
            ));
        }
        if plan_content.len() > MAX_PLAN_SIZE {
            return Err(FunctionCallError::RespondToModel(
                "Plan file too large (>10MB). Please reduce plan size.".to_string(),
            ));
        }

        let plan_file_path_str = plan_file_path.to_string_lossy().to_string();

        // 6. Check approval policy - auto-approve for SpawnAgent
        if stores.should_auto_approve_plan_mode() {
            // Auto-approve: directly exit plan mode
            // Note: NOT setting subsequent permission mode - that's handled by AskForApproval independently
            stores.exit_plan_mode(true).map_err(|e| {
                tracing::error!("failed to exit plan mode: {e}");
                FunctionCallError::RespondToModel(
                    "Failed to exit plan mode. Please try again.".to_string(),
                )
            })?;

            // Send PlanModeExited event (notify TUI, but don't block)
            invocation
                .session
                .send_event(
                    invocation.turn.as_ref(),
                    EventMsg::Ext(ExtEventMsg::PlanModeExited(PlanModeExitedEvent {
                        approved: true,
                    })),
                )
                .await;

            return Ok(ToolOutput::Function {
                content: format!(
                    "Plan approved (auto-approved). You can now start implementing.\n\n\
                     Plan file: {}\n\n\
                     ## Your Plan:\n\n{}",
                    plan_file_path_str, plan_content
                ),
                content_items: None,
                success: Some(true),
            });
        }

        // 7. Require approval: send PlanModeExitRequest event
        invocation
            .session
            .send_event(
                invocation.turn.as_ref(),
                EventMsg::Ext(ExtEventMsg::PlanModeExitRequest(PlanModeExitRequestEvent {
                    plan_content: plan_content.clone(),
                    plan_file_path: plan_file_path_str.clone(),
                })),
            )
            .await;

        // 8. Return pending approval message
        Ok(ToolOutput::Function {
            content: format!(
                "Exit plan mode requested. Waiting for user approval.\n\n\
                 Plan file: {}\n\n\
                 ## Plan Content:\n\n{}",
                plan_file_path_str, plan_content
            ),
            content_items: None,
            success: Some(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_kind() {
        let handler = ExitPlanModeHandler;
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let handler = ExitPlanModeHandler;

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }
}
