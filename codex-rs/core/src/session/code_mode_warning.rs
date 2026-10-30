use codex_features::Feature;
use codex_features::Features;
use codex_protocol::openai_models::CatalogModel;
use codex_protocol::openai_models::ModelOrigin;

pub(super) fn unsupported_code_mode_warning(
    model_info: &CatalogModel,
    features: &Features,
) -> Option<String> {
    let code_mode_enabled =
        features.enabled(Feature::CodeMode) || features.enabled(Feature::CodeModeOnly);
    if !code_mode_enabled
        || model_info.tool_mode.is_some()
        || model_info.origin == ModelOrigin::Synthetic
    {
        return None;
    }

    let model = &model_info.slug;
    Some(format!(
        "Code Mode is enabled in configuration, but model `{model}` does not advertise Code Mode support. This may degrade model performance. Disable `features.code_mode` and `features.code_mode_only`, or select a model whose metadata enables Code Mode."
    ))
}

#[cfg(test)]
#[path = "code_mode_warning_tests.rs"]
mod tests;
