use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::agent_jobs_spec::create_report_agent_job_result_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::*;

pub struct ReportAgentJobResultHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ReportAgentJobResultHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("report_agent_job_result")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_report_agent_job_result_tool())
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "report_agent_job_result handler received unsupported payload".to_string(),
                ));
            }
        };

        handle(session, arguments).await.map(boxed_tool_output)
    }
}

impl CoreToolRuntime for ReportAgentJobResultHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}

pub async fn handle(
    session: Arc<Session>,
    arguments: String,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let args: ReportAgentJobResultArgs = parse_arguments(arguments.as_str())?;
    if !args.result.is_object() {
        return Err(FunctionCallError::RespondToModel(
            "result must be a JSON object".to_string(),
        ));
    }
    let db = required_state_db(&session)?;
    let reporting_thread_id = session.conversation_id.to_string();
    let accepted = db
        .report_agent_job_item_result(
            args.job_id.as_str(),
            args.item_id.as_str(),
            reporting_thread_id.as_str(),
            &args.result,
        )
        .await
        .map_err(|err| {
            let job_id = args.job_id.as_str();
            let item_id = args.item_id.as_str();
            FunctionCallError::RespondToModel(format!(
                "failed to record agent job result for {job_id} / {item_id}: {err}"
            ))
        })?;
    if accepted && args.stop.unwrap_or(false) {
        let message = "cancelled by worker request";
        let _ = db
            .mark_agent_job_cancelled(args.job_id.as_str(), message)
            .await;
    }
    let content =
        serde_json::to_string(&ReportAgentJobResultToolResult { accepted }).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize report_agent_job_result result: {err}"
            ))
        })?;
    Ok(FunctionToolOutput::from_text(content, Some(true)))
}
