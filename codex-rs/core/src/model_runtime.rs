use crate::config::ManagedFeatures;
use codex_features::Feature;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ToolMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModelRuntimeModes {
    tool_mode: ToolMode,
}

impl ModelRuntimeModes {
    pub(crate) fn from_features(features: &ManagedFeatures) -> Self {
        Self {
            tool_mode: tool_mode_from_features(features),
        }
    }

    pub(crate) fn resolve(model_info: &ModelInfo, features: &ManagedFeatures) -> Self {
        Self {
            tool_mode: model_info
                .tool_mode
                .unwrap_or_else(|| tool_mode_from_features(features)),
        }
    }

    pub(crate) fn code_mode_enabled(self) -> bool {
        matches!(self.tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly)
    }

    pub(crate) fn code_mode_only_enabled(self) -> bool {
        self.tool_mode == ToolMode::CodeModeOnly
    }
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
        let modes = ModelRuntimeModes::resolve(
            &model_info(/*tool_mode*/ None),
            &features(&[Feature::CodeModeOnly]),
        );

        assert_eq!(
            modes,
            ModelRuntimeModes {
                tool_mode: ToolMode::CodeModeOnly,
            }
        );
    }

    #[test]
    fn explicit_selector_overrides_feature_flags() {
        let modes = ModelRuntimeModes::resolve(
            &model_info(Some(ToolMode::Direct)),
            &features(&[Feature::CodeModeOnly]),
        );

        assert_eq!(
            modes,
            ModelRuntimeModes {
                tool_mode: ToolMode::Direct,
            }
        );
    }
}
