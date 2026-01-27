use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::instructions::SkillInstructions;
use crate::skills::SkillMetadata;
use codex_otel::OtelManager;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use tokio::fs;

#[derive(Debug, Default)]
pub(crate) struct SkillInjections {
    pub(crate) items: Vec<ResponseItem>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) async fn build_skill_injections(
    mentioned_skills: &[SkillMetadata],
    otel: Option<&OtelManager>,
) -> SkillInjections {
    if mentioned_skills.is_empty() {
        return SkillInjections::default();
    }

    let mut result = SkillInjections {
        items: Vec::with_capacity(mentioned_skills.len()),
        warnings: Vec::new(),
    };

    for skill in mentioned_skills {
        match fs::read_to_string(&skill.path).await {
            Ok(contents) => {
                emit_skill_injected_metric(otel, skill, "ok");
                result.items.push(ResponseItem::from(SkillInstructions {
                    name: skill.name.clone(),
                    path: skill.path.to_string_lossy().into_owned(),
                    contents,
                }));
            }
            Err(err) => {
                emit_skill_injected_metric(otel, skill, "error");
                let message = format!(
                    "Failed to load skill {name} at {path}: {err:#}",
                    name = skill.name,
                    path = skill.path.display()
                );
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

/// Collect explicitly mentioned skills from structured inputs and `$name` text mentions.
///
/// Text inputs are scanned once to extract `$skill-name` tokens, then we iterate `skills`
/// in their existing order to preserve prior ordering semantics.
///
/// Complexity: `O(S + T + N_t * S)` time, `O(S)` space, where:
/// `S` = number of skills, `T` = total text length, `N_t` = number of text inputs.
pub(crate) fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> Vec<SkillMetadata> {
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Index skills for explicit (name, path) lookups; text mentions are parsed once per input.
    let mut by_name_and_path: HashMap<(&str, &Path), &SkillMetadata> = HashMap::new();
    for skill in skills {
        by_name_and_path.insert((skill.name.as_str(), skill.path.as_path()), skill);
    }

    for input in inputs {
        match input {
            UserInput::Skill { name, path } => {
                if seen.insert(name.clone())
                    && !disabled_paths.contains(path)
                    && let Some(skill) = by_name_and_path.get(&(name.as_str(), path.as_path()))
                {
                    selected.push((*skill).clone());
                }
            }
            UserInput::Text { text, .. } => {
                let mentioned_names = extract_skill_mentions(text);
                select_skills_from_mentions(
                    skills,
                    disabled_paths,
                    &mentioned_names,
                    &mut seen,
                    &mut selected,
                );
            }
            _ => {}
        }
    }

    selected
}

/// Extract `$skill-name` mentions from a single text input.
///
/// This is a single pass over the bytes; a token ends at the first non-name character.
fn extract_skill_mentions(text: &str) -> HashSet<&str> {
    let text_bytes = text.as_bytes();
    let mut mentioned_names: HashSet<&str> = HashSet::new();

    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte != b'$' {
            continue;
        }

        let name_start = index + 1;
        let Some(first_name_byte) = text_bytes.get(name_start) else {
            continue;
        };
        if !is_skill_name_char(*first_name_byte) {
            continue;
        }

        let mut name_end = name_start + 1;
        while let Some(next_byte) = text_bytes.get(name_end)
            && is_skill_name_char(*next_byte)
        {
            name_end += 1;
        }

        let name = &text[name_start..name_end];
        mentioned_names.insert(name);
    }

    mentioned_names
}

/// Select mentioned skills while preserving the order of `skills`.
fn select_skills_from_mentions(
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
    mentioned_names: &HashSet<&str>,
    seen: &mut HashSet<String>,
    selected: &mut Vec<SkillMetadata>,
) {
    for skill in skills {
        if disabled_paths.contains(&skill.path) {
            continue;
        }

        if mentioned_names.contains(skill.name.as_str()) && seen.insert(skill.name.clone()) {
            selected.push(skill.clone());
        }
    }
}

#[cfg(test)]
fn text_mentions_skill(text: &str, skill_name: &str) -> bool {
    if skill_name.is_empty() {
        return false;
    }

    let text_bytes = text.as_bytes();
    let skill_bytes = skill_name.as_bytes();

    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte != b'$' {
            continue;
        }

        let name_start = index + 1;
        let Some(rest) = text_bytes.get(name_start..) else {
            continue;
        };
        if !rest.starts_with(skill_bytes) {
            continue;
        }

        let after_index = name_start + skill_bytes.len();
        let after = text_bytes.get(after_index).copied();
        if after.is_none_or(|b| !is_skill_name_char(b)) {
            return true;
        }
    }

    false
}

fn is_skill_name_char(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_skill(name: &str, path: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: format!("{name} skill"),
            short_description: None,
            interface: None,
            dependencies: None,
            path: PathBuf::from(path),
            scope: codex_protocol::protocol::SkillScope::User,
        }
    }

    #[test]
    fn text_mentions_skill_requires_exact_boundary() {
        assert_eq!(
            true,
            text_mentions_skill("use $notion-research-doc please", "notion-research-doc")
        );
        assert_eq!(
            true,
            text_mentions_skill("($notion-research-doc)", "notion-research-doc")
        );
        assert_eq!(
            true,
            text_mentions_skill("$notion-research-doc.", "notion-research-doc")
        );
        assert_eq!(
            false,
            text_mentions_skill("$notion-research-docs", "notion-research-doc")
        );
        assert_eq!(
            false,
            text_mentions_skill("$notion-research-doc_extra", "notion-research-doc")
        );
    }

    #[test]
    fn text_mentions_skill_handles_end_boundary_and_near_misses() {
        assert_eq!(true, text_mentions_skill("$alpha-skill", "alpha-skill"));
        assert_eq!(false, text_mentions_skill("$alpha-skillx", "alpha-skill"));
        assert_eq!(
            true,
            text_mentions_skill("$alpha-skillx and later $alpha-skill ", "alpha-skill")
        );
    }

    #[test]
    fn text_mentions_skill_handles_many_dollars_without_looping() {
        let prefix = "$".repeat(256);
        let text = format!("{prefix} not-a-mention");
        assert_eq!(false, text_mentions_skill(&text, "alpha-skill"));
    }

    #[test]
    fn collect_explicit_skill_mentions_text_respects_skill_order() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let beta = make_skill("beta-skill", "/tmp/beta");
        let skills = vec![beta.clone(), alpha.clone()];
        let inputs = vec![UserInput::Text {
            text: "first $alpha-skill then $beta-skill".to_string(),
            text_elements: Vec::new(),
        }];

        let selected = collect_explicit_skill_mentions(&inputs, &skills, &HashSet::new());

        // Text scanning should not change the previous selection ordering semantics.
        assert_eq!(selected, vec![beta, alpha]);
    }

    #[test]
    fn collect_explicit_skill_mentions_includes_prompt_references() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let beta = make_skill("beta-skill", "/tmp/beta");
        let skills = vec![alpha.clone(), beta.clone()];
        let inputs = vec![
            UserInput::Text {
                text: "please run $alpha-skill".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: "beta-skill".to_string(),
                path: PathBuf::from("/tmp/beta"),
            },
        ];

        let selected = collect_explicit_skill_mentions(&inputs, &skills, &HashSet::new());

        assert_eq!(selected, vec![alpha, beta]);
    }
}
