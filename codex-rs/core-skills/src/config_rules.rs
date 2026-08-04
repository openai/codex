use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::SkillConfig;
use codex_config::SkillsConfig;
pub use codex_skills::SkillConfigRule;
pub use codex_skills::SkillConfigRuleSelector;
pub use codex_skills::SkillConfigRules;
pub use codex_skills::resolve_disabled_skill_paths;
use tracing::warn;

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
#[path = "config_rules_tests.rs"]
mod tests;
