use std::borrow::Cow;

use codex_utils_string::approx_token_count;
use codex_utils_string::take_bytes_at_char_boundary;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::fragments::AvailableSkillsInstructions;

const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
const MAX_SKILL_METADATA_TOKEN_BUDGET: usize = 4_000;
const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;
const MAX_MAIN_PROMPT_BYTES: usize = 8_000;
const MAX_CATALOG_SKILL_DESCRIPTION_CHARS: usize = 1_024;
const TRUNCATED_SKILL_DESCRIPTION_SUFFIX: &str = "...";
pub(crate) const MAX_SKILL_NAME_BYTES: usize = 256;
pub(crate) const MAX_SKILL_PATH_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillCatalogRenderPolicy {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by the host renderer compatibility path in a follow-up"
        )
    )]
    CoreCompatible,
    ExtensionCompatible,
}

impl SkillCatalogRenderPolicy {
    fn description(self, entry: &SkillCatalogEntry) -> &str {
        match self {
            Self::CoreCompatible => entry.description.as_str(),
            Self::ExtensionCompatible => entry
                .short_description
                .as_deref()
                .unwrap_or(entry.description.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillMetadataBudget {
    Tokens(usize),
    Characters(usize),
}

pub(crate) fn capped_skill_metadata_budget(context_window: Option<i64>) -> SkillMetadataBudget {
    context_window
        .and_then(|window| usize::try_from(window).ok())
        .filter(|window| *window > 0)
        .map(|window| {
            SkillMetadataBudget::Tokens(
                window
                    .saturating_mul(SKILL_METADATA_CONTEXT_WINDOW_PERCENT)
                    .saturating_div(100)
                    .clamp(1, MAX_SKILL_METADATA_TOKEN_BUDGET),
            )
        })
        .unwrap_or(SkillMetadataBudget::Characters(
            DEFAULT_SKILL_METADATA_CHAR_BUDGET,
        ))
}

fn metadata_line_cost(budget: SkillMetadataBudget, line: &str) -> usize {
    let line = format!("{line}\n");
    match budget {
        SkillMetadataBudget::Tokens(_) => approx_token_count(&line),
        SkillMetadataBudget::Characters(_) => line.chars().count(),
    }
}

#[tracing::instrument(
    level = "trace",
    skip_all,
    fields(catalog_entry_count = catalog.entries.len())
)]
pub(crate) fn available_skills_fragment(
    catalog: &SkillCatalog,
    include_skills_usage_instructions: bool,
    policy: SkillCatalogRenderPolicy,
    budget: SkillMetadataBudget,
) -> Option<AvailableSkillsInstructions> {
    let budget_limit = match budget {
        SkillMetadataBudget::Tokens(limit) | SkillMetadataBudget::Characters(limit) => limit,
    };
    let mut total_cost = 0usize;
    let mut omitted = 0usize;
    let mut skill_lines = Vec::new();

    for entry in catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.prompt_visible)
    {
        let description = policy.description(entry);
        let description = truncate_catalog_skill_description(description);
        let line = render_skill_line(entry, description.as_ref());
        let next_cost = total_cost.saturating_add(metadata_line_cost(budget, &line));
        if next_cost > budget_limit {
            omitted = omitted.saturating_add(1);
            continue;
        }
        total_cost = next_cost;
        skill_lines.push(line);
    }

    if omitted > 0 {
        loop {
            let marker = omission_marker(omitted);
            if total_cost.saturating_add(metadata_line_cost(budget, &marker)) <= budget_limit {
                skill_lines.push(marker);
                break;
            }
            let line = skill_lines.pop()?;
            total_cost = total_cost.saturating_sub(metadata_line_cost(budget, &line));
            omitted = omitted.saturating_add(1);
        }
    }

    (!skill_lines.is_empty()).then(|| {
        AvailableSkillsInstructions::from_skill_lines(
            skill_lines,
            include_skills_usage_instructions,
        )
    })
}

fn omission_marker(omitted: usize) -> String {
    let skill_word = if omitted == 1 { "skill" } else { "skills" };
    format!("- {omitted} additional {skill_word} omitted from this bounded skills list.")
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

fn render_skill_line(entry: &SkillCatalogEntry, description: &str) -> String {
    let locator_kind = match &entry.authority.kind {
        SkillSourceKind::Host => "file",
        SkillSourceKind::Executor => "environment resource",
        SkillSourceKind::Orchestrator => "orchestrator resource",
        SkillSourceKind::Custom(_) => "custom resource",
    };
    let name = entry.name.as_str();
    let path = entry.rendered_path();
    if description.is_empty() {
        format!("- {name}: ({locator_kind}: {path})")
    } else {
        format!("- {name}: {description} ({locator_kind}: {path})")
    }
}

pub(crate) fn truncate_main_prompt_contents(contents: &str) -> (String, bool) {
    truncate_utf8_to_bytes(contents, MAX_MAIN_PROMPT_BYTES)
}

pub(crate) fn truncate_utf8_to_bytes(contents: &str, max_bytes: usize) -> (String, bool) {
    let truncated = take_bytes_at_char_boundary(contents, max_bytes);
    (truncated.to_string(), truncated.len() < contents.len())
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
