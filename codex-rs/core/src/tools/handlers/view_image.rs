use async_trait::async_trait;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::local_image_content_items_with_label_number;
use codex_protocol::openai_models::InputModality;
use codex_utils_image::PromptImageMode;
use serde::Deserialize;
use tokio::fs;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::ViewImageToolCallEvent;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ViewImageHandler;

const VIEW_IMAGE_UNSUPPORTED_MESSAGE: &str =
    "view_image is not allowed because you do not support image inputs";
const MIN_ORIGINAL_RESOLUTION_MODEL_VERSION: (u32, u32) = (5, 4);

#[derive(Deserialize)]
struct ViewImageArgs {
    path: String,
}

fn supports_original_resolution_model(slug: &str) -> bool {
    // Match `gpt-X.Y...` model slugs and enable original-resolution images for
    // GPT models at version 5.4 or newer.
    let Some(version_suffix) = slug.strip_prefix("gpt-") else {
        return false;
    };
    let version_end = version_suffix
        .find(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .unwrap_or(version_suffix.len());
    let version = &version_suffix[..version_end];
    if version.is_empty() {
        return false;
    }

    let mut parts = version.split('.');
    let Some(major) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return false;
    };
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);

    (major, minor) >= MIN_ORIGINAL_RESOLUTION_MODEL_VERSION
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

        let use_original_resolution = turn
            .config
            .features
            .enabled(Feature::ViewImageOriginalResolution)
            && supports_original_resolution_model(&turn.model_info.slug);
        let image_mode = if use_original_resolution {
            PromptImageMode::Original
        } else {
            PromptImageMode::ResizeToFit
        };
        let image_detail = if use_original_resolution {
            Some(ImageDetail::Original)
        } else {
            None
        };

        let content = local_image_content_items_with_label_number(&abs_path, None, image_mode)
            .into_iter()
            .map(|item| match item {
                ContentItem::InputText { text } => {
                    FunctionCallOutputContentItem::InputText { text }
                }
                ContentItem::InputImage { image_url } => {
                    FunctionCallOutputContentItem::InputImage {
                        image_url,
                        detail: image_detail,
                    }
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

#[cfg(test)]
mod tests {
    use super::supports_original_resolution_model;

    #[test]
    fn supports_original_resolution_model_version_gate() {
        assert!(!supports_original_resolution_model("gpt-4.1"));
        assert!(!supports_original_resolution_model("gpt-5"));
        assert!(!supports_original_resolution_model("gpt-5.3"));
        assert!(supports_original_resolution_model("gpt-5.4"));
        assert!(supports_original_resolution_model("gpt-5.4-mini"));
        assert!(supports_original_resolution_model("gpt-5.5"));
        assert!(supports_original_resolution_model("gpt-5.10"));
        assert!(supports_original_resolution_model("gpt-6"));
        assert!(!supports_original_resolution_model("o3"));
    }
}
