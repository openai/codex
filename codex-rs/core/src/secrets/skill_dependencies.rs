use std::collections::BTreeMap;
use std::collections::HashMap;

use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::secrets::SecretName;
use crate::secrets::SecretScope;
use crate::secrets::SecretsManager;
use crate::secrets::environment_id_from_cwd;
use crate::skills::SkillMetadata;
use crate::skills::model::SkillToolDependency;

const SKILL_SECRET_PROMPT_PREFIX: &str = "skill-secret";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSecretsOutcome {
    pub overrides: HashMap<String, String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct SecretDependencyContext {
    name: SecretName,
    skills: Vec<String>,
    description: Option<String>,
}

#[derive(Debug, Clone)]
struct MissingSecret {
    context: SecretDependencyContext,
    canonical_key: String,
}

pub(crate) async fn resolve_skill_env_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    cancellation_token: &CancellationToken,
    mentioned_skills: &[SkillMetadata],
) -> SkillSecretsOutcome {
    if mentioned_skills.is_empty() {
        return SkillSecretsOutcome::default();
    }

    let config = turn_context.client.config();
    let manager = SecretsManager::new(config.codex_home.clone(), config.secrets_backend);

    let env_scope = match SecretScope::environment(environment_id_from_cwd(&turn_context.cwd)) {
        Ok(scope) => scope,
        Err(err) => {
            return SkillSecretsOutcome {
                overrides: HashMap::new(),
                warnings: vec![format!("failed to resolve environment scope: {err}")],
            };
        }
    };
    let global_scope = SecretScope::Global;

    let mut outcome = SkillSecretsOutcome::default();
    let dependencies = collect_env_var_dependencies(mentioned_skills, &mut outcome.warnings);
    if dependencies.is_empty() {
        return outcome;
    }

    let mut missing = Vec::new();

    for context in dependencies {
        match resolve_secret_value(
            &manager,
            &env_scope,
            &global_scope,
            &context,
            &mut outcome.warnings,
        ) {
            Some(value) => {
                outcome
                    .overrides
                    .insert(context.name.as_str().to_string(), value);
            }
            None => {
                missing.push(MissingSecret {
                    canonical_key: env_scope.canonical_key(&context.name),
                    context,
                });
            }
        }
    }

    if missing.is_empty() {
        return outcome;
    }

    let prompted = sess.skill_secret_prompted().await;
    let mut already_prompted = Vec::new();
    let mut unprompted_missing = Vec::new();
    for entry in missing {
        if prompted.contains(&entry.canonical_key) {
            already_prompted.push(entry);
        } else {
            unprompted_missing.push(entry);
        }
    }

    for entry in already_prompted {
        outcome
            .warnings
            .push(missing_secret_warning(&entry.context));
    }

    if unprompted_missing.is_empty() {
        return outcome;
    }

    let responses = prompt_for_missing_secrets(
        sess,
        turn_context,
        cancellation_token,
        &env_scope,
        &unprompted_missing,
    )
    .await;

    let prompted_keys = unprompted_missing
        .iter()
        .map(|entry| entry.canonical_key.clone());
    sess.record_skill_secret_prompted(prompted_keys).await;

    let mut resolved_from_prompt = HashMap::new();
    for entry in &unprompted_missing {
        let Some(answer) = responses.answers.get(&entry.canonical_key) else {
            outcome
                .warnings
                .push(missing_secret_warning(&entry.context));
            continue;
        };
        let Some(value) = first_non_empty_answer(answer) else {
            outcome
                .warnings
                .push(missing_secret_warning(&entry.context));
            continue;
        };

        if let Err(err) = manager.set(&env_scope, &entry.context.name, &value) {
            outcome.warnings.push(format!(
                "failed to persist secret {}: {err}",
                entry.context.name
            ));
        }

        resolved_from_prompt.insert(entry.context.name.as_str().to_string(), value);
    }

    outcome.overrides.extend(resolved_from_prompt);
    outcome
}

fn collect_env_var_dependencies(
    mentioned_skills: &[SkillMetadata],
    warnings: &mut Vec<String>,
) -> Vec<SecretDependencyContext> {
    let mut contexts: BTreeMap<String, SecretDependencyContext> = BTreeMap::new();

    for skill in mentioned_skills {
        let Some(dependencies) = skill.dependencies.as_ref() else {
            continue;
        };

        for tool in &dependencies.tools {
            if !tool.r#type.eq_ignore_ascii_case("env_var") {
                continue;
            }
            add_dependency_context(&mut contexts, skill, tool, warnings);
        }
    }

    contexts
        .into_values()
        .map(|mut context| {
            context.skills.sort();
            context.skills.dedup();
            context
        })
        .collect()
}

fn add_dependency_context(
    contexts: &mut BTreeMap<String, SecretDependencyContext>,
    skill: &SkillMetadata,
    tool: &SkillToolDependency,
    warnings: &mut Vec<String>,
) {
    let name = match SecretName::new(&tool.value) {
        Ok(name) => name,
        Err(err) => {
            warnings.push(format!(
                "skill {} declares invalid env_var dependency {}: {err}",
                skill.name, tool.value
            ));
            return;
        }
    };

    let key = name.as_str().to_string();
    let description = tool.description.clone();
    contexts
        .entry(key)
        .and_modify(|context| {
            context.skills.push(skill.name.clone());
            if context.description.is_none() {
                context.description = description.clone();
            }
        })
        .or_insert_with(|| SecretDependencyContext {
            name,
            skills: vec![skill.name.clone()],
            description,
        });
}

fn resolve_secret_value(
    manager: &SecretsManager,
    env_scope: &SecretScope,
    global_scope: &SecretScope,
    context: &SecretDependencyContext,
    warnings: &mut Vec<String>,
) -> Option<String> {
    if let Some(env_value) = read_non_empty_env(context.name.as_str()) {
        return Some(env_value);
    }

    match manager.get(env_scope, &context.name) {
        Ok(Some(value)) if !value.trim().is_empty() => return Some(value),
        Ok(Some(_)) | Ok(None) => {}
        Err(err) => warnings.push(format!(
            "failed to read secret {} from env scope: {err}",
            context.name
        )),
    }

    match manager.get(global_scope, &context.name) {
        Ok(Some(value)) if !value.trim().is_empty() => Some(value),
        Ok(Some(_)) | Ok(None) => None,
        Err(err) => {
            warnings.push(format!(
                "failed to read secret {} from global scope: {err}",
                context.name
            ));
            None
        }
    }
}

fn read_non_empty_env(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        Ok(_) => None,
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            warn!("environment variable {name} contains invalid unicode; treating as missing");
            None
        }
    }
}

async fn prompt_for_missing_secrets(
    sess: &Session,
    turn_context: &TurnContext,
    cancellation_token: &CancellationToken,
    env_scope: &SecretScope,
    missing: &[MissingSecret],
) -> RequestUserInputResponse {
    let questions = missing
        .iter()
        .map(|entry| build_secret_question(env_scope, entry))
        .collect();
    let args = RequestUserInputArgs { questions };
    let call_id = format!("{SKILL_SECRET_PROMPT_PREFIX}-{}", turn_context.sub_id);
    let response_fut = sess.request_user_input(turn_context, call_id, args);
    let sub_id = turn_context.sub_id.as_str();

    tokio::select! {
        biased;
        _ = cancellation_token.cancelled() => {
            let empty = RequestUserInputResponse { answers: HashMap::new() };
            sess.notify_user_input_response(sub_id, empty.clone()).await;
            empty
        }
        response = response_fut => response.unwrap_or_else(|| RequestUserInputResponse { answers: HashMap::new() }),
    }
}

fn build_secret_question(
    env_scope: &SecretScope,
    missing: &MissingSecret,
) -> RequestUserInputQuestion {
    let scope_label = match env_scope {
        SecretScope::Global => "global".to_string(),
        SecretScope::Environment(env_id) => format!("env/{env_id}"),
    };
    let skills = missing.context.skills.join(", ");
    let description = missing
        .context
        .description
        .as_deref()
        .map(|text| format!(" {text}"))
        .unwrap_or_default();
    RequestUserInputQuestion {
        id: missing.canonical_key.clone(),
        header: format!("Missing secret: {}", missing.context.name),
        question: format!(
            "Skill(s) {skills} require {}.{description} Enter a value to store for {scope_label}.",
            missing.context.name,
        ),
        is_other: false,
        options: None,
    }
}

fn first_non_empty_answer(answer: &RequestUserInputAnswer) -> Option<String> {
    answer
        .answers
        .iter()
        .find(|value| !value.trim().is_empty())
        .map(|value| value.to_string())
}

fn missing_secret_warning(context: &SecretDependencyContext) -> String {
    let skills = context.skills.join(", ");
    format!(
        "Required secret {} is missing for skill(s) {skills}. Use `codex secrets set {}` to configure it.",
        context.name, context.name
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn skill_with_env(name: &str, env_var: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: name.to_string(),
            short_description: None,
            interface: None,
            dependencies: Some(crate::skills::model::SkillDependencies {
                tools: vec![crate::skills::model::SkillToolDependency {
                    r#type: "env_var".to_string(),
                    value: env_var.to_string(),
                    description: Some("token".to_string()),
                    transport: None,
                    command: None,
                    url: None,
                }],
            }),
            path: PathBuf::from(name),
            scope: codex_protocol::protocol::SkillScope::User,
        }
    }

    #[tokio::test]
    async fn resolves_from_environment_without_prompting() -> anyhow::Result<()> {
        let (session, turn) = crate::codex::make_session_and_context().await;
        let home = std::env::var("HOME").context("HOME must be set for tests")?;
        let skills = vec![skill_with_env("skill", "HOME")];
        let outcome =
            resolve_skill_env_dependencies(&session, &turn, &CancellationToken::new(), &skills)
                .await;

        assert_eq!(outcome.warnings, Vec::<String>::new());
        assert_eq!(outcome.overrides.get("HOME"), Some(&home));
        Ok(())
    }
}
