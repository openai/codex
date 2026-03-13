use crate::features::Feature;
use crate::features::Features;
use codex_protocol::openai_models::ModelInfo;

pub(crate) fn can_request_original_image_detail(
    features: &Features,
    model_info: &ModelInfo,
) -> bool {
    model_info.supports_image_detail_original && features.enabled(Feature::ImageDetailOriginal)
}

#[cfg(test)]
#[path = "original_image_detail_tests.rs"]
mod tests;
