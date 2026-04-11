use std::collections::HashSet;

use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::SkillConfig;
use codex_config::SkillsConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

use crate::SkillMetadata;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkillConfigRuleSelector {
    Name(String),
    Path(AbsolutePathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkillConfigRule {
    pub selector: SkillConfigRuleSelector,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SkillConfigRules {
    pub entries: Vec<SkillConfigRule>,
}

pub fn skill_config_rules_from_stack(config_layer_stack: &ConfigLayerStack) -> SkillConfigRules {
    let mut entries = Vec::new();
    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ true,
    ) {
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

pub fn resolve_disabled_skill_paths(
    skills: &[SkillMetadata],
    rules: &SkillConfigRules,
) -> HashSet<AbsolutePathBuf> {
    let mut disabled_paths = HashSet::new();

    for entry in &rules.entries {
        match &entry.selector {
            SkillConfigRuleSelector::Path(path) => {
                if entry.enabled {
                    disabled_paths.remove(path);
                } else {
                    disabled_paths.insert(path.clone());
                }
            }
            SkillConfigRuleSelector::Name(name) => {
                for path in skills
                    .iter()
                    .filter(|skill| skill.name == *name)
                    .map(|skill| skill.path_to_skills_md.clone())
                {
                    if entry.enabled {
                        disabled_paths.remove(&path);
                    } else {
                        disabled_paths.insert(path);
                    }
                }
            }
        }
    }

    disabled_paths
}

fn skill_config_rule_selector(entry: &SkillConfig) -> Option<SkillConfigRuleSelector> {
    match (entry.path.as_ref(), entry.name.as_deref()) {
        (Some(path), None) => match path.canonicalize() {
            Ok(path) => Some(SkillConfigRuleSelector::Path(path)),
            Err(err) => {
                warn!(
                    "ignoring skills.config path override that could not be canonicalized: {}: {err}",
                    path.display()
                );
                None
            }
        },
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
