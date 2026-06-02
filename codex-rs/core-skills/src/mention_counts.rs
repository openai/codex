use std::collections::HashMap;
use std::collections::HashSet;

use super::SkillMetadata;
use codex_exec_server::EnvironmentPathRef;

/// Counts how often each skill name appears (exact and ASCII-lowercase), excluding disabled paths.
pub fn build_skill_name_counts(
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<EnvironmentPathRef>,
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    build_skill_name_counts_with_disabled(skills, |skill| {
        disabled_paths.contains(&skill.source_path)
    })
}

fn build_skill_name_counts_with_disabled(
    skills: &[SkillMetadata],
    mut is_disabled: impl FnMut(&SkillMetadata) -> bool,
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut exact_counts: HashMap<String, usize> = HashMap::new();
    let mut lower_counts: HashMap<String, usize> = HashMap::new();
    for skill in skills {
        if is_disabled(skill) {
            continue;
        }
        *exact_counts.entry(skill.name.clone()).or_insert(0) += 1;
        *lower_counts
            .entry(skill.name.to_ascii_lowercase())
            .or_insert(0) += 1;
    }
    (exact_counts, lower_counts)
}
