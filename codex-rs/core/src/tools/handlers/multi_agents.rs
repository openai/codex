//! Implements the collaboration tool surface for spawning and managing sub-agents.
//!
//! This handler translates model tool calls into `AgentControl` operations and keeps spawned
//! agents aligned with the live turn that created them. Sub-agents start from the turn's effective
//! config, inherit runtime-only state such as provider, approval policy, sandbox, and cwd, and
//! then optionally layer role-specific config on top.

use crate::agent::AgentStatus;
use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::CodexErr;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::models_manager::manager::RefreshStrategy;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::AgentSpawnMode;
use codex_protocol::protocol::CollabAgentInteractionBeginEvent;
use codex_protocol::protocol::CollabAgentInteractionEndEvent;
use codex_protocol::protocol::CollabAgentRef;
use codex_protocol::protocol::CollabAgentSpawnBeginEvent;
use codex_protocol::protocol::CollabAgentSpawnEndEvent;
use codex_protocol::protocol::CollabAgentStatusEntry;
use codex_protocol::protocol::CollabCloseBeginEvent;
use codex_protocol::protocol::CollabCloseEndEvent;
use codex_protocol::protocol::CollabResumeBeginEvent;
use codex_protocol::protocol::CollabResumeEndEvent;
use codex_protocol::protocol::CollabWaitingBeginEvent;
use codex_protocol::protocol::CollabWaitingEndEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) use close_agent::Handler as CloseAgentHandler;
pub(crate) use resume_agent::Handler as ResumeAgentHandler;
pub(crate) use send_input::Handler as SendInputHandler;
pub(crate) use spawn::Handler as SpawnAgentHandler;
pub(crate) use wait::Handler as WaitHandler;

/// Minimum wait timeout to prevent tight polling loops from burning CPU.
pub(crate) const MIN_WAIT_TIMEOUT_MS: i64 = 10_000;
pub(crate) const DEFAULT_WAIT_TIMEOUT_MS: i64 = 30_000;
pub(crate) const MAX_WAIT_TIMEOUT_MS: i64 = 3600 * 1000;

#[derive(Debug, Deserialize)]
struct CloseAgentArgs {
    id: String,
}

fn function_arguments(payload: ToolPayload) -> Result<String, FunctionCallError> {
    match payload {
        ToolPayload::Function { arguments } => Ok(arguments),
        _ => Err(FunctionCallError::RespondToModel(
            "collab handler received unsupported payload".to_string(),
        )),
    }
}

fn tool_output_json_text<T>(value: &T, tool_name: &str) -> String
where
    T: Serialize,
{
    serde_json::to_string(value).unwrap_or_else(|err| {
        JsonValue::String(format!("failed to serialize {tool_name} result: {err}")).to_string()
    })
}

fn tool_output_response_item<T>(
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

fn tool_output_code_mode_result<T>(value: &T, tool_name: &str) -> JsonValue
where
    T: Serialize,
{
    serde_json::to_value(value).unwrap_or_else(|err| {
        JsonValue::String(format!("failed to serialize {tool_name} result: {err}"))
    })
}

mod spawn {
    use super::*;
    use crate::agent::control::SpawnAgentOptions;
    use crate::agent::role::DEFAULT_ROLE_NAME;
    use crate::agent::role::apply_role_to_config;

    use crate::agent::exceeds_thread_spawn_depth_limit;
    use crate::agent::next_thread_spawn_depth;

    pub(crate) struct Handler;

    #[async_trait]
    impl ToolHandler for Handler {
        type Output = SpawnAgentResult;

        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        fn matches_kind(&self, payload: &ToolPayload) -> bool {
            matches!(payload, ToolPayload::Function { .. })
        }

        async fn handle(
            &self,
            invocation: ToolInvocation,
        ) -> Result<Self::Output, FunctionCallError> {
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
            let prompt = input_preview(&input_items);
            let session_source = turn.session_source.clone();
            let child_depth = next_thread_spawn_depth(&session_source);
            let max_depth = turn.config.agent_max_depth;
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
            apply_requested_spawn_agent_model_overrides(
                &session,
                turn.as_ref(),
                &mut config,
                args.model.as_deref(),
                args.reasoning_effort,
            )
            .await?;
            apply_role_to_config(&mut config, role_name)
                .await
                .map_err(FunctionCallError::RespondToModel)?;
            apply_spawn_agent_runtime_overrides(&mut config, turn.as_ref())?;
            apply_spawn_agent_overrides(&mut config, child_depth);

            let result = session
                .services
                .agent_control
                .spawn_agent_with_options(
                    config,
                    input_items,
                    Some(thread_spawn_source(
                        session.conversation_id,
                        child_depth,
                        role_name,
                    )),
                    SpawnAgentOptions {
                        fork_parent_spawn_call_id: args.fork_context.then(|| call_id.clone()),
                    },
                )
                .await
                .map_err(collab_spawn_error);
            let (new_thread_id, status) = match &result {
                Ok(thread_id) => (
                    Some(*thread_id),
                    session.services.agent_control.get_status(*thread_id).await,
                ),
                Err(_) => (None, AgentStatus::NotFound),
            };
            let (new_agent_nickname, new_agent_role) = match new_thread_id {
                Some(thread_id) => session
                    .services
                    .agent_control
                    .get_agent_nickname_and_role(thread_id)
                    .await
                    .unwrap_or((None, None)),
                None => (None, None),
            };
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
                        model: args.model.clone().unwrap_or_default(),
                        reasoning_effort: args.reasoning_effort.unwrap_or_default(),
                        spawn_mode: if args.fork_context {
                            AgentSpawnMode::Fork
                        } else {
                            AgentSpawnMode::Spawn
                        },
                        status,
                    }
                    .into(),
                )
                .await;
            let new_thread_id = result?;
            let role_tag = role_name.unwrap_or(DEFAULT_ROLE_NAME);
            turn.session_telemetry
                .counter("codex.multi_agent.spawn", 1, &[("role", role_tag)]);

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
}

mod send_input {
    use super::*;

    pub(crate) struct Handler;

    #[async_trait]
    impl ToolHandler for Handler {
        type Output = SendInputResult;

        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        fn matches_kind(&self, payload: &ToolPayload) -> bool {
            matches!(payload, ToolPayload::Function { .. })
        }

        async fn handle(
            &self,
            invocation: ToolInvocation,
        ) -> Result<Self::Output, FunctionCallError> {
            let ToolInvocation {
                session,
                turn,
                payload,
                call_id,
                ..
            } = invocation;
            let arguments = function_arguments(payload)?;
            let args: SendInputArgs = parse_arguments(&arguments)?;
            let receiver_thread_id = agent_id(&args.id)?;
            let input_items = parse_collab_input(args.message, args.items)?;
            let prompt = input_preview(&input_items);
            let (receiver_agent_nickname, receiver_agent_role) = session
                .services
                .agent_control
                .get_agent_nickname_and_role(receiver_thread_id)
                .await
                .unwrap_or((None, None));
            if args.interrupt {
                session
                    .services
                    .agent_control
                    .interrupt_agent(receiver_thread_id)
                    .await
                    .map_err(|err| collab_agent_error(receiver_thread_id, err))?;
            }
            session
                .send_event(
                    &turn,
                    CollabAgentInteractionBeginEvent {
                        call_id: call_id.clone(),
                        sender_thread_id: session.conversation_id,
                        receiver_thread_id,
                        prompt: prompt.clone(),
                    }
                    .into(),
                )
                .await;
            let result = session
                .services
                .agent_control
                .send_input(receiver_thread_id, input_items)
                .await
                .map_err(|err| collab_agent_error(receiver_thread_id, err));
            let status = session
                .services
                .agent_control
                .get_status(receiver_thread_id)
                .await;
            session
                .send_event(
                    &turn,
                    CollabAgentInteractionEndEvent {
                        call_id,
                        sender_thread_id: session.conversation_id,
                        receiver_thread_id,
                        receiver_agent_nickname,
                        receiver_agent_role,
                        prompt,
                        status,
                    }
                    .into(),
                )
                .await;
            let submission_id = result?;

            Ok(SendInputResult { submission_id })
        }
    }

    #[derive(Debug, Deserialize)]
    struct SendInputArgs {
        id: String,
        message: Option<String>,
        items: Option<Vec<UserInput>>,
        #[serde(default)]
        interrupt: bool,
    }

    #[derive(Debug, Serialize)]
    pub(crate) struct SendInputResult {
        submission_id: String,
    }

    impl ToolOutput for SendInputResult {
        fn log_preview(&self) -> String {
            tool_output_json_text(self, "send_input")
        }

        fn success_for_logging(&self) -> bool {
            true
        }

        fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
            tool_output_response_item(call_id, payload, self, Some(true), "send_input")
        }

        fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
            tool_output_code_mode_result(self, "send_input")
        }
    }
}

mod resume_agent {
    use super::*;
    use crate::agent::next_thread_spawn_depth;

    pub(crate) struct Handler;

    #[async_trait]
    impl ToolHandler for Handler {
        type Output = ResumeAgentResult;

        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        fn matches_kind(&self, payload: &ToolPayload) -> bool {
            matches!(payload, ToolPayload::Function { .. })
        }

        async fn handle(
            &self,
            invocation: ToolInvocation,
        ) -> Result<Self::Output, FunctionCallError> {
            let ToolInvocation {
                session,
                turn,
                payload,
                call_id,
                ..
            } = invocation;
            let arguments = function_arguments(payload)?;
            let args: ResumeAgentArgs = parse_arguments(&arguments)?;
            let receiver_thread_id = agent_id(&args.id)?;
            let (receiver_agent_nickname, receiver_agent_role) = session
                .services
                .agent_control
                .get_agent_nickname_and_role(receiver_thread_id)
                .await
                .unwrap_or((None, None));
            let child_depth = next_thread_spawn_depth(&turn.session_source);
            let max_depth = turn.config.agent_max_depth;
            if exceeds_thread_spawn_depth_limit(child_depth, max_depth) {
                return Err(FunctionCallError::RespondToModel(
                    "Agent depth limit reached. Solve the task yourself.".to_string(),
                ));
            }

            session
                .send_event(
                    &turn,
                    CollabResumeBeginEvent {
                        call_id: call_id.clone(),
                        sender_thread_id: session.conversation_id,
                        receiver_thread_id,
                        receiver_agent_nickname: receiver_agent_nickname.clone(),
                        receiver_agent_role: receiver_agent_role.clone(),
                    }
                    .into(),
                )
                .await;

            let mut status = session
                .services
                .agent_control
                .get_status(receiver_thread_id)
                .await;
            let error = if matches!(status, AgentStatus::NotFound) {
                match try_resume_closed_agent(&session, &turn, receiver_thread_id, child_depth)
                    .await
                {
                    Ok(resumed_status) => {
                        status = resumed_status;
                        None
                    }
                    Err(err) => {
                        status = session
                            .services
                            .agent_control
                            .get_status(receiver_thread_id)
                            .await;
                        Some(err)
                    }
                }
            } else {
                None
            };

            let (receiver_agent_nickname, receiver_agent_role) = session
                .services
                .agent_control
                .get_agent_nickname_and_role(receiver_thread_id)
                .await
                .unwrap_or((receiver_agent_nickname, receiver_agent_role));
            session
                .send_event(
                    &turn,
                    CollabResumeEndEvent {
                        call_id,
                        sender_thread_id: session.conversation_id,
                        receiver_thread_id,
                        receiver_agent_nickname,
                        receiver_agent_role,
                        status: status.clone(),
                    }
                    .into(),
                )
                .await;

            if let Some(err) = error {
                return Err(err);
            }
            turn.session_telemetry
                .counter("codex.multi_agent.resume", 1, &[]);

            Ok(ResumeAgentResult { status })
        }
    }

    #[derive(Debug, Deserialize)]
    struct ResumeAgentArgs {
        id: String,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub(crate) struct ResumeAgentResult {
        pub(crate) status: AgentStatus,
    }

    impl ToolOutput for ResumeAgentResult {
        fn log_preview(&self) -> String {
            tool_output_json_text(self, "resume_agent")
        }

        fn success_for_logging(&self) -> bool {
            true
        }

        fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
            tool_output_response_item(call_id, payload, self, Some(true), "resume_agent")
        }

        fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
            tool_output_code_mode_result(self, "resume_agent")
        }
    }

    async fn try_resume_closed_agent(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        receiver_thread_id: ThreadId,
        child_depth: i32,
    ) -> Result<AgentStatus, FunctionCallError> {
        let config = build_agent_resume_config(turn.as_ref(), child_depth)?;
        let resumed_thread_id = session
            .services
            .agent_control
            .resume_agent_from_rollout(
                config,
                receiver_thread_id,
                thread_spawn_source(session.conversation_id, child_depth, None),
            )
            .await
            .map_err(|err| collab_agent_error(receiver_thread_id, err))?;

        Ok(session
            .services
            .agent_control
            .get_status(resumed_thread_id)
            .await)
    }
}

pub(crate) mod wait {
    use super::*;
    use crate::agent::status::is_final;
    use futures::FutureExt;
    use futures::StreamExt;
    use futures::stream::FuturesUnordered;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::sync::watch::Receiver;
    use tokio::time::Instant;

    use tokio::time::timeout_at;

    pub(crate) struct Handler;

    #[async_trait]
    impl ToolHandler for Handler {
        type Output = WaitResult;

        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        fn matches_kind(&self, payload: &ToolPayload) -> bool {
            matches!(payload, ToolPayload::Function { .. })
        }

        async fn handle(
            &self,
            invocation: ToolInvocation,
        ) -> Result<Self::Output, FunctionCallError> {
            let ToolInvocation {
                session,
                turn,
                payload,
                call_id,
                ..
            } = invocation;
            let arguments = function_arguments(payload)?;
            let args: WaitArgs = parse_arguments(&arguments)?;
            if args.ids.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "ids must be non-empty".to_owned(),
                ));
            }
            let receiver_thread_ids = args
                .ids
                .iter()
                .map(|id| agent_id(id))
                .collect::<Result<Vec<_>, _>>()?;
            let mut receiver_agents = Vec::with_capacity(receiver_thread_ids.len());
            for receiver_thread_id in &receiver_thread_ids {
                let (agent_nickname, agent_role) = session
                    .services
                    .agent_control
                    .get_agent_nickname_and_role(*receiver_thread_id)
                    .await
                    .unwrap_or((None, None));
                receiver_agents.push(CollabAgentRef {
                    thread_id: *receiver_thread_id,
                    agent_nickname,
                    agent_role,
                });
            }

            let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
            let timeout_ms = match timeout_ms {
                ms if ms <= 0 => {
                    return Err(FunctionCallError::RespondToModel(
                        "timeout_ms must be greater than zero".to_owned(),
                    ));
                }
                ms => ms.clamp(MIN_WAIT_TIMEOUT_MS, MAX_WAIT_TIMEOUT_MS),
            };

            session
                .send_event(
                    &turn,
                    CollabWaitingBeginEvent {
                        sender_thread_id: session.conversation_id,
                        receiver_thread_ids: receiver_thread_ids.clone(),
                        receiver_agents: receiver_agents.clone(),
                        call_id: call_id.clone(),
                    }
                    .into(),
                )
                .await;

            let mut status_rxs = Vec::with_capacity(receiver_thread_ids.len());
            let mut initial_final_statuses = Vec::new();
            for id in &receiver_thread_ids {
                match session.services.agent_control.subscribe_status(*id).await {
                    Ok(rx) => {
                        let status = rx.borrow().clone();
                        if is_final(&status) {
                            initial_final_statuses.push((*id, status));
                        }
                        status_rxs.push((*id, rx));
                    }
                    Err(CodexErr::ThreadNotFound(_)) => {
                        initial_final_statuses.push((*id, AgentStatus::NotFound));
                    }
                    Err(err) => {
                        let mut statuses = HashMap::with_capacity(1);
                        statuses.insert(*id, session.services.agent_control.get_status(*id).await);
                        session
                            .send_event(
                                &turn,
                                CollabWaitingEndEvent {
                                    sender_thread_id: session.conversation_id,
                                    call_id: call_id.clone(),
                                    agent_statuses: build_wait_agent_statuses(
                                        &statuses,
                                        &receiver_agents,
                                    ),
                                    statuses,
                                }
                                .into(),
                            )
                            .await;
                        return Err(collab_agent_error(*id, err));
                    }
                }
            }

            let statuses = if !initial_final_statuses.is_empty() {
                initial_final_statuses
            } else {
                let mut futures = FuturesUnordered::new();
                for (id, rx) in status_rxs.into_iter() {
                    let session = session.clone();
                    futures.push(wait_for_final_status(session, id, rx));
                }
                let mut results = Vec::new();
                let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
                loop {
                    match timeout_at(deadline, futures.next()).await {
                        Ok(Some(Some(result))) => {
                            results.push(result);
                            break;
                        }
                        Ok(Some(None)) => continue,
                        Ok(None) | Err(_) => break,
                    }
                }
                if !results.is_empty() {
                    loop {
                        match futures.next().now_or_never() {
                            Some(Some(Some(result))) => results.push(result),
                            Some(Some(None)) => continue,
                            Some(None) | None => break,
                        }
                    }
                }
                results
            };

            let statuses_map = statuses.clone().into_iter().collect::<HashMap<_, _>>();
            let agent_statuses = build_wait_agent_statuses(&statuses_map, &receiver_agents);
            let result = WaitResult {
                status: statuses_map.clone(),
                timed_out: statuses.is_empty(),
            };

            session
                .send_event(
                    &turn,
                    CollabWaitingEndEvent {
                        sender_thread_id: session.conversation_id,
                        call_id,
                        agent_statuses,
                        statuses: statuses_map,
                    }
                    .into(),
                )
                .await;

            Ok(result)
        }
    }

    #[derive(Debug, Deserialize)]
    struct WaitArgs {
        ids: Vec<String>,
        timeout_ms: Option<i64>,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub(crate) struct WaitResult {
        pub(crate) status: HashMap<ThreadId, AgentStatus>,
        pub(crate) timed_out: bool,
    }

    impl ToolOutput for WaitResult {
        fn log_preview(&self) -> String {
            tool_output_json_text(self, "wait")
        }

        fn success_for_logging(&self) -> bool {
            true
        }

        fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
            tool_output_response_item(call_id, payload, self, None, "wait")
        }

        fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
            tool_output_code_mode_result(self, "wait")
        }
    }

    async fn wait_for_final_status(
        session: Arc<Session>,
        thread_id: ThreadId,
        mut status_rx: Receiver<AgentStatus>,
    ) -> Option<(ThreadId, AgentStatus)> {
        let mut status = status_rx.borrow().clone();
        if is_final(&status) {
            return Some((thread_id, status));
        }

        loop {
            if status_rx.changed().await.is_err() {
                let latest = session.services.agent_control.get_status(thread_id).await;
                return is_final(&latest).then_some((thread_id, latest));
            }
            status = status_rx.borrow().clone();
            if is_final(&status) {
                return Some((thread_id, status));
            }
        }
    }
}

pub mod close_agent {
    use super::*;

    pub(crate) struct Handler;

    #[async_trait]
    impl ToolHandler for Handler {
        type Output = CloseAgentResult;

        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        fn matches_kind(&self, payload: &ToolPayload) -> bool {
            matches!(payload, ToolPayload::Function { .. })
        }

        async fn handle(
            &self,
            invocation: ToolInvocation,
        ) -> Result<Self::Output, FunctionCallError> {
            let ToolInvocation {
                session,
                turn,
                payload,
                call_id,
                ..
            } = invocation;
            let arguments = function_arguments(payload)?;
            let args: CloseAgentArgs = parse_arguments(&arguments)?;
            let agent_id = agent_id(&args.id)?;
            let (receiver_agent_nickname, receiver_agent_role) = session
                .services
                .agent_control
                .get_agent_nickname_and_role(agent_id)
                .await
                .unwrap_or((None, None));
            session
                .send_event(
                    &turn,
                    CollabCloseBeginEvent {
                        call_id: call_id.clone(),
                        sender_thread_id: session.conversation_id,
                        receiver_thread_id: agent_id,
                    }
                    .into(),
                )
                .await;
            let status = match session
                .services
                .agent_control
                .subscribe_status(agent_id)
                .await
            {
                Ok(mut status_rx) => status_rx.borrow_and_update().clone(),
                Err(err) => {
                    let status = session.services.agent_control.get_status(agent_id).await;
                    session
                        .send_event(
                            &turn,
                            CollabCloseEndEvent {
                                call_id: call_id.clone(),
                                sender_thread_id: session.conversation_id,
                                receiver_thread_id: agent_id,
                                receiver_agent_nickname: receiver_agent_nickname.clone(),
                                receiver_agent_role: receiver_agent_role.clone(),
                                status,
                            }
                            .into(),
                        )
                        .await;
                    return Err(collab_agent_error(agent_id, err));
                }
            };
            let result = if !matches!(status, AgentStatus::Shutdown) {
                session
                    .services
                    .agent_control
                    .shutdown_agent(agent_id)
                    .await
                    .map_err(|err| collab_agent_error(agent_id, err))
                    .map(|_| ())
            } else {
                Ok(())
            };
            session
                .send_event(
                    &turn,
                    CollabCloseEndEvent {
                        call_id,
                        sender_thread_id: session.conversation_id,
                        receiver_thread_id: agent_id,
                        receiver_agent_nickname,
                        receiver_agent_role,
                        status: status.clone(),
                    }
                    .into(),
                )
                .await;
            result?;

            Ok(CloseAgentResult { status })
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub(crate) struct CloseAgentResult {
        pub(crate) status: AgentStatus,
    }

    impl ToolOutput for CloseAgentResult {
        fn log_preview(&self) -> String {
            tool_output_json_text(self, "close_agent")
        }

        fn success_for_logging(&self) -> bool {
            true
        }

        fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
            tool_output_response_item(call_id, payload, self, Some(true), "close_agent")
        }

        fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
            tool_output_code_mode_result(self, "close_agent")
        }
    }
}

fn agent_id(id: &str) -> Result<ThreadId, FunctionCallError> {
    ThreadId::from_string(id)
        .map_err(|e| FunctionCallError::RespondToModel(format!("invalid agent id {id}: {e:?}")))
}

fn build_wait_agent_statuses(
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

fn collab_spawn_error(err: CodexErr) -> FunctionCallError {
    match err {
        CodexErr::UnsupportedOperation(_) => {
            FunctionCallError::RespondToModel("collab manager unavailable".to_string())
        }
        err => FunctionCallError::RespondToModel(format!("collab spawn failed: {err}")),
    }
}

fn collab_agent_error(agent_id: ThreadId, err: CodexErr) -> FunctionCallError {
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

fn thread_spawn_source(
    parent_thread_id: ThreadId,
    depth: i32,
    agent_role: Option<&str>,
) -> SessionSource {
    SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id,
        depth,
        agent_nickname: None,
        agent_role: agent_role.map(str::to_string),
    })
}

fn parse_collab_input(
    message: Option<String>,
    items: Option<Vec<UserInput>>,
) -> Result<Vec<UserInput>, FunctionCallError> {
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
            }])
        }
        (None, Some(items)) => {
            if items.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "Items can't be empty".to_string(),
                ));
            }
            Ok(items)
        }
    }
}

fn input_preview(items: &[UserInput]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|item| match item {
            UserInput::Text { text, .. } => text.clone(),
            UserInput::Image { .. } => "[image]".to_string(),
            UserInput::LocalImage { path } => format!("[local_image:{}]", path.display()),
            UserInput::Skill { name, path } => {
                format!("[skill:${name}]({})", path.display())
            }
            UserInput::Mention { name, path } => format!("[mention:${name}]({path})"),
            _ => "[input]".to_string(),
        })
        .collect();

    parts.join("\n")
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

fn build_agent_resume_config(
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
fn apply_spawn_agent_runtime_overrides(
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
    Ok(())
}

fn apply_spawn_agent_overrides(config: &mut Config, child_depth: i32) {
    if child_depth >= config.agent_max_depth {
        let _ = config.features.disable(Feature::SpawnCsv);
        let _ = config.features.disable(Feature::Collab);
    }
}

async fn apply_requested_spawn_agent_model_overrides(
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
#[path = "multi_agents_tests.rs"]
mod tests;
