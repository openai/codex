//! Built-in tool handlers for thread-local persistent timer management.
//!
//! These handlers bridge timer and queued-message tool calls onto the current
//! thread session's timer registry.

use serde::Deserialize;
use std::collections::BTreeMap;

use crate::function_tool::FunctionCallError;
use crate::messages::MessagePayload;
use crate::timers::ThreadTimerTrigger;
use crate::timers::TimerDelivery;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

#[derive(Deserialize)]
struct CreateTimerArgs {
    trigger: ThreadTimerTrigger,
    content: Option<String>,
    prompt: Option<String>,
    instructions: Option<String>,
    #[serde(default)]
    meta: BTreeMap<String, String>,
    delivery: TimerDelivery,
}

#[derive(Deserialize)]
struct DeleteTimerArgs {
    id: String,
}

#[derive(Deserialize)]
struct QueueMessageArgs {
    thread_id: String,
    content: String,
    instructions: Option<String>,
    #[serde(default)]
    meta: BTreeMap<String, String>,
    #[serde(default = "default_delivery")]
    delivery: TimerDelivery,
}

fn default_delivery() -> TimerDelivery {
    TimerDelivery::AfterTurn
}

pub struct CreateTimerHandler;

impl ToolHandler for CreateTimerHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "create_timer received unsupported payload".to_string(),
            ));
        };
        let args: CreateTimerArgs = parse_arguments(&arguments)?;
        let content = args.content.or(args.prompt).ok_or_else(|| {
            FunctionCallError::RespondToModel("create_timer requires `content`".to_string())
        })?;
        let timer = invocation
            .session
            .create_timer(
                args.trigger,
                MessagePayload {
                    content,
                    instructions: args.instructions,
                    meta: args.meta,
                },
                args.delivery,
            )
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::to_string(&timer).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize create_timer response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

pub struct QueueMessageHandler;

impl ToolHandler for QueueMessageHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "queue_message received unsupported payload".to_string(),
            ));
        };
        let args: QueueMessageArgs = parse_arguments(&arguments)?;
        let message = invocation
            .session
            .queue_message_to_thread(
                args.thread_id,
                MessagePayload {
                    content: args.content,
                    instructions: args.instructions,
                    meta: args.meta,
                },
                args.delivery,
            )
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::to_string(&message).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize queue_message response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

pub struct DeleteTimerHandler;

impl ToolHandler for DeleteTimerHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "delete_timer received unsupported payload".to_string(),
            ));
        };
        let args: DeleteTimerArgs = parse_arguments(&arguments)?;
        let deleted = invocation
            .session
            .delete_timer(&args.id)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::json!({ "deleted": deleted }).to_string();
        Ok(FunctionToolOutput::from_text(content, Some(deleted)))
    }
}

pub struct ListTimersHandler;

impl ToolHandler for ListTimersHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        match invocation.payload {
            ToolPayload::Function { .. } => {}
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "list_timers received unsupported payload".to_string(),
                ));
            }
        }
        let timers = invocation.session.list_timers().await;
        let content = serde_json::to_string(&timers).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize list_timers response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}
