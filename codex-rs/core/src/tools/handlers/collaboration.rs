use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::context_manager::ContextManager;
use crate::environment_context::EnvironmentContext;
use crate::function_tool::FunctionCallError;
use crate::protocol::SandboxPolicy;
use crate::state::AgentId;
use crate::state::AgentLifecycleState;
use crate::state::AgentState as InternalAgentState;
use crate::state::ContextStrategy;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::spec::JsonSchema;
use crate::user_instructions::DeveloperInstructions;
use crate::user_instructions::SkillInstructions;
use crate::user_instructions::UserInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use std::time::Duration;
use std::time::Instant;
use tracing::info;

type AgenticError = String;
type ApprovalRequest = String;

#[derive(Debug, Deserialize)]
struct CollaborationInitAgentInput {
    #[serde(default)]
    agent_idx: String,
    #[serde(default)]
    agent: String,
    #[serde(default)]
    context_strategy: Option<ContextStrategy>,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationInitAgentOutput {
    result: Result<(), AgenticError>,
}

#[derive(Debug, Deserialize)]
struct CollaborationSendInput {
    recipients: Vec<String>,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationSendOutput {
    content: Result<(), AgenticError>,
}

#[derive(Debug, Deserialize)]
struct CollaborationWaitInput {
    max_duration: u32,
    #[serde(default)]
    agent_idx: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationWaitOutput {
    states: Vec<AgentState>,
}

#[derive(Debug, Deserialize)]
struct CollaborationGetStateInput {
    #[serde(default)]
    agent_idx: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationGetStateOutput {
    states: Vec<AgentState>,
}

#[derive(Debug, Deserialize)]
struct CollaborationCloseInput {
    #[serde(default)]
    return_states: bool,
    #[serde(default)]
    agent_idx: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationCloseOutput {
    states: Vec<AgentState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentState {
    agent_id: String,
    state: State,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Running,
    WaitingForApproval(ApprovalRequest),
    Done(String),
    Error(String),
}

fn serialize_function_output<T: Serialize>(
    output: &T,
    success: bool,
) -> Result<ToolOutput, FunctionCallError> {
    let content = serde_json::to_string(output).map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize collaboration output: {err:?}"))
    })?;
    Ok(ToolOutput::Function {
        content,
        content_items: None,
        success: Some(success),
    })
}

fn parse_agent_id(raw: &str) -> Result<AgentId, AgenticError> {
    let trimmed = raw.trim();
    let parsed: i32 = trimmed
        .parse()
        .map_err(|_| format!("invalid agent_idx {raw}"))?;
    if parsed < 0 {
        return Err(format!("invalid agent_idx {raw}"));
    }
    Ok(AgentId(parsed))
}

fn state_from_lifecycle(status: &AgentLifecycleState) -> State {
    match status {
        AgentLifecycleState::Running => State::Running,
        AgentLifecycleState::WaitingForApproval { request } => {
            State::WaitingForApproval(request.clone())
        }
        AgentLifecycleState::Error { error } => State::Error(error.clone()),
        AgentLifecycleState::Exhausted => State::Done("exhausted".to_string()),
        AgentLifecycleState::Closed => State::Done("closed".to_string()),
        AgentLifecycleState::Idle { last_agent_message } => State::Done(
            last_agent_message
                .clone()
                .unwrap_or_else(|| "idle".to_string()),
        ),
    }
}

fn snapshot_agent_state(agent: &InternalAgentState) -> AgentState {
    AgentState {
        agent_id: agent.id.0.to_string(),
        state: state_from_lifecycle(&agent.status),
    }
}

fn build_agent_system_message(
    agent_id: AgentId,
    agent_name: &str,
    parent_id: AgentId,
    depth: i32,
) -> ResponseItem {
    let lines = [format!(
        "You are agent {agent_id} ({agent_name}) (parent {parent_id}, depth {depth}).",
        agent_id = agent_id.0,
        parent_id = parent_id.0
    )];

    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: lines.join("\n"),
        }],
    }
}

fn strip_prompt_items(items: &mut Vec<ResponseItem>) {
    items.retain(|item| match item {
        ResponseItem::Message { role, content, .. } => {
            if role == "developer" {
                return false;
            }
            if role == "user" {
                if UserInstructions::is_user_instructions(content)
                    || SkillInstructions::is_skill_instructions(content)
                {
                    return false;
                }
                if let [ContentItem::InputText { text }] = content.as_slice()
                    && text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG)
                {
                    return false;
                }
            }
            true
        }
        _ => true,
    });
}

fn build_history_for_child(
    session: &crate::codex::Session,
    strategy: ContextStrategy,
    parent: &InternalAgentState,
    child_config: &crate::codex::SessionConfiguration,
    system_message: ResponseItem,
) -> ContextManager {
    let (mut items, token_info) = match strategy {
        ContextStrategy::New => (Vec::new(), None),
        ContextStrategy::Fork => {
            let mut parent_history = parent.history.clone();
            (
                parent_history.get_history_for_prompt(),
                parent.history.token_info(),
            )
        }
        ContextStrategy::Replace(history) => (history, None),
    };

    strip_prompt_items(&mut items);

    let mut merged = Vec::with_capacity(items.len() + 3);
    if let Some(prompt) = child_config.developer_instructions() {
        merged.push(DeveloperInstructions::new(prompt).into());
    }
    merged.push(ResponseItem::from(EnvironmentContext::new(
        Some(child_config.cwd().clone()),
        Some(child_config.approval_policy()),
        Some(child_config.sandbox_policy()),
        session.user_shell().as_ref().clone(),
    )));
    merged.push(system_message);
    merged.append(&mut items);

    let mut history = ContextManager::new();
    history.replace(merged);
    history.set_token_info(token_info);
    history
}

pub struct CollaborationHandler;

impl Default for CollaborationHandler {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl ToolHandler for CollaborationHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker: _,
            call_id: _,
            tool_name,
            payload,
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "collaboration handler received unsupported payload".to_string(),
                ));
            }
        };

        match tool_name.as_str() {
            "collaboration_init_agent" => handle_init_agent(session, &turn, &arguments).await,
            "collaboration_send" => handle_send(session, &turn, &arguments).await,
            "collaboration_wait" => handle_wait(session, &turn, &arguments).await,
            "collaboration_get_state" => handle_get_state(session, &turn, &arguments).await,
            "collaboration_close" => handle_close(session, &turn, &arguments).await,
            _ => Err(FunctionCallError::RespondToModel(format!(
                "unknown collaboration tool: {tool_name}"
            ))),
        }
    }
}

async fn handle_init_agent(
    session: Arc<crate::codex::Session>,
    turn: &crate::codex::TurnContext,
    arguments: &str,
) -> Result<ToolOutput, FunctionCallError> {
    let input: CollaborationInitAgentInput = serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err:?}"))
    })?;

    let Some(agents_config) = session.agents_config() else {
        let output = CollaborationInitAgentOutput {
            result: Err("collaboration agents are not configured".to_string()),
        };
        return serialize_function_output(&output, false);
    };

    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    let mut collab = session.collaboration_state().lock().await;
    collab.ensure_root_agent(&session_configuration, &session_history);
    let caller_id = turn.collaboration_agent();
    let parent_id = if input.agent_idx.trim().is_empty() {
        caller_id
    } else {
        match parse_agent_id(&input.agent_idx) {
            Ok(agent_id) => agent_id,
            Err(err) => {
                let output = CollaborationInitAgentOutput { result: Err(err) };
                return serialize_function_output(&output, false);
            }
        }
    };
    if parent_id != caller_id {
        let output = CollaborationInitAgentOutput {
            result: Err("agent_idx must match the calling agent".to_string()),
        };
        return serialize_function_output(&output, false);
    }
    let Some(parent) = collab.agent(parent_id).cloned() else {
        let output = CollaborationInitAgentOutput {
            result: Err(format!(
                "unknown caller agent {parent_id}",
                parent_id = parent_id.0
            )),
        };
        return serialize_function_output(&output, false);
    };

    let depth = parent.depth + 1;
    let limits = *collab.limits();

    if collab.agents().len() as i32 >= limits.max_agents {
        let output = CollaborationInitAgentOutput {
            result: Err("max agent count reached".to_string()),
        };
        return serialize_function_output(&output, false);
    }

    if depth > limits.max_depth {
        let output = CollaborationInitAgentOutput {
            result: Err("max collaboration depth reached".to_string()),
        };
        return serialize_function_output(&output, false);
    }

    let Some(parent_definition) = agents_config.agent(parent.name.as_str()) else {
        let output = CollaborationInitAgentOutput {
            result: Err(format!(
                "unknown parent agent type {parent_name}",
                parent_name = parent.name
            )),
        };
        return serialize_function_output(&output, false);
    };

    let agent_name = if input.agent.trim().is_empty() {
        parent.name.clone()
    } else {
        input.agent.clone()
    };
    if !parent_definition.sub_agents.contains(&agent_name) {
        let parent_name = parent_definition.name.as_str();
        let output = CollaborationInitAgentOutput {
            result: Err(format!(
                "agent {parent_name} cannot spawn agent {agent_name}"
            )),
        };
        return serialize_function_output(&output, false);
    }

    let Some(agent_definition) = agents_config.agent(agent_name.as_str()) else {
        let output = CollaborationInitAgentOutput {
            result: Err(format!("unknown agent {agent_name}")),
        };
        return serialize_function_output(&output, false);
    };

    let model = agent_definition
        .model
        .clone()
        .unwrap_or_else(|| session_configuration.model().to_string());
    let sandbox_policy = if agent_definition.read_only {
        SandboxPolicy::ReadOnly
    } else {
        session.default_sandbox_policy().clone()
    };

    let mut update = crate::codex::SessionSettingsUpdate {
        sandbox_policy: Some(sandbox_policy),
        model: Some(model),
        ..Default::default()
    };
    if let Some(effort) = agent_definition.reasoning_effort {
        update.reasoning_effort = Some(Some(effort));
    }
    let child_configuration = session_configuration
        .clone()
        .apply(&update)
        .with_instructions(agent_definition.instructions.clone(), None);

    let assigned_id = collab.next_agent_id();
    let system_message =
        build_agent_system_message(assigned_id, agent_name.as_str(), parent.id, depth);
    let history = build_history_for_child(
        session.as_ref(),
        input.context_strategy.unwrap_or_default(),
        &parent,
        &child_configuration,
        system_message,
    );

    let child = InternalAgentState::new_child(
        assigned_id,
        agent_name.clone(),
        parent.id,
        depth,
        child_configuration,
        agent_definition.instructions.clone(),
        history,
    );

    let assigned_id = collab
        .add_child(child)
        .map_err(FunctionCallError::RespondToModel)?;
    let has_message = !input.message.trim().is_empty();
    if has_message {
        let text = format!(
            "From agent {parent_id}: {message}",
            parent_id = parent_id.0,
            message = input.message
        );
        collab.record_message_for_agent(
            assigned_id,
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text }],
            },
        );
        if let Some(agent) = collab.agent_mut(assigned_id) {
            agent.status = AgentLifecycleState::Running;
        }
    }
    drop(collab);

    if has_message {
        let supervisor = session.ensure_collaboration_supervisor().await;
        if let Err(err) = supervisor.start_agents(vec![assigned_id], i32::MAX).await {
            let mut collab = session.collaboration_state().lock().await;
            if let Some(agent) = collab.agent_mut(assigned_id)
                && matches!(&agent.status, AgentLifecycleState::Running)
            {
                agent.status = AgentLifecycleState::Idle {
                    last_agent_message: None,
                };
            }
            let output = CollaborationInitAgentOutput {
                result: Err(format!("failed to start child agent: {err}")),
            };
            return serialize_function_output(&output, false);
        }
    }

    let output = CollaborationInitAgentOutput { result: Ok(()) };
    info!(
        "collaboration_init_agent: assigned_id={}, parent_id={}, depth={}",
        assigned_id.0, parent.id.0, depth
    );
    serialize_function_output(&output, true)
}

async fn handle_send(
    session: Arc<crate::codex::Session>,
    turn: &crate::codex::TurnContext,
    arguments: &str,
) -> Result<ToolOutput, FunctionCallError> {
    let input: CollaborationSendInput = serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err:?}"))
    })?;

    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    let sender_id = turn.collaboration_agent();

    let (
        sender_name,
        valid_recipients,
        invalid_recipients,
        busy_recipients,
        direct_children,
        previous_statuses,
    ) = {
        let mut collab = session.collaboration_state().lock().await;
        collab.ensure_root_agent(&session_configuration, &session_history);

        let Some(sender_name) = collab.agent(sender_id).map(|agent| agent.name.clone()) else {
            let output = CollaborationSendOutput {
                content: Err(format!(
                    "unknown caller agent {sender_id}",
                    sender_id = sender_id.0
                )),
            };
            return serialize_function_output(&output, false);
        };

        let direct_children = collab
            .agents()
            .iter()
            .filter(|agent| agent.parent == Some(sender_id))
            .map(|agent| format!("{} ({})", agent.id.0, agent.name))
            .collect::<Vec<_>>();

        let mut invalid_recipients = Vec::new();
        let mut busy_recipients = Vec::new();
        let mut valid_recipients = Vec::new();
        let mut previous_statuses = Vec::new();

        for raw in &input.recipients {
            let candidate = match parse_agent_id(raw) {
                Ok(candidate) => candidate,
                Err(_) => {
                    invalid_recipients.push(raw.clone());
                    continue;
                }
            };
            if let Some(agent) = collab.agent(candidate)
                && collab.is_direct_child(sender_id, candidate)
            {
                if matches!(
                    &agent.status,
                    AgentLifecycleState::Closed
                        | AgentLifecycleState::Exhausted
                        | AgentLifecycleState::Error { .. }
                ) {
                    invalid_recipients.push(raw.clone());
                    continue;
                }

                if matches!(
                    &agent.status,
                    AgentLifecycleState::Running | AgentLifecycleState::WaitingForApproval { .. }
                ) {
                    let status = match &agent.status {
                        AgentLifecycleState::Running => "running",
                        AgentLifecycleState::WaitingForApproval { .. } => "waiting_for_approval",
                        _ => "unknown",
                    };
                    busy_recipients.push(format!(
                        "{} ({}) status={status}",
                        candidate.0,
                        agent.name.as_str()
                    ));
                    continue;
                }

                valid_recipients.push(candidate);
            } else {
                invalid_recipients.push(raw.clone());
            }
        }

        if input.recipients.is_empty()
            || !invalid_recipients.is_empty()
            || !busy_recipients.is_empty()
            || valid_recipients.is_empty()
        {
            (
                sender_name,
                valid_recipients,
                invalid_recipients,
                busy_recipients,
                direct_children,
                previous_statuses,
            )
        } else {
            for recipient in &valid_recipients {
                let text = format!(
                    "From agent {sender_id}: {message}",
                    sender_id = sender_id.0,
                    message = input.message
                );
                collab.record_message_for_agent(
                    *recipient,
                    ResponseItem::Message {
                        id: None,
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText { text }],
                    },
                );
                if let Some(agent) = collab.agent_mut(*recipient) {
                    previous_statuses.push((*recipient, agent.status.clone()));
                    agent.status = AgentLifecycleState::Running;
                }
            }

            (
                sender_name,
                valid_recipients,
                invalid_recipients,
                busy_recipients,
                direct_children,
                previous_statuses,
            )
        }
    };

    if input.recipients.is_empty()
        || !invalid_recipients.is_empty()
        || !busy_recipients.is_empty()
        || valid_recipients.is_empty()
    {
        let content = if input.recipients.is_empty() {
            "No recipients provided. You can only send to your direct child agents.".to_string()
        } else if !invalid_recipients.is_empty() {
            if direct_children.is_empty() {
                format!(
                    "Invalid recipients {:?}. You have no direct child agents to send to.",
                    input.recipients
                )
            } else {
                format!(
                    "Invalid recipients {:?}. You can only send to your direct child agents: {}.",
                    input.recipients,
                    direct_children.join(", ")
                )
            }
        } else if !busy_recipients.is_empty() {
            format!(
                "Some recipients are busy: {}. Wait for them to finish (collaboration_wait) before sending another message.",
                busy_recipients.join(", ")
            )
        } else {
            "No eligible recipients.".to_string()
        };

        let output = CollaborationSendOutput {
            content: Err(content.clone()),
        };
        info!(
            "collaboration_send: sender={}, recipients={:?}, status=error: {content}",
            sender_id.0, input.recipients
        );
        return serialize_function_output(&output, false);
    }

    let supervisor = session.ensure_collaboration_supervisor().await;
    if let Err(err) = supervisor
        .start_agents(valid_recipients.clone(), i32::MAX)
        .await
    {
        if !previous_statuses.is_empty() {
            let mut collab = session.collaboration_state().lock().await;
            for (id, prev) in previous_statuses {
                if let Some(agent) = collab.agent_mut(id)
                    && matches!(&agent.status, AgentLifecycleState::Running)
                {
                    agent.status = prev;
                }
            }
        }

        let output = CollaborationSendOutput {
            content: Err(format!("Failed to start child agents: {err}")),
        };
        return serialize_function_output(&output, false);
    }

    let output = CollaborationSendOutput { content: Ok(()) };
    info!(
        "collaboration_send: sender={} ({}), recipients={:?}, status={}",
        sender_id.0, sender_name, valid_recipients, "Message sent successfully."
    );
    serialize_function_output(&output, true)
}

async fn handle_wait(
    session: Arc<crate::codex::Session>,
    turn: &crate::codex::TurnContext,
    arguments: &str,
) -> Result<ToolOutput, FunctionCallError> {
    let input: CollaborationWaitInput = serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err:?}"))
    })?;

    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    {
        let mut collab = session.collaboration_state().lock().await;
        collab.ensure_root_agent(&session_configuration, &session_history);
    }
    let caller_id = turn.collaboration_agent();

    {
        let collab = session.collaboration_state().lock().await;
        if collab.agent(caller_id).is_none() {
            let output = CollaborationWaitOutput {
                states: vec![AgentState {
                    agent_id: caller_id.0.to_string(),
                    state: State::Error("unknown caller agent".to_string()),
                }],
            };
            return serialize_function_output(&output, false);
        }
    }

    let mut error_states = Vec::new();
    let targets = if input.agent_idx.is_empty() {
        let collab = session.collaboration_state().lock().await;
        collab
            .agents()
            .iter()
            .filter(|agent| agent.parent == Some(caller_id))
            .map(|agent| agent.id)
            .collect::<Vec<AgentId>>()
    } else {
        let collab = session.collaboration_state().lock().await;
        let mut targets = Vec::new();
        for raw in &input.agent_idx {
            match parse_agent_id(raw) {
                Ok(id) if collab.agent(id).is_some() && collab.is_direct_child(caller_id, id) => {
                    targets.push(id);
                }
                Ok(_) => error_states.push(AgentState {
                    agent_id: raw.clone(),
                    state: State::Error("invalid or non-child agent".to_string()),
                }),
                Err(err) => error_states.push(AgentState {
                    agent_id: raw.clone(),
                    state: State::Error(err),
                }),
            }
        }
        targets
    };

    if targets.is_empty() {
        let success = error_states.is_empty();
        let output = CollaborationWaitOutput {
            states: error_states,
        };
        return serialize_function_output(&output, success);
    }

    // Passive wait: observe running children only; do not start new work.
    let supervisor = session.ensure_collaboration_supervisor().await;
    let mut rx = supervisor.subscribe();

    let mut active = {
        let collab = session.collaboration_state().lock().await;
        targets
            .iter()
            .filter(|id| {
                collab
                    .agent(**id)
                    .is_some_and(|agent| matches!(&agent.status, AgentLifecycleState::Running))
            })
            .copied()
            .collect::<HashSet<AgentId>>()
    };

    if !active.is_empty() {
        let deadline = Instant::now() + Duration::from_millis(u64::from(input.max_duration));
        while !active.is_empty() {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining_time = deadline.saturating_duration_since(now);
            let event = match tokio::time::timeout(remaining_time, rx.recv()).await {
                Ok(Ok(evt)) => evt,
                _ => break,
            };
            if !active.contains(&event.agent) {
                continue;
            }
            if !matches!(event.status, AgentLifecycleState::Running) {
                active.remove(&event.agent);
            }
        }
    }

    let success = error_states.is_empty();
    let mut states = Vec::new();
    {
        let collab = session.collaboration_state().lock().await;
        for id in &targets {
            if let Some(agent) = collab.agent(*id) {
                states.push(snapshot_agent_state(agent));
            }
        }
    }
    states.extend(error_states);

    let output = CollaborationWaitOutput { states };
    info!(
        "collaboration_wait: caller={}, targets={:?}, status=complete",
        caller_id.0, targets
    );
    serialize_function_output(&output, success)
}

async fn handle_get_state(
    session: Arc<crate::codex::Session>,
    turn: &crate::codex::TurnContext,
    arguments: &str,
) -> Result<ToolOutput, FunctionCallError> {
    let input: CollaborationGetStateInput = serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err:?}"))
    })?;

    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    {
        let mut collab = session.collaboration_state().lock().await;
        collab.ensure_root_agent(&session_configuration, &session_history);
    }

    let caller_id = turn.collaboration_agent();
    {
        let collab = session.collaboration_state().lock().await;
        if collab.agent(caller_id).is_none() {
            let output = CollaborationGetStateOutput {
                states: vec![AgentState {
                    agent_id: caller_id.0.to_string(),
                    state: State::Error("unknown caller agent".to_string()),
                }],
            };
            return serialize_function_output(&output, false);
        }
    }

    let mut error_states = Vec::new();
    let targets = if input.agent_idx.is_empty() {
        let collab = session.collaboration_state().lock().await;
        collab
            .agents()
            .iter()
            .filter(|agent| agent.parent == Some(caller_id))
            .map(|agent| agent.id)
            .collect::<Vec<AgentId>>()
    } else {
        let collab = session.collaboration_state().lock().await;
        let mut targets = Vec::new();
        for raw in &input.agent_idx {
            match parse_agent_id(raw) {
                Ok(id) if collab.agent(id).is_some() && collab.is_direct_child(caller_id, id) => {
                    targets.push(id);
                }
                Ok(_) => error_states.push(AgentState {
                    agent_id: raw.clone(),
                    state: State::Error("invalid or non-child agent".to_string()),
                }),
                Err(err) => error_states.push(AgentState {
                    agent_id: raw.clone(),
                    state: State::Error(err),
                }),
            }
        }
        targets
    };

    let mut states = Vec::new();
    {
        let collab = session.collaboration_state().lock().await;
        for id in &targets {
            if let Some(agent) = collab.agent(*id) {
                states.push(snapshot_agent_state(agent));
            }
        }
    }
    states.extend(error_states);

    let success = states
        .iter()
        .all(|state| !matches!(state.state, State::Error(_)));
    let output = CollaborationGetStateOutput { states };
    info!(
        "collaboration_get_state: caller={}, targets={:?}",
        caller_id.0, targets
    );
    serialize_function_output(&output, success)
}

async fn handle_close(
    session: Arc<crate::codex::Session>,
    turn: &crate::codex::TurnContext,
    arguments: &str,
) -> Result<ToolOutput, FunctionCallError> {
    let input: CollaborationCloseInput = serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err:?}"))
    })?;

    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    let mut collab = session.collaboration_state().lock().await;
    collab.ensure_root_agent(&session_configuration, &session_history);
    let caller_id = turn.collaboration_agent();

    if collab.agent(caller_id).is_none() {
        let output = CollaborationCloseOutput {
            states: vec![AgentState {
                agent_id: caller_id.0.to_string(),
                state: State::Error("unknown caller agent".to_string()),
            }],
        };
        return serialize_function_output(&output, false);
    }

    let mut error_states = Vec::new();
    let mut targets = Vec::new();

    for raw in &input.agent_idx {
        match parse_agent_id(raw) {
            Ok(id) if collab.agent(id).is_some() && collab.is_direct_child(caller_id, id) => {
                targets.push(id);
            }
            Ok(_) => error_states.push(AgentState {
                agent_id: raw.clone(),
                state: State::Error("invalid or non-child agent".to_string()),
            }),
            Err(err) => error_states.push(AgentState {
                agent_id: raw.clone(),
                state: State::Error(err),
            }),
        }
    }

    if targets.is_empty() {
        let success = error_states.is_empty();
        let output = CollaborationCloseOutput {
            states: error_states,
        };
        return serialize_function_output(&output, success);
    }

    let all = collab.descendants(&targets);
    let mut closed_ids = Vec::new();
    let mut states = Vec::new();
    for id in all {
        if let Some(agent) = collab.agent_mut(id) {
            if input.return_states {
                states.push(snapshot_agent_state(agent));
            }
            agent.status = AgentLifecycleState::Closed;
            closed_ids.push(id);
        }
    }

    let success = error_states.is_empty();
    if input.return_states {
        states.extend(error_states);
    } else {
        states = error_states;
    }

    {
        let supervisor = session.ensure_collaboration_supervisor().await;
        supervisor.close_agents(closed_ids).await;
    }
    let output = CollaborationCloseOutput { states };
    info!(
        "collaboration_close: caller={}, closed={:?}",
        caller_id.0, targets
    );
    serialize_function_output(&output, success)
}

pub(crate) fn create_collaboration_init_agent_tool(allowed_agents: &[String]) -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "agent_idx".to_string(),
        JsonSchema::String {
            description: Some("Agent index for the calling agent (string).".to_string()),
            enum_values: None,
        },
    );
    properties.insert(
        "agent".to_string(),
        JsonSchema::String {
            description: Some("Agent profile to spawn.".to_string()),
            enum_values: Some(allowed_agents.to_vec()),
        },
    );
    properties.insert(
        "context_strategy".to_string(),
        JsonSchema::String {
            description: Some(
                "Context strategy for the new agent: \"new\" (default) or \"fork\".".to_string(),
            ),
            enum_values: None,
        },
    );
    properties.insert(
        "message".to_string(),
        JsonSchema::String {
            description: Some("Optional initial message to send to the new agent.".to_string()),
            enum_values: None,
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_init_agent".to_string(),
        description: "Create a new logical agent as a child of the calling agent.".to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
    })
}

pub(crate) fn create_collaboration_send_tool() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "recipients".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("Agent index (string).".to_string()),
                enum_values: None,
            }),
            description: Some("Indices of the agents to send the message to.".to_string()),
        },
    );
    properties.insert(
        "message".to_string(),
        JsonSchema::String {
            description: Some("The message to send.".to_string()),
            enum_values: None,
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_send".to_string(),
        description:
            "Send a textual message from the calling agent to one or more direct child agents."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["recipients".to_string(), "message".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

pub(crate) fn create_collaboration_wait_tool() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "max_duration".to_string(),
        JsonSchema::Number {
            description: Some(
                "Maximum duration to wait in milliseconds. Must be >= 0. A good default value is 10,000."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "agent_idx".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("Agent index (string).".to_string()),
                enum_values: None,
            }),
            description: Some(
                "Optional list of child agents to wait for; defaults to all direct children."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_wait".to_string(),
        description:
            "Yield control and synchronize with child agents that are already running in their own loops."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["max_duration".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

pub(crate) fn create_collaboration_get_state_tool() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "agent_idx".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("Agent index (string).".to_string()),
                enum_values: None,
            }),
            description: Some(
                "Optional list of child agents to query; defaults to all direct children."
                    .to_string(),
            ),
        },
    );
    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_get_state".to_string(),
        description: "Return a high-level view of the calling agent's direct child agents."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
    })
}

pub(crate) fn create_collaboration_close_tool() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "return_states".to_string(),
        JsonSchema::Boolean {
            description: Some("Whether to include agent states before closing them.".to_string()),
        },
    );
    properties.insert(
        "agent_idx".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("Agent index (string).".to_string()),
                enum_values: None,
            }),
            description: Some(
                "Indices of the child agents to close. Each must be a direct child of the caller."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_close".to_string(),
        description:
            "Close one or more child agents (and their descendants), preventing further model calls."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["agent_idx".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context_with_rx;
    use crate::state::AgentLifecycleState;
    use crate::state::AgentState as InternalAgentState;
    use crate::state::CollaborationLimits;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    #[tokio::test]
    async fn init_agent_forks_parent_history() {
        let (session, turn, _rx) = make_session_and_context_with_rx();
        let seed = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "seed".to_string(),
            }],
        };
        session
            .record_conversation_items(&turn, std::slice::from_ref(&seed))
            .await;

        let args = serde_json::json!({ "context_strategy": "fork" }).to_string();
        let output = handle_init_agent(Arc::clone(&session), turn.as_ref(), &args)
            .await
            .expect("init agent should succeed");

        let content = match output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationInitAgentOutput =
            serde_json::from_str(&content).expect("parse init_agent output");
        assert!(parsed.result.is_ok());

        let collab = session.collaboration_state().lock().await;
        let child = collab.agent(AgentId(1)).expect("child agent should exist");
        let mut history = child.history.clone();
        let items = history.get_history();

        assert_eq!(items.len(), 2);
        assert!(matches!(
            items.first(),
            Some(ResponseItem::Message { role, .. }) if role == "user"
        ));
        assert!(matches!(
            items.get(1),
            Some(ResponseItem::Message { content, .. })
                if content.iter().any(|item| matches!(
                    item,
                    ContentItem::InputText { text } if text == "seed"
                ))
        ));
    }

    #[tokio::test]
    async fn send_rejects_non_child_recipients() {
        let (session, turn, _rx) = make_session_and_context_with_rx();
        let init_args = "{}";
        let init_output = handle_init_agent(Arc::clone(&session), turn.as_ref(), init_args)
            .await
            .expect("init agent should succeed");

        let init_content = match init_output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationInitAgentOutput =
            serde_json::from_str(&init_content).expect("parse init_agent output");
        assert!(parsed.result.is_ok());

        let session_configuration = session.current_session_configuration().await;
        let session_history = session.clone_history().await;
        let mut collab = session.collaboration_state().lock().await;
        collab.ensure_root_agent(&session_configuration, &session_history);
        let grandchild = InternalAgentState::new_child(
            AgentId(2),
            "grandchild".to_string(),
            AgentId(1),
            2,
            session_configuration,
            None,
            ContextManager::new(),
        );
        collab.add_child(grandchild).expect("add grandchild");
        drop(collab);

        let args = serde_json::json!({
            "recipients": ["2"],
            "message": "hello",
        })
        .to_string();
        let output = handle_send(Arc::clone(&session), turn.as_ref(), &args)
            .await
            .expect("send should not error fatally");

        let content = match output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationSendOutput =
            serde_json::from_str(&content).expect("parse send output");
        assert!(parsed.content.is_err());
    }

    #[tokio::test]
    async fn init_agent_rejects_mismatched_agent_idx() {
        let (session, turn, _rx) = make_session_and_context_with_rx();
        let args = serde_json::json!({ "agent_idx": "1" }).to_string();
        let output = handle_init_agent(Arc::clone(&session), turn.as_ref(), &args)
            .await
            .expect("init agent should return output");

        let content = match output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationInitAgentOutput =
            serde_json::from_str(&content).expect("parse init_agent output");
        assert!(parsed.result.is_err());
    }

    #[tokio::test]
    async fn get_state_reports_last_agent_message() {
        let (session, turn, _rx) = make_session_and_context_with_rx();
        handle_init_agent(Arc::clone(&session), turn.as_ref(), "{}")
            .await
            .expect("init agent should return output");

        {
            let session_configuration = session.current_session_configuration().await;
            let session_history = session.clone_history().await;
            let mut collab = session.collaboration_state().lock().await;
            collab.ensure_root_agent(&session_configuration, &session_history);
            if let Some(agent) = collab.agent_mut(AgentId(1)) {
                agent.status = AgentLifecycleState::Idle {
                    last_agent_message: Some("wrapped up".to_string()),
                };
            }
        }

        let state_output = handle_get_state(Arc::clone(&session), turn.as_ref(), "{}")
            .await
            .expect("get_state should succeed");
        let content = match state_output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationGetStateOutput =
            serde_json::from_str(&content).expect("parse get_state output");
        let child = parsed
            .states
            .iter()
            .find(|agent| agent.agent_id == "1")
            .expect("child agent present");
        match &child.state {
            State::Done(message) => assert_eq!(message, "wrapped up"),
            other => panic!("expected done state, got {other:?}"),
        }
    }

    #[test]
    fn collaboration_limits_default_values() {
        let limits = CollaborationLimits::default();
        assert!(limits.max_agents > 0);
        assert!(limits.max_depth > 0);
    }
}
