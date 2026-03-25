use super::*;

use crate::config::test_config;
use crate::models_manager::manager::ModelsManager;
use codex_features::Features;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn image_detail_original_feature_enables_explicit_original_without_force() {
    let config = test_config().await;
    let mut model_info =
        ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
    model_info.supports_image_detail_original = true;
    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);

    assert!(can_request_original_image_detail(&features, &model_info));
    assert_eq!(
        normalize_output_image_detail(&features, &model_info, Some(ImageDetail::Original)),
        Some(ImageDetail::Original)
    );
    assert_eq!(
        normalize_output_image_detail(&features, &model_info, None),
        None
    );
}

#[tokio::test]
async fn explicit_original_is_dropped_without_feature_or_model_support() {
    let config = test_config().await;
    let mut model_info =
        ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
    model_info.supports_image_detail_original = true;
    let features = Features::with_defaults();

    assert_eq!(
        normalize_output_image_detail(&features, &model_info, Some(ImageDetail::Original)),
        None
    );

    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);
    model_info.supports_image_detail_original = false;
    assert_eq!(
        normalize_output_image_detail(&features, &model_info, Some(ImageDetail::Original)),
        None
    );
}

#[tokio::test]
async fn unsupported_non_original_detail_is_dropped() {
    let config = test_config().await;
    let mut model_info =
        ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
    model_info.supports_image_detail_original = true;
    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);

    assert_eq!(
        normalize_output_image_detail(&features, &model_info, Some(ImageDetail::Low)),
        None
    );
}
