use std::collections::HashSet;
use std::path::PathBuf;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::instructions::SkillInstructions;
use crate::skills::SkillLoadOutcome;
use crate::skills::SkillMetadata;
use crate::skills::handle_skill_dependencies;
use codex_otel::OtelManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use tokio::fs;

#[derive(Debug, Default)]
pub(crate) struct SkillInjections {
    pub(crate) items: Vec<ResponseItem>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) async fn build_skill_injections(
    session: &Session,
    turn: &TurnContext,
    inputs: &[UserInput],
    skills: Option<&SkillLoadOutcome>,
    otel: Option<&OtelManager>,
) -> SkillInjections {
    if inputs.is_empty() {
        return SkillInjections::default();
    }

    let Some(outcome) = skills else {
        return SkillInjections::default();
    };

    let mentioned_skills =
        collect_explicit_skill_mentions(inputs, &outcome.skills, &outcome.disabled_paths);
    if mentioned_skills.is_empty() {
        return SkillInjections::default();
    }

    let mut result = SkillInjections {
        items: Vec::with_capacity(mentioned_skills.len()),
        warnings: Vec::new(),
    };

    for skill in mentioned_skills {
        let call_id = {
            let skill_name = skill.name.as_str();
            format!("skill-{skill_name}-mcp-deps")
        };
        if let Some(message) = handle_skill_dependencies(session, turn, call_id, &skill).await {
            result.items.push(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: message }],
                end_turn: None,
            });
        }
        match fs::read_to_string(&skill.path).await {
            Ok(contents) => {
                emit_skill_injected_metric(otel, &skill, "ok");
                result.items.push(ResponseItem::from(SkillInstructions {
                    name: skill.name,
                    path: skill.path.to_string_lossy().into_owned(),
                    contents,
                }));
            }
            Err(err) => {
                emit_skill_injected_metric(otel, &skill, "error");
                let skill_name = skill.name.as_str();
                let skill_path = skill.path.display();
                let message = format!("Failed to load skill {skill_name} at {skill_path}: {err:#}");
                result.warnings.push(message);
            }
        }
    }

    result
}

fn emit_skill_injected_metric(otel: Option<&OtelManager>, skill: &SkillMetadata, status: &str) {
    let Some(otel) = otel else {
        return;
    };

    otel.counter(
        "codex.skill.injected",
        1,
        &[("status", status), ("skill", skill.name.as_str())],
    );
}

fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> Vec<SkillMetadata> {
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path } = input
            && seen.insert(name.clone())
            && let Some(skill) = skills.iter().find(|s| s.name == *name && s.path == *path)
            && !disabled_paths.contains(&skill.path)
        {
            selected.push(skill.clone());
        }

        if let UserInput::Text { text, .. } = input {
            for name in extract_inline_skill_mentions(text) {
                if let Some(skill) = skills.iter().find(|skill| skill.name == name)
                    && !disabled_paths.contains(&skill.path)
                    && seen.insert(name.clone())
                {
                    selected.push(skill.clone());
                }
            }
        }
    }

    selected
}

fn extract_inline_skill_mentions(text: &str) -> Vec<String> {
    const MAX_SKILL_NAME_LEN: usize = 99;
    let mut names = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'$' {
            let prev_is_word = index > 0 && is_word_boundary_char(bytes[index - 1]);
            if !prev_is_word {
                let name_start = index + 1;
                let mut name_end = name_start;
                while name_end < bytes.len() && is_skill_name_char(bytes[name_end]) {
                    name_end += 1;
                }
                let name_len = name_end.saturating_sub(name_start);
                if name_len > 0 && name_len <= MAX_SKILL_NAME_LEN {
                    let name = &text[name_start..name_end];
                    names.push(name.to_string());
                }
                index = name_end;
                continue;
            }
        }
        index += 1;
    }

    names
}

fn is_word_boundary_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_skill_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

#[cfg(test)]
mod tests {
    use super::extract_inline_skill_mentions;
    use pretty_assertions::assert_eq;

    #[test]
    fn extracts_inline_skill_mentions() {
        let text = "Use $notion-research-documentation to generate docs.";
        let names = extract_inline_skill_mentions(text);
        assert_eq!(names, vec!["notion-research-documentation".to_string()]);
    }

    #[test]
    fn extracts_mentions_inside_markdown_links() {
        let text = "This is [$not-a-link] and [$ok](path) for later.";
        let names = extract_inline_skill_mentions(text);
        assert_eq!(names, vec!["not-a-link".to_string(), "ok".to_string()]);
    }

    #[test]
    fn extracts_dash_names() {
        let text = "Use $spaced-name please.";
        let names = extract_inline_skill_mentions(text);
        assert_eq!(names, vec!["spaced-name".to_string()]);
    }

    #[test]
    fn ignores_names_in_the_middle_of_words() {
        let text = "ignore foo$bar but accept $good_name.";
        let names = extract_inline_skill_mentions(text);
        assert_eq!(names, vec!["good_name".to_string()]);
    }

    #[test]
    fn skips_overly_long_names() {
        let long_name = "a".repeat(100);
        let text = format!("Use ${long_name} for now.");
        let names = extract_inline_skill_mentions(&text);
        assert_eq!(names, Vec::<String>::new());
    }
}
