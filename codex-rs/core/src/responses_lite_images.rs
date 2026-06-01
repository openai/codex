use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_utils_image::ImageProcessingError;
use codex_utils_image::PromptImageMode;
use codex_utils_image::load_data_url_for_prompt;
use tracing::warn;

const IMAGE_PROCESSING_ERROR_PLACEHOLDER: &str =
    "image content omitted because it could not be processed";

#[derive(Debug, thiserror::Error)]
enum ResponsesLiteImagePreparationError {
    #[error("Responses Lite image detail only supports `original`, `high`, or `auto`; got `low`")]
    UnsupportedLowDetail,
    #[error("Responses Lite failed to prepare image: {0}")]
    ImageProcessing(#[from] ImageProcessingError),
}

pub(crate) fn prepare_response_items_for_responses_lite(items: &mut [ResponseItem]) {
    for item in items {
        prepare_response_item(item);
    }
}

fn prepare_response_item(item: &mut ResponseItem) {
    match item {
        ResponseItem::Message { content, .. } => prepare_content_items(content),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => {
            if let Some(content_items) = output.content_items_mut() {
                prepare_function_call_output_content_items(content_items);
            }
        }
        ResponseItem::Reasoning { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => {}
    }
}

fn prepare_content_items(items: &mut [ContentItem]) {
    for item in items {
        if let ContentItem::InputImage { image_url, detail } = item
            && let Err(err) = prepare_image_url_for_responses_lite(image_url, detail)
        {
            warn!(error = %err, "failed to prepare Responses Lite message image");
            *item = ContentItem::InputText {
                text: IMAGE_PROCESSING_ERROR_PLACEHOLDER.to_string(),
            };
        }
    }
}

fn prepare_function_call_output_content_items(items: &mut [FunctionCallOutputContentItem]) {
    for item in items {
        if let FunctionCallOutputContentItem::InputImage { image_url, detail } = item
            && let Err(err) = prepare_image_url_for_responses_lite(image_url, detail)
        {
            warn!(error = %err, "failed to prepare Responses Lite tool output image");
            *item = FunctionCallOutputContentItem::InputText {
                text: IMAGE_PROCESSING_ERROR_PLACEHOLDER.to_string(),
            };
        }
    }
}

fn prepare_image_url_for_responses_lite(
    image_url: &mut String,
    detail: &mut Option<ImageDetail>,
) -> Result<(), ResponsesLiteImagePreparationError> {
    // Local-image and view_image producers may have already prepared their data URLs.
    // Keep this pass as the Responses Lite image contract; the shared image cache
    // and preserve-within-bounds path avoid a second lossy encode for common formats.
    let mode = prompt_image_mode_for_responses_lite_detail(*detail)?;
    let image = load_data_url_for_prompt(image_url, mode)?;
    *image_url = image.into_data_url();
    *detail = None;
    Ok(())
}

fn prompt_image_mode_for_responses_lite_detail(
    detail: Option<ImageDetail>,
) -> Result<PromptImageMode, ResponsesLiteImagePreparationError> {
    match detail {
        None | Some(ImageDetail::Auto | ImageDetail::Original) => {
            Ok(PromptImageMode::ResponsesLiteOriginal)
        }
        Some(ImageDetail::High) => Ok(PromptImageMode::ResizeToFit),
        Some(ImageDetail::Low) => Err(ResponsesLiteImagePreparationError::UnsupportedLowDetail),
    }
}

#[cfg(test)]
#[path = "responses_lite_images_tests.rs"]
mod tests;
