use std::collections::HashSet;

use crate::skills::SkillLoadOutcome;
use crate::skills::SkillMetadata;
use crate::user_instructions::SkillInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use tokio::fs;

const CODEX_CLI_DOCS_SKILL_NAME: &str = "codex-cli-docs";

#[derive(Debug, Default)]
pub(crate) struct SkillInjections {
    pub(crate) items: Vec<ResponseItem>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) async fn build_skill_injections(
    inputs: &[UserInput],
    skills: Option<&SkillLoadOutcome>,
) -> SkillInjections {
    if inputs.is_empty() {
        return SkillInjections::default();
    }

    let Some(outcome) = skills else {
        return SkillInjections::default();
    };

    let mut mentioned_skills = collect_explicit_skill_mentions(inputs, &outcome.skills);
    extend_with_implicit_skills(inputs, &outcome.skills, &mut mentioned_skills);
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
                result.items.push(ResponseItem::from(SkillInstructions {
                    name: skill.name,
                    path: skill.path.to_string_lossy().into_owned(),
                    contents,
                }));
            }
            Err(err) => {
                let message = format!(
                    "Failed to load skill {} at {}: {err:#}",
                    skill.name,
                    skill.path.display()
                );
                result.warnings.push(message);
            }
        }
    }

    result
}

fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
) -> Vec<SkillMetadata> {
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path } = input
            && seen.insert(name.clone())
            && let Some(skill) = skills.iter().find(|s| s.name == *name && s.path == *path)
        {
            selected.push(skill.clone());
        }
    }

    selected
}

fn extend_with_implicit_skills(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    selected: &mut Vec<SkillMetadata>,
) {
    if selected
        .iter()
        .any(|skill| skill.name == CODEX_CLI_DOCS_SKILL_NAME)
    {
        return;
    }

    let codex_cli_query = inputs.iter().any(|input| match input {
        UserInput::Text { text } => is_codex_cli_question(text),
        _ => false,
    });
    if !codex_cli_query {
        return;
    }

    if let Some(skill) = skills
        .iter()
        .find(|skill| skill.name == CODEX_CLI_DOCS_SKILL_NAME)
    {
        selected.push(skill.clone());
    }
}

fn is_codex_cli_question(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if !lowered.contains("codex") {
        return false;
    }

    // Prefer high-signal phrases to avoid injecting when "codex" refers to a code symbol.
    const TRIGGERS: &[&str] = &[
        "codex cli",
        "codex-cli",
        "install",
        "brew",
        "npm",
        "login",
        "sign in",
        "auth",
        "api key",
        "approval-mode",
        "approval mode",
        "approval_policy",
        "sandbox",
        "sandbox_mode",
        "mcp",
        "config.toml",
        ".codex/",
        "~/.codex",
        "slash command",
        "slash_commands",
        "telemetry",
        "openai_api_key",
    ];

    TRIGGERS.iter().any(|trigger| lowered.contains(trigger))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn fake_skill(name: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: "desc".to_string(),
            short_description: None,
            path: std::path::PathBuf::from("/tmp/skill/SKILL.md"),
            scope: codex_protocol::protocol::SkillScope::System,
        }
    }

    #[test]
    fn codex_cli_question_requires_codex_and_trigger() {
        assert!(is_codex_cli_question(
            "How do I configure Codex CLI sandbox?"
        ));
        assert!(is_codex_cli_question("codex-cli approval-mode full-auto"));
        assert!(!is_codex_cli_question("this codebase has a Codex module"));
        assert!(!is_codex_cli_question("what does config.toml mean?"));
    }

    #[test]
    fn injects_codex_cli_docs_skill_when_available_and_query_matches() {
        let inputs = [UserInput::Text {
            text: "Does Codex CLI support MCP?".to_string(),
        }];
        let mut selected = Vec::new();
        extend_with_implicit_skills(
            &inputs,
            &[fake_skill(CODEX_CLI_DOCS_SKILL_NAME)],
            &mut selected,
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, CODEX_CLI_DOCS_SKILL_NAME);
    }

    #[test]
    fn does_not_inject_when_skill_missing() {
        let inputs = [UserInput::Text {
            text: "Does Codex CLI support MCP?".to_string(),
        }];
        let mut selected = Vec::new();
        extend_with_implicit_skills(&inputs, &[fake_skill("other")], &mut selected);
        assert!(selected.is_empty());
    }

    #[test]
    fn does_not_double_inject() {
        let inputs = [UserInput::Text {
            text: "Does Codex CLI support MCP?".to_string(),
        }];
        let mut selected = vec![fake_skill(CODEX_CLI_DOCS_SKILL_NAME)];
        extend_with_implicit_skills(
            &inputs,
            &[fake_skill(CODEX_CLI_DOCS_SKILL_NAME)],
            &mut selected,
        );
        assert_eq!(selected.len(), 1);
    }
}
