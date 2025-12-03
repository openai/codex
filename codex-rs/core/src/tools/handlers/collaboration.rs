use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::context_manager::ContextManager;
use crate::function_tool::FunctionCallError;
use crate::protocol::SandboxPolicy;
use crate::state::AgentId;
use crate::state::AgentLifecycleState;
use crate::state::AgentState;
use crate::state::ContextStrategy;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::spec::JsonSchema;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tracing::info;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ExtraMetadata(pub HashMap<String, serde_json::Value>);

fn empty_extra_metadata() -> ExtraMetadata {
    ExtraMetadata(HashMap::new())
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationInitAgentMetadata {
    message_tool_call_success: bool,
    message_tool_call_error_should_penalize_model: bool,
    extra: ExtraMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationInitAgentOutput {
    content: String,
    metadata: CollaborationInitAgentMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationSendMetadata {
    message_tool_call_success: bool,
    message_tool_call_error_should_penalize_model: bool,
    is_send_success_msg: Option<bool>,
    message_content_str: Option<String>,
    extra: ExtraMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationSendOutput {
    content: String,
    metadata: CollaborationSendMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationWaitMetadata {
    message_tool_call_success: bool,
    message_tool_call_error_should_penalize_model: bool,
    is_wait_success_msg: Option<bool>,
    extra: ExtraMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationWaitOutput {
    content: String,
    metadata: CollaborationWaitMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationGetStateMetadata {
    message_tool_call_success: Option<bool>,
    message_tool_call_error_should_penalize_model: Option<bool>,
    extra: ExtraMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationGetStateOutput {
    content: String,
    metadata: CollaborationGetStateMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationCloseMetadata {
    message_tool_call_success: bool,
    message_tool_call_error_should_penalize_model: bool,
    extra: ExtraMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CollaborationCloseOutput {
    content: String,
    metadata: CollaborationCloseMetadata,
}

fn roots_subset(child: &[PathBuf], parent: &[PathBuf]) -> bool {
    let parent_roots: HashSet<&PathBuf> = parent.iter().collect();
    child.iter().all(|root| parent_roots.contains(root))
}

fn sandbox_is_at_least_as_strict(child: &SandboxPolicy, parent: &SandboxPolicy) -> bool {
    match parent {
        SandboxPolicy::DangerFullAccess => true,
        SandboxPolicy::ReadOnly => matches!(child, SandboxPolicy::ReadOnly),
        SandboxPolicy::WorkspaceWrite {
            writable_roots: parent_roots,
            network_access: parent_network,
            exclude_tmpdir_env_var: parent_exclude_tmpdir,
            exclude_slash_tmp: parent_exclude_slash_tmp,
        } => match child {
            SandboxPolicy::DangerFullAccess => false,
            SandboxPolicy::ReadOnly => true,
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => {
                if *network_access && !parent_network {
                    return false;
                }
                if !exclude_tmpdir_env_var && *parent_exclude_tmpdir {
                    return false;
                }
                if !exclude_slash_tmp && *parent_exclude_slash_tmp {
                    return false;
                }
                roots_subset(writable_roots, parent_roots)
            }
        },
    }
}

#[derive(Debug, Deserialize)]
struct CollaborationInitAgentInput {
    #[serde(default)]
    #[allow(dead_code)]
    agent_idx: i32,
    #[serde(default)]
    context_strategy: Option<ContextStrategy>,
    #[serde(default)]
    #[allow(dead_code)]
    instructions: Option<String>,
    #[serde(default)]
    sandbox_policy: Option<SandboxPolicy>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CollaborationSendInput {
    recipients: Vec<i32>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CollaborationWaitInput {
    max_duration: i32,
    #[serde(default)]
    recipients: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
struct CollaborationCloseInput {
    recipients: Vec<i32>,
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

fn status_label(status: &AgentLifecycleState) -> &'static str {
    match status {
        AgentLifecycleState::Idle { .. } => "idle",
        AgentLifecycleState::Running => "running",
        AgentLifecycleState::Exhausted => "exhausted",
        AgentLifecycleState::Error { .. } => "error",
        AgentLifecycleState::Closed => "closed",
        AgentLifecycleState::WaitingForApproval { .. } => "waiting_for_approval",
    }
}

fn status_detail(status: &AgentLifecycleState) -> Option<String> {
    match status {
        AgentLifecycleState::Idle {
            last_agent_message, ..
        } => last_agent_message
            .as_ref()
            .map(|msg| format!("last_message={msg}")),
        AgentLifecycleState::Error { error } => Some(format!("error={error}")),
        AgentLifecycleState::WaitingForApproval { request } => {
            Some(format!("awaiting_approval={request}"))
        }
        _ => None,
    }
}

fn last_agent_message(status: &AgentLifecycleState) -> Option<&str> {
    match status {
        AgentLifecycleState::Idle {
            last_agent_message, ..
        } => last_agent_message.as_deref(),
        _ => None,
    }
}

fn build_agent_system_message(
    agent_id: AgentId,
    parent_id: AgentId,
    depth: i32,
    instructions: Option<&str>,
) -> ResponseItem {
    let mut lines = vec![format!(
        "You are agent {agent_id} (parent {parent_id}, depth {depth}).",
        agent_id = agent_id.0,
        parent_id = parent_id.0
    )];

    if let Some(text) = instructions {
        lines.push(format!("Instructions: {text}"));
    }

    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: lines.join("\n"),
        }],
    }
}

fn build_history_for_child(
    strategy: ContextStrategy,
    parent: &AgentState,
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
        ContextStrategy::Replace { history } => (history, None),
    };

    let mut merged = Vec::with_capacity(items.len() + 1);
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
            "collaboration_get_state" => handle_get_state(session, &turn).await,
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

    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    let mut collab = session.collaboration_state().lock().await;
    collab.ensure_root_agent(&session_configuration, &session_history);
    let parent_id = turn.collaboration_agent();
    let Some(parent) = collab.agent(parent_id).cloned() else {
        let metadata = CollaborationInitAgentMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: true,
            extra: ExtraMetadata(HashMap::new()),
        };
        let output = CollaborationInitAgentOutput {
            content: format!("unknown caller agent {}", parent_id.0),
            metadata,
        };
        return serialize_function_output(&output, false);
    };

    let depth = parent.depth + 1;
    let limits = *collab.limits();

    if collab.agents().len() as i32 >= limits.max_agents {
        let metadata = CollaborationInitAgentMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: false,
            extra: ExtraMetadata(HashMap::new()),
        };
        let output = CollaborationInitAgentOutput {
            content: "max agent count reached".to_string(),
            metadata,
        };
        return serialize_function_output(&output, false);
    }

    if depth > limits.max_depth {
        let metadata = CollaborationInitAgentMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: false,
            extra: ExtraMetadata(HashMap::new()),
        };
        let output = CollaborationInitAgentOutput {
            content: "max collaboration depth reached".to_string(),
            metadata,
        };
        return serialize_function_output(&output, false);
    }

    let mut updated_configuration = parent.config.clone();

    if let Some(policy) = input.sandbox_policy.clone() {
        if !sandbox_is_at_least_as_strict(&policy, &parent.config.sandbox_policy()) {
            let metadata = CollaborationInitAgentMetadata {
                message_tool_call_success: false,
                message_tool_call_error_should_penalize_model: true,
                extra: ExtraMetadata(HashMap::new()),
            };
            let output = CollaborationInitAgentOutput {
                content: "sandbox_policy cannot be more permissive than the parent".to_string(),
                metadata,
            };
            return serialize_function_output(&output, false);
        }
        let update = crate::codex::SessionSettingsUpdate {
            sandbox_policy: Some(policy),
            ..Default::default()
        };
        updated_configuration = updated_configuration.apply(&update);
    }

    if let Some(model) = input.model.clone() {
        let update = crate::codex::SessionSettingsUpdate {
            model: Some(model),
            ..Default::default()
        };
        updated_configuration = updated_configuration.apply(&update);
    }

    let instructions = parent
        .instructions
        .clone()
        .or(session_configuration.developer_instructions())
        .or(session_configuration.user_instructions());

    let assigned_id = collab.next_agent_id();
    let system_message =
        build_agent_system_message(assigned_id, parent.id, depth, instructions.as_deref());
    let history = build_history_for_child(
        input.context_strategy.unwrap_or_default(),
        &parent,
        system_message,
    );

    let child = AgentState::new_child(
        assigned_id,
        parent.id,
        depth,
        updated_configuration,
        instructions,
        history,
    );

    let assigned_id = collab
        .add_child(child)
        .map_err(FunctionCallError::RespondToModel)?;
    drop(collab);

    let mut extra = HashMap::new();
    extra.insert("agent_idx".to_string(), json!(assigned_id.0));
    extra.insert("parent_agent_idx".to_string(), json!(parent.id.0));
    extra.insert("depth".to_string(), json!(depth));

    let metadata = CollaborationInitAgentMetadata {
        message_tool_call_success: true,
        message_tool_call_error_should_penalize_model: false,
        extra: ExtraMetadata(extra),
    };

    let content = format!("Initialized agent {} at depth {}", assigned_id.0, depth);

    let output = CollaborationInitAgentOutput { content, metadata };
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

    let mut collab = session.collaboration_state().lock().await;
    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    collab.ensure_root_agent(&session_configuration, &session_history);
    let sender_id = turn.collaboration_agent();

    if collab.agent(sender_id).is_none() {
        let metadata = CollaborationSendMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: true,
            is_send_success_msg: Some(false),
            message_content_str: Some(input.message.clone()),
            extra: ExtraMetadata(HashMap::new()),
        };
        let output = CollaborationSendOutput {
            content: format!("unknown caller agent {}", sender_id.0),
            metadata,
        };
        return serialize_function_output(&output, false);
    }

    let mut invalid_recipients = Vec::new();
    let mut valid_recipients = Vec::new();

    for raw in &input.recipients {
        let candidate = AgentId(*raw);
        if let Some(agent) = collab.agent(candidate)
            && collab.is_direct_child(sender_id, candidate)
            && !matches!(agent.status, AgentLifecycleState::Closed)
        {
            valid_recipients.push(candidate);
        } else {
            invalid_recipients.push(*raw);
        }
    }

    if valid_recipients.is_empty() {
        let mut extra = HashMap::new();
        extra.insert("recipients".to_string(), json!(input.recipients));
        let metadata = CollaborationSendMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: true,
            is_send_success_msg: Some(false),
            message_content_str: Some(input.message.clone()),
            extra: ExtraMetadata(extra),
        };
        let content = if invalid_recipients.is_empty() {
            "no valid recipients provided".to_string()
        } else {
            format!(
                "invalid or non-child agent indices: {}",
                invalid_recipients
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        };
        let output = CollaborationSendOutput { content, metadata };
        info!(
            "collaboration_send: sender={}, recipients={:?}, status=error: {}",
            sender_id.0, input.recipients, output.content
        );
        return serialize_function_output(&output, false);
    }

    for recipient in &valid_recipients {
        let text = format!("From agent {}: {}", sender_id.0, input.message);
        collab.record_message_for_agent(
            *recipient,
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text }],
            },
        );
    }

    let mut extra = HashMap::new();
    extra.insert(
        "recipients".to_string(),
        json!(valid_recipients.iter().map(|id| id.0).collect::<Vec<i32>>()),
    );

    let metadata = CollaborationSendMetadata {
        message_tool_call_success: invalid_recipients.is_empty(),
        message_tool_call_error_should_penalize_model: !invalid_recipients.is_empty(),
        is_send_success_msg: Some(invalid_recipients.is_empty()),
        message_content_str: Some(input.message.clone()),
        extra: ExtraMetadata(extra),
    };

    let content = if invalid_recipients.is_empty() {
        "Message sent successfully.".to_string()
    } else {
        format!(
            "Message sent to some recipients; invalid indices: {}",
            invalid_recipients
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join(", ")
        )
    };

    let success = metadata.message_tool_call_success;

    if success && !valid_recipients.is_empty() {
        let supervisor = session.ensure_collaboration_supervisor().await;
        for recipient in valid_recipients
            .iter()
            .copied()
            .filter(|recipient| recipient.0 != 0)
        {
            supervisor.kick_agent(recipient).await;
        }
    }

    let output = CollaborationSendOutput { content, metadata };
    info!(
        "collaboration_send: sender={}, recipients={:?}, status={}",
        sender_id.0, valid_recipients, output.content
    );
    serialize_function_output(&output, success)
}

async fn handle_wait(
    session: Arc<crate::codex::Session>,
    turn: &crate::codex::TurnContext,
    arguments: &str,
) -> Result<ToolOutput, FunctionCallError> {
    let input: CollaborationWaitInput = serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err:?}"))
    })?;

    if input.max_duration < 0 {
        let metadata = CollaborationWaitMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: true,
            is_wait_success_msg: Some(false),
            extra: empty_extra_metadata(),
        };
        let output = CollaborationWaitOutput {
            content: "max_duration must be non-negative".to_string(),
            metadata,
        };
        return serialize_function_output(&output, false);
    }

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
            let metadata = CollaborationWaitMetadata {
                message_tool_call_success: false,
                message_tool_call_error_should_penalize_model: true,
                is_wait_success_msg: Some(false),
                extra: empty_extra_metadata(),
            };
            let output = CollaborationWaitOutput {
                content: format!("unknown caller agent {}", caller_id.0),
                metadata,
            };
            return serialize_function_output(&output, false);
        }
    }

    let candidates = {
        let collab = session.collaboration_state().lock().await;
        if let Some(ids) = input.recipients {
            ids.into_iter().map(AgentId).collect::<Vec<AgentId>>()
        } else {
            collab
                .agents()
                .iter()
                .filter(|agent| agent.parent == Some(caller_id))
                .map(|agent| agent.id)
                .collect::<Vec<AgentId>>()
        }
    };

    let mut invalid = Vec::new();
    let mut targets = Vec::new();
    {
        let collab = session.collaboration_state().lock().await;
        for id in candidates {
            if collab.agent(id).is_some() && collab.is_direct_child(caller_id, id) {
                targets.push(id);
            } else {
                invalid.push(id.0);
            }
        }
    }

    if !invalid.is_empty() || targets.is_empty() {
        let penalize = !invalid.is_empty();
        let metadata = CollaborationWaitMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: penalize,
            is_wait_success_msg: Some(false),
            extra: empty_extra_metadata(),
        };
        let content = if !invalid.is_empty() {
            format!(
                "invalid or non-child agent indices: {}",
                invalid
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        } else {
            "no eligible child agents to wait for".to_string()
        };
        let output = CollaborationWaitOutput { content, metadata };
        info!(
            "collaboration_wait: caller={}, targets={:?}, status=error: {}",
            caller_id.0, targets, output.content
        );
        return serialize_function_output(&output, false);
    }

    // Passive wait: observe running children only; do not start new work.
    let supervisor = session.ensure_collaboration_supervisor().await;
    let mut rx = supervisor.subscribe();

    let snapshot = |agent: &AgentState, delta_tokens: i32, sub_id: Option<String>| {
        json!({
            "agent_idx": agent.id.0,
            "delta_tokens": delta_tokens,
            "status": status_label(&agent.status),
            "status_detail": status_detail(&agent.status),
            "last_agent_message": last_agent_message(&agent.status),
            "sub_id": sub_id,
        })
    };

    let mut agents_ran = Vec::new();
    let mut recorded: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut active: std::collections::HashSet<i32> = {
        let collab = session.collaboration_state().lock().await;
        targets
            .iter()
            .filter(|id| {
                collab.agent(**id).map_or(false, |agent| {
                    matches!(agent.status, AgentLifecycleState::Running)
                })
            })
            .map(|id| id.0)
            .collect()
    };

    if active.is_empty() {
        let collab = session.collaboration_state().lock().await;
        let mut extra = HashMap::new();
        extra.insert(
            "agents_ran".to_string(),
            json!(
                targets
                    .iter()
                    .filter_map(|id| collab.agent(*id))
                    .map(|agent| snapshot(agent, 0, None))
                    .collect::<Vec<_>>()
            ),
        );
        extra.insert("token_budget_exhausted".to_string(), json!(false));
        let metadata = CollaborationWaitMetadata {
            message_tool_call_success: true,
            message_tool_call_error_should_penalize_model: false,
            is_wait_success_msg: Some(true),
            extra: ExtraMetadata(extra),
        };
        let output = CollaborationWaitOutput {
            content: "Finished waiting.".to_string(),
            metadata,
        };
        info!(
            "collaboration_wait: caller={}, targets={:?}, status=ready",
            caller_id.0, targets
        );
        return serialize_function_output(&output, true);
    }

    let deadline = Instant::now() + Duration::from_millis(input.max_duration.max(0) as u64);

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
        if !active.contains(&event.agent.0) {
            continue;
        }

        let detail = status_detail(&event.status);
        let fallback_last = {
            let collab = session.collaboration_state().lock().await;
            collab
                .agent(event.agent)
                .and_then(|agent| last_agent_message(&agent.status).map(str::to_string))
        };
        agents_ran.push(json!({
            "agent_idx": event.agent.0,
            "delta_tokens": event.delta_tokens,
            "status": status_label(&event.status),
            "status_detail": detail,
            "last_agent_message": event.last_message.or(fallback_last),
            "sub_id": event.sub_id,
        }));
        recorded.insert(event.agent.0);
        if !matches!(event.status, AgentLifecycleState::Running) {
            active.remove(&event.agent.0);
        }
    }

    {
        let collab = session.collaboration_state().lock().await;
        for id in &targets {
            if recorded.contains(&id.0) {
                continue;
            }
            if let Some(agent) = collab.agent(*id) {
                agents_ran.push(snapshot(agent, 0, None));
            }
        }
    }

    let mut extra = HashMap::new();
    extra.insert("agents_ran".to_string(), json!(agents_ran));

    let metadata = CollaborationWaitMetadata {
        message_tool_call_success: true,
        message_tool_call_error_should_penalize_model: false,
        is_wait_success_msg: Some(active.is_empty()),
        extra: ExtraMetadata(extra),
    };

    let content = "Finished waiting.".to_string();
    let output = CollaborationWaitOutput { content, metadata };
    info!(
        "collaboration_wait: caller={}, targets={:?}, ran={:?}",
        caller_id.0, targets, agents_ran
    );
    serialize_function_output(&output, true)
}

async fn handle_get_state(
    session: Arc<crate::codex::Session>,
    _turn: &crate::codex::TurnContext,
) -> Result<ToolOutput, FunctionCallError> {
    let session_configuration = session.current_session_configuration().await;
    let session_history = session.clone_history().await;
    let mut collab = session.collaboration_state().lock().await;
    collab.ensure_root_agent(&session_configuration, &session_history);

    let agents = collab.agents();
    let mut lines = Vec::new();
    let mut structured = Vec::new();

    for agent in agents {
        let status = status_label(&agent.status);
        let detail = status_detail(&agent.status);
        let detail_suffix = detail
            .as_ref()
            .map_or_else(String::new, |d| format!(" ({d})"));
        let last_message = last_agent_message(&agent.status).map(str::to_string);
        let parent_idx = agent.parent.map(|id| id.0);
        lines.push(format!(
            "agent {} (parent {:?}, depth {}): {}{}",
            agent.id.0, parent_idx, agent.depth, status, detail_suffix
        ));
        structured.push(json!({
            "agent_idx": agent.id.0,
            "parent_agent_idx": parent_idx,
            "depth": agent.depth,
            "status": status,
            "status_detail": detail,
            "last_agent_message": last_message,
        }));
    }

    let description = if lines.is_empty() {
        "no collaboration agents initialized yet".to_string()
    } else {
        lines.join("\n")
    };

    let mut extra = HashMap::new();
    extra.insert("agents".to_string(), json!(structured));

    let metadata = CollaborationGetStateMetadata {
        message_tool_call_success: Some(true),
        message_tool_call_error_should_penalize_model: Some(false),
        extra: ExtraMetadata(extra),
    };

    let output = CollaborationGetStateOutput {
        content: description,
        metadata,
    };
    info!("collaboration_get_state: agents={}", agents.len());
    serialize_function_output(&output, true)
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
        let metadata = CollaborationCloseMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: true,
            extra: empty_extra_metadata(),
        };
        let output = CollaborationCloseOutput {
            content: format!("unknown caller agent {}", caller_id.0),
            metadata,
        };
        return serialize_function_output(&output, false);
    }

    let mut invalid = Vec::new();
    let mut targets = Vec::new();

    for raw in &input.recipients {
        let id = AgentId(*raw);
        if collab.agent(id).is_some() && collab.is_direct_child(caller_id, id) {
            targets.push(id);
        } else {
            invalid.push(*raw);
        }
    }

    if !invalid.is_empty() || targets.is_empty() {
        let metadata = CollaborationCloseMetadata {
            message_tool_call_success: false,
            message_tool_call_error_should_penalize_model: true,
            extra: empty_extra_metadata(),
        };
        let content = if invalid.is_empty() {
            "no valid agent indices provided to close".to_string()
        } else {
            format!(
                "invalid or non-child agent indices: {}",
                invalid
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        };
        let output = CollaborationCloseOutput { content, metadata };
        return serialize_function_output(&output, false);
    }

    let all = collab.descendants(&targets);
    let mut closed_ids = Vec::new();
    let mut closed_indices = Vec::new();
    for id in all {
        if let Some(agent) = collab.agent_mut(id) {
            agent.status = AgentLifecycleState::Closed;
            closed_ids.push(id);
            closed_indices.push(id.0);
        }
    }

    let mut extra = HashMap::new();
    extra.insert("closed_agent_indices".to_string(), json!(closed_indices));

    let metadata = CollaborationCloseMetadata {
        message_tool_call_success: true,
        message_tool_call_error_should_penalize_model: false,
        extra: ExtraMetadata(extra),
    };

    let content = format!(
        "Closed {} agents (and their descendants).",
        closed_indices.len()
    );
    let output = CollaborationCloseOutput { content, metadata };
    {
        let supervisor = session.ensure_collaboration_supervisor().await;
        supervisor.close_agents(closed_ids).await;
    }
    info!(
        "collaboration_close: caller={}, closed={:?}",
        caller_id.0, closed_indices
    );
    serialize_function_output(&output, true)
}

pub(crate) fn create_collaboration_init_agent_tool() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "agent_idx".to_string(),
        JsonSchema::Number {
            description: Some("Index proposed by the model; ignored by the server.".to_string()),
        },
    );
    properties.insert(
        "context_strategy".to_string(),
        JsonSchema::String {
            description: Some(
                "Context strategy for the new agent: \"new\" (default) or \"fork\".".to_string(),
            ),
        },
    );
    properties.insert(
        "instructions".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional high-level instructions / persona for this agent.".to_string(),
            ),
        },
    );
    properties.insert(
        "sandbox_policy".to_string(),
        JsonSchema::Object {
            properties: std::collections::BTreeMap::new(),
            required: None,
            additional_properties: Some(true.into()),
        },
    );
    properties.insert(
        "model".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional per-agent model name; defaults to the current session model.".to_string(),
            ),
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
            items: Box::new(JsonSchema::Number { description: None }),
            description: Some("Indices of the agents to send the message to.".to_string()),
        },
    );
    properties.insert(
        "message".to_string(),
        JsonSchema::String {
            description: Some("The message to send.".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_send".to_string(),
        description:
            "Send a textual message from the calling agent to one or more recipient agents."
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
                "Maximum duration to wait in ms, measured in tokens. Must be >= 0. A good default value is 10.000".to_string(),
            ),
        },
    );
    properties.insert(
        "recipients".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::Number { description: None }),
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
    ToolSpec::Function(ResponsesApiTool {
        name: "collaboration_get_state".to_string(),
        description:
            "Return a high-level view of the collaboration graph (agents, statuses, depth)."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties: std::collections::BTreeMap::new(),
            required: None,
            additional_properties: Some(false.into()),
        },
    })
}

pub(crate) fn create_collaboration_close_tool() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "recipients".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::Number { description: None }),
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
            required: Some(vec!["recipients".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context_with_rx;
    use crate::state::AgentLifecycleState;
    use crate::state::CollaborationLimits;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::PathBuf;
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
        assert!(parsed.metadata.message_tool_call_success);
        assert_eq!(
            parsed
                .metadata
                .message_tool_call_error_should_penalize_model,
            false
        );

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
        assert!(parsed.metadata.message_tool_call_success);

        let session_configuration = session.current_session_configuration().await;
        let session_history = session.clone_history().await;
        let mut collab = session.collaboration_state().lock().await;
        collab.ensure_root_agent(&session_configuration, &session_history);
        let grandchild = AgentState::new_child(
            AgentId(2),
            AgentId(1),
            2,
            session_configuration,
            None,
            ContextManager::new(),
        );
        collab.add_child(grandchild).expect("add grandchild");
        drop(collab);

        let args = serde_json::json!({
            "recipients": [2],
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

        assert_eq!(parsed.metadata.message_tool_call_success, false);
        assert_eq!(parsed.metadata.is_send_success_msg, Some(false));
        assert!(
            parsed
                .content
                .contains("invalid or non-child agent indices")
        );
    }

    #[tokio::test]
    async fn init_agent_rejects_more_permissive_sandbox() {
        let (session, turn, _rx) = make_session_and_context_with_rx();
        session
            .update_settings(crate::codex::SessionSettingsUpdate {
                sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![PathBuf::from("/tmp")],
                    network_access: false,
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                }),
                ..Default::default()
            })
            .await;

        let args =
            serde_json::json!({ "sandbox_policy": { "type": "danger-full-access" } }).to_string();
        let output = handle_init_agent(Arc::clone(&session), turn.as_ref(), &args)
            .await
            .expect("init agent should return output");

        let content = match output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationInitAgentOutput =
            serde_json::from_str(&content).expect("parse init_agent output");
        assert_eq!(parsed.metadata.message_tool_call_success, false);
        assert!(
            parsed
                .content
                .contains("sandbox_policy cannot be more permissive than the parent")
        );
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

        let state_output = handle_get_state(Arc::clone(&session), turn.as_ref())
            .await
            .expect("get_state should succeed");
        let content = match state_output {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("expected function output"),
        };
        let parsed: CollaborationGetStateOutput =
            serde_json::from_str(&content).expect("parse get_state output");
        let agents = parsed
            .metadata
            .extra
            .0
            .get("agents")
            .and_then(|value| value.as_array())
            .cloned()
            .expect("agents array");
        let child = agents
            .iter()
            .find(|value| value.get("agent_idx") == Some(&json!(1)))
            .expect("child agent present");

        assert_eq!(child.get("last_agent_message"), Some(&json!("wrapped up")));
        assert_eq!(
            child.get("status_detail"),
            Some(&json!("last_message=wrapped up"))
        );
    }

    #[test]
    fn collaboration_limits_default_values() {
        let limits = CollaborationLimits::default();
        assert!(limits.max_agents > 0);
        assert!(limits.max_depth > 0);
    }

    #[test]
    fn extra_metadata_wraps_map() {
        let mut inner = HashMap::new();
        inner.insert("k".to_string(), serde_json::Value::from("v"));
        let meta = ExtraMetadata(inner.clone());
        assert_eq!(meta.0.len(), 1);
    }
}
