use crate::FeatureConfig;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MultiAgentV2ConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_spawn_agent_metadata: Option<bool>,
}

impl FeatureConfig for MultiAgentV2ConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrefixCompactionConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Percentage of the auto-compaction threshold that starts background
    /// prefix compaction. When omitted, Codex uses 60%.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_percent: Option<i64>,
}

impl FeatureConfig for PrefixCompactionConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}
