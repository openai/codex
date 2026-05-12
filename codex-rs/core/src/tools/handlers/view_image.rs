use codex_protocol::items::ImageViewItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::InputModality;
use codex_utils_image::PromptImageMode;
use codex_utils_image::load_for_prompt_bytes;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::original_image_detail::can_request_original_image_detail;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::handlers::view_image_spec::ViewImageToolOptions;
use crate::tools::handlers::view_image_spec::create_view_image_tool;
use crate::tools::registry::ToolHandler;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub struct ViewImageHandler {
    options: ViewImageToolOptions,
}

impl Default for ViewImageHandler {
    fn default() -> Self {
        Self {
            options: ViewImageToolOptions {
                can_request_original_image_detail: false,
                include_environment_id: false,
            },
        }
    }
}

impl ViewImageHandler {
    pub(crate) fn new(options: ViewImageToolOptions) -> Self {
        Self { options }
    }
}

const VIEW_IMAGE_UNSUPPORTED_MESSAGE: &str =
    "view_image is not allowed because you do not support image inputs";

#[derive(Deserialize)]
struct ViewImageArgs {
    path: String,
    #[serde(default)]
    environment_id: Option<String>,
    detail: Option<String>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ViewImageDetail {
    Original,
}

impl ToolHandler for ViewImageHandler {
    type Output = ViewImageOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("view_image")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_view_image_tool(self.options))
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
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

        let ViewImageArgs {
            path,
            environment_id,
            detail,
        } = parse_arguments(&arguments)?;
        // `view_image` accepts only its documented detail values: omit
        // `detail` for the default path or set it to `original`.
        // Other string values remain invalid rather than being silently
        // reinterpreted.
        let detail = match detail.as_deref() {
            None => None,
            Some("original") => Some(ViewImageDetail::Original),
            Some(detail) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "view_image.detail only supports `original`; omit `detail` for default resized behavior, got `{detail}`"
                )));
            }
        };

        let Some(turn_environment) =
            resolve_tool_environment(turn.as_ref(), environment_id.as_deref())?
        else {
            return Err(FunctionCallError::RespondToModel(
                "view_image is unavailable in this session".to_string(),
            ));
        };
        let cwd = turn_environment.cwd.clone();
        let abs_path = cwd.join(path);
        let sandbox = turn_environment.environment.is_remote().then(|| {
            let mut sandbox =
                turn.file_system_sandbox_context(/*additional_permissions*/ None);
            sandbox.cwd = Some(cwd.clone());
            sandbox
        });
        let fs = turn_environment.environment.get_filesystem();

        let metadata = fs
            .get_metadata(&abs_path, sandbox.as_ref())
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
        let file_bytes = fs
            .read_file(&abs_path, sandbox.as_ref())
            .await
            .map_err(|error| {
                FunctionCallError::RespondToModel(format!(
                    "unable to read image at `{}`: {error}",
                    abs_path.display()
                ))
            })?;
        let event_path = abs_path.clone();

        let can_request_original_detail = can_request_original_image_detail(&turn.model_info);
        let use_original_detail =
            can_request_original_detail && matches!(detail, Some(ViewImageDetail::Original));
        let image_mode = if use_original_detail {
            PromptImageMode::Original
        } else {
            PromptImageMode::ResizeToFit
        };
        let image_detail = Some(if use_original_detail {
            ImageDetail::Original
        } else {
            DEFAULT_IMAGE_DETAIL
        });

        let image =
            load_for_prompt_bytes(abs_path.as_path(), file_bytes, image_mode).map_err(|error| {
                FunctionCallError::RespondToModel(format!(
                    "unable to process image at `{}`: {error}",
                    abs_path.display()
                ))
            })?;
        let image_url = image.into_data_url();

        let item = TurnItem::ImageView(ImageViewItem {
            id: call_id,
            path: event_path,
        });
        session.emit_turn_item_started(turn.as_ref(), &item).await;
        session.emit_turn_item_completed(turn.as_ref(), item).await;

        Ok(ViewImageOutput {
            image_url,
            image_detail,
        })
    }
}

pub struct ViewImageOutput {
    image_url: String,
    image_detail: Option<ImageDetail>,
}

impl ToolOutput for ViewImageOutput {
    fn log_preview(&self) -> String {
        self.image_url.clone()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let body =
            FunctionCallOutputBody::ContentItems(vec![FunctionCallOutputContentItem::InputImage {
                image_url: self.image_url.clone(),
                detail: self.image_detail,
            }]);
        let output = FunctionCallOutputPayload {
            body,
            success: Some(true),
        };

        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output,
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> serde_json::Value {
        serde_json::json!({
            "image_url": self.image_url,
            "detail": self.image_detail
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn code_mode_result_returns_image_url_object() {
        let output = ViewImageOutput {
            image_url: "data:image/png;base64,AAA".to_string(),
            image_detail: Some(DEFAULT_IMAGE_DETAIL),
        };

        let result = output.code_mode_result(&ToolPayload::Function {
            arguments: "{}".to_string(),
        });

        assert_eq!(
            result,
            json!({
                "image_url": "data:image/png;base64,AAA",
                "detail": "high",
            })
        );
    }
}
