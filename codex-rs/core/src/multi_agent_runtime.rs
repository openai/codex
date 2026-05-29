use crate::config::Config;
use crate::config::DEFAULT_AGENT_MAX_THREADS;
use crate::config::ManagedFeatures;
use codex_features::Feature;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::MultiAgentVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MultiAgentRuntime {
    version: Option<MultiAgentVersion>,
    collab_tools_disabled: bool,
}

impl MultiAgentRuntime {
    #[cfg(test)]
    pub(crate) fn from_features(features: &ManagedFeatures) -> Self {
        Self {
            version: multi_agent_version_from_features(features),
            collab_tools_disabled: false,
        }
    }

    pub(crate) fn resolve(model_info: &ModelInfo, features: &ManagedFeatures) -> Self {
        Self {
            version: model_info
                .multi_agent_version
                .or_else(|| multi_agent_version_from_features(features)),
            collab_tools_disabled: false,
        }
    }

    pub(crate) async fn resolve_for_config(
        models_manager: &SharedModelsManager,
        config: &Config,
    ) -> Self {
        let model = models_manager
            .get_default_model(&config.model, RefreshStrategy::Offline)
            .await;
        let model_info = models_manager
            .get_model_info(&model, &config.to_models_manager_config())
            .await;
        Self::resolve(&model_info, &config.features)
    }

    pub(crate) fn with_model(self, model_info: &ModelInfo, features: &ManagedFeatures) -> Self {
        Self {
            collab_tools_disabled: self.collab_tools_disabled,
            ..Self::resolve(model_info, features)
        }
    }

    pub(crate) fn collab_tools_enabled(self) -> bool {
        !self.collab_tools_disabled && self.version.is_some()
    }

    pub(crate) fn multi_agent_v2_enabled(self) -> bool {
        self.version == Some(MultiAgentVersion::V2)
    }

    pub(crate) fn agent_max_threads(self, config: &Config) -> Option<usize> {
        if self.multi_agent_v2_enabled() {
            Some(
                config
                    .multi_agent_v2
                    .max_concurrent_threads_per_session
                    .saturating_sub(1),
            )
        } else if config.features.enabled(Feature::MultiAgentV2) {
            DEFAULT_AGENT_MAX_THREADS
        } else {
            config.agent_max_threads
        }
    }

    pub(crate) fn disable_collab_tools(&mut self) {
        self.collab_tools_disabled = true;
    }
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
        let runtime = MultiAgentRuntime::resolve(
            &model_info(/*multi_agent_version*/ None),
            &features(&[Feature::MultiAgentV2]),
        );

        assert_eq!(
            runtime,
            MultiAgentRuntime {
                version: Some(MultiAgentVersion::V2),
                collab_tools_disabled: false,
            }
        );
    }

    #[test]
    fn explicit_selector_overrides_feature_flags() {
        let runtime = MultiAgentRuntime::resolve(
            &model_info(Some(MultiAgentVersion::V1)),
            &features(&[Feature::MultiAgentV2]),
        );

        assert_eq!(
            runtime,
            MultiAgentRuntime {
                version: Some(MultiAgentVersion::V1),
                collab_tools_disabled: false,
            }
        );
    }
}
