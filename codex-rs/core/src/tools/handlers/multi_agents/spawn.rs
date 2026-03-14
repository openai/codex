use super::*;
use crate::agent::RemovedWatchdog;
use crate::agent::WatchdogRegistration;
use crate::agent::control::LiveAgent;
use crate::agent::control::SpawnAgentOptions;
use crate::agent::control::render_input_preview;
use crate::config::Config;
use crate::agent::next_thread_spawn_depth;
use crate::agent::role::DEFAULT_ROLE_NAME;
use crate::agent::role::apply_role_to_config;
use crate::agent::role::default_spawn_mode_for_role;
use crate::agent::role::watchdog_interval_for_role;
use codex_features::Feature;
use codex_protocol::protocol::AgentSpawnMode;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use std::collections::HashSet;

pub(crate) struct Handler;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SpawnMode {
    Spawn,
    Fork,
}

impl From<SpawnMode> for AgentSpawnMode {
    fn from(value: SpawnMode) -> Self {
        match value {
            SpawnMode::Spawn => AgentSpawnMode::Spawn,
            SpawnMode::Fork => AgentSpawnMode::Fork,
        }
    }
}

#[async_trait]
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
        let default_spawn_mode = match default_spawn_mode_for_role(&turn.config, role_name) {
            crate::config::AgentRoleSpawnMode::Spawn => SpawnMode::Spawn,
            crate::config::AgentRoleSpawnMode::Fork => SpawnMode::Fork,
        };
        let spawn_mode = args
            .spawn_mode
            .or_else(|| (args.fork_context && args.spawn_mode.is_none()).then_some(SpawnMode::Fork))
            .unwrap_or(default_spawn_mode);
        let watchdog_interval_s = watchdog_interval_for_role(&turn.config, role_name);
        let is_watchdog = watchdog_interval_s.is_some();

        if is_watchdog && !turn.config.features.enabled(Feature::AgentWatchdog) {
            return Err(FunctionCallError::RespondToModel(
                "watchdogs are disabled".to_string(),
            ));
        }
        if is_watchdog && matches!(session_source, SessionSource::SubAgent(_)) {
            return Err(FunctionCallError::RespondToModel(
                "watchdogs can only be spawned by root agents".to_string(),
            ));
        }
        if exceeds_thread_spawn_depth_limit(child_depth, max_depth) {
            return Err(FunctionCallError::RespondToModel(
                "Agent depth limit reached. Solve the task yourself.".to_string(),
            ));
        }

        session
            .send_event(
                &turn,
                CollabAgentSpawnBeginEvent {
                    call_id: call_id.clone(),
                    sender_thread_id: session.conversation_id,
                    prompt: prompt.clone(),
                    model: args.model.clone().unwrap_or_default(),
                    reasoning_effort: args.reasoning_effort.unwrap_or_default(),
                }
                .into(),
            )
            .await;

        let mut config =
            build_agent_spawn_config(&session.get_base_instructions().await, turn.as_ref())?;
        if !args.fork_context {
            apply_requested_spawn_agent_model_overrides(
                &session,
                turn.as_ref(),
                &mut config,
                args.model.as_deref(),
                args.reasoning_effort,
            )
            .await?;
        }
        apply_role_to_config(&mut config, role_name)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        apply_spawn_agent_runtime_overrides(&mut config, turn.as_ref())?;
        apply_spawn_agent_overrides(&mut config, child_depth);

        let spawn_source = thread_spawn_source(
            session.conversation_id,
            &turn.session_source,
            child_depth,
            role_name,
            args.task_name.clone(),
        )?;
        let result = if let Some(watchdog_interval_s) = watchdog_interval_s {
            let thread_id = spawn_watchdog(
                &session.services.agent_control,
                config,
                prompt.clone(),
                session.conversation_id,
                child_depth,
                watchdog_interval_s,
                spawn_source,
            )
            .await
            .map_err(collab_spawn_error)?;
            Ok(LiveAgent {
                thread_id,
                metadata: session
                    .services
                    .agent_control
                    .get_agent_metadata(thread_id)
                    .unwrap_or_default(),
                status: session.services.agent_control.get_status(thread_id).await,
            })
        } else {
            match spawn_mode {
                SpawnMode::Spawn => {
                    session
                        .services
                        .agent_control
                        .spawn_agent_with_metadata(
                            config,
                            input_items,
                            Some(spawn_source),
                            SpawnAgentOptions::default(),
                        )
                        .await
                }
                SpawnMode::Fork if args.fork_context => {
                    session
                        .services
                        .agent_control
                        .spawn_agent_with_metadata(
                            config,
                            input_items,
                            Some(spawn_source),
                            SpawnAgentOptions {
                                fork_parent_spawn_call_id: Some(call_id.clone()),
                            },
                        )
                        .await
                }
                SpawnMode::Fork => {
                    let thread_id = session
                        .services
                        .agent_control
                        .fork_agent(
                            config,
                            match input_items {
                                Op::UserInput { items, .. } => items,
                                _ => {
                                    return Err(FunctionCallError::RespondToModel(
                                        "fork_agent requires user input".to_string(),
                                    ));
                                }
                            },
                            session.conversation_id,
                            usize::MAX,
                            spawn_source,
                        )
                        .await
                        .map_err(collab_spawn_error)?;
                    Ok(LiveAgent {
                        thread_id,
                        metadata: session
                            .services
                            .agent_control
                            .get_agent_metadata(thread_id)
                            .unwrap_or_default(),
                        status: session.services.agent_control.get_status(thread_id).await,
                    })
                }
            }
        }
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
            .and_then(|snapshot| snapshot.reasoning_effort)
            .unwrap_or(args.reasoning_effort.unwrap_or_default());
        let nickname = new_agent_nickname.clone();
        session
            .send_event(
                &turn,
                CollabAgentSpawnEndEvent {
                    call_id,
                    sender_thread_id: session.conversation_id,
                    new_thread_id,
                    new_agent_nickname,
                    new_agent_role,
                    prompt,
                    model: effective_model,
                    reasoning_effort: effective_reasoning_effort,
                    // Preserve the actual spawn mode; the TUI uses this to render watchdog rows
                    // distinctly and to avoid regressing watchdog state display on future rebases.
                    spawn_mode: if is_watchdog {
                        AgentSpawnMode::Watchdog
                    } else {
                        spawn_mode.into()
                    },
                    status,
                }
                .into(),
            )
            .await;
        let new_thread_id = result?.thread_id;
        let role_tag = role_name.unwrap_or(DEFAULT_ROLE_NAME);
        turn.session_telemetry.counter(
            "codex.multi_agent.spawn",
            /*inc*/ 1,
            &[("role", role_tag)],
        );

        Ok(SpawnAgentResult {
            agent_id: Some(new_thread_id.to_string()),
            task_name: None,
            nickname,
        })
    }
}

async fn spawn_watchdog(
    agent_control: &crate::agent::AgentControl,
    config: Config,
    prompt: String,
    owner_thread_id: ThreadId,
    child_depth: i32,
    interval_s: i64,
    spawn_source: SessionSource,
) -> crate::error::Result<ThreadId> {
    let target_thread_id = agent_control
        .spawn_agent_handle(config.clone(), Some(spawn_source))
        .await?;
    let superseded_before_register = agent_control
        .unregister_watchdogs_for_owner(owner_thread_id)
        .await;
    shutdown_removed_watchdogs(agent_control, superseded_before_register).await;
    let registration = WatchdogRegistration {
        owner_thread_id,
        target_thread_id,
        child_depth,
        interval_s,
        prompt,
        config,
    };
    let superseded_after_register = match agent_control.register_watchdog(registration).await {
        Ok(removed) => removed,
        Err(err) => {
            let _ = agent_control.close_agent(target_thread_id).await;
            return Err(err);
        }
    };
    shutdown_removed_watchdogs(agent_control, superseded_after_register).await;
    Ok(target_thread_id)
}

async fn shutdown_removed_watchdogs(
    agent_control: &crate::agent::AgentControl,
    removed_watchdogs: Vec<RemovedWatchdog>,
) {
    let mut thread_ids = HashSet::new();
    for removed in removed_watchdogs {
        thread_ids.insert(removed.target_thread_id);
        if let Some(helper_id) = removed.active_helper_id {
            thread_ids.insert(helper_id);
        }
    }
    let mut thread_ids = thread_ids.into_iter().collect::<Vec<_>>();
    thread_ids.sort_by_key(ToString::to_string);
    for thread_id in thread_ids {
        let _ = agent_control.close_agent(thread_id).await;
    }
}

#[derive(Debug, Deserialize)]
struct SpawnAgentArgs {
    message: Option<String>,
    items: Option<Vec<UserInput>>,
    task_name: Option<String>,
    agent_type: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, alias = "mode")]
    spawn_mode: Option<SpawnMode>,
    #[serde(default)]
    fork_context: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SpawnAgentResult {
    agent_id: Option<String>,
    task_name: Option<String>,
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
