use crate::error::InlineImageRequestLimitExceededError;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;

pub(crate) const DEFAULT_INLINE_IMAGE_REQUEST_LIMIT_BYTES: i64 = 512 * 1024 * 1024;
pub(crate) const DEFAULT_INLINE_IMAGE_REQUEST_LIMIT_IMAGE_COUNT: i64 = 1_500;

pub(crate) fn inline_image_request_limit_bytes(model_info: &ModelInfo) -> usize {
    model_info
        .inline_image_request_limit_bytes
        .filter(|limit| *limit > 0)
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(usize::try_from(DEFAULT_INLINE_IMAGE_REQUEST_LIMIT_BYTES).unwrap_or(usize::MAX))
}

pub(crate) fn inline_image_request_limit_image_count(model_info: &ModelInfo) -> usize {
    model_info
        .inline_image_request_limit_image_count
        .filter(|limit| *limit > 0)
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(
            usize::try_from(DEFAULT_INLINE_IMAGE_REQUEST_LIMIT_IMAGE_COUNT).unwrap_or(usize::MAX),
        )
}

pub(crate) fn visit_response_item_input_images(
    item: &ResponseItem,
    mut visitor: impl FnMut(&str, Option<ImageDetail>),
) {
    match item {
        ResponseItem::Message { content, .. } => {
            for content_item in content {
                if let ContentItem::InputImage { image_url } = content_item {
                    visitor(image_url, None);
                }
            }
        }
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => {
            if let FunctionCallOutputBody::ContentItems(items) = &output.body {
                for content_item in items {
                    if let FunctionCallOutputContentItem::InputImage { image_url, detail } =
                        content_item
                    {
                        visitor(image_url, *detail);
                    }
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn is_inline_data_url(url: &str) -> bool {
    url.get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

pub(crate) fn parse_base64_image_data_url(url: &str) -> Option<&str> {
    if !is_inline_data_url(url) {
        return None;
    }
    let comma_index = url.find(',')?;
    let metadata = &url[..comma_index];
    let payload = &url[comma_index + 1..];
    let metadata_without_scheme = &metadata["data:".len()..];
    let mut metadata_parts = metadata_without_scheme.split(';');
    let mime_type = metadata_parts.next().unwrap_or_default();
    let has_base64_marker = metadata_parts.any(|part| part.eq_ignore_ascii_case("base64"));
    if !mime_type
        .get(.."image/".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("image/"))
    {
        return None;
    }
    if !has_base64_marker {
        return None;
    }
    Some(payload)
}

pub(crate) fn total_inline_image_request_bytes(items: &[ResponseItem]) -> usize {
    let mut total = 0usize;
    for item in items {
        visit_response_item_input_images(item, |image_url, _detail| {
            if is_inline_data_url(image_url) {
                total = total.saturating_add(image_url.len());
            }
        });
    }
    total
}

pub(crate) fn total_image_request_count(items: &[ResponseItem]) -> usize {
    let mut total = 0usize;
    for item in items {
        visit_response_item_input_images(item, |_image_url, _detail| {
            total = total.saturating_add(1);
        });
    }
    total
}

pub(crate) fn inline_image_request_limit_error(
    items: &[ResponseItem],
    model_info: &ModelInfo,
) -> Option<InlineImageRequestLimitExceededError> {
    let total_inline_image_bytes = total_inline_image_request_bytes(items);
    let limit_bytes = inline_image_request_limit_bytes(model_info);
    let total_images = total_image_request_count(items);
    let limit_images = inline_image_request_limit_image_count(model_info);
    let exceeds_bytes = total_inline_image_bytes > limit_bytes;
    let exceeds_images = total_images > limit_images;
    if !exceeds_bytes && !exceeds_images {
        return None;
    }

    Some(if exceeds_bytes && exceeds_images {
        InlineImageRequestLimitExceededError::local_preflight_bytes_and_images(
            total_inline_image_bytes,
            limit_bytes,
            total_images,
            limit_images,
        )
    } else if exceeds_bytes {
        InlineImageRequestLimitExceededError::local_preflight_bytes(
            total_inline_image_bytes,
            limit_bytes,
        )
    } else {
        InlineImageRequestLimitExceededError::local_preflight_images(total_images, limit_images)
    })
}

#[cfg(test)]
#[path = "inline_image_request_limit_tests.rs"]
mod tests;
