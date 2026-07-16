use super::*;
use crate::agent::control::SpawnAgentForkMode;
use crate::agent::control::SpawnAgentOptions;
use crate::agent::control::render_input_preview;
use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::agent::next_thread_spawn_depth;
use crate::agent::role::DEFAULT_ROLE_NAME;
use crate::tools::handlers::multi_agents_spec::SpawnAgentToolOptions;
use crate::tools::handlers::multi_agents_spec::create_spawn_agent_tool_v1;
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
        ToolName::namespaced(MULTI_AGENT_V1_NAMESPACE, "spawn_agent")
    }

    fn spec(&self) -> ToolSpec {
        create_spawn_agent_tool_v1(self.options.clone())
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        multi_agent_tool_search_info(
            "spawn_agent spawn agent subagent sub-agent delegate delegation parallel work worker explorer no-apps fork model reasoning",
            self.spec(),
        )
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
    session
        .emit_turn_item_started(
            &turn,
            &TurnItem::CollabAgentToolCall(CollabAgentToolCallItem {
                id: call_id.clone(),
                tool: CollabAgentTool::SpawnAgent,
                status: CollabAgentToolCallStatus::InProgress,
                sender_thread_id: session.thread_id,
                receiver_thread_ids: Vec::new(),
                receiver_agents: Vec::new(),
                prompt: Some(prompt.clone()),
                model: Some(args.model.clone().unwrap_or_default()),
                reasoning_effort: Some(args.reasoning_effort.clone().unwrap_or_default()),
                agents_states: Default::default(),
            }),
        )
        .await;
    let mut config =
        build_agent_spawn_config(&session.get_base_instructions().await, turn.as_ref())?;
    if let Some(service_tier) = args.service_tier.as_ref() {
        config.service_tier = Some(service_tier.clone());
    }
    if args.fork_context {
        reject_full_fork_agent_type_override(role_name)?;
    }
    apply_requested_spawn_agent_model_overrides(
        &session,
        turn.as_ref(),
        &mut config,
        args.model.as_deref(),
        args.reasoning_effort.clone(),
    )
    .await?;
    if !args.fork_context {
        apply_spawn_agent_role(&session, &mut config, role_name).await?;
    }
    apply_spawn_agent_service_tier(
        &session,
        &mut config,
        turn.config.service_tier.as_deref(),
        args.service_tier.as_deref(),
    )
    .await?;
    apply_spawn_agent_runtime_overrides(&mut config, turn.as_ref())?;

    let result = Box::pin(session.services.agent_control.spawn_agent_with_metadata(
        config,
        input_items,
        Some(thread_spawn_source(
            session.thread_id,
            &turn.session_source,
            child_depth,
            role_name,
            /*task_name*/ None,
        )?),
        SpawnAgentOptions {
            fork_parent_spawn_call_id: args.fork_context.then(|| call_id.clone()),
            fork_mode: args.fork_context.then_some(SpawnAgentForkMode::FullHistory),
            parent_thread_id: Some(session.thread_id),
            environments: Some(turn.environments.to_selections()),
        },
    ))
    .await
    .map_err(collab_spawn_error);
    let (new_thread_id, new_agent_metadata, status) = match &result {
        Ok(spawned_agent) => (
            Some(spawned_agent.thread_id),
            Some(spawned_agent.metadata.clone()),
            spawned_agent.status.clone(),
        ),
        Err(_) => (None, None, AgentStatus::NotFound),
    };
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
        .and_then(|snapshot| snapshot.reasoning_effort.clone())
        .unwrap_or(args.reasoning_effort.unwrap_or_default());
    let nickname = new_agent_nickname.clone();
    let receiver_thread_ids = new_thread_id.into_iter().collect();
    let receiver_agents = new_thread_id
        .map(|thread_id| CollabAgentRef {
            thread_id,
            agent_nickname: new_agent_nickname,
            agent_role: new_agent_role,
        })
        .into_iter()
        .collect();
    let agents_states = new_thread_id
        .map(|thread_id| [(thread_id, status.clone())].into_iter().collect())
        .unwrap_or_default();
    session
        .emit_turn_item_completed(
            &turn,
            TurnItem::CollabAgentToolCall(CollabAgentToolCallItem {
                id: call_id,
                tool: CollabAgentTool::SpawnAgent,
                status: collab_tool_call_status(&status, new_thread_id),
                sender_thread_id: session.thread_id,
                receiver_thread_ids,
                receiver_agents,
                prompt: Some(prompt),
                model: Some(effective_model),
                reasoning_effort: Some(effective_reasoning_effort),
                agents_states,
            }),
        )
        .await;
    let new_thread_id = result?.thread_id;
    let role_tag = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    turn.session_telemetry.counter(
        "codex.multi_agent.spawn",
        /*inc*/ 1,
        &[("role", role_tag), ("version", "v1")],
    );

    Ok(SpawnAgentResult {
        agent_id: new_thread_id.to_string(),
        nickname,
    })
}

impl CoreToolRuntime for Handler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}

#[derive(Debug, Deserialize)]
struct SpawnAgentArgs {
    message: Option<String>,
    items: Option<Vec<UserInput>>,
    agent_type: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    service_tier: Option<String>,
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
