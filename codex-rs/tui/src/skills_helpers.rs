use codex_core_skills::model::SkillMetadata;
use codex_utils_fuzzy_match::fuzzy_match;

pub(crate) fn skill_display_name(skill: &SkillMetadata) -> String {
    if let Some(display_name) = skill
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
    {
        return display_name.to_string();
    }

    if let Some((plugin_name, skill_name)) = skill.name.split_once(':')
        && !plugin_name.is_empty()
        && !skill_name.is_empty()
    {
        return format!("{skill_name} ({plugin_name})");
    }

    skill.name.clone()
}

pub(crate) fn skill_description(skill: &SkillMetadata) -> &str {
    skill
        .interface
        .as_ref()
        .and_then(|interface| interface.short_description.as_deref())
        .or(skill.short_description.as_deref())
        .unwrap_or(&skill.description)
}

pub(crate) fn match_skill(
    filter: &str,
    display_name: &str,
    skill_name: &str,
) -> Option<(Option<Vec<usize>>, i32)> {
    if let Some((indices, score)) = fuzzy_match(display_name, filter) {
        return Some((Some(indices), score));
    }
    if display_name != skill_name
        && let Some((_indices, score)) = fuzzy_match(skill_name, filter)
    {
        return Some((None, score));
    }
    None
}
