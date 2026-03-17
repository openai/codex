use async_trait::async_trait;
use codex_environment::ExecutorFileSystem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::local_image_content_items_with_label_number;
use codex_protocol::openai_models::InputModality;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_image::PromptImageMode;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::original_image_detail::can_request_original_image_detail;
use crate::protocol::EventMsg;
use crate::protocol::ViewImageToolCallEvent;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ViewImageHandler;

const VIEW_IMAGE_UNSUPPORTED_MESSAGE: &str =
    "view_image is not allowed because you do not support image inputs";

#[derive(Deserialize)]
struct ViewImageArgs {
    path: String,
    detail: Option<String>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ViewImageDetail {
    Original,
}

#[async_trait]
impl ToolHandler for ViewImageHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
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
        // `view_image` accepts only its documented detail values: omit
        // `detail` for the default path or set it to `original`.
        // Other string values remain invalid rather than being silently
        // reinterpreted.
        let detail = match args.detail.as_deref() {
            None => None,
            Some("original") => Some(ViewImageDetail::Original),
            Some(detail) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "view_image.detail only supports `original`; omit `detail` for default resized behavior, got `{detail}`"
                )));
            }
        };

        let abs_path =
            AbsolutePathBuf::try_from(turn.resolve_path(Some(args.path))).map_err(|error| {
                FunctionCallError::RespondToModel(format!("unable to resolve image path: {error}"))
            })?;

        let metadata = turn
            .environment
            .get_filesystem()
            .get_metadata(&abs_path)
            .await
            .map_err(|error| {
                FunctionCallError::RespondToModel(format!(
                    "unable to locate image at `{}`: {error}",
                    abs_path.display()
                ))
            })?;

        if !metadata.is_file {
            return Err(FunctionCallError::RespondToModel(format!(
                "image path `{}` is not a file",
                abs_path.display()
            )));
        }
        let file_bytes = turn
            .environment
            .get_filesystem()
            .read_file(&abs_path)
            .await
            .map_err(|error| {
                FunctionCallError::RespondToModel(format!(
                    "unable to read image at `{}`: {error}",
                    abs_path.display()
                ))
            })?;
        let event_path = abs_path.to_path_buf();

        let can_request_original_detail =
            can_request_original_image_detail(turn.features.get(), &turn.model_info);
        let use_original_detail =
            can_request_original_detail && matches!(detail, Some(ViewImageDetail::Original));
        let image_mode = if use_original_detail {
            PromptImageMode::Original
        } else {
            PromptImageMode::ResizeToFit
        };
        let image_detail = use_original_detail.then_some(ImageDetail::Original);

        let content = local_image_content_items_with_label_number(
            abs_path.as_path(),
            file_bytes,
            /*label_number*/ None,
            image_mode,
        )
        .into_iter()
        .map(|item| match item {
            ContentItem::InputText { text } => FunctionCallOutputContentItem::InputText { text },
            ContentItem::InputImage { image_url } => FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: image_detail,
            },
            ContentItem::OutputText { text } => FunctionCallOutputContentItem::InputText { text },
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

        Ok(FunctionToolOutput::from_content(content, Some(true)))
    }
}
