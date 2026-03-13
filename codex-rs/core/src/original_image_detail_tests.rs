use super::*;

use crate::config::test_config;
use crate::features::Features;
use crate::models_manager::manager::ModelsManager;

#[test]
fn image_detail_original_requires_feature_and_model_support() {
    let config = test_config();
    let mut model_info =
        ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
    let features = Features::with_defaults();

    model_info.supports_image_detail_original = true;
    assert!(!can_request_original_image_detail(&features, &model_info));

    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);
    assert!(can_request_original_image_detail(&features, &model_info));

    model_info.supports_image_detail_original = false;
    assert!(!can_request_original_image_detail(&features, &model_info));
}
