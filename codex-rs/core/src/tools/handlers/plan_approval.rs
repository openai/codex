use async_trait::async_trait;
use codex_protocol::plan_approval::PlanProposal;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde::Deserialize;
use serde_json::json;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub(crate) const APPROVE_PLAN_TOOL_NAME: &str = "approve_plan";

pub struct PlanApprovalHandler;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovePlanArgs {
    proposal: PlanProposal,
}

#[async_trait]
impl ToolHandler for PlanApprovalHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for {tool_name}"
            )));
        };

        let source = turn.client.get_session_source();
        if let SessionSource::SubAgent(SubAgentSource::Other(label)) = &source
            && label.starts_with("plan_variant")
        {
            return Err(FunctionCallError::RespondToModel(
                "approve_plan is not supported in non-interactive planning subagents".to_string(),
            ));
        }

        let args: ApprovePlanArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;

        if args.proposal.title.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "proposal.title must be non-empty".to_string(),
            ));
        }
        if args.proposal.plan.plan.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "proposal.plan.plan must contain at least 1 step".to_string(),
            ));
        }

        let response = session
            .request_plan_approval(turn.as_ref(), call_id, args.proposal)
            .await;

        Ok(ToolOutput::Function {
            content: json!({ "response": response }).to_string(),
            content_items: None,
            success: Some(true),
        })
    }
}
