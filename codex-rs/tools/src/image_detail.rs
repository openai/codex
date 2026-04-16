use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::openai_models::ModelInfo;

pub fn can_request_original_image_detail(model_info: &ModelInfo) -> bool {
    model_info.supports_image_detail_original
}

pub fn normalize_output_image_detail(
    model_info: &ModelInfo,
    detail: Option<ImageDetail>,
) -> Option<ImageDetail> {
    match detail {
        Some(ImageDetail::Original) if can_request_original_image_detail(model_info) => {
            Some(ImageDetail::Original)
        }
        Some(ImageDetail::Original) | Some(_) | None => None,
    }
}

pub fn sanitize_original_image_detail(
    can_request_original_image_detail: bool,
    items: &mut [FunctionCallOutputContentItem],
) {
    if can_request_original_image_detail {
        return;
    }

    for item in items {
        if let FunctionCallOutputContentItem::InputImage { detail, .. } = item
            && matches!(detail, Some(ImageDetail::Original))
        {
            *detail = None;
        }
    }
}

#[cfg(test)]
#[path = "image_detail_tests.rs"]
mod tests;
