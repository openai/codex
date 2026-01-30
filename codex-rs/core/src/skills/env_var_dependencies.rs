use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::sync::Arc;

use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::skills::SkillMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillDependencyInfo {
    pub(crate) skill_name: String,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
}

/// Resolve required dependency values (session cache, then env vars),
/// and prompt the UI for any missing ones.
pub(crate) async fn resolve_skill_dependencies_for_turn(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    dependencies: &[SkillDependencyInfo],
) {
    if dependencies.is_empty() {
        return;
    }

    let existing_env = sess.dependency_env().await;
    let mut loaded_values = HashMap::new();
    let mut missing = Vec::new();
    let mut seen_names = HashSet::new();

    for dependency in dependencies {
        let name = dependency.name.clone();
        if !seen_names.insert(name.clone()) {
            continue;
        }
        if existing_env.contains_key(&name) {
            continue;
        }
        match env::var(&name) {
            Ok(value) => {
                loaded_values.insert(name.clone(), value);
                continue;
            }
            Err(env::VarError::NotPresent) => {}
            Err(err) => {
                warn!("failed to read env var {name}: {err}");
            }
        }
        missing.push(dependency.clone());
    }

    if !loaded_values.is_empty() {
        sess.set_dependency_env(loaded_values).await;
    }

    if !missing.is_empty() {
        request_skill_dependencies(sess, turn_context, &missing).await;
    }
}

pub(crate) fn collect_env_var_dependencies(
    mentioned_skills: &[SkillMetadata],
) -> Vec<SkillDependencyInfo> {
    let mut dependencies = Vec::new();
    for skill in mentioned_skills {
        let Some(skill_dependencies) = &skill.dependencies else {
            continue;
        };
        for tool in &skill_dependencies.tools {
            if tool.r#type != "env_var" {
                continue;
            }
            if tool.value.is_empty() {
                continue;
            }
            dependencies.push(SkillDependencyInfo {
                skill_name: skill.name.clone(),
                name: tool.value.clone(),
                description: tool.description.clone(),
            });
        }
    }
    dependencies
}

/// Prompt via request_user_input to gather missing env vars.
pub(crate) async fn request_skill_dependencies(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    dependencies: &[SkillDependencyInfo],
) {
    let questions = dependencies
        .iter()
        .map(|dep| {
            let requirement = dep.description.as_ref().map_or_else(
                || format!("The skill \"{}\" requires \"{}\" to be set.", dep.skill_name, dep.name),
                |description| {
                    format!(
                        "The skill \"{}\" requires \"{}\" to be set ({}).",
                        dep.skill_name, dep.name, description
                    )
                },
            );
            let question = format!(
                "{requirement} This is an experimental internal feature. The value is stored in memory for this session only.",
            );
            RequestUserInputQuestion {
                id: dep.name.clone(),
                header: "Skill requires environment variable".to_string(),
                question,
                is_other: false,
                is_secret: true,
                options: None,
            }
        })
        .collect::<Vec<_>>();

    if questions.is_empty() {
        return;
    }

    let args = RequestUserInputArgs { questions };
    let call_id = format!("skill-deps-{}", turn_context.sub_id);
    let response = sess
        .request_user_input(turn_context, call_id, args)
        .await
        .unwrap_or_else(|| RequestUserInputResponse {
            answers: HashMap::new(),
        });

    if response.answers.is_empty() {
        return;
    }

    let mut values = HashMap::new();
    for (name, answer) in response.answers {
        let mut user_note = None;
        for entry in &answer.answers {
            if let Some(note) = entry.strip_prefix("user_note: ")
                && !note.trim().is_empty()
            {
                user_note = Some(note.trim().to_string());
            }
        }
        if let Some(value) = user_note {
            values.insert(name, value);
        }
    }

    if values.is_empty() {
        return;
    }

    sess.set_dependency_env(values).await;
}
