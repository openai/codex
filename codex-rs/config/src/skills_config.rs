//! Skill-related configuration types shared across crates.

use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

const fn default_enabled() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SkillConfig {
    /// Path-based selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<AbsolutePathBuf>,
    /// Name-based selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SkillsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundled: Option<BundledSkillsConfig>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<SkillConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BundledSkillsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for BundledSkillsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl TryFrom<toml::Value> for SkillsConfig {
    type Error = toml::de::Error;

    fn try_from(value: toml::Value) -> Result<Self, Self::Error> {
        SkillsConfig::deserialize(value)
    }
}
