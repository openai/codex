use std::borrow::Cow;

use codex_core_skills::SkillRenderSideEffects;
use codex_core_skills::build_available_skills_from_catalog;
use codex_core_skills::default_skill_metadata_budget;
use codex_utils_string::take_bytes_at_char_boundary;

use crate::catalog::SkillCatalog;
use crate::fragments::AvailableSkillsInstructions;

const MAX_CATALOG_SKILL_DESCRIPTION_CHARS: usize = 1_024;
const TRUNCATED_SKILL_DESCRIPTION_SUFFIX: &str = "...";

#[tracing::instrument(
    level = "trace",
    skip_all,
    fields(catalog_entry_count = catalog.entries.len())
)]
pub(crate) fn available_skills_fragment(
    catalog: &SkillCatalog,
) -> Option<(AvailableSkillsInstructions, Option<String>)> {
    let available = build_available_skills_from_catalog(
        catalog,
        /*host_outcome*/ None,
        default_skill_metadata_budget(/*context_window*/ None),
        SkillRenderSideEffects::None,
    )?;
    let warning = available.warning_message.clone();
    Some((available.into(), warning))
}

pub(crate) fn truncate_catalog_skill_description(description: &str) -> Cow<'_, str> {
    if description
        .char_indices()
        .nth(MAX_CATALOG_SKILL_DESCRIPTION_CHARS)
        .is_none()
    {
        return Cow::Borrowed(description);
    }

    let prefix_chars = MAX_CATALOG_SKILL_DESCRIPTION_CHARS
        .saturating_sub(TRUNCATED_SKILL_DESCRIPTION_SUFFIX.chars().count());
    let prefix_end = description
        .char_indices()
        .nth(prefix_chars)
        .map_or(description.len(), |(index, _)| index);
    let mut truncated = description[..prefix_end].to_string();
    truncated.push_str(TRUNCATED_SKILL_DESCRIPTION_SUFFIX);
    Cow::Owned(truncated)
}

pub(crate) fn truncate_utf8_to_bytes(contents: &str, max_bytes: usize) -> (String, bool) {
    let truncated = take_bytes_at_char_boundary(contents, max_bytes);
    (truncated.to_string(), truncated.len() < contents.len())
}
