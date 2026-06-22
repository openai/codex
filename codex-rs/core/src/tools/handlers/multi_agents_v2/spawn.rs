use super::*;
use crate::agent::control::SpawnAgentForkMode;
use crate::agent::control::SpawnAgentOptions;
use crate::agent::next_thread_spawn_depth;
use crate::agent::role::DEFAULT_ROLE_NAME;
use crate::agent::role::apply_role_to_config;
use crate::tools::handlers::multi_agents_spec::SpawnAgentToolOptions;
use crate::tools::handlers::multi_agents_spec::create_spawn_agent_tool_v2;
use crate::turn_timing::now_unix_timestamp_ms;
use codex_protocol::AgentPath;
use codex_protocol::protocol::Op;
use codex_tools::ToolSpec;

#[derive(Default)]
pub(crate) struct Handler {
    options: SpawnAgentToolOptions,
}

impl Handler {
    pub(crate) fn new(options: SpawnAgentToolOptions) -> Self {
        Self { options }
    }
}

impl ToolExecutor<ToolInvocation> for Handler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("spawn_agent")
    }

    fn spec(&self) -> ToolSpec {
        create_spawn_agent_tool_v2(self.options.clone())
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move { handle_spawn_agent(invocation).await.map(boxed_tool_output) })
    }
}

async fn handle_spawn_agent(
    invocation: ToolInvocation,
) -> Result<SpawnAgentResult, FunctionCallError> {
    let ToolInvocation {
        session,
        turn,
        payload,
        call_id,
        ..
    } = invocation;
    let arguments = function_arguments(payload)?;
    let args: SpawnAgentArgs = parse_arguments(&arguments)?;
    let fork_mode = args.fork_mode()?;
    let multi_agent_mode = crate::session::multi_agents::effective_multi_agent_mode(
        turn.multi_agent_version,
        &turn.session_source,
        turn.multi_agent_mode,
    );
    let role_name = args
        .agent_type
        .as_deref()
        .map(str::trim)
        .filter(|role| !role.is_empty());

    let message = args.message.clone();
    let initial_operation = parse_collab_input(Some(args.message), /*items*/ None)?;
    let session_source = turn.session_source.clone();
    let child_depth = next_thread_spawn_depth(&session_source);
    let mut config =
        build_agent_spawn_config(&session.get_base_instructions().await, turn.as_ref())?;
    if let Some(service_tier) = args.service_tier.as_ref() {
        config.service_tier = Some(service_tier.clone());
    }
    if matches!(fork_mode, Some(SpawnAgentForkMode::FullHistory)) {
        reject_full_fork_spawn_overrides(
            role_name,
            args.model.as_deref(),
            args.reasoning_effort.clone(),
        )?;
    } else {
        apply_requested_spawn_agent_model_overrides(
            &session,
            turn.as_ref(),
            &mut config,
            args.model.as_deref(),
            args.reasoning_effort.clone(),
        )
        .await?;
        apply_role_to_config(&mut config, role_name)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
    }
    apply_spawn_agent_service_tier(
        &session,
        &mut config,
        turn.config.service_tier.as_deref(),
        args.service_tier.as_deref(),
    )
    .await?;
    apply_spawn_agent_runtime_overrides(&mut config, turn.as_ref())?;

    let spawn_source = thread_spawn_source(
        session.thread_id,
        &turn.session_source,
        child_depth,
        role_name,
        Some(args.task_name.clone()),
    )?;
    let new_agent_path = spawn_source.get_agent_path().ok_or_else(|| {
        FunctionCallError::RespondToModel(
            "spawned agent is missing a canonical task name".to_string(),
        )
    })?;
    let spawned_agent = Box::pin(
        session.services.agent_control.spawn_agent_with_metadata(
            config,
            match initial_operation {
                Op::UserInput { items, .. }
                    if items
                        .iter()
                        .all(|item| matches!(item, UserInput::Text { .. })) =>
                {
                    let author = turn
                        .session_source
                        .get_agent_path()
                        .unwrap_or_else(AgentPath::root);
                    let communication =
                        communication_from_tool_message(author, new_agent_path.clone(), message);
                    Op::InterAgentCommunication { communication }
                }
                initial_operation => initial_operation,
            },
            Some(spawn_source),
            SpawnAgentOptions {
                fork_parent_spawn_call_id: fork_mode.as_ref().map(|_| call_id.clone()),
                fork_mode,
                parent_thread_id: Some(session.thread_id),
                environments: Some(turn.environments.to_selections()),
                initial_multi_agent_mode: multi_agent_mode,
            },
        ),
    )
    .await
    .map_err(collab_spawn_error)?;
    let new_thread_id = spawned_agent.thread_id;
    let agent_snapshot = session
        .services
        .agent_control
        .get_agent_config_snapshot(new_thread_id)
        .await;
    let nickname = agent_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.session_source.get_nickname())
        .or(spawned_agent.metadata.agent_nickname);
    session
        .send_event(
            &turn,
            SubAgentActivityEvent {
                event_id: call_id,
                occurred_at_ms: now_unix_timestamp_ms(),
                agent_thread_id: new_thread_id,
                agent_path: new_agent_path.clone(),
                kind: SubAgentActivityKind::Started,
            }
            .into(),
        )
        .await;
    let role_tag = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    turn.session_telemetry.counter(
        "codex.multi_agent.spawn",
        /*inc*/ 1,
        &[("role", role_tag), ("version", "v2")],
    );
    let task_name = String::from(new_agent_path);

    let hide_agent_metadata = turn.config.multi_agent_v2.hide_spawn_agent_metadata;
    if hide_agent_metadata {
        Ok(SpawnAgentResult::HiddenMetadata { task_name })
    } else {
        Ok(SpawnAgentResult::WithNickname {
            task_name,
            nickname,
        })
    }
}

impl CoreToolRuntime for Handler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnAgentArgs {
    message: String,
    task_name: String,
    agent_type: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    service_tier: Option<String>,
    fork_turns: Option<String>,
    fork_context: Option<bool>,
}

impl SpawnAgentArgs {
    fn fork_mode(&self) -> Result<Option<SpawnAgentForkMode>, FunctionCallError> {
        if self.fork_context.is_some() {
            return Err(FunctionCallError::RespondToModel(
                "fork_context is not supported in MultiAgentV2; use fork_turns instead".to_string(),
            ));
        }

        let fork_turns = self
            .fork_turns
            .as_deref()
            .map(str::trim)
            .filter(|fork_turns| !fork_turns.is_empty())
            .unwrap_or("all");

        if fork_turns.eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        if fork_turns.eq_ignore_ascii_case("all") {
            return Ok(Some(SpawnAgentForkMode::FullHistory));
        }

        let last_n_turns = fork_turns.parse::<usize>().map_err(|_| {
            FunctionCallError::RespondToModel(
                "fork_turns must be `none`, `all`, or a positive integer string".to_string(),
            )
        })?;
        if last_n_turns == 0 {
            return Err(FunctionCallError::RespondToModel(
                "fork_turns must be `none`, `all`, or a positive integer string".to_string(),
            ));
        }

        Ok(Some(SpawnAgentForkMode::LastNTurns(last_n_turns)))
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum SpawnAgentResult {
    WithNickname {
        task_name: String,
        nickname: Option<String>,
    },
    HiddenMetadata {
        task_name: String,
    },
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
