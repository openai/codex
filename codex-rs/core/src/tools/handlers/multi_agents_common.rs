use crate::agent::AgentStatus;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::CodexErr;
use crate::function_tool::FunctionCallError;
use crate::models_manager::manager::RefreshStrategy;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::CollabAgentRef;
use codex_protocol::protocol::CollabAgentSpawnBeginEvent;
use codex_protocol::protocol::CollabAgentSpawnEndEvent;
use codex_protocol::protocol::CollabAgentStatusEntry;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout;

/// Minimum wait timeout to prevent tight polling loops from burning CPU.
pub(crate) const MIN_WAIT_TIMEOUT_MS: i64 = 10_000;
pub(crate) const DEFAULT_WAIT_TIMEOUT_MS: i64 = 30_000;
pub(crate) const MAX_WAIT_TIMEOUT_MS: i64 = 3600 * 1000;
const ASYNC_QUOTA_EXHAUSTION_STATUS_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) enum SpawnAttemptRetryDecision {
    Accept(AgentStatus),
    Retry(AgentStatus),
}

pub(crate) fn spawn_attempt_event_call_id(call_id: &str, attempt_index: usize) -> String {
    if attempt_index == 0 {
        call_id.to_string()
    } else {
        format!("{call_id}#{}", attempt_index + 1)
    }
}

pub(crate) fn function_arguments(payload: ToolPayload) -> Result<String, FunctionCallError> {
    match payload {
        ToolPayload::Function { arguments } => Ok(arguments),
        _ => Err(FunctionCallError::RespondToModel(
            "collab handler received unsupported payload".to_string(),
        )),
    }
}

pub(crate) fn tool_output_json_text<T>(value: &T, tool_name: &str) -> String
where
    T: Serialize,
{
    serde_json::to_string(value).unwrap_or_else(|err| {
        JsonValue::String(format!("failed to serialize {tool_name} result: {err}")).to_string()
    })
}

pub(crate) fn tool_output_response_item<T>(
    call_id: &str,
    payload: &ToolPayload,
    value: &T,
    success: Option<bool>,
    tool_name: &str,
) -> ResponseInputItem
where
    T: Serialize,
{
    FunctionToolOutput::from_text(tool_output_json_text(value, tool_name), success)
        .to_response_item(call_id, payload)
}

pub(crate) fn tool_output_code_mode_result<T>(value: &T, tool_name: &str) -> JsonValue
where
    T: Serialize,
{
    serde_json::to_value(value).unwrap_or_else(|err| {
        JsonValue::String(format!("failed to serialize {tool_name} result: {err}"))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnAgentModelCandidate {
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SpawnAgentModelFallbackCandidate {
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
}

pub(crate) fn collect_spawn_agent_model_candidates(
    model_fallback_list: Option<&Vec<SpawnAgentModelFallbackCandidate>>,
    requested_model: Option<&str>,
    requested_reasoning_effort: Option<ReasoningEffort>,
) -> Vec<SpawnAgentModelCandidate> {
    if let Some(model_fallback_list) = model_fallback_list {
        return model_fallback_list
            .iter()
            .map(|candidate| SpawnAgentModelCandidate {
                model: Some(candidate.model.clone()),
                reasoning_effort: candidate.reasoning_effort.or(requested_reasoning_effort),
            })
            .collect();
    }

    let mut candidates = Vec::new();
    if requested_model.is_some() || requested_reasoning_effort.is_some() {
        candidates.push(SpawnAgentModelCandidate {
            model: requested_model.map(ToString::to_string),
            reasoning_effort: requested_reasoning_effort,
        });
    }
    candidates
}

pub(crate) async fn close_quota_exhausted_spawn_attempt(
    agent_control: &crate::agent::control::AgentControl,
    thread_id: ThreadId,
    retry_status: AgentStatus,
) -> SpawnAttemptRetryDecision {
    let retry_decision =
        recheck_spawn_attempt_retry_decision(retry_status, thread_id, agent_control).await;
    let SpawnAttemptRetryDecision::Retry(status) = retry_decision else {
        return retry_decision;
    };

    // There is still a narrow TOCTOU window: a child can leave `PendingInit` after the final
    // status read above and before `close_agent` runs. `AgentControl` does not currently expose
    // a compare-and-close primitive, so this is the strongest local mitigation available.
    if let Err(err) = agent_control.close_agent(thread_id).await
        && !matches!(
            err,
            CodexErr::ThreadNotFound(_) | CodexErr::InternalAgentDied
        )
    {
        tracing::warn!("failed to close quota-exhausted spawn attempt {thread_id}: {err}");
    }
    SpawnAttemptRetryDecision::Retry(status)
}
pub(crate) fn spawn_should_retry_on_quota_exhaustion(error: &CodexErr) -> bool {
    matches!(
        error,
        CodexErr::QuotaExceeded | CodexErr::UsageLimitReached(_)
    )
}

pub(crate) async fn probe_spawn_attempt_for_async_quota_exhaustion(
    thread_status: AgentStatus,
    thread_id: ThreadId,
    agent_control: &crate::agent::control::AgentControl,
) -> SpawnAttemptRetryDecision {
    match thread_status {
        AgentStatus::Completed(_)
        | AgentStatus::Errored(_)
        | AgentStatus::Shutdown
        | AgentStatus::NotFound => {
            return retry_decision_for_final_spawn_status(thread_status);
        }
        AgentStatus::PendingInit | AgentStatus::Running | AgentStatus::Interrupted => {}
    }

    let Ok(mut status_rx) = agent_control.subscribe_status(thread_id).await else {
        return match thread_status {
            AgentStatus::Running | AgentStatus::Interrupted => {
                SpawnAttemptRetryDecision::Accept(thread_status)
            }
            _ => SpawnAttemptRetryDecision::Retry(AgentStatus::PendingInit),
        };
    };
    let deadline = Instant::now() + ASYNC_QUOTA_EXHAUSTION_STATUS_TIMEOUT;

    loop {
        let status = status_rx.borrow_and_update().clone();
        match status {
            AgentStatus::Completed(_)
            | AgentStatus::Errored(_)
            | AgentStatus::Shutdown
            | AgentStatus::NotFound => {
                return retry_decision_for_final_spawn_status(status);
            }
            AgentStatus::PendingInit | AgentStatus::Running | AgentStatus::Interrupted => {}
        }

        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return match status {
                AgentStatus::PendingInit => {
                    SpawnAttemptRetryDecision::Retry(AgentStatus::PendingInit)
                }
                AgentStatus::Running | AgentStatus::Interrupted => {
                    SpawnAttemptRetryDecision::Accept(status)
                }
                AgentStatus::Completed(_)
                | AgentStatus::Errored(_)
                | AgentStatus::Shutdown
                | AgentStatus::NotFound => retry_decision_for_final_spawn_status(status),
            };
        };
        match timeout(remaining, status_rx.changed()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return SpawnAttemptRetryDecision::Retry(AgentStatus::PendingInit),
            Err(_) => return SpawnAttemptRetryDecision::Retry(AgentStatus::PendingInit),
        }
    }
}

pub(crate) async fn recheck_spawn_attempt_retry_decision(
    status: AgentStatus,
    thread_id: ThreadId,
    agent_control: &crate::agent::control::AgentControl,
) -> SpawnAttemptRetryDecision {
    if !matches!(status, AgentStatus::PendingInit) {
        return SpawnAttemptRetryDecision::Retry(status);
    }

    let latest_status = agent_control.get_status(thread_id).await;
    match latest_status {
        AgentStatus::Running | AgentStatus::Interrupted => {
            SpawnAttemptRetryDecision::Accept(latest_status)
        }
        AgentStatus::Completed(_)
        | AgentStatus::Errored(_)
        | AgentStatus::Shutdown
        | AgentStatus::NotFound => retry_decision_for_final_spawn_status(latest_status),
        AgentStatus::PendingInit => SpawnAttemptRetryDecision::Retry(AgentStatus::PendingInit),
    }
}

fn retry_decision_for_final_spawn_status(status: AgentStatus) -> SpawnAttemptRetryDecision {
    if spawn_should_retry_on_quota_exhaustion_status(&status) {
        SpawnAttemptRetryDecision::Retry(status)
    } else {
        SpawnAttemptRetryDecision::Accept(status)
    }
}

fn spawn_should_retry_on_quota_exhaustion_status(status: &AgentStatus) -> bool {
    match status {
        AgentStatus::Errored(message) => {
            let message = message.to_lowercase();
            message.contains("insufficient_quota")
                || message.contains("usage limit")
                || message.contains("quota")
        }
        AgentStatus::NotFound => false,
        _ => false,
    }
}

pub(crate) fn build_wait_agent_statuses(
    statuses: &HashMap<ThreadId, AgentStatus>,
    receiver_agents: &[CollabAgentRef],
) -> Vec<CollabAgentStatusEntry> {
    if statuses.is_empty() {
        return Vec::new();
    }

    let mut entries = Vec::with_capacity(statuses.len());
    let mut seen = HashMap::with_capacity(receiver_agents.len());
    for receiver_agent in receiver_agents {
        seen.insert(receiver_agent.thread_id, ());
        if let Some(status) = statuses.get(&receiver_agent.thread_id) {
            entries.push(CollabAgentStatusEntry {
                thread_id: receiver_agent.thread_id,
                agent_nickname: receiver_agent.agent_nickname.clone(),
                agent_role: receiver_agent.agent_role.clone(),
                status: status.clone(),
            });
        }
    }

    let mut extras = statuses
        .iter()
        .filter(|(thread_id, _)| !seen.contains_key(thread_id))
        .map(|(thread_id, status)| CollabAgentStatusEntry {
            thread_id: *thread_id,
            agent_nickname: None,
            agent_role: None,
            status: status.clone(),
        })
        .collect::<Vec<_>>();
    extras.sort_by(|left, right| left.thread_id.to_string().cmp(&right.thread_id.to_string()));
    entries.extend(extras);
    entries
}

pub(crate) fn collab_spawn_error(err: CodexErr) -> FunctionCallError {
    match err {
        CodexErr::UnsupportedOperation(message) if message == "thread manager dropped" => {
            FunctionCallError::RespondToModel("collab manager unavailable".to_string())
        }
        CodexErr::UnsupportedOperation(message) => FunctionCallError::RespondToModel(message),
        err => FunctionCallError::RespondToModel(format!("collab spawn failed: {err}")),
    }
}

pub(crate) async fn send_collab_agent_spawn_error_event(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    prompt: String,
    model: String,
    reasoning_effort: ReasoningEffort,
    err: &CodexErr,
) {
    session
        .send_event(
            turn,
            CollabAgentSpawnEndEvent {
                call_id,
                sender_thread_id: session.conversation_id,
                new_thread_id: None,
                new_agent_nickname: None,
                new_agent_role: None,
                prompt,
                model,
                reasoning_effort,
                status: match err {
                    CodexErr::ThreadNotFound(_) => AgentStatus::NotFound,
                    err => AgentStatus::Errored(err.to_string()),
                },
            }
            .into(),
        )
        .await;
}

pub(crate) async fn send_collab_agent_spawn_begin_event(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    prompt: String,
    model: String,
    reasoning_effort: ReasoningEffort,
) {
    session
        .send_event(
            turn,
            CollabAgentSpawnBeginEvent {
                call_id,
                sender_thread_id: session.conversation_id,
                prompt,
                model,
                reasoning_effort,
            }
            .into(),
        )
        .await;
}

pub(crate) async fn send_collab_agent_spawn_retry_preempted_event(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    prompt: String,
    model: String,
    reasoning_effort: ReasoningEffort,
    status: AgentStatus,
) {
    session
        .send_event(
            turn,
            CollabAgentSpawnEndEvent {
                call_id,
                sender_thread_id: session.conversation_id,
                new_thread_id: None,
                new_agent_nickname: None,
                new_agent_role: None,
                prompt,
                model,
                reasoning_effort,
                status,
            }
            .into(),
        )
        .await;
}

pub(crate) fn collab_agent_error(agent_id: ThreadId, err: CodexErr) -> FunctionCallError {
    match err {
        CodexErr::ThreadNotFound(id) => {
            FunctionCallError::RespondToModel(format!("agent with id {id} not found"))
        }
        CodexErr::InternalAgentDied => {
            FunctionCallError::RespondToModel(format!("agent with id {agent_id} is closed"))
        }
        CodexErr::UnsupportedOperation(_) => {
            FunctionCallError::RespondToModel("collab manager unavailable".to_string())
        }
        err => FunctionCallError::RespondToModel(format!("collab tool failed: {err}")),
    }
}

pub(crate) fn thread_spawn_source(
    parent_thread_id: ThreadId,
    parent_session_source: &SessionSource,
    depth: i32,
    agent_role: Option<&str>,
    task_name: Option<String>,
) -> Result<SessionSource, FunctionCallError> {
    let agent_path = task_name
        .as_deref()
        .map(|task_name| {
            parent_session_source
                .get_agent_path()
                .unwrap_or_else(AgentPath::root)
                .join(task_name)
                .map_err(FunctionCallError::RespondToModel)
        })
        .transpose()?;
    Ok(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id,
        depth,
        agent_path,
        agent_nickname: None,
        agent_role: agent_role.map(str::to_string),
    }))
}

pub(crate) fn parse_collab_input(
    message: Option<String>,
    items: Option<Vec<UserInput>>,
) -> Result<Op, FunctionCallError> {
    match (message, items) {
        (Some(_), Some(_)) => Err(FunctionCallError::RespondToModel(
            "Provide either message or items, but not both".to_string(),
        )),
        (None, None) => Err(FunctionCallError::RespondToModel(
            "Provide one of: message or items".to_string(),
        )),
        (Some(message), None) => {
            if message.trim().is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "Empty message can't be sent to an agent".to_string(),
                ));
            }
            Ok(vec![UserInput::Text {
                text: message,
                text_elements: Vec::new(),
            }]
            .into())
        }
        (None, Some(items)) => {
            if items.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "Items can't be empty".to_string(),
                ));
            }
            Ok(items.into())
        }
    }
}

/// Builds the base config snapshot for a newly spawned sub-agent.
///
/// The returned config starts from the parent's effective config and then refreshes the
/// runtime-owned fields carried on `turn`, including model selection, reasoning settings,
/// approval policy, sandbox, and cwd. Role-specific overrides are layered after this step;
/// skipping this helper and cloning stale config state directly can send the child agent out with
/// the wrong provider or runtime policy.
pub(crate) fn build_agent_spawn_config(
    base_instructions: &BaseInstructions,
    turn: &TurnContext,
) -> Result<Config, FunctionCallError> {
    let mut config = build_agent_shared_config(turn)?;
    config.base_instructions = Some(base_instructions.text.clone());
    Ok(config)
}

pub(crate) fn build_agent_resume_config(
    turn: &TurnContext,
    child_depth: i32,
) -> Result<Config, FunctionCallError> {
    let mut config = build_agent_shared_config(turn)?;
    apply_spawn_agent_overrides(&mut config, child_depth);
    // For resume, keep base instructions sourced from rollout/session metadata.
    config.base_instructions = None;
    Ok(config)
}

fn build_agent_shared_config(turn: &TurnContext) -> Result<Config, FunctionCallError> {
    let base_config = turn.config.clone();
    let mut config = (*base_config).clone();
    config.model = Some(turn.model_info.slug.clone());
    config.model_provider = turn.provider.clone();
    config.model_reasoning_effort = turn.reasoning_effort;
    config.model_reasoning_summary = Some(turn.reasoning_summary);
    config.developer_instructions = turn.developer_instructions.clone();
    config.compact_prompt = turn.compact_prompt.clone();
    apply_spawn_agent_runtime_overrides(&mut config, turn)?;

    Ok(config)
}

/// Copies runtime-only turn state onto a child config before it is handed to `AgentControl`.
///
/// These values are chosen by the live turn rather than persisted config, so leaving them stale
/// can make a child agent disagree with its parent about approval policy, cwd, or sandboxing.
pub(crate) fn apply_spawn_agent_runtime_overrides(
    config: &mut Config,
    turn: &TurnContext,
) -> Result<(), FunctionCallError> {
    config
        .permissions
        .approval_policy
        .set(turn.approval_policy.value())
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("approval_policy is invalid: {err}"))
        })?;
    config.permissions.shell_environment_policy = turn.shell_environment_policy.clone();
    config.codex_linux_sandbox_exe = turn.codex_linux_sandbox_exe.clone();
    config.cwd = turn.cwd.clone();
    config
        .permissions
        .sandbox_policy
        .set(turn.sandbox_policy.get().clone())
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("sandbox_policy is invalid: {err}"))
        })?;
    config.permissions.file_system_sandbox_policy = turn.file_system_sandbox_policy.clone();
    config.permissions.network_sandbox_policy = turn.network_sandbox_policy;
    Ok(())
}

pub(crate) fn apply_spawn_agent_overrides(config: &mut Config, child_depth: i32) {
    if child_depth >= config.agent_max_depth && !config.features.enabled(Feature::MultiAgentV2) {
        let _ = config.features.disable(Feature::SpawnCsv);
        let _ = config.features.disable(Feature::Collab);
    }
}

pub(crate) async fn apply_requested_spawn_agent_model_overrides(
    session: &Session,
    turn: &TurnContext,
    config: &mut Config,
    requested_model: Option<&str>,
    requested_reasoning_effort: Option<ReasoningEffort>,
) -> Result<(), FunctionCallError> {
    if requested_model.is_none() && requested_reasoning_effort.is_none() {
        return Ok(());
    }

    if let Some(requested_model) = requested_model {
        let available_models = session
            .services
            .models_manager
            .list_models(RefreshStrategy::Offline)
            .await;
        let selected_model_name = find_spawn_agent_model_name(&available_models, requested_model)?;
        let selected_model_info = session
            .services
            .models_manager
            .get_model_info(&selected_model_name, config)
            .await;

        config.model = Some(selected_model_name.clone());
        if let Some(reasoning_effort) = requested_reasoning_effort {
            validate_spawn_agent_reasoning_effort(
                &selected_model_name,
                &selected_model_info.supported_reasoning_levels,
                reasoning_effort,
            )?;
            config.model_reasoning_effort = Some(reasoning_effort);
        } else {
            config.model_reasoning_effort = selected_model_info.default_reasoning_level;
        }

        return Ok(());
    }

    if let Some(reasoning_effort) = requested_reasoning_effort {
        validate_spawn_agent_reasoning_effort(
            &turn.model_info.slug,
            &turn.model_info.supported_reasoning_levels,
            reasoning_effort,
        )?;
        config.model_reasoning_effort = Some(reasoning_effort);
    }

    Ok(())
}

fn find_spawn_agent_model_name(
    available_models: &[codex_protocol::openai_models::ModelPreset],
    requested_model: &str,
) -> Result<String, FunctionCallError> {
    available_models
        .iter()
        .find(|model| model.model == requested_model)
        .map(|model| model.model.clone())
        .ok_or_else(|| {
            let available = available_models
                .iter()
                .map(|model| model.model.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            FunctionCallError::RespondToModel(format!(
                "Unknown model `{requested_model}` for spawn_agent. Available models: {available}"
            ))
        })
}

fn validate_spawn_agent_reasoning_effort(
    model: &str,
    supported_reasoning_levels: &[ReasoningEffortPreset],
    requested_reasoning_effort: ReasoningEffort,
) -> Result<(), FunctionCallError> {
    if supported_reasoning_levels
        .iter()
        .any(|preset| preset.effort == requested_reasoning_effort)
    {
        return Ok(());
    }

    let supported = supported_reasoning_levels
        .iter()
        .map(|preset| preset.effort.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(FunctionCallError::RespondToModel(format!(
        "Reasoning effort `{requested_reasoning_effort}` is not supported for model `{model}`. Supported reasoning efforts: {supported}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UsageLimitReachedError;
    use crate::protocol::AgentStatus;

    #[test]
    fn collect_spawn_agent_model_candidates_prefers_fallback_list() {
        let candidates = collect_spawn_agent_model_candidates(
            Some(&vec![
                SpawnAgentModelFallbackCandidate {
                    model: "fallback-a".to_string(),
                    reasoning_effort: Some(ReasoningEffort::High),
                },
                SpawnAgentModelFallbackCandidate {
                    model: "fallback-b".to_string(),
                    reasoning_effort: Some(ReasoningEffort::Minimal),
                },
            ]),
            Some("legacy-model"),
            Some(ReasoningEffort::Low),
        );

        assert_eq!(
            candidates,
            vec![
                SpawnAgentModelCandidate {
                    model: Some("fallback-a".to_string()),
                    reasoning_effort: Some(ReasoningEffort::High),
                },
                SpawnAgentModelCandidate {
                    model: Some("fallback-b".to_string()),
                    reasoning_effort: Some(ReasoningEffort::Minimal),
                },
            ]
        );
    }

    #[test]
    fn collect_spawn_agent_model_candidates_falls_back_to_legacy_args() {
        let candidates = collect_spawn_agent_model_candidates(
            /*model_fallback_list*/ None,
            Some("legacy-model"),
            Some(ReasoningEffort::Minimal),
        );
        assert_eq!(
            candidates,
            vec![SpawnAgentModelCandidate {
                model: Some("legacy-model".to_string()),
                reasoning_effort: Some(ReasoningEffort::Minimal),
            }]
        );
    }

    #[test]
    fn collect_spawn_agent_model_candidates_empty_when_no_model_is_set() {
        let candidates = collect_spawn_agent_model_candidates(
            /*model_fallback_list*/ None, /*requested_model*/ None,
            /*requested_reasoning_effort*/ None,
        );
        assert_eq!(candidates, Vec::new());
    }

    #[test]
    fn spawn_should_retry_on_quota_exhaustion_checks_expected_error_variants() {
        assert!(spawn_should_retry_on_quota_exhaustion(
            &CodexErr::QuotaExceeded
        ));
        assert!(spawn_should_retry_on_quota_exhaustion(
            &CodexErr::UsageLimitReached(UsageLimitReachedError {
                plan_type: None,
                resets_at: None,
                rate_limits: None,
                promo_message: None,
            })
        ));
        assert!(!spawn_should_retry_on_quota_exhaustion(
            &CodexErr::UnsupportedOperation("thread manager dropped".to_string())
        ));
    }

    #[test]
    fn collab_spawn_error_handles_thread_manager_drop() {
        assert_eq!(
            collab_spawn_error(CodexErr::UnsupportedOperation(
                "thread manager dropped".to_string()
            )),
            FunctionCallError::RespondToModel("collab manager unavailable".to_string())
        );
    }

    #[test]
    fn build_wait_agent_statuses_includes_extras_in_sorted_order() {
        let receiver_agents = vec![];
        let mut statuses = HashMap::new();
        let thread_a = ThreadId::new();
        let thread_b = ThreadId::new();
        statuses.insert(thread_b, AgentStatus::Completed(Some("done".to_string())));
        statuses.insert(thread_a, AgentStatus::Completed(Some("done".to_string())));

        let entries = build_wait_agent_statuses(&statuses, &receiver_agents);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].thread_id, thread_a);
        assert_eq!(entries[1].thread_id, thread_b);
    }
}
