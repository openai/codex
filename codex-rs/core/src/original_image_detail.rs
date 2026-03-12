use crate::features::Feature;
use crate::features::Features;
use codex_protocol::models::ImageDetail;
use codex_protocol::openai_models::ModelInfo;

pub(crate) fn can_request_original_image_detail(
    features: &Features,
    model_info: &ModelInfo,
) -> bool {
    model_info.supports_image_detail_original && features.enabled(Feature::ImageDetailOriginal)
}

pub(crate) fn normalize_output_image_detail(
    features: &Features,
    model_info: &ModelInfo,
    detail: Option<ImageDetail>,
) -> Option<ImageDetail> {
    match detail {
        Some(ImageDetail::Original) if can_request_original_image_detail(features, model_info) => {
            Some(ImageDetail::Original)
        }
        Some(ImageDetail::Original) | Some(_) | None => None,
    }
}

#[cfg(test)]
#[path = "original_image_detail_tests.rs"]
mod tests;
