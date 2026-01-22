use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct SlackNotifyHandler;

#[derive(Debug, Deserialize)]
struct NotifyArgs {
    message: String,
}

#[derive(Debug, Serialize)]
struct NotifyResult {
    ok: bool,
    channel: String,
    ts: String,
    thread_ts: String,
}

#[async_trait]
impl ToolHandler for SlackNotifyHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "notify_user handler received unsupported payload".to_string(),
                ));
            }
        };
        let args: NotifyArgs = parse_arguments(&arguments)?;
        let slack = session
            .services
            .slack
            .as_ref()
            .ok_or_else(|| FunctionCallError::RespondToModel("Slack integration is not configured. Set SLACKMCP_NOTIFY_CHANNEL and SLACK_TOKEN/SLACK_COOKIE.".to_string()))?;
        let result = slack.notify_user(&args.message).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("Slack notify failed: {err:#}"))
        })?;
        let payload = NotifyResult {
            ok: true,
            channel: result.channel,
            ts: result.ts,
            thread_ts: result.thread_ts,
        };
        let content = serde_json::to_string(&payload).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize notify_user result: {err}"))
        })?;
        Ok(ToolOutput::Function {
            content,
            success: Some(true),
            content_items: None,
        })
    }
}
