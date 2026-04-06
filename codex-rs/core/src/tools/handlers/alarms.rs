//! Built-in tool handlers for thread-local persistent alarm management.
//!
//! These handlers bridge `AlarmCreate`, `AlarmDelete`, and `AlarmList` tool
//! calls onto the current thread session's alarm registry.

use async_trait::async_trait;
use serde::Deserialize;

use crate::alarms::AlarmDelivery;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

#[derive(Deserialize)]
struct AlarmCreateArgs {
    cron_expression: String,
    prompt: String,
    run_once: Option<bool>,
    delivery: AlarmDelivery,
}

#[derive(Deserialize)]
struct AlarmDeleteArgs {
    id: String,
}

pub struct AlarmCreateHandler;

#[async_trait]
impl ToolHandler for AlarmCreateHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "AlarmCreate received unsupported payload".to_string(),
            ));
        };
        let args: AlarmCreateArgs = parse_arguments(&arguments)?;
        let alarm = invocation
            .session
            .create_alarm(
                args.cron_expression,
                args.prompt,
                args.run_once.unwrap_or(false),
                args.delivery,
            )
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::to_string(&alarm).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize AlarmCreate response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

pub struct AlarmDeleteHandler;

#[async_trait]
impl ToolHandler for AlarmDeleteHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "AlarmDelete received unsupported payload".to_string(),
            ));
        };
        let args: AlarmDeleteArgs = parse_arguments(&arguments)?;
        let deleted = invocation
            .session
            .delete_alarm(&args.id)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::json!({ "deleted": deleted }).to_string();
        Ok(FunctionToolOutput::from_text(content, Some(deleted)))
    }
}

pub struct AlarmListHandler;

#[async_trait]
impl ToolHandler for AlarmListHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        match invocation.payload {
            ToolPayload::Function { .. } => {}
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "AlarmList received unsupported payload".to_string(),
                ));
            }
        }
        let alarms = invocation.session.list_alarms().await;
        let content = serde_json::to_string(&alarms).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize AlarmList response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}
