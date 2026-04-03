use super::*;
use crate::agent::control::SpawnAgentForkMode;
use crate::agent::control::SpawnAgentOptions;
use crate::agent::control::render_input_preview;
use crate::agent::role::DEFAULT_ROLE_NAME;
use crate::agent::role::apply_role_to_config;

use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::agent::next_thread_spawn_depth;

pub(crate) struct Handler;

impl ToolHandler for Handler {
    type Output = SpawnAgentResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;
        let arguments = function_arguments(payload)?;
        let args: SpawnAgentArgs = parse_arguments(&arguments)?;
        let role_name = args
            .agent_type
            .as_deref()
            .map(str::trim)
            .filter(|role| !role.is_empty());
        let input_items = parse_collab_input(args.message, args.items)?;
        let prompt = render_input_preview(&input_items);
        let session_source = turn.session_source.clone();
        let child_depth = next_thread_spawn_depth(&session_source);
        let max_depth = turn.config.agent_max_depth;
        if exceeds_thread_spawn_depth_limit(child_depth, max_depth) {
            return Err(FunctionCallError::RespondToModel(
                "Agent depth limit reached. Solve the task yourself.".to_string(),
            ));
        }
        let config =
            build_agent_spawn_config(&session.get_base_instructions().await, turn.as_ref())?;

        let mut candidates_to_try = collect_spawn_agent_model_candidates(
            args.model_fallback_list.as_ref(),
            args.model.as_deref(),
            args.reasoning_effort,
        );
        if candidates_to_try.is_empty() {
            candidates_to_try.push(SpawnAgentModelCandidate {
                model: None,
                reasoning_effort: None,
            });
        }

        let mut spawn_result = None;
        for (idx, candidate) in candidates_to_try.iter().enumerate() {
            let attempt_call_id = spawn_attempt_event_call_id(&call_id, idx);
            let candidate_model = candidate.model.clone().unwrap_or_default();
            let candidate_reasoning_effort = candidate.reasoning_effort.unwrap_or_default();
            send_collab_agent_spawn_begin_event(
                &session,
                &turn,
                attempt_call_id.clone(),
                prompt.clone(),
                candidate_model.clone(),
                candidate_reasoning_effort,
            )
            .await;
            let mut candidate_config = config.clone();
            apply_requested_spawn_agent_model_overrides(
                &session,
                turn.as_ref(),
                &mut candidate_config,
                candidate.model.as_deref(),
                candidate.reasoning_effort,
            )
            .await?;
            apply_role_to_config(&mut candidate_config, role_name)
                .await
                .map_err(FunctionCallError::RespondToModel)?;
            apply_spawn_agent_runtime_overrides(&mut candidate_config, turn.as_ref())?;
            apply_spawn_agent_overrides(&mut candidate_config, child_depth);
            let attempt_result = session
                .services
                .agent_control
                .spawn_agent_with_metadata(
                    candidate_config,
                    input_items.clone(),
                    Some(thread_spawn_source(
                        session.conversation_id,
                        &turn.session_source,
                        child_depth,
                        role_name,
                        /*task_name*/ None,
                    )?),
                    SpawnAgentOptions {
                        fork_parent_spawn_call_id: args.fork_context.then(|| call_id.clone()),
                        fork_mode: args.fork_context.then_some(SpawnAgentForkMode::FullHistory),
                    },
                )
                .await;
            match attempt_result {
                Ok(spawned_agent) => {
                    let status = if idx + 1 < candidates_to_try.len() {
                        match probe_spawn_attempt_for_async_quota_exhaustion(
                            spawned_agent.status.clone(),
                            spawned_agent.thread_id,
                            &session.services.agent_control,
                        )
                        .await
                        {
                            SpawnAttemptRetryDecision::Accept(status) => status,
                            SpawnAttemptRetryDecision::Retry(retry_status) => {
                                match close_quota_exhausted_spawn_attempt(
                                    &session.services.agent_control,
                                    spawned_agent.thread_id,
                                    retry_status,
                                )
                                .await
                                {
                                    SpawnAttemptRetryDecision::Accept(status) => status,
                                    SpawnAttemptRetryDecision::Retry(status) => {
                                        send_collab_agent_spawn_retry_preempted_event(
                                            &session,
                                            &turn,
                                            attempt_call_id,
                                            prompt.clone(),
                                            candidate_model,
                                            candidate_reasoning_effort,
                                            status,
                                        )
                                        .await;
                                        continue;
                                    }
                                }
                            }
                        }
                    } else {
                        spawned_agent.status.clone()
                    };
                    spawn_result = Some((spawned_agent, status, attempt_call_id));
                    break;
                }
                Err(err) => {
                    send_collab_agent_spawn_error_event(
                        &session,
                        &turn,
                        attempt_call_id,
                        prompt.clone(),
                        candidate_model,
                        candidate_reasoning_effort,
                        &err,
                    )
                    .await;
                    if spawn_should_retry_on_quota_exhaustion(&err)
                        && idx + 1 < candidates_to_try.len()
                    {
                        continue;
                    }
                    return Err(collab_spawn_error(err));
                }
            }
        }
        let Some((spawned_agent, status, spawn_event_call_id)) = spawn_result else {
            return Err(FunctionCallError::RespondToModel(
                "No spawn attempts were executed".to_string(),
            ));
        };
        let new_thread_id = Some(spawned_agent.thread_id);
        let new_agent_metadata = Some(spawned_agent.metadata.clone());
        let agent_snapshot = match new_thread_id {
            Some(thread_id) => {
                session
                    .services
                    .agent_control
                    .get_agent_config_snapshot(thread_id)
                    .await
            }
            None => None,
        };
        let (_new_agent_path, new_agent_nickname, new_agent_role) =
            match (&agent_snapshot, new_agent_metadata) {
                (Some(snapshot), _) => (
                    snapshot.session_source.get_agent_path().map(String::from),
                    snapshot.session_source.get_nickname(),
                    snapshot.session_source.get_agent_role(),
                ),
                (None, Some(metadata)) => (
                    metadata.agent_path.map(String::from),
                    metadata.agent_nickname,
                    metadata.agent_role,
                ),
                (None, None) => (None, None, None),
            };
        let effective_model = agent_snapshot
            .as_ref()
            .map(|snapshot| snapshot.model.clone())
            .unwrap_or_else(|| args.model.clone().unwrap_or_default());
        let effective_reasoning_effort = agent_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.reasoning_effort)
            .unwrap_or(args.reasoning_effort.unwrap_or_default());
        let nickname = new_agent_nickname.clone();
        session
            .send_event(
                &turn,
                CollabAgentSpawnEndEvent {
                    call_id: spawn_event_call_id,
                    sender_thread_id: session.conversation_id,
                    new_thread_id,
                    new_agent_nickname,
                    new_agent_role,
                    prompt,
                    model: effective_model,
                    reasoning_effort: effective_reasoning_effort,
                    status,
                }
                .into(),
            )
            .await;
        let new_thread_id = spawned_agent.thread_id;
        let role_tag = role_name.unwrap_or(DEFAULT_ROLE_NAME);
        turn.session_telemetry.counter(
            "codex.multi_agent.spawn",
            /*inc*/ 1,
            &[("role", role_tag)],
        );

        Ok(SpawnAgentResult {
            agent_id: new_thread_id.to_string(),
            nickname,
        })
    }
}

#[derive(Debug, Deserialize)]
struct SpawnAgentArgs {
    message: Option<String>,
    items: Option<Vec<UserInput>>,
    agent_type: Option<String>,
    model: Option<String>,
    model_fallback_list: Option<Vec<SpawnAgentModelFallbackCandidate>>,
    reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    fork_context: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpawnAgentResult {
    agent_id: String,
    nickname: Option<String>,
}

impl ToolOutput for SpawnAgentResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "spawn_agent")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "spawn_agent")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "spawn_agent")
    }
}
