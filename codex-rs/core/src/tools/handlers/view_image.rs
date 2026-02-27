use async_trait::async_trait;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::openai_models::InputModality;
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
use codex_protocol::models::local_image_content_items_with_label_number;

pub struct ViewImageHandler;

const VIEW_IMAGE_UNSUPPORTED_MESSAGE: &str =
    "view_image is not allowed because you do not support image inputs";

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
        if !invocation
            .turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Err(FunctionCallError::RespondToModel(
                VIEW_IMAGE_UNSUPPORTED_MESSAGE.to_string(),
            ));
        }

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

        let content = local_image_content_items_with_label_number(&abs_path, None);
        let content = content
            .into_iter()
            .map(|item| match item {
                ContentItem::InputText { text } => {
                    FunctionCallOutputContentItem::InputText { text }
                }
                ContentItem::InputImage { image_url } => {
                    FunctionCallOutputContentItem::InputImage { image_url }
                }
                ContentItem::OutputText { text } => {
                    FunctionCallOutputContentItem::InputText { text }
                }
            })
            .collect();

        session
            .send_event(
                turn.as_ref(),
                EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
                    call_id,
                    path: event_path,
                }),
            )
            .await;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::ContentItems(content),
            success: Some(true),
        })
    }
}
