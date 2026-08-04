use std::collections::HashSet;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::SkillConfigRuleSelector;
use crate::SkillConfigRules;
use crate::SkillMetadata;

/// Resolves disabled `SKILL.md` paths by applying configuration rules in order.
///
/// Later rules override earlier rules for every skill path they select.
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

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
