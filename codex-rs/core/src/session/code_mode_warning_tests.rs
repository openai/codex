use super::unsupported_code_mode_warning;
use codex_features::Feature;
use codex_features::Features;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ToolMode;
use pretty_assertions::assert_eq;

const MODEL_SLUG: &str = "test-model";

fn known_model_info() -> ModelInfo {
    ModelInfo {
        used_fallback_model_metadata: false,
        ..model_info_from_slug(MODEL_SLUG)
    }
}

#[test]
fn warns_when_code_mode_is_enabled_without_model_selector() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);

    assert_eq!(
        unsupported_code_mode_warning(&known_model_info(), &features),
        Some(format!(
            "Code Mode is enabled in configuration, but model `{MODEL_SLUG}` does not advertise Code Mode support. This may degrade model performance. Disable `features.code_mode` and `features.code_mode_only`, or select a model whose metadata enables Code Mode."
        ))
    );
}

#[test]
fn warns_when_code_mode_only_is_enabled_without_model_selector() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeModeOnly);

    assert!(unsupported_code_mode_warning(&known_model_info(), &features).is_some());
}

#[test]
fn does_not_warn_when_code_mode_is_disabled() {
    assert_eq!(
        unsupported_code_mode_warning(&known_model_info(), &Features::with_defaults()),
        None
    );
}

#[test]
fn does_not_warn_when_model_has_tool_mode_selector() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeModeOnly);

    for tool_mode in [ToolMode::Direct, ToolMode::CodeMode, ToolMode::CodeModeOnly] {
        let model_info = ModelInfo {
            tool_mode: Some(tool_mode),
            ..known_model_info()
        };
        assert_eq!(unsupported_code_mode_warning(&model_info, &features), None);
    }
}

#[test]
fn fallback_metadata_only_uses_existing_warning() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);

    assert_eq!(
        unsupported_code_mode_warning(&model_info_from_slug(MODEL_SLUG), &features),
        None
    );
}
