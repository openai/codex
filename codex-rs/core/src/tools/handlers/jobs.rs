//! Built-in tool handlers for thread-local runtime job management.
//!
//! These handlers bridge `JobCreate`, `JobDelete`, and `JobList` tool calls
//! onto the current thread session's in-memory job registry.

use async_trait::async_trait;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

#[derive(Deserialize)]
struct JobCreateArgs {
    cron_expression: String,
    prompt: String,
    run_once: Option<bool>,
}

#[derive(Deserialize)]
struct JobDeleteArgs {
    id: String,
}

pub struct JobCreateHandler;

#[async_trait]
impl ToolHandler for JobCreateHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "JobCreate received unsupported payload".to_string(),
            ));
        };
        let args: JobCreateArgs = parse_arguments(&arguments)?;
        let job = invocation
            .session
            .create_job(
                args.cron_expression,
                args.prompt,
                args.run_once.unwrap_or(false),
            )
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::to_string(&job).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize JobCreate response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

pub struct JobDeleteHandler;

#[async_trait]
impl ToolHandler for JobDeleteHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "JobDelete received unsupported payload".to_string(),
            ));
        };
        let args: JobDeleteArgs = parse_arguments(&arguments)?;
        let deleted = invocation.session.delete_job(&args.id).await;
        let content = serde_json::json!({ "deleted": deleted }).to_string();
        Ok(FunctionToolOutput::from_text(content, Some(deleted)))
    }
}

pub struct JobListHandler;

#[async_trait]
impl ToolHandler for JobListHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        match invocation.payload {
            ToolPayload::Function { .. } => {}
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "JobList received unsupported payload".to_string(),
                ));
            }
        }
        let jobs = invocation.session.list_jobs().await;
        let content = serde_json::to_string(&jobs).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize JobList response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}
