use crate::agent::AgentStatus;
use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::Constrained;
use crate::error::CodexErr;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::AskForApproval;
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
use std::collections::HashMap;

pub struct MultiAgentHandler;

/// Minimum wait timeout to prevent tight polling loops from burning CPU.
pub(crate) const MIN_WAIT_TIMEOUT_MS: i64 = 10_000;
pub(crate) const DEFAULT_WAIT_TIMEOUT_MS: i64 = 30_000;
pub(crate) const MAX_WAIT_TIMEOUT_MS: i64 = 300_000;

#[derive(Debug, Deserialize)]
struct CloseAgentArgs {
    id: String,
}

#[async_trait]
impl ToolHandler for MultiAgentHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tool_name,
            payload,
            call_id,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "collab handler received unsupported payload".to_string(),
                ));
            }
        };

        match tool_name.as_str() {
            "spawn_agent" => spawn::handle(session, turn, call_id, arguments).await,
            "send_input" => send_input::handle(session, turn, call_id, arguments).await,
            "resume_agent" => resume_agent::handle(session, turn, call_id, arguments).await,
            "wait" => wait::handle(session, turn, call_id, arguments).await,
            "close_agent" => close_agent::handle(session, turn, call_id, arguments).await,
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported collab tool {other}"
            ))),
        }
    }
}

mod spawn {
    use super::*;
    use crate::agent::role::apply_role_to_config;

    use crate::agent::exceeds_thread_spawn_depth_limit;
    use crate::agent::next_thread_spawn_depth;
    use std::sync::Arc;

    #[derive(Debug, Deserialize)]
    struct SpawnAgentArgs {
        message: Option<String>,
        items: Option<Vec<UserInput>>,
        agent_type: Option<String>,
    }

    #[derive(Debug, Serialize)]
    struct SpawnAgentResult {
        agent_id: String,
    }

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
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
        if exceeds_thread_spawn_depth_limit(child_depth, turn.config.agent_max_depth) {
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
                }
                .into(),
            )
            .await;
        let mut config = build_agent_spawn_config(
            &session.get_base_instructions().await,
            turn.as_ref(),
            child_depth,
        )?;
        apply_role_to_config(&mut config, role_name)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        apply_spawn_agent_overrides(&mut config, child_depth);

        let result = session
            .services
            .agent_control
            .spawn_agent(
                config,
                input_items,
                Some(thread_spawn_source(
                    session.conversation_id,
                    child_depth,
                    role_name,
                )),
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
                    status,
                }
                .into(),
            )
            .await;
        let new_thread_id = result?;

        let content = serde_json::to_string(&SpawnAgentResult {
            agent_id: new_thread_id.to_string(),
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize spawn_agent result: {err}"))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

mod send_input {
    use super::*;
    use std::sync::Arc;

    #[derive(Debug, Deserialize)]
    struct SendInputArgs {
        id: String,
        message: Option<String>,
        items: Option<Vec<UserInput>>,
        #[serde(default)]
        interrupt: bool,
    }

    #[derive(Debug, Serialize)]
    struct SendInputResult {
        submission_id: String,
    }

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
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

        let content = serde_json::to_string(&SendInputResult { submission_id }).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize send_input result: {err}"))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

mod resume_agent {
    use super::*;
    use crate::agent::next_thread_spawn_depth;
    use std::sync::Arc;

    #[derive(Debug, Deserialize)]
    struct ResumeAgentArgs {
        id: String,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub(super) struct ResumeAgentResult {
        pub(super) status: AgentStatus,
    }

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: ResumeAgentArgs = parse_arguments(&arguments)?;
        let receiver_thread_id = agent_id(&args.id)?;
        let (receiver_agent_nickname, receiver_agent_role) = session
            .services
            .agent_control
            .get_agent_nickname_and_role(receiver_thread_id)
            .await
            .unwrap_or((None, None));
        let child_depth = next_thread_spawn_depth(&turn.session_source);
        if exceeds_thread_spawn_depth_limit(child_depth, turn.config.agent_max_depth) {
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
            // If the thread is no longer active, attempt to restore it from rollout.
            match try_resume_closed_agent(&session, &turn, receiver_thread_id, child_depth).await {
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

        let content = serde_json::to_string(&ResumeAgentResult { status }).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize resume_agent result: {err}"))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
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
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::watch::Receiver;
    use tokio::time::Instant;

    use tokio::time::timeout_at;

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

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
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

        // Validate timeout.
        // Very short timeouts encourage busy-polling loops in the orchestrator prompt and can
        // cause high CPU usage even with a single active worker, so clamp to a minimum.
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
            // Wait for the first agent to reach a final status.
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
                // Drain the unlikely last elements to prevent race.
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

        // Convert payload.
        let statuses_map = statuses.clone().into_iter().collect::<HashMap<_, _>>();
        let agent_statuses = build_wait_agent_statuses(&statuses_map, &receiver_agents);
        let result = WaitResult {
            status: statuses_map.clone(),
            timed_out: statuses.is_empty(),
        };

        // Final event emission.
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

        let content = serde_json::to_string(&result).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize wait result: {err}"))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: None,
        })
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
    use std::sync::Arc;

    #[derive(Debug, Deserialize, Serialize)]
    pub(super) struct CloseAgentResult {
        pub(super) status: AgentStatus,
    }

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
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

        let content = serde_json::to_string(&CloseAgentResult { status }).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize close_agent result: {err}"))
        })?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
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

fn build_agent_spawn_config(
    base_instructions: &BaseInstructions,
    turn: &TurnContext,
    child_depth: i32,
) -> Result<Config, FunctionCallError> {
    let mut config = build_agent_shared_config(turn, child_depth)?;
    config.base_instructions = Some(base_instructions.text.clone());
    Ok(config)
}

fn build_agent_resume_config(
    turn: &TurnContext,
    child_depth: i32,
) -> Result<Config, FunctionCallError> {
    let mut config = build_agent_shared_config(turn, child_depth)?;
    // For resume, keep base instructions sourced from rollout/session metadata.
    config.base_instructions = None;
    Ok(config)
}

fn build_agent_shared_config(
    turn: &TurnContext,
    child_depth: i32,
) -> Result<Config, FunctionCallError> {
    let base_config = turn.config.clone();
    let mut config = (*base_config).clone();
    config.model = Some(turn.model_info.slug.clone());
    config.model_provider = turn.provider.clone();
    config.model_reasoning_effort = turn.reasoning_effort;
    config.model_reasoning_summary = turn.reasoning_summary;
    config.developer_instructions = turn.developer_instructions.clone();
    config.compact_prompt = turn.compact_prompt.clone();
    config.permissions.shell_environment_policy = turn.shell_environment_policy.clone();
    config.codex_linux_sandbox_exe = turn.codex_linux_sandbox_exe.clone();
    config.cwd = turn.cwd.clone();
    config
        .permissions
        .sandbox_policy
        .set(turn.sandbox_policy.clone())
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("sandbox_policy is invalid: {err}"))
        })?;
    apply_spawn_agent_overrides(&mut config, child_depth);

    Ok(config)
}

fn apply_spawn_agent_overrides(config: &mut Config, child_depth: i32) {
    config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    if exceeds_thread_spawn_depth_limit(child_depth + 1, config.agent_max_depth) {
        config.features.disable(Feature::Collab);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthManager;
    use crate::CodexAuth;
    use crate::ThreadManager;
    use crate::built_in_model_providers;
    use crate::codex::make_session_and_context;
    use crate::config::DEFAULT_AGENT_MAX_DEPTH;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::function_tool::FunctionCallError;
    use crate::protocol::AskForApproval;
    use crate::protocol::Op;
    use crate::protocol::SandboxPolicy;
    use crate::protocol::SessionSource;
    use crate::protocol::SubAgentSource;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::ThreadId;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::InitialHistory;
    use codex_protocol::protocol::RolloutItem;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::time::timeout;

    fn invocation(
        session: Arc<crate::codex::Session>,
        turn: Arc<TurnContext>,
        tool_name: &str,
        payload: ToolPayload,
    ) -> ToolInvocation {
        ToolInvocation {
            session,
            turn,
            tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
            call_id: "call-1".to_string(),
            tool_name: tool_name.to_string(),
            payload,
        }
    }

    fn function_payload(args: serde_json::Value) -> ToolPayload {
        ToolPayload::Function {
            arguments: args.to_string(),
        }
    }

    fn thread_manager() -> ThreadManager {
        ThreadManager::with_models_provider_for_tests(
            CodexAuth::from_api_key("dummy"),
            built_in_model_providers()["openai"].clone(),
        )
    }

    #[tokio::test]
    async fn handler_rejects_non_function_payloads() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            ToolPayload::Custom {
                input: "hello".to_string(),
            },
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("payload should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "collab handler received unsupported payload".to_string()
            )
        );
    }

    #[tokio::test]
    async fn handler_rejects_unknown_tool() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "unknown_tool",
            function_payload(json!({})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("tool should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel("unsupported collab tool unknown_tool".to_string())
        );
    }

    #[tokio::test]
    async fn spawn_agent_rejects_empty_message() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            function_payload(json!({"message": "   "})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("empty message should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "Empty message can't be sent to an agent".to_string()
            )
        );
    }

    #[tokio::test]
    async fn spawn_agent_rejects_when_message_and_items_are_both_set() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            function_payload(json!({
                "message": "hello",
                "items": [{"type": "mention", "name": "drive", "path": "app://drive"}]
            })),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("message+items should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "Provide either message or items, but not both".to_string()
            )
        );
    }

    #[tokio::test]
    #[ignore = "No role requiring it for now"]
    async fn spawn_agent_uses_explorer_role_and_sets_never_approval_policy() {
        #[derive(Debug, Deserialize)]
        struct SpawnAgentResult {
            agent_id: String,
        }

        let (mut session, mut turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let mut config = (*turn.config).clone();
        config
            .permissions
            .approval_policy
            .set(AskForApproval::OnRequest)
            .expect("approval policy should be set");
        turn.config = Arc::new(config);

        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            function_payload(json!({
                "message": "inspect this repo",
                "agent_type": "explorer"
            })),
        );
        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("spawn_agent should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: SpawnAgentResult =
            serde_json::from_str(&content).expect("spawn_agent result should be json");
        let agent_id = agent_id(&result.agent_id).expect("agent_id should be valid");
        let snapshot = manager
            .get_thread(agent_id)
            .await
            .expect("spawned agent thread should exist")
            .config_snapshot()
            .await;
        assert_eq!(snapshot.model, "gpt-5.1-codex-mini");
        assert_eq!(snapshot.approval_policy, AskForApproval::Never);
    }

    #[tokio::test]
    async fn spawn_agent_errors_when_manager_dropped() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            function_payload(json!({"message": "hello"})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("spawn should fail without a manager");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel("collab manager unavailable".to_string())
        );
    }

    #[tokio::test]
    async fn spawn_agent_rejects_when_depth_limit_exceeded() {
        let (mut session, mut turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();

        turn.session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: session.conversation_id,
            depth: DEFAULT_AGENT_MAX_DEPTH,
            agent_nickname: None,
            agent_role: None,
        });

        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            function_payload(json!({"message": "hello"})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("spawn should fail when depth limit exceeded");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "Agent depth limit reached. Solve the task yourself.".to_string()
            )
        );
    }

    #[tokio::test]
    async fn spawn_agent_allows_depth_up_to_configured_max_depth() {
        #[derive(Debug, Deserialize)]
        struct SpawnAgentResult {
            agent_id: String,
        }

        let (mut session, mut turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();

        let mut config = (*turn.config).clone();
        config.agent_max_depth = DEFAULT_AGENT_MAX_DEPTH + 1;
        turn.config = Arc::new(config);
        turn.session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: session.conversation_id,
            depth: DEFAULT_AGENT_MAX_DEPTH,
            agent_nickname: None,
            agent_role: None,
        });

        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "spawn_agent",
            function_payload(json!({"message": "hello"})),
        );
        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("spawn should succeed within configured depth");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: SpawnAgentResult =
            serde_json::from_str(&content).expect("spawn_agent result should be json");
        assert!(!result.agent_id.is_empty());
        assert_eq!(success, Some(true));
    }

    #[tokio::test]
    async fn send_input_rejects_empty_message() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "send_input",
            function_payload(json!({"id": ThreadId::new().to_string(), "message": ""})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("empty message should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "Empty message can't be sent to an agent".to_string()
            )
        );
    }

    #[tokio::test]
    async fn send_input_rejects_when_message_and_items_are_both_set() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "send_input",
            function_payload(json!({
                "id": ThreadId::new().to_string(),
                "message": "hello",
                "items": [{"type": "mention", "name": "drive", "path": "app://drive"}]
            })),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("message+items should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "Provide either message or items, but not both".to_string()
            )
        );
    }

    #[tokio::test]
    async fn send_input_rejects_invalid_id() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "send_input",
            function_payload(json!({"id": "not-a-uuid", "message": "hi"})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("invalid id should be rejected");
        };
        let FunctionCallError::RespondToModel(msg) = err else {
            panic!("expected respond-to-model error");
        };
        assert!(msg.starts_with("invalid agent id not-a-uuid:"));
    }

    #[tokio::test]
    async fn send_input_reports_missing_agent() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let agent_id = ThreadId::new();
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "send_input",
            function_payload(json!({"id": agent_id.to_string(), "message": "hi"})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("missing agent should be reported");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(format!("agent with id {agent_id} not found"))
        );
    }

    #[tokio::test]
    async fn send_input_interrupts_before_prompt() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "send_input",
            function_payload(json!({
                "id": agent_id.to_string(),
                "message": "hi",
                "interrupt": true
            })),
        );
        MultiAgentHandler
            .handle(invocation)
            .await
            .expect("send_input should succeed");

        let ops = manager.captured_ops();
        let ops_for_agent: Vec<&Op> = ops
            .iter()
            .filter_map(|(id, op)| (*id == agent_id).then_some(op))
            .collect();
        assert_eq!(ops_for_agent.len(), 2);
        assert!(matches!(ops_for_agent[0], Op::Interrupt));
        assert!(matches!(ops_for_agent[1], Op::UserInput { .. }));

        let _ = thread
            .thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");
    }

    #[tokio::test]
    async fn send_input_accepts_structured_items() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "send_input",
            function_payload(json!({
                "id": agent_id.to_string(),
                "items": [
                    {"type": "mention", "name": "drive", "path": "app://google_drive"},
                    {"type": "text", "text": "read the folder"}
                ]
            })),
        );
        MultiAgentHandler
            .handle(invocation)
            .await
            .expect("send_input should succeed");

        let expected = Op::UserInput {
            items: vec![
                UserInput::Mention {
                    name: "drive".to_string(),
                    path: "app://google_drive".to_string(),
                },
                UserInput::Text {
                    text: "read the folder".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            final_output_json_schema: None,
        };
        let captured = manager
            .captured_ops()
            .into_iter()
            .find(|(id, op)| *id == agent_id && *op == expected);
        assert_eq!(captured, Some((agent_id, expected)));

        let _ = thread
            .thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");
    }

    #[tokio::test]
    async fn resume_agent_rejects_invalid_id() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "resume_agent",
            function_payload(json!({"id": "not-a-uuid"})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("invalid id should be rejected");
        };
        let FunctionCallError::RespondToModel(msg) = err else {
            panic!("expected respond-to-model error");
        };
        assert!(msg.starts_with("invalid agent id not-a-uuid:"));
    }

    #[tokio::test]
    async fn resume_agent_reports_missing_agent() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let agent_id = ThreadId::new();
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "resume_agent",
            function_payload(json!({"id": agent_id.to_string()})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("missing agent should be reported");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(format!("agent with id {agent_id} not found"))
        );
    }

    #[tokio::test]
    async fn resume_agent_noops_for_active_agent() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let status_before = manager.agent_control().get_status(agent_id).await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "resume_agent",
            function_payload(json!({"id": agent_id.to_string()})),
        );

        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("resume_agent should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: resume_agent::ResumeAgentResult =
            serde_json::from_str(&content).expect("resume_agent result should be json");
        assert_eq!(result.status, status_before);
        assert_eq!(success, Some(true));

        let thread_ids = manager.list_thread_ids().await;
        assert_eq!(thread_ids, vec![agent_id]);

        let _ = thread
            .thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");
    }

    #[tokio::test]
    async fn resume_agent_restores_closed_agent_and_accepts_send_input() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager
            .resume_thread_with_history(
                config,
                InitialHistory::Forked(vec![RolloutItem::ResponseItem(ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "materialized".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                })]),
                AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy")),
                false,
            )
            .await
            .expect("start thread");
        let agent_id = thread.thread_id;
        let _ = manager
            .agent_control()
            .shutdown_agent(agent_id)
            .await
            .expect("shutdown agent");
        assert_eq!(
            manager.agent_control().get_status(agent_id).await,
            AgentStatus::NotFound
        );
        let session = Arc::new(session);
        let turn = Arc::new(turn);

        let resume_invocation = invocation(
            session.clone(),
            turn.clone(),
            "resume_agent",
            function_payload(json!({"id": agent_id.to_string()})),
        );
        let output = MultiAgentHandler
            .handle(resume_invocation)
            .await
            .expect("resume_agent should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: resume_agent::ResumeAgentResult =
            serde_json::from_str(&content).expect("resume_agent result should be json");
        assert_ne!(result.status, AgentStatus::NotFound);
        assert_eq!(success, Some(true));

        let send_invocation = invocation(
            session,
            turn,
            "send_input",
            function_payload(json!({"id": agent_id.to_string(), "message": "hello"})),
        );
        let output = MultiAgentHandler
            .handle(send_invocation)
            .await
            .expect("send_input should succeed after resume");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: serde_json::Value =
            serde_json::from_str(&content).expect("send_input result should be json");
        let submission_id = result
            .get("submission_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert!(!submission_id.is_empty());
        assert_eq!(success, Some(true));

        let _ = manager
            .agent_control()
            .shutdown_agent(agent_id)
            .await
            .expect("shutdown resumed agent");
    }

    #[tokio::test]
    async fn resume_agent_rejects_when_depth_limit_exceeded() {
        let (mut session, mut turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();

        turn.session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: session.conversation_id,
            depth: DEFAULT_AGENT_MAX_DEPTH,
            agent_nickname: None,
            agent_role: None,
        });

        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "resume_agent",
            function_payload(json!({"id": ThreadId::new().to_string()})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("resume should fail when depth limit exceeded");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "Agent depth limit reached. Solve the task yourself.".to_string()
            )
        );
    }

    #[tokio::test]
    async fn wait_rejects_non_positive_timeout() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({
                "ids": [ThreadId::new().to_string()],
                "timeout_ms": 0
            })),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("non-positive timeout should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel("timeout_ms must be greater than zero".to_string())
        );
    }

    #[tokio::test]
    async fn wait_rejects_invalid_id() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({"ids": ["invalid"]})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("invalid id should be rejected");
        };
        let FunctionCallError::RespondToModel(msg) = err else {
            panic!("expected respond-to-model error");
        };
        assert!(msg.starts_with("invalid agent id invalid:"));
    }

    #[tokio::test]
    async fn wait_rejects_empty_ids() {
        let (session, turn) = make_session_and_context().await;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({"ids": []})),
        );
        let Err(err) = MultiAgentHandler.handle(invocation).await else {
            panic!("empty ids should be rejected");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel("ids must be non-empty".to_string())
        );
    }

    #[tokio::test]
    async fn wait_returns_not_found_for_missing_agents() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let id_a = ThreadId::new();
        let id_b = ThreadId::new();
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({
                "ids": [id_a.to_string(), id_b.to_string()],
                "timeout_ms": 1000
            })),
        );
        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("wait should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: wait::WaitResult =
            serde_json::from_str(&content).expect("wait result should be json");
        assert_eq!(
            result,
            wait::WaitResult {
                status: HashMap::from([
                    (id_a, AgentStatus::NotFound),
                    (id_b, AgentStatus::NotFound),
                ]),
                timed_out: false
            }
        );
        assert_eq!(success, None);
    }

    #[tokio::test]
    async fn wait_times_out_when_status_is_not_final() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({
                "ids": [agent_id.to_string()],
                "timeout_ms": MIN_WAIT_TIMEOUT_MS
            })),
        );
        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("wait should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: wait::WaitResult =
            serde_json::from_str(&content).expect("wait result should be json");
        assert_eq!(
            result,
            wait::WaitResult {
                status: HashMap::new(),
                timed_out: true
            }
        );
        assert_eq!(success, None);

        let _ = thread
            .thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");
    }

    #[tokio::test]
    async fn wait_clamps_short_timeouts_to_minimum() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({
                "ids": [agent_id.to_string()],
                "timeout_ms": 10
            })),
        );

        let early = timeout(
            Duration::from_millis(50),
            MultiAgentHandler.handle(invocation),
        )
        .await;
        assert!(
            early.is_err(),
            "wait should not return before the minimum timeout clamp"
        );

        let _ = thread
            .thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");
    }

    #[tokio::test]
    async fn wait_returns_final_status_without_timeout() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let mut status_rx = manager
            .agent_control()
            .subscribe_status(agent_id)
            .await
            .expect("subscribe should succeed");

        let _ = thread
            .thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");
        let _ = timeout(Duration::from_secs(1), status_rx.changed())
            .await
            .expect("shutdown status should arrive");

        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "wait",
            function_payload(json!({
                "ids": [agent_id.to_string()],
                "timeout_ms": 1000
            })),
        );
        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("wait should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: wait::WaitResult =
            serde_json::from_str(&content).expect("wait result should be json");
        assert_eq!(
            result,
            wait::WaitResult {
                status: HashMap::from([(agent_id, AgentStatus::Shutdown)]),
                timed_out: false
            }
        );
        assert_eq!(success, None);
    }

    #[tokio::test]
    async fn close_agent_submits_shutdown_and_returns_status() {
        let (mut session, turn) = make_session_and_context().await;
        let manager = thread_manager();
        session.services.agent_control = manager.agent_control();
        let config = turn.config.as_ref().clone();
        let thread = manager.start_thread(config).await.expect("start thread");
        let agent_id = thread.thread_id;
        let status_before = manager.agent_control().get_status(agent_id).await;

        let invocation = invocation(
            Arc::new(session),
            Arc::new(turn),
            "close_agent",
            function_payload(json!({"id": agent_id.to_string()})),
        );
        let output = MultiAgentHandler
            .handle(invocation)
            .await
            .expect("close_agent should succeed");
        let ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success,
            ..
        } = output
        else {
            panic!("expected function output");
        };
        let result: close_agent::CloseAgentResult =
            serde_json::from_str(&content).expect("close_agent result should be json");
        assert_eq!(result.status, status_before);
        assert_eq!(success, Some(true));

        let ops = manager.captured_ops();
        let submitted_shutdown = ops
            .iter()
            .any(|(id, op)| *id == agent_id && matches!(op, Op::Shutdown));
        assert_eq!(submitted_shutdown, true);

        let status_after = manager.agent_control().get_status(agent_id).await;
        assert_eq!(status_after, AgentStatus::NotFound);
    }

    #[tokio::test]
    async fn build_agent_spawn_config_uses_turn_context_values() {
        fn pick_allowed_sandbox_policy(
            constraint: &crate::config::Constrained<SandboxPolicy>,
            base: SandboxPolicy,
        ) -> SandboxPolicy {
            let candidates = [
                SandboxPolicy::new_read_only_policy(),
                SandboxPolicy::new_workspace_write_policy(),
                SandboxPolicy::DangerFullAccess,
            ];
            candidates
                .into_iter()
                .find(|candidate| *candidate != base && constraint.can_set(candidate).is_ok())
                .unwrap_or(base)
        }

        let (_session, mut turn) = make_session_and_context().await;
        let base_instructions = BaseInstructions {
            text: "base".to_string(),
        };
        turn.developer_instructions = Some("dev".to_string());
        turn.compact_prompt = Some("compact".to_string());
        turn.shell_environment_policy = ShellEnvironmentPolicy {
            use_profile: true,
            ..ShellEnvironmentPolicy::default()
        };
        let temp_dir = tempfile::tempdir().expect("temp dir");
        turn.cwd = temp_dir.path().to_path_buf();
        turn.codex_linux_sandbox_exe = Some(PathBuf::from("/bin/echo"));
        turn.sandbox_policy = pick_allowed_sandbox_policy(
            &turn.config.permissions.sandbox_policy,
            turn.config.permissions.sandbox_policy.get().clone(),
        );

        let config = build_agent_spawn_config(&base_instructions, &turn, 0).expect("spawn config");
        let mut expected = (*turn.config).clone();
        expected.base_instructions = Some(base_instructions.text);
        expected.model = Some(turn.model_info.slug.clone());
        expected.model_provider = turn.provider.clone();
        expected.model_reasoning_effort = turn.reasoning_effort;
        expected.model_reasoning_summary = turn.reasoning_summary;
        expected.developer_instructions = turn.developer_instructions.clone();
        expected.compact_prompt = turn.compact_prompt.clone();
        expected.permissions.shell_environment_policy = turn.shell_environment_policy.clone();
        expected.codex_linux_sandbox_exe = turn.codex_linux_sandbox_exe.clone();
        expected.cwd = turn.cwd.clone();
        expected
            .permissions
            .approval_policy
            .set(AskForApproval::Never)
            .expect("approval policy set");
        expected
            .permissions
            .sandbox_policy
            .set(turn.sandbox_policy)
            .expect("sandbox policy set");
        assert_eq!(config, expected);
    }

    #[tokio::test]
    async fn build_agent_spawn_config_preserves_base_user_instructions() {
        let (_session, mut turn) = make_session_and_context().await;
        let mut base_config = (*turn.config).clone();
        base_config.user_instructions = Some("base-user".to_string());
        turn.user_instructions = Some("resolved-user".to_string());
        turn.config = Arc::new(base_config.clone());
        let base_instructions = BaseInstructions {
            text: "base".to_string(),
        };

        let config = build_agent_spawn_config(&base_instructions, &turn, 0).expect("spawn config");

        assert_eq!(config.user_instructions, base_config.user_instructions);
    }

    #[tokio::test]
    async fn build_agent_resume_config_clears_base_instructions() {
        let (_session, mut turn) = make_session_and_context().await;
        let mut base_config = (*turn.config).clone();
        base_config.base_instructions = Some("caller-base".to_string());
        turn.config = Arc::new(base_config);

        let config = build_agent_resume_config(&turn, 0).expect("resume config");

        let mut expected = (*turn.config).clone();
        expected.base_instructions = None;
        expected.model = Some(turn.model_info.slug.clone());
        expected.model_provider = turn.provider.clone();
        expected.model_reasoning_effort = turn.reasoning_effort;
        expected.model_reasoning_summary = turn.reasoning_summary;
        expected.developer_instructions = turn.developer_instructions.clone();
        expected.compact_prompt = turn.compact_prompt.clone();
        expected.permissions.shell_environment_policy = turn.shell_environment_policy.clone();
        expected.codex_linux_sandbox_exe = turn.codex_linux_sandbox_exe.clone();
        expected.cwd = turn.cwd.clone();
        expected
            .permissions
            .approval_policy
            .set(AskForApproval::Never)
            .expect("approval policy set");
        expected
            .permissions
            .sandbox_policy
            .set(turn.sandbox_policy)
            .expect("sandbox policy set");
        assert_eq!(config, expected);
    }
}
