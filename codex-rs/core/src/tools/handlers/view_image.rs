use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs;

use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::ViewImageToolCallEvent;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::user_input::UserInput;

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

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
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

        let args: ViewImageArgs = parse_arguments(&arguments)?;

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
            .send_event(
                turn.as_ref(),
                EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
                    call_id,
                    path: event_path,
                }),
            )
            .await;

        let response_input: ResponseInputItem = vec![UserInput::LocalImage {
            path: abs_path.clone(),
        }]
        .into();
        let image_url = match response_input {
            ResponseInputItem::Message { content, .. } => match content.into_iter().next() {
                Some(ContentItem::InputImage { image_url }) => image_url,
                Some(ContentItem::InputText { text }) => {
                    return Err(FunctionCallError::RespondToModel(text));
                }
                _ => {
                    return Err(FunctionCallError::RespondToModel(
                        "unexpected image input payload".to_string(),
                    ));
                }
            },
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "unexpected image input payload".to_string(),
                ));
            }
        };

        Ok(ToolOutput::Function {
            content: "attached local image path".to_string(),
            content_items: Some(vec![FunctionCallOutputContentItem::InputImage {
                image_url,
            }]),
            success: Some(true),
        })
    }
}
