use codex_utils_string::take_bytes_at_char_boundary;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::fragments::AvailableSkillsInstructions;

const MAX_AVAILABLE_SKILLS_BYTES: usize = 8_000;
const MAX_MAIN_PROMPT_BYTES: usize = 8_000;
pub(crate) const MAX_SKILL_NAME_BYTES: usize = 256;
pub(crate) const MAX_SKILL_PATH_BYTES: usize = 1_024;

pub(crate) fn available_skills_fragment(
    catalog: &SkillCatalog,
) -> Option<AvailableSkillsInstructions> {
    let mut total_bytes = 0usize;
    let mut omitted = 0usize;
    let mut skill_lines = Vec::new();

    for entry in catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.prompt_visible)
    {
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

    if skill_lines.is_empty() {
        return None;
    }
    if omitted > 0 {
        let skill_word = if omitted == 1 { "skill" } else { "skills" };
        skill_lines.push(format!(
            "- {omitted} additional {skill_word} omitted from this bounded skills list."
        ));
    }

    Some(AvailableSkillsInstructions::from_skill_lines(skill_lines))
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
