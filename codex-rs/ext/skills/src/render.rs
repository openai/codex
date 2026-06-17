use codex_utils_string::take_bytes_at_char_boundary;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::fragments::AvailableSkillsInstructions;
use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::build_available_skills;
use codex_core_skills::default_skill_metadata_budget;
use codex_core_skills::render::SkillCatalogMode;
use codex_core_skills::render::SkillRenderSideEffects;

const MAX_AVAILABLE_SKILLS_BYTES: usize = 8_000;
const MAX_MAIN_PROMPT_BYTES: usize = 8_000;
pub(crate) const MAX_SKILL_NAME_BYTES: usize = 256;
pub(crate) const MAX_SKILL_PATH_BYTES: usize = 1_024;

pub(crate) fn available_skills_fragment(
    catalog: &SkillCatalog,
    host_snapshot: Option<&HostSkillsSnapshot>,
    model_context_window: Option<i64>,
) -> Option<(AvailableSkillsInstructions, Option<String>)> {
    let visible_entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.prompt_visible)
        .collect::<Vec<_>>();
    if visible_entries
        .iter()
        .any(|entry| entry.authority.kind == SkillSourceKind::Host)
        && let Some(host_snapshot) = host_snapshot
    {
        let external_lines = bounded_skill_lines(
            visible_entries
                .iter()
                .copied()
                .filter(|entry| entry.authority.kind != SkillSourceKind::Host),
        );
        if let Some(available) = build_available_skills(
            host_snapshot.outcome(),
            default_skill_metadata_budget(model_context_window),
            SkillRenderSideEffects::None,
        ) {
            let warning = available.warning_message.clone();
            return Some((
                AvailableSkillsInstructions::from_available_skills_with_additional_lines(
                    available,
                    external_lines,
                ),
                warning,
            ));
        }
        if !external_lines.is_empty() {
            return Some((
                AvailableSkillsInstructions::from_skill_lines(
                    external_lines,
                    SkillCatalogMode::Mixed,
                ),
                None,
            ));
        }
        return None;
    }

    let mode = if visible_entries
        .iter()
        .any(|entry| entry.authority.kind != SkillSourceKind::Host)
    {
        SkillCatalogMode::Mixed
    } else {
        SkillCatalogMode::HostOnly
    };
    let skill_lines = bounded_skill_lines(visible_entries);
    (!skill_lines.is_empty()).then(|| {
        (
            AvailableSkillsInstructions::from_skill_lines(skill_lines, mode),
            None,
        )
    })
}

fn bounded_skill_lines<'a>(
    entries: impl IntoIterator<Item = &'a SkillCatalogEntry>,
) -> Vec<String> {
    let mut total_bytes = 0usize;
    let mut omitted = 0usize;
    let mut skill_lines = Vec::new();

    for entry in entries {
        let description = entry
            .short_description
            .as_deref()
            .unwrap_or(entry.description.as_str());
        let line = render_skill_line(entry, description);
        let next_bytes = total_bytes.saturating_add(line.len());
        if next_bytes > MAX_AVAILABLE_SKILLS_BYTES {
            omitted = omitted.saturating_add(1);
            continue;
        }
        total_bytes = next_bytes;
        skill_lines.push(line);
    }

    if omitted > 0 {
        let skill_word = if omitted == 1 { "skill" } else { "skills" };
        skill_lines.push(format!(
            "- {omitted} additional {skill_word} omitted from this bounded skills list."
        ));
    }
    skill_lines
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
