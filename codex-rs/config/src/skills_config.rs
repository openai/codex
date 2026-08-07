//! Skill-related configuration types shared across crates.

use std::collections::HashSet;

use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;

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

    /// Whether turns receive the automatic skills instructions block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_instructions: Option<bool>,

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

/// Selects configured skills by their name or canonical document path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkillConfigRuleSelector {
    Name(String),
    Path(AbsolutePathBuf),
}

/// Enables or disables every skill matched by its selector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkillConfigRule {
    pub selector: SkillConfigRuleSelector,
    pub enabled: bool,
}

/// Ordered effective skill enablement rules from configuration layers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SkillConfigRules {
    pub entries: Vec<SkillConfigRule>,
}

impl SkillConfigRules {
    /// Applies rules in order; later rules override earlier rules for matching skills.
    ///
    /// Explicit path selectors remain effective even when no current skill matches.
    pub fn resolve_disabled_paths<'a>(
        &self,
        skills: impl IntoIterator<Item = (&'a str, &'a AbsolutePathBuf)> + Clone,
    ) -> HashSet<AbsolutePathBuf> {
        let mut disabled_paths = HashSet::new();

        for entry in &self.entries {
            match &entry.selector {
                SkillConfigRuleSelector::Path(path) => {
                    if entry.enabled {
                        disabled_paths.remove(path);
                    } else {
                        disabled_paths.insert(path.clone());
                    }
                }
                SkillConfigRuleSelector::Name(name) => {
                    for (skill_name, path) in skills.clone() {
                        if skill_name != name {
                            continue;
                        }
                        if entry.enabled {
                            disabled_paths.remove(path);
                        } else {
                            disabled_paths.insert(path.clone());
                        }
                    }
                }
            }
        }

        disabled_paths
    }
}

/// Resolves skill enablement rules from user and session configuration layers.
pub fn skill_config_rules_from_stack(config_layer_stack: &ConfigLayerStack) -> SkillConfigRules {
    let mut entries = Vec::new();
    for layer in config_layer_stack.all_layers_low_to_high() {
        if !matches!(
            layer.name,
            ConfigLayerSource::User { .. } | ConfigLayerSource::SessionFlags
        ) {
            continue;
        }

        let Some(skills_value) = layer.config.get("skills") else {
            continue;
        };
        let skills: SkillsConfig = match skills_value.clone().try_into() {
            Ok(skills) => skills,
            Err(err) => {
                warn!("invalid skills config: {err}");
                continue;
            }
        };

        for entry in skills.config {
            let Some(selector) = skill_config_rule_selector(&entry) else {
                continue;
            };
            // Preserve layer order so a later name selector can override an earlier path selector
            // for the same loaded skill.
            entries.retain(|entry: &SkillConfigRule| entry.selector != selector);
            entries.push(SkillConfigRule {
                selector,
                enabled: entry.enabled,
            });
        }
    }

    SkillConfigRules { entries }
}

fn skill_config_rule_selector(entry: &SkillConfig) -> Option<SkillConfigRuleSelector> {
    match (entry.path.as_ref(), entry.name.as_deref()) {
        (Some(path), None) => Some(SkillConfigRuleSelector::Path(
            path.canonicalize().unwrap_or_else(|_| path.clone()),
        )),
        (None, Some(name)) => {
            let name = name.trim();
            if name.is_empty() {
                warn!("ignoring empty skills.config name override");
                None
            } else {
                Some(SkillConfigRuleSelector::Name(name.to_string()))
            }
        }
        (Some(_), Some(_)) => {
            warn!("ignoring skills.config entry with both path and name selectors");
            None
        }
        (None, None) => {
            warn!("ignoring skills.config entry without a path or name selector");
            None
        }
    }
}

#[cfg(test)]
#[path = "skills_config_tests.rs"]
mod tests;
