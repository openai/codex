//! Enter Plan Mode Tool Handler
//!
//! Requests to enter Plan Mode and triggers user approval flow.
//! Aligned with Claude Code's EnterPlanMode (chunks.130.mjs:2336-2398).

use crate::function_tool::FunctionCallError;
use crate::subagent::expect_session_state;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol_ext::ExtEventMsg;
use codex_protocol::protocol_ext::PlanModeEnteredEvent;
use codex_protocol::protocol_ext::PlanModeEntryRequestEvent;

/// Enter Plan Mode Tool Handler
///
/// This tool:
/// 1. Validates we are NOT already in Plan Mode
/// 2. Sends PlanModeEntryRequest event for user approval
/// 3. Returns a message indicating approval is pending
pub struct EnterPlanModeHandler;

#[async_trait]
impl ToolHandler for EnterPlanModeHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Get stores and check plan mode state
        let stores = expect_session_state(&invocation.session.conversation_id);
        let plan_mode_state = stores.get_plan_mode_state().map_err(|e| {
            tracing::error!("failed to get plan mode state: {e}");
            FunctionCallError::RespondToModel(
                "Failed to get plan mode state. Please try again.".to_string(),
            )
        })?;

        // 2. Check if already in plan mode
        if plan_mode_state.is_active {
            return Err(FunctionCallError::RespondToModel(
                "Already in plan mode. Use ExitPlanMode when you have finished planning."
                    .to_string(),
            ));
        }

        // 3. Check approval policy - auto-approve for SpawnAgent
        if stores.should_auto_approve_plan_mode() {
            // Auto-approve: directly enter plan mode
            let plan_file_path = stores
                .enter_plan_mode(invocation.session.conversation_id)
                .map_err(|e| {
                    tracing::error!("failed to enter plan mode: {e}");
                    FunctionCallError::RespondToModel(
                        "Failed to enter plan mode. Please try again.".to_string(),
                    )
                })?;

            // Send PlanModeEntered event (notify TUI, but don't block)
            invocation
                .session
                .send_event(
                    invocation.turn.as_ref(),
                    EventMsg::Ext(ExtEventMsg::PlanModeEntered(PlanModeEnteredEvent {
                        plan_file_path: plan_file_path.display().to_string(),
                    })),
                )
                .await;

            return Ok(ToolOutput::Function {
                content: format!(
                    "Plan mode entered (auto-approved). Plan file: {}\n\n\
                     You are now in plan mode. You should:\n\
                     1. Explore the codebase to understand existing patterns and architecture\n\
                     2. Identify similar features and approaches\n\
                     3. Design a concrete implementation strategy\n\
                     4. Write your plan to the plan file\n\
                     5. Call ExitPlanMode when ready to implement\n\n\
                     Remember: DO NOT write or edit any files yet (except the plan file). \
                     This is a read-only exploration and planning phase.",
                    plan_file_path.display()
                ),
                content_items: None,
                success: Some(true),
            });
        }

        // 4. Require approval: send PlanModeEntryRequest event
        invocation
            .session
            .send_event(
                invocation.turn.as_ref(),
                EventMsg::Ext(ExtEventMsg::PlanModeEntryRequest(
                    PlanModeEntryRequestEvent {},
                )),
            )
            .await;

        // 5. Return pending approval message with detailed guidance
        Ok(ToolOutput::Function {
            content: "Plan mode entry requested. Waiting for user approval.\n\n\
                     If the user approves, you will enter plan mode. In plan mode, you should:\n\
                     1. Thoroughly explore the codebase to understand existing patterns\n\
                     2. Identify similar features and architectural approaches\n\
                     3. Consider multiple approaches and their trade-offs\n\
                     4. Use AskUserQuestion if you need to clarify the approach\n\
                     5. Design a concrete implementation strategy\n\
                     6. When ready, use ExitPlanMode to present your plan for approval\n\n\
                     Remember: DO NOT write or edit any files yet (except the plan file). \
                     This is a read-only exploration and planning phase."
                .to_string(),
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
        let handler = EnterPlanModeHandler;
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let handler = EnterPlanModeHandler;

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }
}
