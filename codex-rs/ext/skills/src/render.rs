use codex_core_skills::render_available_skills_body;
use codex_extension_api::ContextualUserFragment;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

use crate::catalog::SkillCatalog;

const MAX_AVAILABLE_SKILLS_CHARS: usize = 8_000;
const MAX_MAIN_PROMPT_CHARS: usize = 40_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableSkillsFragment {
    body: String,
}

impl ContextualUserFragment for AvailableSkillsFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn body(&self) -> String {
        self.body.clone()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG)
    }
}

pub(crate) fn available_skills_fragment(catalog: &SkillCatalog) -> Option<AvailableSkillsFragment> {
    let mut total_chars = 0usize;
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
        let line = render_skill_line(entry.name.as_str(), description, entry.rendered_path());
        let next_chars = total_chars.saturating_add(line.chars().count());
        if next_chars > MAX_AVAILABLE_SKILLS_CHARS {
            omitted = omitted.saturating_add(1);
            continue;
        }
        total_chars = next_chars;
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

    Some(AvailableSkillsFragment {
        body: render_available_skills_body(&[], &skill_lines),
    })
}

fn render_skill_line(name: &str, description: &str, path: &str) -> String {
    if description.is_empty() {
        format!("- {name}: (file: {path})")
    } else {
        format!("- {name}: {description} (file: {path})")
    }
}

pub(crate) fn truncate_main_prompt_contents(contents: &str) -> (String, bool) {
    let mut chars = 0usize;
    for (index, _) in contents.char_indices() {
        if chars == MAX_MAIN_PROMPT_CHARS {
            return (contents[..index].to_string(), true);
        }
        chars = chars.saturating_add(1);
    }
    (contents.to_string(), false)
}
