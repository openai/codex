use crate::config::ManagedFeatures;
use codex_features::Feature;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::MultiAgentVersion;

pub(crate) fn resolve_multi_agent_version(
    model_info: &ModelInfo,
    features: &ManagedFeatures,
) -> Option<MultiAgentVersion> {
    model_info
        .multi_agent_version
        .or_else(|| multi_agent_version_from_features(features))
}

fn multi_agent_version_from_features(features: &ManagedFeatures) -> Option<MultiAgentVersion> {
    if features.enabled(Feature::MultiAgentV2) {
        Some(MultiAgentVersion::V2)
    } else if features.enabled(Feature::Collab) {
        Some(MultiAgentVersion::V1)
    } else {
        None
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

    fn model_info(multi_agent_version: Option<MultiAgentVersion>) -> ModelInfo {
        let mut model_info = codex_models_manager::model_info::model_info_from_slug("test-model");
        model_info.multi_agent_version = multi_agent_version;
        model_info
    }

    #[test]
    fn omitted_selector_follows_feature_flags() {
        let multi_agent_version = resolve_multi_agent_version(
            &model_info(/*multi_agent_version*/ None),
            &features(&[Feature::MultiAgentV2]),
        );

        assert_eq!(multi_agent_version, Some(MultiAgentVersion::V2));
    }

    #[test]
    fn explicit_selector_overrides_feature_flags() {
        let multi_agent_version = resolve_multi_agent_version(
            &model_info(Some(MultiAgentVersion::V1)),
            &features(&[Feature::MultiAgentV2]),
        );

        assert_eq!(multi_agent_version, Some(MultiAgentVersion::V1));
    }
}
