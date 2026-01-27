use std::collections::HashSet;
use std::path::PathBuf;

use crate::instructions::SkillInstructions;
use crate::skills::SkillLoadOutcome;
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

fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> Vec<SkillMetadata> {
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path } = input
            && let Some(skill) = skills.iter().find(|s| s.name == *name && s.path == *path)
            && !disabled_paths.contains(&skill.path)
            && seen.insert(skill.path.clone())
        {
            selected.push(skill.clone());
        }
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillMetadata;
    use codex_protocol::protocol::SkillScope;
    use codex_protocol::user_input::UserInput;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn skill(name: &str, path: PathBuf) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: "desc".to_string(),
            short_description: None,
            interface: None,
            path,
            scope: SkillScope::Repo,
        }
    }

    #[test]
    fn valid_skill_not_blocked_by_disabled_skill_with_same_name() {
        let disabled_path = PathBuf::from("/skills/test/SKILL.md");
        let enabled_path = PathBuf::from("/skills/test-copy/SKILL.md");
        let skills = vec![
            skill("test", disabled_path.clone()),
            skill("test", enabled_path.clone()),
        ];
        let disabled_paths = HashSet::from([disabled_path.clone()]);
        let inputs = vec![
            UserInput::Skill {
                name: "test".to_string(),
                path: disabled_path,
            },
            UserInput::Skill {
                name: "test".to_string(),
                path: enabled_path.clone(),
            },
        ];

        let selected = collect_explicit_skill_mentions(&inputs, &skills, &disabled_paths);

        assert_eq!(vec![skill("test", enabled_path)], selected);
    }
}
