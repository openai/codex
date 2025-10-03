use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs;

use crate::function_tool::FunctionCallError;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::InputItem;
use crate::protocol::ViewImageToolCallEvent;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ViewImageHandler;

#[derive(Deserialize)]
struct ViewImageArgs {
    path: String,
}

#[async_trait]
impl ToolHandler for ViewImageHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(
        &self,
        invocation: ToolInvocation<'_>,
    ) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            sub_id,
            call_id,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "view_image handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ViewImageArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;

        let abs_path = turn.resolve_path(Some(args.path));

        let metadata = fs::metadata(&abs_path).await.map_err(|error| {
            FunctionCallError::RespondToModel(format!(
                "unable to locate image at `{}`: {error}",
                abs_path.display()
            ))
        })?;

        if !metadata.is_file() {
            return Err(FunctionCallError::RespondToModel(format!(
                "image path `{}` is not a file",
                abs_path.display()
            )));
        }
        let event_path = abs_path.clone();

        session
            .inject_input(vec![InputItem::LocalImage { path: abs_path }])
            .await
            .map_err(|_| {
                FunctionCallError::RespondToModel(
                    "unable to attach image (no active task)".to_string(),
                )
            })?;

        session
            .send_event(Event {
                id: sub_id.to_string(),
                msg: EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
                    call_id,
                    path: event_path,
                }),
            })
            .await;

        Ok(ToolOutput::Function {
            content: "attached local image path".to_string(),
            success: Some(true),
        })
    }
}
