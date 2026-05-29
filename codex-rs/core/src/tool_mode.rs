use crate::config::ManagedFeatures;
use codex_features::Feature;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ToolMode;

pub(crate) fn resolve_tool_mode(model_info: &ModelInfo, features: &ManagedFeatures) -> ToolMode {
    model_info
        .tool_mode
        .unwrap_or_else(|| tool_mode_from_features(features))
}

fn tool_mode_from_features(features: &ManagedFeatures) -> ToolMode {
    if features.enabled(Feature::CodeModeOnly) {
        ToolMode::CodeModeOnly
    } else if features.enabled(Feature::CodeMode) {
        ToolMode::CodeMode
    } else {
        ToolMode::Direct
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_features::Features;

    fn features(enabled: &[Feature]) -> ManagedFeatures {
        let mut features = Features::default();
        for feature in enabled {
            features.enable(*feature);
        }
        features.into()
    }

    fn model_info(tool_mode: Option<ToolMode>) -> ModelInfo {
        let mut model_info = codex_models_manager::model_info::model_info_from_slug("test-model");
        model_info.tool_mode = tool_mode;
        model_info
    }

    #[test]
    fn omitted_selector_follows_feature_flags() {
        let tool_mode = resolve_tool_mode(
            &model_info(/*tool_mode*/ None),
            &features(&[Feature::CodeModeOnly]),
        );

        assert_eq!(tool_mode, ToolMode::CodeModeOnly);
    }

    #[test]
    fn explicit_selector_overrides_feature_flags() {
        let tool_mode = resolve_tool_mode(
            &model_info(Some(ToolMode::Direct)),
            &features(&[Feature::CodeModeOnly]),
        );

        assert_eq!(tool_mode, ToolMode::Direct);
    }
}
