use super::*;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::models::ImageDetail;
use codex_protocol::openai_models::ModelInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

fn model_info() -> ModelInfo {
    serde_json::from_value(json!({
        "slug": "test-model",
        "display_name": "Test Model",
        "description": null,
        "supported_reasoning_levels": [],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 1,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": "base",
        "model_messages": null,
        "supports_reasoning_summaries": false,
        "default_reasoning_summary": "auto",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "truncation_policy": {
            "mode": "bytes",
            "limit": 10000
        },
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": true,
        "context_window": null,
        "auto_compact_token_limit": null,
        "effective_context_window_percent": 95,
        "experimental_supported_tools": [],
        "input_modalities": ["text", "image"],
        "supports_search_tool": false
    }))
    .expect("deserialize test model")
}

#[test]
fn image_detail_original_feature_enables_explicit_original_without_force() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);

    assert!(can_request_original_image_detail(&features, &model_info));
    assert_eq!(
        normalize_output_image_detail(&features, &model_info, Some(ImageDetail::Original)),
        Some(ImageDetail::Original)
    );
    assert_eq!(
        normalize_output_image_detail(&features, &model_info, /*detail*/ None),
        None
    );
}

#[test]
fn explicit_original_is_dropped_without_feature_or_model_support() {
    let mut model_info = model_info();
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

#[test]
fn unsupported_non_original_detail_is_dropped() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);

    assert_eq!(
        normalize_output_image_detail(&features, &model_info, Some(ImageDetail::Low)),
        None
    );
}
