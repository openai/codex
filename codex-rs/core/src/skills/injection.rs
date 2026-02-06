use std::collections::HashSet;

use crate::skills::SkillLoadOutcome;
use crate::skills::SkillMetadata;
use crate::user_instructions::SkillInstructions;
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
) -> SkillInjections {
    if inputs.is_empty() {
        return SkillInjections::default();
    }

    let Some(outcome) = skills else {
        return SkillInjections::default();
    };

    let mentioned_skills = collect_explicit_skill_mentions(inputs, &outcome.skills);
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
