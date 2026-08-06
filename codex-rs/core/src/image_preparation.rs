use crate::config::ManagedFeatures;
use crate::context::ContextualUserFragment;
use crate::context::ImageResizeNotice;
use crate::context::ImageResizeNoticeSource;
use crate::context::ResizedImage;
use crate::original_image_detail::can_request_original_image_detail;
use codex_analytics::ImageDetailSetting;
use codex_analytics::ImagePreparationMetadata;
use codex_features::Feature;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
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
const REMOTE_IMAGE_URL_PLACEHOLDER: &str =
    "image content omitted because remote image URLs are not supported";

const HIGH_DETAIL_LIMITS: PromptImageResizeLimits = PromptImageResizeLimits {
    max_dimension: 2048,
    max_patches: 2_500,
};
const UNIFIED_IMAGE_LIMITS: PromptImageResizeLimits = PromptImageResizeLimits {
    max_dimension: 6000,
    max_patches: 10_000,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImagePreparationMode {
    DetailBased,
    UnifiedBudget,
}

pub(crate) fn unified_image_budget_enabled(
    features: &ManagedFeatures,
    model_info: &ModelInfo,
) -> bool {
    features.enabled(Feature::UnifiedImageBudget)
        && (model_info.use_responses_lite || can_request_original_image_detail(model_info))
}

#[derive(Clone, Copy, Debug)]
struct ImageOrigin<'a> {
    message_role: Option<&'a str>,
    item_id: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageResizeNoticeMode {
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreparedImageResize {
    source_width: u32,
    source_height: u32,
    prepared_width: u32,
    prepared_height: u32,
}

#[derive(Debug, thiserror::Error)]
enum ImagePreparationError {
    #[error("remote image URLs are not supported")]
    RemoteUrlUnsupported,
    #[error("image detail `low` is not supported")]
    UnsupportedLowDetail,
    #[error(transparent)]
    Processing(#[from] ImageProcessingError),
}

impl ImagePreparationError {
    fn placeholder(&self) -> &'static str {
        match self {
            ImagePreparationError::RemoteUrlUnsupported => REMOTE_IMAGE_URL_PLACEHOLDER,
            ImagePreparationError::UnsupportedLowDetail => UNSUPPORTED_LOW_DETAIL_PLACEHOLDER,
            ImagePreparationError::Processing(ImageProcessingError::ImageTooLarge { .. }) => {
                IMAGE_TOO_LARGE_PLACEHOLDER
            }
            ImagePreparationError::Processing(_) => IMAGE_PROCESSING_ERROR_PLACEHOLDER,
        }
    }
}

pub(crate) fn prepare_response_items(
    items: &mut Vec<ResponseItem>,
    mode: ImagePreparationMode,
    resize_notice_mode: ImageResizeNoticeMode,
) -> Vec<ImagePreparationMetadata> {
    let mut metadata = Vec::new();
    let mut prepared_items = Vec::with_capacity(items.len());
    for mut item in std::mem::take(items) {
        let resize_notice = match &mut item {
            ResponseItem::Message { role, content, .. } => {
                let resized_images = prepare_message_content(
                    content,
                    ImageOrigin {
                        message_role: Some(role),
                        item_id: None,
                    },
                    if role == "user" {
                        resize_notice_mode
                    } else {
                        ImageResizeNoticeMode::Disabled
                    },
                    &mut metadata,
                    mode,
                );
                (!resized_images.is_empty()).then(|| {
                    ImageResizeNotice::new(ImageResizeNoticeSource::UserMessage, resized_images)
                })
            }
            ResponseItem::FunctionCallOutput {
                call_id, output, ..
            }
            | ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } => output.content_items_mut().and_then(|content| {
                let resized_images = prepare_tool_output_content(
                    content,
                    ImageOrigin {
                        message_role: None,
                        item_id: Some(call_id),
                    },
                    resize_notice_mode,
                    &mut metadata,
                    mode,
                );
                (!resized_images.is_empty()).then(|| {
                    ImageResizeNotice::new(ImageResizeNoticeSource::ToolOutput, resized_images)
                })
            }),
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => None,
        };
        prepared_items.push(item);
        if let Some(resize_notice) = resize_notice {
            prepared_items.push(ContextualUserFragment::into(resize_notice));
        }
    }
    *items = prepared_items;
    metadata
}

fn prepare_message_content(
    items: &mut [ContentItem],
    origin: ImageOrigin<'_>,
    resize_notice_mode: ImageResizeNoticeMode,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
) -> Vec<ResizedImage> {
    let image_count = items
        .iter()
        .filter(|item| matches!(item, ContentItem::InputImage { .. }))
        .count();
    let mut image_number = 0;
    let mut resized_images = Vec::new();
    for item in items {
        if let ContentItem::InputImage { image_url, detail } = item {
            image_number += 1;
            match prepare_image(image_url, detail, origin, metadata, mode) {
                Ok(Some(resize)) if resize_notice_mode == ImageResizeNoticeMode::Enabled => {
                    resized_images.push(ResizedImage {
                        image_number,
                        image_count,
                        source_width: resize.source_width,
                        source_height: resize.source_height,
                        prepared_width: resize.prepared_width,
                        prepared_height: resize.prepared_height,
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    warn!(%error, "failed to prepare message image");
                    *item = ContentItem::InputText {
                        text: error.placeholder().to_string(),
                    };
                }
            }
        }
    }
    resized_images
}

fn prepare_tool_output_content(
    items: &mut [FunctionCallOutputContentItem],
    origin: ImageOrigin<'_>,
    resize_notice_mode: ImageResizeNoticeMode,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
) -> Vec<ResizedImage> {
    let image_count = items
        .iter()
        .filter(|item| matches!(item, FunctionCallOutputContentItem::InputImage { .. }))
        .count();
    let mut image_number = 0;
    let mut resized_images = Vec::new();
    for item in items {
        if let FunctionCallOutputContentItem::InputImage { image_url, detail } = item {
            image_number += 1;
            match prepare_image(image_url, detail, origin, metadata, mode) {
                Ok(Some(resize)) if resize_notice_mode == ImageResizeNoticeMode::Enabled => {
                    resized_images.push(ResizedImage {
                        image_number,
                        image_count,
                        source_width: resize.source_width,
                        source_height: resize.source_height,
                        prepared_width: resize.prepared_width,
                        prepared_height: resize.prepared_height,
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    warn!(%error, "failed to prepare tool output image");
                    *item = FunctionCallOutputContentItem::InputText {
                        text: error.placeholder().to_string(),
                    };
                }
            }
        }
    }
    resized_images
}

fn is_remote_image_url(image_url: &str) -> bool {
    image_url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn is_data_url(image_url: &str) -> bool {
    image_url
        .get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn prepare_image(
    image_url: &mut String,
    detail: &mut Option<ImageDetail>,
    origin: ImageOrigin<'_>,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
) -> Result<Option<PreparedImageResize>, ImagePreparationError> {
    if is_remote_image_url(image_url) {
        return Err(ImagePreparationError::RemoteUrlUnsupported);
    }
    if !is_data_url(image_url) {
        return Ok(None);
    }

    let (effective_detail, limits) = match mode {
        ImagePreparationMode::UnifiedBudget => (ImageDetailSetting::Original, UNIFIED_IMAGE_LIMITS),
        ImagePreparationMode::DetailBased => match detail {
            None | Some(ImageDetail::Auto | ImageDetail::High) => {
                (ImageDetailSetting::High, HIGH_DETAIL_LIMITS)
            }
            Some(ImageDetail::Original) => (ImageDetailSetting::Original, UNIFIED_IMAGE_LIMITS),
            Some(ImageDetail::Low) => return Err(ImagePreparationError::UnsupportedLowDetail),
        },
    };
    let image = load_data_url_for_prompt(image_url, PromptImageMode::ResizeWithLimits(limits))?;
    metadata.push(ImagePreparationMetadata {
        message_role: origin.message_role.map(str::to_string),
        item_id: origin.item_id.map(str::to_string),
        effective_detail,
        source_width: image.source_width,
        source_height: image.source_height,
        prepared_width: image.width,
        prepared_height: image.height,
    });
    let resize = ((image.source_width, image.source_height) != (image.width, image.height))
        .then_some(PreparedImageResize {
            source_width: image.source_width,
            source_height: image.source_height,
            prepared_width: image.width,
            prepared_height: image.height,
        });
    *image_url = image.into_data_url();
    if mode == ImagePreparationMode::UnifiedBudget {
        // Preserve accurate context-window accounting while older transports still require an
        // image detail field. Responses Lite removes this compatibility hint before sending.
        *detail = Some(ImageDetail::Original);
    }
    Ok(resize)
}

#[cfg(test)]
#[path = "image_preparation_tests.rs"]
mod tests;
