use crate::config::ManagedFeatures;
use crate::context::ContextualUserFragment;
use crate::context::ImageResizeNotice;
use crate::context::ImageResizeNoticeSource;
use crate::context::ResizedImage;
use crate::original_image_detail::can_request_original_image_detail;
use codex_analytics::ImageDetailSetting;
use codex_analytics::ImagePreparationMetadata;
use codex_attachment_store::AttachmentStore;
use codex_attachment_store::UploadRequest;
use codex_attachment_store::UploadResult;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::set_annotated_content;
use codex_context_fragments::to_annotated_content;
use codex_features::Feature;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_utils_image::EncodedImage;
use codex_utils_image::ImageProcessingError;
use codex_utils_image::PromptImageMode;
use codex_utils_image::data_url_from_bytes;
use codex_utils_image::load_data_url_for_prompt;
use tracing::warn;

pub(crate) const IMAGE_PROCESSING_ERROR_PLACEHOLDER: &str =
    "image content omitted because it could not be processed";
const IMAGE_TOO_LARGE_PLACEHOLDER: &str =
    "image content omitted because it exceeded the supported size limit; use a smaller image";
const UNSUPPORTED_LOW_DETAIL_PLACEHOLDER: &str = "image content omitted because detail 'low' is not supported; use 'high', 'original', or 'auto'";
const REMOTE_IMAGE_URL_PLACEHOLDER: &str =
    "image content omitted because remote image URLs are not supported";

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
    thread_id: &'a str,
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

pub(crate) struct PreparedInlineImage {
    encoded: EncodedImage,
    effective_detail: ImageDetailSetting,
    resize: Option<PreparedImageResize>,
}

impl PreparedInlineImage {
    pub(crate) fn into_data_url(self) -> String {
        self.encoded.into_data_url()
    }

    fn metadata(&self, origin: ImageOrigin<'_>) -> ImagePreparationMetadata {
        ImagePreparationMetadata {
            message_role: origin.message_role.map(str::to_string),
            item_id: origin.item_id.map(str::to_string),
            effective_detail: self.effective_detail,
            source_width: self.encoded.source_width,
            source_height: self.encoded.source_height,
            prepared_width: self.encoded.width,
            prepared_height: self.encoded.height,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ImagePreparationError {
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

pub(crate) async fn prepare_response_items(
    thread_id: &str,
    items: &mut Vec<ResponseItem>,
    mode: ImagePreparationMode,
    resize_notice_mode: ImageResizeNoticeMode,
    image_store: &dyn AttachmentStore,
) -> Vec<ImagePreparationMetadata> {
    let mut metadata = Vec::new();
    let mut prepared_items = Vec::with_capacity(items.len());
    for mut item in std::mem::take(items) {
        let mut annotated_content = to_annotated_content(&mut item);
        let resize_notice = match &mut item {
            ResponseItem::Message { role, .. } => {
                let Some(mut content) = annotated_content.take() else {
                    continue;
                };
                let resized_images = prepare_message_content(
                    &mut content,
                    ImageOrigin {
                        thread_id,
                        message_role: Some(role.as_str()),
                        item_id: None,
                    },
                    if role.as_str() == "user" {
                        resize_notice_mode
                    } else {
                        ImageResizeNoticeMode::Disabled
                    },
                    &mut metadata,
                    mode,
                    image_store,
                )
                .await;
                let _ = set_annotated_content(&mut item, content);
                (!resized_images.is_empty()).then(|| {
                    ImageResizeNotice::new(ImageResizeNoticeSource::UserMessage, resized_images)
                })
            }
            ResponseItem::FunctionCallOutput {
                call_id, output, ..
            } => {
                prepare_tool_output(
                    thread_id,
                    output,
                    call_id.as_deref(),
                    resize_notice_mode,
                    &mut metadata,
                    mode,
                    image_store,
                )
                .await
            }
            ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } => {
                prepare_tool_output(
                    thread_id,
                    output,
                    Some(call_id.as_str()),
                    resize_notice_mode,
                    &mut metadata,
                    mode,
                    image_store,
                )
                .await
            }
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
            | ResponseItem::ConfigurationUpdate { .. }
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

async fn prepare_tool_output(
    thread_id: &str,
    output: &mut FunctionCallOutputPayload,
    item_id: Option<&str>,
    resize_notice_mode: ImageResizeNoticeMode,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
    image_store: &dyn AttachmentStore,
) -> Option<ImageResizeNotice> {
    let content = output.content_items_mut()?;
    let resized_images = prepare_tool_output_content(
        content,
        ImageOrigin {
            thread_id,
            message_role: None,
            item_id,
        },
        resize_notice_mode,
        metadata,
        mode,
        image_store,
    )
    .await;
    if resized_images.is_empty() {
        None
    } else {
        Some(ImageResizeNotice::new(
            ImageResizeNoticeSource::ToolOutput,
            resized_images,
        ))
    }
}

async fn prepare_message_content(
    items: &mut [AnnotatedContent],
    origin: ImageOrigin<'_>,
    resize_notice_mode: ImageResizeNoticeMode,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
    image_store: &dyn AttachmentStore,
) -> Vec<ResizedImage> {
    // File-backed images are already prepared and pass through while retaining their positions in
    // resize notices.
    let image_count = items
        .iter()
        .filter(|item| matches!(item.content(), ContentItem::InputImage { .. }))
        .count();
    let mut image_number = 0;
    let mut resized_images = Vec::new();
    for item in items {
        if let ContentItem::InputImage { image, detail } = item.content_mut() {
            image_number += 1;
            match prepare_image(image, detail, origin, metadata, mode, image_store).await {
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
                    *item = AnnotatedContent::input_text(
                        error.placeholder(),
                        ContentItemKind("images.preparation_error".to_string()),
                    );
                }
            }
        }
    }
    resized_images
}

async fn prepare_tool_output_content(
    items: &mut [FunctionCallOutputContentItem],
    origin: ImageOrigin<'_>,
    resize_notice_mode: ImageResizeNoticeMode,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
    image_store: &dyn AttachmentStore,
) -> Vec<ResizedImage> {
    let image_count = items
        .iter()
        .filter(|item| matches!(item, FunctionCallOutputContentItem::InputImage { .. }))
        .count();
    let mut image_number = 0;
    let mut resized_images = Vec::new();
    for item in items {
        if let FunctionCallOutputContentItem::InputImage { image, detail } = item {
            image_number += 1;
            match prepare_image(image, detail, origin, metadata, mode, image_store).await {
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

async fn prepare_image(
    image: &mut ImageReference,
    detail: &mut Option<ImageDetail>,
    origin: ImageOrigin<'_>,
    metadata: &mut Vec<ImagePreparationMetadata>,
    mode: ImagePreparationMode,
    image_store: &dyn AttachmentStore,
) -> Result<Option<PreparedImageResize>, ImagePreparationError> {
    let ImageReference::Inline { image_url } = image else {
        return Ok(None);
    };
    let Some(prepared) = resize_image(image_url, detail, mode)? else {
        return Ok(None);
    };
    metadata.push(prepared.metadata(origin));
    let resize = prepared.resize;
    let request = UploadRequest {
        thread_id: origin.thread_id.to_owned(),
        file_name: None,
        data: prepared.encoded.bytes.to_vec(),
    };
    *image = match image_store.upload(request).await {
        Ok(UploadResult::Inline { bytes }) => ImageReference::Inline {
            image_url: data_url_from_bytes(&prepared.encoded.mime, &bytes),
        },
        Ok(UploadResult::File { file_id }) => ImageReference::File { file_id },
        Err(error) => {
            warn!(kind = ?error.kind(), "failed to upload prepared image");
            ImageReference::Inline {
                image_url: prepared.into_data_url(),
            }
        }
    };
    Ok(resize)
}

pub(crate) fn resize_image(
    image_url: &str,
    detail: &mut Option<ImageDetail>,
    mode: ImagePreparationMode,
) -> Result<Option<PreparedInlineImage>, ImagePreparationError> {
    if is_remote_image_url(image_url) {
        return Err(ImagePreparationError::RemoteUrlUnsupported);
    }
    if !is_data_url(image_url) {
        return Ok(None);
    }

    let (effective_detail, image_mode) = match mode {
        ImagePreparationMode::UnifiedBudget => (
            ImageDetailSetting::Original,
            PromptImageMode::ORIGINAL_DETAIL,
        ),
        ImagePreparationMode::DetailBased => match detail {
            None | Some(ImageDetail::Auto | ImageDetail::High) => {
                (ImageDetailSetting::High, PromptImageMode::HIGH_DETAIL)
            }
            Some(ImageDetail::Original) => (
                ImageDetailSetting::Original,
                PromptImageMode::ORIGINAL_DETAIL,
            ),
            Some(ImageDetail::Low) => return Err(ImagePreparationError::UnsupportedLowDetail),
        },
    };
    let encoded = load_data_url_for_prompt(image_url, image_mode)?;
    let resize = ((encoded.source_width, encoded.source_height) != (encoded.width, encoded.height))
        .then_some(PreparedImageResize {
            source_width: encoded.source_width,
            source_height: encoded.source_height,
            prepared_width: encoded.width,
            prepared_height: encoded.height,
        });
    if mode == ImagePreparationMode::UnifiedBudget {
        // Preserve accurate context-window accounting while older transports still require an
        // image detail field. Responses Lite removes this compatibility hint before sending.
        *detail = Some(ImageDetail::Original);
    }
    Ok(Some(PreparedInlineImage {
        encoded,
        effective_detail,
        resize,
    }))
}

#[cfg(test)]
#[path = "image_preparation_tests.rs"]
mod tests;
