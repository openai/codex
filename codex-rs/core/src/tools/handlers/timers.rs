//! Built-in tool handlers for thread-local persistent timer management.
//!
//! These handlers bridge `TimerCreate`, `TimerDelete`, and `TimerList` tool
//! calls onto the current thread session's timer registry.

use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::timers::ThreadTimerTrigger;
use crate::timers::TimerDelivery;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

#[derive(Deserialize)]
struct TimerCreateArgs {
    trigger: ThreadTimerTrigger,
    prompt: String,
    delivery: TimerDelivery,
}

#[derive(Deserialize)]
struct TimerDeleteArgs {
    id: String,
}

pub struct TimerCreateHandler;

impl ToolHandler for TimerCreateHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "TimerCreate received unsupported payload".to_string(),
            ));
        };
        let args: TimerCreateArgs = parse_arguments(&arguments)?;
        let timer = invocation
            .session
            .create_timer(args.trigger, args.prompt, args.delivery)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::to_string(&timer).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize TimerCreate response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

pub struct TimerDeleteHandler;

impl ToolHandler for TimerDeleteHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolPayload::Function { arguments } = invocation.payload else {
            return Err(FunctionCallError::RespondToModel(
                "TimerDelete received unsupported payload".to_string(),
            ));
        };
        let args: TimerDeleteArgs = parse_arguments(&arguments)?;
        let deleted = invocation
            .session
            .delete_timer(&args.id)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let content = serde_json::json!({ "deleted": deleted }).to_string();
        Ok(FunctionToolOutput::from_text(content, Some(deleted)))
    }
}

pub struct TimerListHandler;

impl ToolHandler for TimerListHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        match invocation.payload {
            ToolPayload::Function { .. } => {}
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "TimerList received unsupported payload".to_string(),
                ));
            }
        }
        let timers = invocation.session.list_timers().await;
        let content = serde_json::to_string(&timers).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize TimerList response: {err}"))
        })?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}
