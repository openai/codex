use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_utils_image::ImageProcessingError;
use codex_utils_image::PromptImageMode;
use codex_utils_image::PromptImageResizeLimits;
use codex_utils_image::load_data_url_for_prompt;
use tracing::warn;

pub(crate) const IMAGE_PROCESSING_ERROR_PLACEHOLDER: &str =
    "image content omitted because it could not be processed";
const IMAGE_TOO_LARGE_PLACEHOLDER: &str =
    "image content omitted because it exceeded the supported size limit; use a smaller image";
const UNSUPPORTED_LOW_DETAIL_PLACEHOLDER: &str = "image content omitted because detail 'low' is not supported; use 'high', 'original', or 'auto'";

const HIGH_DETAIL_LIMITS: PromptImageResizeLimits = PromptImageResizeLimits {
    max_dimension: 2048,
    max_patches: 2_500,
};
const ORIGINAL_DETAIL_LIMITS: PromptImageResizeLimits = PromptImageResizeLimits {
    max_dimension: 6000,
    max_patches: 10_000,
};
#[derive(Debug, thiserror::Error)]
enum ImagePreparationError {
    #[error("image detail `low` is not supported")]
    UnsupportedLowDetail,
    #[error(transparent)]
    Processing(#[from] ImageProcessingError),
}

impl ImagePreparationError {
    fn placeholder(&self) -> &'static str {
        match self {
            ImagePreparationError::UnsupportedLowDetail => UNSUPPORTED_LOW_DETAIL_PLACEHOLDER,
            ImagePreparationError::Processing(ImageProcessingError::ImageTooLarge { .. }) => {
                IMAGE_TOO_LARGE_PLACEHOLDER
            }
            ImagePreparationError::Processing(_) => IMAGE_PROCESSING_ERROR_PLACEHOLDER,
        }
    }
}

pub(crate) fn prepare_response_items(items: &mut [ResponseItem]) {
    for item in items {
        match item {
            ResponseItem::Message { content, .. } => prepare_message_content(content),
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content) = output.content_items_mut() {
                    prepare_tool_output_content(content);
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
}

fn prepare_message_content(items: &mut [ContentItem]) {
    for item in items {
        if let ContentItem::InputImage { image_url, detail } = item
            && is_data_url(image_url)
            && let Err(error) = prepare_image(image_url, *detail)
        {
            warn!(%error, "failed to prepare message image");
            *item = ContentItem::InputText {
                text: error.placeholder().to_string(),
            };
        }
    }
}

fn prepare_tool_output_content(items: &mut [FunctionCallOutputContentItem]) {
    for item in items {
        if let FunctionCallOutputContentItem::InputImage { image_url, detail } = item
            && is_data_url(image_url)
            && let Err(error) = prepare_image(image_url, *detail)
        {
            warn!(%error, "failed to prepare tool output image");
            *item = FunctionCallOutputContentItem::InputText {
                text: error.placeholder().to_string(),
            };
        }
    }
}

fn is_data_url(image_url: &str) -> bool {
    image_url
        .get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn prepare_image(
    image_url: &mut String,
    detail: Option<ImageDetail>,
) -> Result<(), ImagePreparationError> {
    let limits = match detail {
        None | Some(ImageDetail::Auto | ImageDetail::High) => HIGH_DETAIL_LIMITS,
        Some(ImageDetail::Original) => ORIGINAL_DETAIL_LIMITS,
        Some(ImageDetail::Low) => return Err(ImagePreparationError::UnsupportedLowDetail),
    };
    let image = load_data_url_for_prompt(image_url, PromptImageMode::ResizeWithLimits(limits))?;
    *image_url = image.into_data_url();
    Ok(())
}

#[cfg(test)]
#[path = "image_preparation_tests.rs"]
mod tests;
