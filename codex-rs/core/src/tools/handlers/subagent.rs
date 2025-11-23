use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use chrono::SecondsFormat;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::AgentId;
use codex_protocol::ConversationId;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::ExecCommandSource;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::parse_command::parse_command;
use crate::subagents::ForkRequest;
use crate::subagents::LogEntry;
use crate::subagents::PruneRequest;
use crate::subagents::SendMessageRequest;
use crate::subagents::SpawnRequest;
use crate::subagents::SubagentCompletion;
use crate::subagents::SubagentManagerError;
use crate::subagents::SubagentMetadata;
use crate::subagents::SubagentOrigin;
use crate::subagents::SubagentStatus;
use crate::subagents::WatchdogAction;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::TaskCompleteEvent;
use codex_protocol::protocol::TokenCountEvent;

const MAX_AWAIT_TIMEOUT_SECS: u64 = 30 * 60;
const MIN_WATCHDOG_INTERVAL_SECS: u64 = 30;
const DEFAULT_WATCHDOG_INTERVAL_SECS: u64 = 300;

const ROOT_AGENT_ID: AgentId = 0;

#[derive(Clone)]
struct InvocationContext {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    tool_name: String,
    arguments: String,
    caller_id: ConversationId,
    registry_entries: Vec<SubagentMetadata>,
    registry_by_agent: HashMap<AgentId, SubagentMetadata>,
    manager: Arc<crate::subagents::SubagentManager>,
    is_root_agent: bool,
}

impl InvocationContext {
    fn new(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        tool_name: String,
        arguments: String,
        caller_id: ConversationId,
        registry_entries: Vec<SubagentMetadata>,
        registry_by_agent: HashMap<AgentId, SubagentMetadata>,
        manager: Arc<crate::subagents::SubagentManager>,
        is_root_agent: bool,
    ) -> Self {
        Self {
            session,
            turn,
            call_id,
            tool_name,
            arguments,
            caller_id,
            registry_entries,
            registry_by_agent,
            manager,
            is_root_agent,
        }
    }

    fn agent_session(&self, agent_id: AgentId) -> Result<ConversationId, FunctionCallError> {
        require_agent_session(&self.registry_by_agent, agent_id)
    }

    fn sender_metadata(&self) -> Option<SubagentMetadata> {
        self.registry_entries
            .iter()
            .find(|meta| meta.session_id == self.caller_id)
            .cloned()
    }

    fn sender_agent_id(&self) -> AgentId {
        self.sender_metadata()
            .map(|meta| meta.agent_id)
            .unwrap_or(ROOT_AGENT_ID)
    }

    fn root_session_id(&self) -> ConversationId {
        let mut root_session_id = self.caller_id;
        loop {
            if let Some(meta) = self
                .registry_entries
                .iter()
                .find(|m| m.session_id == root_session_id)
                && let Some(parent) = meta.parent_session_id
            {
                root_session_id = parent;
                continue;
            }
            break;
        }
        root_session_id
    }
}

struct ExecEventLogger {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    command: Vec<String>,
    cwd: PathBuf,
    parsed_cmd: Vec<ParsedCommand>,
    start: Instant,
}

impl ExecEventLogger {
    async fn new(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        call_id: String,
        command: Vec<String>,
    ) -> Self {
        let parsed_cmd = parse_command(&command);
        let cwd = turn.cwd.clone();
        session
            .send_event(
                &turn,
                EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                    call_id: call_id.clone(),
                    turn_id: turn.sub_id.clone(),
                    command: command.clone(),
                    cwd: cwd.clone(),
                    parsed_cmd: parsed_cmd.clone(),
                    source: ExecCommandSource::Agent,
                    is_user_shell_command: false,
                    interaction_input: None,
                }),
            )
            .await;

        Self {
            session,
            turn,
            call_id,
            command,
            cwd,
            parsed_cmd,
            start: Instant::now(),
        }
    }

    async fn success(&self, output: &str) {
        self.finish(output, 0).await;
    }

    async fn failure(&self, message: &str) {
        self.finish(message, 1).await;
    }

    async fn finish(&self, aggregated_output: &str, exit_code: i32) {
        let duration = self.start.elapsed();
        let aggregated_output = clip_output(aggregated_output);
        let stderr = if exit_code == 0 {
            String::new()
        } else {
            aggregated_output.clone()
        };

        self.session
            .send_event(
                &self.turn,
                EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                    call_id: self.call_id.clone(),
                    turn_id: self.turn.sub_id.clone(),
                    command: self.command.clone(),
                    cwd: self.cwd.clone(),
                    parsed_cmd: self.parsed_cmd.clone(),
                    source: ExecCommandSource::Agent,
                    interaction_input: None,
                    stdout: String::new(),
                    stderr,
                    aggregated_output: aggregated_output.clone(),
                    exit_code,
                    duration,
                    formatted_output: aggregated_output,
                }),
            )
            .await;
    }
}

fn is_root_session(
    caller_id: ConversationId,
    registry: &HashMap<AgentId, SubagentMetadata>,
) -> bool {
    !registry
        .values()
        .any(|meta| meta.session_id == caller_id && meta.agent_id != ROOT_AGENT_ID)
}

fn clip_output(out: &str) -> String {
    let mut text = out.replace('\r', "");
    if text.len() > 4000 {
        text.truncate(4000);
        text.push('…');
    }
    text
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind")]
enum SubagentRender {
    Spawn {
        label: String,
        model: Option<String>,
        summary: Option<String>,
    },
    Fork {
        label: String,
        model: Option<String>,
        summary: Option<String>,
    },
    SendMessage {
        label: String,
        summary: Option<String>,
    },
    List {
        count: usize,
    },
    Await {
        label: String,
        timed_out: bool,
        message: Option<String>,
        lifecycle_status: Option<String>,
    },
    Watchdog {
        action: String,
        interval_s: u64,
        message: String,
    },
    Cancel {
        label: String,
    },
    Prune {
        counts: serde_json::Value,
    },
    Logs {
        rendered: String,
    },
    Raw {
        text: String,
    },
}

fn summarize_tool_output(tool_name: &str, _arguments: &str, output: &ToolOutput) -> String {
    let content = match output {
        ToolOutput::Function { content, .. } => content,
        ToolOutput::Mcp { result } => return format!("{result:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(content).unwrap_or_default();
    let render = match tool_name {
        "subagent_spawn" | "subagent_fork" => {
            let summary = parsed
                .get("summary")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let model = parsed
                .get("model")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let label = parsed
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("<unlabeled>")
                .to_string();
            if tool_name == "subagent_spawn" {
                SubagentRender::Spawn {
                    label,
                    model,
                    summary,
                }
            } else {
                SubagentRender::Fork {
                    label,
                    model,
                    summary,
                }
            }
        }
        "subagent_send_message" => {
            let summary = parsed
                .get("summary")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let label = parsed
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("<unlabeled>")
                .to_string();
            SubagentRender::SendMessage { label, summary }
        }
        "subagent_watchdog" => {
            let action = parsed
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("started")
                .to_string();
            let interval_s = parsed
                .get("interval_s")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(DEFAULT_WATCHDOG_INTERVAL_SECS);
            let msg = parsed
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or(
                    "Watchdog ping — report current status, next step, and PLAN.md progress.",
                )
                .to_string();
            SubagentRender::Watchdog {
                action,
                interval_s,
                message: msg,
            }
        }

        "subagent_list" => {
            let count = parsed
                .get("sessions")
                .and_then(|v| v.as_array())
                .map(Vec::len)
                .unwrap_or(0);
            SubagentRender::List { count }
        }
        "subagent_await" => {
            let timed_out = parsed
                .get("timed_out")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let label = parsed
                .get("metadata")
                .and_then(|m| m.get("label"))
                .and_then(|v| v.as_str())
                .unwrap_or("<unlabeled>")
                .to_string();
            let inbox_msg = parsed
                .get("messages")
                .and_then(|m| m.as_array())
                .and_then(|arr| arr.first())
                .and_then(|m| m.get("prompt"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let completion_msg = parsed.get("completion").and_then(|c| {
                if let Some(msg) = c
                    .get("Completed")
                    .and_then(|v| v.get("last_message"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    Some(format!("Completed: \"{msg}\""))
                } else if let Some(msg) = c
                    .get("Failed")
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    Some(format!("Failed: {msg}"))
                } else if c.get("Canceled").is_some() {
                    Some("Canceled".to_string())
                } else if c.get("Completed").is_some() {
                    Some("Completed".to_string())
                } else {
                    None
                }
            });
            let merged_message = completion_msg.or(inbox_msg);
            let lifecycle_status = parsed
                .get("lifecycle_status")
                .or_else(|| parsed.get("status"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            SubagentRender::Await {
                label,
                timed_out,
                message: merged_message,
                lifecycle_status,
            }
        }
        "subagent_cancel" => {
            let label = parsed
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("<unlabeled>")
                .to_string();
            SubagentRender::Cancel { label }
        }
        "subagent_prune" => {
            let counts = parsed.get("counts").cloned().unwrap_or_default();
            SubagentRender::Prune { counts }
        }
        "subagent_logs" => {
            let returned = parsed
                .get("returned")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let events = parsed
                .get("events")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Array(vec![]));
            if let Ok(entries) = serde_json::from_value::<Vec<LogEntry>>(events) {
                let session_id = parsed
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| ConversationId::from_string(s).ok())
                    .unwrap_or_default();
                let text = render_logs_as_text_with_max_lines(
                    session_id,
                    &entries,
                    parsed
                        .get("earliest_ms")
                        .and_then(serde_json::Value::as_i64),
                    parsed.get("latest_ms").and_then(serde_json::Value::as_i64),
                    returned as usize,
                    parsed
                        .get("total_available")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize,
                    parsed
                        .get("more_available")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    30,
                    PageDirection::Backward,
                );
                SubagentRender::Logs { rendered: text }
            } else {
                SubagentRender::Raw {
                    text: content.clone(),
                }
            }
        }
        _ => SubagentRender::Raw {
            text: content.clone(),
        },
    };

    serde_json::to_string(&serde_json::json!({ "subagent_render": render }))
        .unwrap_or_else(|_| content.clone())
}

fn parse_args<T: DeserializeOwned>(
    arguments: &str,
    tool_name: &str,
) -> Result<T, FunctionCallError> {
    serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse {tool_name} arguments: {err}"))
    })
}

fn display_label(label: &Option<String>) -> String {
    label.clone().unwrap_or_else(|| "<unlabeled>".to_string())
}

fn display_label_or_metadata(label: &Option<String>, meta: Option<&SubagentMetadata>) -> String {
    label
        .clone()
        .or_else(|| meta.and_then(|m| m.label.clone()))
        .unwrap_or_else(|| "<unlabeled>".to_string())
}

async fn run_with_logging<F, Fut>(
    ctx: &InvocationContext,
    command: Vec<String>,
    op: F,
) -> Result<ToolOutput, FunctionCallError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<ToolOutput, FunctionCallError>>,
{
    let logger = if ctx.is_root_agent {
        Some(
            ExecEventLogger::new(
                ctx.session.clone(),
                ctx.turn.clone(),
                ctx.call_id.clone(),
                command,
            )
            .await,
        )
    } else {
        None
    };

    let result = op().await;

    if let Some(ref logger) = logger {
        match &result {
            Ok(out) => {
                let summary = summarize_tool_output(&ctx.tool_name, &ctx.arguments, out);
                logger.success(&summary).await;
            }
            Err(err) => logger.failure(&err.to_string()).await,
        }
    }

    result
}

#[derive(Serialize)]
struct ListEntry {
    agent_id: AgentId,
    parent_agent_id: Option<AgentId>,
    session_id: ConversationId,
    parent_session_id: Option<ConversationId>,
    origin: SubagentOrigin,
    status: SubagentStatus,
    label: Option<String>,
    summary: Option<String>,
    reasoning_header: Option<String>,
    started_at_ms: i64,
    initial_message_count: usize,
    pending_messages: usize,
    pending_interrupts: usize,
}

#[derive(Deserialize)]
struct SpawnArgsRaw {
    prompt: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    sandbox_mode: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
struct ForkArgsRaw {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    sandbox_mode: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

fn parse_sandbox_mode(raw: Option<String>) -> Result<Option<SandboxMode>, FunctionCallError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let norm = raw.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    let collapsed: Cow<'_, str> = Cow::Owned(norm.replace('-', ""));

    let mode = match norm.as_str() {
        "read-only" | "read" | "readonly" => Some(SandboxMode::ReadOnly),
        "workspace-write" | "workspacewrite" | "workspace" => Some(SandboxMode::WorkspaceWrite),
        "danger-full-access" | "dangerfullaccess" | "danger" | "fullaccess" => {
            Some(SandboxMode::DangerFullAccess)
        }
        _ => match collapsed.as_ref() {
            "readonly" => Some(SandboxMode::ReadOnly),
            "workspacewrite" => Some(SandboxMode::WorkspaceWrite),
            "dangerfullaccess" | "danger" | "fullaccess" => Some(SandboxMode::DangerFullAccess),
            _ => None,
        },
    };

    mode.map(Some).ok_or_else(|| {
        FunctionCallError::RespondToModel(format!(
            "unknown sandbox_mode '{raw}'; expected one of read-only, workspace-write, danger-full-access"
        ))
    })
}

#[derive(Deserialize)]
struct SendMessageArgs {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    label: Option<String>,
    agent_id: AgentId,
    #[serde(default)]
    interrupt: bool,
}

#[derive(Deserialize)]
struct WatchdogArgs {
    agent_id: AgentId,
    #[serde(default)]
    interval_s: Option<u64>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    cancel: Option<bool>,
}

#[derive(Deserialize)]
struct AwaitArgs {
    #[serde(default)]
    timeout_s: Option<u64>,
}

#[derive(Deserialize)]
struct CancelArgs {
    agent_id: AgentId,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PruneArgs {
    ByIds {
        agent_ids: Vec<AgentId>,
        #[serde(default)]
        completed_only: Option<bool>,
    },
    All {
        all: bool,
        #[serde(default)]
        completed_only: Option<bool>,
    },
}

async fn handle_spawn(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let SpawnArgsRaw {
        prompt,
        label: raw_label,
        sandbox_mode,
        model,
    } = parse_args(&ctx.arguments, "subagent_spawn")?;

    let sandbox_mode = parse_sandbox_mode(sandbox_mode)?;
    let label = normalize_label(raw_label);
    let summary = summarize_prompt(&prompt);
    let log_command = vec![format!("Spawned subagent {}", display_label(&label))];

    let manager = ctx.manager.clone();
    let session = ctx.session.clone();
    let turn = ctx.turn.clone();

    run_with_logging(ctx, log_command, move || {
        let manager = manager.clone();
        let session = session.clone();
        let turn = turn.clone();
        let label = label.clone();
        let summary = summary.clone();
        let prompt = prompt.clone();
        let model = model.clone();
        async move {
            let metadata = manager
                .spawn(
                    session,
                    turn,
                    SpawnRequest {
                        prompt,
                        label,
                        summary,
                        sandbox_mode,
                        model: model.clone(),
                    },
                )
                .await
                .map_err(|err| map_manager_error(err, None))?;

            let response = build_subagent_response(&metadata, [("model", json!(model))]);

            Ok(ToolOutput::Function {
                content: response.to_string(),
                content_items: None,
                success: Some(true),
            })
        }
    })
    .await
}

async fn handle_fork(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let ForkArgsRaw {
        prompt,
        label: raw_label,
        sandbox_mode,
        model,
    } = parse_args(&ctx.arguments, "subagent_fork")?;

    let sandbox_mode = parse_sandbox_mode(sandbox_mode)?;
    let label = normalize_label(raw_label);
    let summary = summarize_optional_prompt(&prompt);
    let initial_message_count = ctx.session.history_len().await;
    let parent_session_id = ctx.caller_id;
    let prompt_clone = prompt.clone();
    let call_id = ctx.call_id.clone();
    let arguments = ctx.arguments.clone();

    let log_command = vec![format!("Forked subagent {}", display_label(&label))];

    let manager = ctx.manager.clone();
    let session = ctx.session.clone();
    let turn = ctx.turn.clone();

    run_with_logging(ctx, log_command, move || {
        let manager = manager.clone();
        let session = session.clone();
        let turn = turn.clone();
        let label = label.clone();
        let summary = summary.clone();
        let prompt = prompt.clone();
        let model = model.clone();
        async move {
            let metadata = manager
                .fork(
                    session,
                    turn,
                    ForkRequest {
                        parent_session_id,
                        initial_message_count,
                        label,
                        summary,
                        call_id,
                        arguments,
                        prompt,
                        sandbox_mode,
                        model: model.clone(),
                    },
                )
                .await
                .map_err(|err| map_manager_error(err, None))?;

            let child_id = metadata.session_id;
            let parent_payload = json!({
                "role": "parent",
                "child_session_id": child_id,
                "parent_session_id": parent_session_id,
                "label": metadata.label,
                "summary": metadata.summary,
                "prompt": prompt_clone,
            });

            let response = build_subagent_response(
                &metadata,
                [("model", json!(model)), ("payload", parent_payload)],
            );

            Ok(ToolOutput::Function {
                content: response.to_string(),
                content_items: None,
                success: Some(true),
            })
        }
    })
    .await
}

async fn handle_send_message(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let SendMessageArgs {
        prompt,
        label: raw_label,
        agent_id,
        interrupt,
    } = parse_args(&ctx.arguments, "subagent_send_message")?;

    if agent_id == 0 && interrupt {
        return Err(FunctionCallError::RespondToModel(
            "cannot send an interrupt to agent 0 (the root UI thread)".to_string(),
        ));
    }

    let label = normalize_label(raw_label);
    let summary = summarize_optional_prompt(&prompt);
    let sender_metadata = ctx.sender_metadata();
    let sender_agent_id = ctx.sender_agent_id();

    if agent_id == 0 {
        if sender_agent_id == ROOT_AGENT_ID {
            return Err(FunctionCallError::RespondToModel(
                "root agent cannot target agent 0; send a normal user message instead".to_string(),
            ));
        }

        let sender_metadata = sender_metadata.ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "cannot send a message to agent 0 from an unknown subagent".to_string(),
            )
        })?;

        let root_session_id = ctx.root_session_id();
        let caller_id = ctx.caller_id;
        let log_command = vec![format!(
            "Sent message to root from subagent {}",
            display_label(&label)
        )];

        let manager = ctx.manager.clone();

        run_with_logging(ctx, log_command, move || {
            let manager = manager.clone();
            let label = label.clone();
            let summary = summary.clone();
            let prompt = prompt.clone();
            let sender_metadata = sender_metadata.clone();
            async move {
                manager
                    .send_message_to_root(root_session_id, caller_id, prompt, sender_metadata)
                    .await
                    .map_err(|err| map_manager_error(err, Some(agent_id)))?;

                let response = json!({
                    "session_id": root_session_id,
                    "agent_id": agent_id,
                    "label": label,
                    "summary": summary,
                });

                Ok(ToolOutput::Function {
                    content: response.to_string(),
                    content_items: None,
                    success: Some(true),
                })
            }
        })
        .await
    } else {
        let session_id = ctx.agent_session(agent_id)?;
        let display_label = display_label_or_metadata(&label, ctx.registry_by_agent.get(&agent_id));
        let log_command = vec![format!("Sent message to subagent {display_label}")];

        let manager = ctx.manager.clone();

        run_with_logging(ctx, log_command, move || {
            let manager = manager.clone();
            let label = label.clone();
            let summary = summary.clone();
            let prompt = prompt.clone();
            async move {
                let metadata = manager
                    .send_message(SendMessageRequest {
                        session_id,
                        label,
                        summary,
                        prompt,
                        agent_id,
                        sender_agent_id,
                        interrupt,
                    })
                    .await
                    .map_err(|err| map_manager_error(err, Some(agent_id)))?;

                let response =
                    build_subagent_response(&metadata, std::iter::empty::<(&'static str, Value)>());

                Ok(ToolOutput::Function {
                    content: response.to_string(),
                    content_items: None,
                    success: Some(true),
                })
            }
        })
        .await
    }
}

async fn handle_watchdog(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let args: WatchdogArgs = parse_args(&ctx.arguments, "subagent_watchdog")?;

    let interval_raw = args.interval_s.unwrap_or(DEFAULT_WATCHDOG_INTERVAL_SECS);
    if interval_raw < MIN_WATCHDOG_INTERVAL_SECS {
        return Err(FunctionCallError::RespondToModel(format!(
            "interval_s must be at least {MIN_WATCHDOG_INTERVAL_SECS} seconds"
        )));
    }
    let message = args
        .message
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| {
            "Watchdog ping — report current status, next step, and PLAN.md progress.".to_string()
        });
    let cancel = args.cancel.unwrap_or(false);

    let caller_agent_id = if ctx.is_root_agent {
        ROOT_AGENT_ID
    } else {
        ctx.registry_by_agent
            .iter()
            .find(|(_, meta)| meta.session_id == ctx.caller_id)
            .map(|(id, _)| *id)
            .unwrap_or(ROOT_AGENT_ID)
    };

    let action = ctx
        .manager
        .watchdog_action(
            ctx.session.conversation_id(),
            caller_agent_id,
            args.agent_id,
            interval_raw,
            message.clone(),
            cancel,
        )
        .await
        .map_err(|err| map_manager_error(err, Some(args.agent_id)))?;

    let response = json!({
        "agent_id": args.agent_id,
        "interval_s": interval_raw,
        "message": message,
        "action": action,
    });

    let output = ToolOutput::Function {
        content: response.to_string(),
        content_items: None,
        success: Some(action != WatchdogAction::NotFound),
    };

    if ctx.is_root_agent {
        let verb = match action {
            WatchdogAction::Started => "Started",
            WatchdogAction::Replaced => "Replaced",
            WatchdogAction::Canceled => "Canceled",
            WatchdogAction::NotFound => "No watchdog",
        };
        let logger = ExecEventLogger::new(
            ctx.session.clone(),
            ctx.turn.clone(),
            ctx.call_id.clone(),
            vec![format!("{verb} watchdog for agent {}", args.agent_id)],
        )
        .await;
        let summary = summarize_tool_output(&ctx.tool_name, &ctx.arguments, &output);
        logger.success(&summary).await;
    }

    Ok(output)
}

async fn handle_list(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let entries = ctx
        .registry_entries
        .iter()
        .filter(|m| m.parent_session_id == Some(ctx.caller_id))
        .map(|entry| ListEntry {
            agent_id: entry.agent_id,
            parent_agent_id: Some(entry.parent_agent_id.unwrap_or(ROOT_AGENT_ID)),
            session_id: entry.session_id,
            parent_session_id: entry.parent_session_id,
            origin: entry.origin,
            status: entry.status,
            label: entry.label.clone(),
            summary: entry.summary.clone(),
            reasoning_header: entry.reasoning_header.clone(),
            started_at_ms: entry.created_at_ms,
            initial_message_count: entry.initial_message_count,
            pending_messages: entry.pending_messages,
            pending_interrupts: entry.pending_interrupts,
        })
        .collect::<Vec<_>>();

    let payload = json!({ "sessions": entries });

    run_with_logging(ctx, vec!["Listed subagents".to_string()], || async move {
        Ok(ToolOutput::Function {
            content: payload.to_string(),
            content_items: None,
            success: Some(true),
        })
    })
    .await
}

async fn handle_await(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let args: AwaitArgs = parse_args(&ctx.arguments, "subagent_await")?;
    let timeout = resolve_await_timeout(args.timeout_s)?;

    let target_session = ctx
        .registry_entries
        .iter()
        .filter(|m| m.parent_session_id == Some(ctx.caller_id))
        .max_by_key(|m| m.pending_messages)
        .map(|m| m.session_id);

    let Some(session_id) = target_session else {
        return Err(FunctionCallError::RespondToModel(
            "no active subagents to await; start one with subagent_spawn first".to_string(),
        ));
    };

    let display_label = ctx
        .registry_entries
        .iter()
        .find(|m| m.session_id == session_id)
        .and_then(|m| m.label.clone())
        .unwrap_or_else(|| "subagent".to_string());

    let manager = ctx.manager.clone();

    run_with_logging(
        ctx,
        vec![format!("Awaited subagent {display_label}")],
        move || {
            let manager = manager.clone();
            async move {
                match manager
                    .await_inbox_and_completion(&session_id, Some(timeout))
                    .await
                {
                    Ok(result) => {
                        let completion_status = result.completion.as_ref().map(completion_status);
                        let lifecycle_status = result.metadata.status;
                        let started_at_ms = result.metadata.created_at_ms;
                        let response = json!({
                            "session_id": result.metadata.session_id,
                            "completion_status": completion_status,
                            "lifecycle_status": lifecycle_status,
                            "started_at_ms": started_at_ms,
                            "timed_out": false,
                            "messages": result.messages,
                            "completion": result.completion,
                            "metadata": result.metadata,
                            "injected": false,
                        });
                        Ok(ToolOutput::Function {
                            content: response.to_string(),
                            content_items: None,
                            success: Some(true),
                        })
                    }
                    Err(SubagentManagerError::AwaitTimedOut {
                        session_id: sid, ..
                    }) => {
                        let metadata = manager.metadata(&sid).await;
                        let lifecycle_status = metadata.as_ref().map(|meta| meta.status);
                        let started_at_ms = metadata.as_ref().map(|meta| meta.created_at_ms);
                        let response = json!({
                            "session_id": sid,
                            "completion_status": None::<SubagentCompletion>,
                            "lifecycle_status": lifecycle_status,
                            "started_at_ms": started_at_ms,
                            "timed_out": true,
                            "messages": Vec::<String>::new(),
                            "completion": None::<SubagentCompletion>,
                            "metadata": metadata,
                            "injected": false,
                        });
                        Ok(ToolOutput::Function {
                            content: response.to_string(),
                            content_items: None,
                            success: Some(false),
                        })
                    }
                    Err(err) => Err(map_manager_error(err, None)),
                }
            }
        },
    )
    .await
}

async fn handle_prune(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let args: PruneArgs = parse_args(&ctx.arguments, "subagent_prune")?;
    let request = match args {
        PruneArgs::ByIds {
            agent_ids,
            completed_only,
        } => {
            let session_ids = agent_ids
                .into_iter()
                .filter_map(|id| ctx.registry_by_agent.get(&id).map(|m| m.session_id))
                .collect::<Vec<_>>();
            PruneRequest {
                session_ids: Some(session_ids),
                all: false,
                completed_only: completed_only.unwrap_or(true),
            }
        }
        PruneArgs::All {
            all,
            completed_only,
        } => PruneRequest {
            session_ids: None,
            all,
            completed_only: completed_only.unwrap_or(true),
        },
    };

    let request_echo =
        serde_json::from_str::<serde_json::Value>(&ctx.arguments).unwrap_or_default();
    let manager = ctx.manager.clone();

    run_with_logging(ctx, vec!["Pruned subagents".to_string()], move || {
        let manager = manager.clone();
        async move {
            let report = manager
                .prune(request)
                .await
                .map_err(|err| map_manager_error(err, None))?;

            let response = json!({
                "request": request_echo,
                "pruned": report.pruned,
                "skipped_active": report.skipped_active,
                "unknown": report.unknown,
                "errors": report.errors,
                "counts": {
                    "pruned": report.pruned.len(),
                    "skipped_active": report.skipped_active.len(),
                    "unknown": report.unknown.len(),
                    "errors": report.errors.len(),
                }
            });

            Ok(ToolOutput::Function {
                content: response.to_string(),
                content_items: None,
                success: Some(report.errors.is_empty()),
            })
        }
    })
    .await
}

async fn handle_logs(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let LogsArgs {
        agent_id,
        limit,
        max_bytes,
        since_ms,
        before_ms,
    } = parse_args(&ctx.arguments, "subagent_logs")?;

    let session_id = ctx.agent_session(agent_id)?;
    let display_label = display_label_or_metadata(&None, ctx.registry_by_agent.get(&agent_id));

    let manager = ctx.manager.clone();

    run_with_logging(
        ctx,
        vec![format!("Fetched subagent logs {display_label}")],
        move || {
            let manager = manager.clone();
            async move {
                let snapshot = manager
                    .snapshot_logs(&session_id)
                    .await
                    .map_err(|err| map_manager_error(err, Some(agent_id)))?;

                let response = render_logs_payload(
                    session_id, snapshot, limit, max_bytes, since_ms, before_ms,
                );

                Ok(ToolOutput::Function {
                    content: response.to_string(),
                    content_items: None,
                    success: Some(true),
                })
            }
        },
    )
    .await
}

async fn handle_cancel(ctx: &InvocationContext) -> Result<ToolOutput, FunctionCallError> {
    let CancelArgs { agent_id } = parse_args(&ctx.arguments, "subagent_cancel")?;
    let session_id = ctx.agent_session(agent_id)?;
    let display_label = display_label_or_metadata(&None, ctx.registry_by_agent.get(&agent_id));

    let manager = ctx.manager.clone();

    run_with_logging(
        ctx,
        vec![format!("Canceled subagent {display_label}")],
        move || {
            let manager = manager.clone();
            async move {
                let metadata = manager
                    .cancel(session_id)
                    .await
                    .map_err(|err| map_manager_error(err, Some(agent_id)))?;

                let response = json!({
                    "session_id": metadata.session_id,
                    "origin": metadata.origin,
                    "status": metadata.status,
                    "label": metadata.label,
                    "summary": metadata.summary,
                    "parent_session_id": metadata.parent_session_id,
                    "started_at_ms": metadata.created_at_ms,
                    "initial_message_count": metadata.initial_message_count,
                });

                Ok(ToolOutput::Function {
                    content: response.to_string(),
                    content_items: None,
                    success: Some(true),
                })
            }
        },
    )
    .await
}

pub struct SubagentToolHandler;

#[async_trait]
impl ToolHandler for SubagentToolHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "subagent tools require function arguments".to_string(),
                ));
            }
        };

        let registry = session.services.subagents.clone();
        let manager = Arc::new(session.services.subagent_manager.clone());
        let caller_id = session.conversation_id();
        let registry_entries = registry.list().await;
        let registry_by_agent: HashMap<AgentId, SubagentMetadata> = registry_entries
            .iter()
            .map(|entry| (entry.agent_id, entry.clone()))
            .collect();
        let is_root_agent = is_root_session(caller_id, &registry_by_agent);

        let ctx = InvocationContext::new(
            session,
            turn,
            call_id,
            tool_name.clone(),
            arguments.clone(),
            caller_id,
            registry_entries,
            registry_by_agent,
            manager,
            is_root_agent,
        );

        match tool_name.as_str() {
            "subagent_spawn" => handle_spawn(&ctx).await,
            "subagent_fork" => handle_fork(&ctx).await,
            "subagent_send_message" => handle_send_message(&ctx).await,
            "subagent_watchdog" => handle_watchdog(&ctx).await,
            "subagent_list" => handle_list(&ctx).await,
            "subagent_await" => handle_await(&ctx).await,
            "subagent_prune" => handle_prune(&ctx).await,
            "subagent_logs" => handle_logs(&ctx).await,
            "subagent_cancel" => handle_cancel(&ctx).await,
            _ => Err(FunctionCallError::RespondToModel(
                "unknown subagent tool".to_string(),
            )),
        }
    }
}

fn build_subagent_response(
    metadata: &SubagentMetadata,
    extras: impl IntoIterator<Item = (&'static str, Value)>,
) -> Value {
    let mut map = Map::new();
    map.insert("session_id".to_string(), json!(metadata.session_id));
    map.insert("agent_id".to_string(), json!(metadata.agent_id));
    map.insert(
        "parent_agent_id".to_string(),
        json!(metadata.parent_agent_id),
    );
    map.insert("origin".to_string(), json!(metadata.origin));
    map.insert("status".to_string(), json!(metadata.status));
    for (key, value) in extras {
        map.insert(key.to_string(), value);
    }
    map.insert("label".to_string(), json!(metadata.label));
    map.insert("summary".to_string(), json!(metadata.summary));
    map.insert(
        "parent_session_id".to_string(),
        json!(metadata.parent_session_id),
    );
    map.insert("started_at_ms".to_string(), json!(metadata.created_at_ms));
    map.insert(
        "initial_message_count".to_string(),
        json!(metadata.initial_message_count),
    );
    map.insert(
        "pending_messages".to_string(),
        json!(metadata.pending_messages),
    );
    map.insert(
        "pending_interrupts".to_string(),
        json!(metadata.pending_interrupts),
    );
    Value::Object(map)
}

/// Build the JSON payload returned by `subagent_logs` for a given log snapshot.
pub fn render_logs_payload(
    session_id: ConversationId,
    logs: Vec<LogEntry>,
    limit: Option<usize>,
    max_bytes: Option<usize>,
    since_ms: Option<i64>,
    before_ms: Option<i64>,
) -> serde_json::Value {
    // Default limit of 5 if not specified or invalid (0).
    let count = limit.unwrap_or(5).max(1);

    // Filter by since_ms / before_ms if provided.
    let filtered = logs
        .into_iter()
        .filter(|e| {
            let ts = e.timestamp_ms;
            let since_ok = since_ms.map(|s| ts > s).unwrap_or(true);
            let before_ok = before_ms.map(|b| ts < b).unwrap_or(true);
            since_ok && before_ok
        })
        .collect::<Vec<_>>();

    let total_available = filtered.len();
    let (window, truncated_by_bytes) = apply_log_window(filtered, count, max_bytes);

    let returned = window.len();
    let more_available = returned < total_available || truncated_by_bytes;
    let earliest_ms = window.first().map(|e| e.timestamp_ms);
    let latest_ms = window.last().map(|e| e.timestamp_ms);

    json!({
        "session_id": session_id,
        "returned": returned,
        "total_available": total_available,
        "more_available": more_available,
        "earliest_ms": earliest_ms,
        "latest_ms": latest_ms,
        "events": window,
    })
}

fn format_timestamp(ts_ms: i64) -> String {
    if ts_ms <= 0 {
        return "0000-00-00T00:00:00.000Z".to_string();
    }

    // Convert ms since Unix epoch to an RFC3339 timestamp with millisecond
    // precision. Clamp invalid inputs to a stable placeholder instead of
    // panicking.
    let secs = ts_ms.div_euclid(1_000);
    let millis = ts_ms.rem_euclid(1_000).unsigned_abs() as u32;
    let nanos = millis * 1_000_000;

    match Utc.timestamp_opt(secs, nanos).single() {
        Some(dt) => dt.to_rfc3339_opts(SecondsFormat::Millis, true),
        None => "0000-00-00T00:00:00.000Z".to_string(),
    }
}

/// High-level activity state derived from a log window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentActivity {
    /// The model is actively producing reasoning or message deltas.
    Working,
    /// Waiting on a tool call (exec, web, etc.) to complete.
    WaitingOnTool,
    /// No obvious in-flight work in this window.
    Idle,
}

pub fn classify_activity(logs: &[LogEntry]) -> SubagentActivity {
    let mut saw_exec_begin = false;
    let mut saw_exec_end = false;
    let mut saw_streaming = false;

    for entry in logs {
        match &entry.event.msg {
            EventMsg::ExecCommandBegin(_) => {
                saw_exec_begin = true;
            }
            EventMsg::ExecCommandEnd(ExecCommandEndEvent { .. }) => {
                saw_exec_end = true;
            }
            EventMsg::AgentMessageContentDelta(_) | EventMsg::ReasoningContentDelta(_) => {
                saw_streaming = true;
            }
            _ => {}
        }
    }

    if saw_exec_begin && !saw_exec_end {
        SubagentActivity::WaitingOnTool
    } else if saw_streaming {
        SubagentActivity::Working
    } else {
        SubagentActivity::Idle
    }
}

/// Direction for paged rendering of log lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDirection {
    /// Render the most recent lines, suitable for a "tail" view.
    Backward,
    /// Render the earliest lines in the window, suitable for forward paging
    /// given a `since_ms` cursor.
    Forward,
}

/// A page of rendered log output plus cursors for navigation.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub lines: Vec<String>,
    pub status: SubagentActivity,
    pub first_timestamp_ms: Option<i64>,
    pub last_timestamp_ms: Option<i64>,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

fn render_logs_lines(
    session_id: ConversationId,
    logs: &[LogEntry],
    _earliest_ms: Option<i64>,
    latest_ms: Option<i64>,
    _returned: usize,
    _total_available: usize,
    _more_available: bool,
) -> (SubagentActivity, Vec<String>) {
    let latest_ms = latest_ms
        .or_else(|| logs.iter().map(|e| e.timestamp_ms).max())
        .unwrap_or(0);

    let mut lines = Vec::new();
    let activity = classify_activity(logs);
    let activity_str = match activity {
        SubagentActivity::Working => "working",
        SubagentActivity::WaitingOnTool => "waiting_on_tool",
        SubagentActivity::Idle => "idle",
    };
    // Header focuses on high-level state; paging hints are added later in
    // `render_logs_page` once we know whether there is older/newer history
    // in the current window.
    lines.push(format!("Session {session_id} • status={activity_str}"));

    // Accumulators for the simple shapes we care about today.
    let mut last_assistant_message: Option<(i64, String)> = None;
    let mut reasoning_summary: Option<(i64, String)> = None;
    let mut task_complete_ts: Option<i64> = None;

    // Exec-specific / streaming accumulators.
    let mut reasoning_delta: Option<(i64, usize, String)> = None;
    let mut message_delta: Option<(i64, usize, String)> = None;
    let mut exec_begin: Option<(i64, ExecCommandBeginEvent)> = None;

    for entry in logs {
        let ts = entry.timestamp_ms;
        match &entry.event.msg {
            EventMsg::AgentMessage(ev) => {
                last_assistant_message = Some((ts, ev.message.clone()));
            }
            EventMsg::RawResponseItem(ev) => match &ev.item {
                ResponseItem::Reasoning { summary, .. } => {
                    if let Some(ReasoningItemReasoningSummary::SummaryText { text }) =
                        summary.first()
                    {
                        reasoning_summary = Some((ts, text.clone()));
                    }
                }
                ResponseItem::Message { content, .. } => {
                    if let Some(first) = content.first()
                        && let ContentItem::OutputText { text } = first
                    {
                        last_assistant_message = Some((ts, text.clone()));
                    }
                }
                _ => {}
            },
            EventMsg::TokenCount(TokenCountEvent { .. }) => {
                // Deliberately ignored in the human transcript; callers that
                // care about usage can inspect the raw JSON events instead.
            }
            EventMsg::TaskComplete(TaskCompleteEvent { .. }) => {
                task_complete_ts = Some(ts);
            }
            EventMsg::ReasoningContentDelta(ev) => {
                let entry = reasoning_delta.get_or_insert((ts, 0usize, String::new()));
                // Timestamp reflects the most recent contributing delta so the
                // rendered line is an "as of" marker, not just the start.
                entry.0 = ts;
                entry.1 = entry.1.saturating_add(1);
                entry.2.push_str(&ev.delta);
            }
            EventMsg::AgentMessageContentDelta(ev) => {
                let entry = message_delta.get_or_insert((ts, 0usize, String::new()));
                entry.0 = ts;
                entry.1 = entry.1.saturating_add(1);
                entry.2.push_str(&ev.delta);
            }
            EventMsg::ExecCommandBegin(ev) => {
                exec_begin = Some((ts, ev.clone()));
            }
            EventMsg::ItemCompleted(ItemCompletedEvent { item, .. }) => {
                if let TurnItem::Reasoning(reasoning) = item
                    && let Some(first) = reasoning.summary_text.first()
                {
                    reasoning_summary = Some((ts, first.clone()));
                }
            }
            _ => {}
        }
    }

    let has_exec = exec_begin.is_some();

    if has_exec {
        if let Some((ts, count, full_text)) = reasoning_delta {
            let ts_str = format_timestamp(ts);
            let label = if count == 1 { "delta" } else { "deltas" };
            lines.push(format!("{ts_str} Thinking: {full_text} ({count} {label})"));
        }

        if let Some((ts, summary)) = reasoning_summary {
            let ts_str = format_timestamp(ts);
            lines.push(format!("{ts_str} Reasoning summary: {summary}"));
        }

        if let Some((ts, begin)) = exec_begin {
            let ts_str = format_timestamp(ts);
            let dur_ms = (latest_ms - ts).max(0);
            let dur_secs = dur_ms as f64 / 1000.0;
            lines.push(format!(
                "{ts_str} 🛠 exec {} · cwd={} · running ({dur_secs:.1}s)",
                begin.command.join(" "),
                begin.cwd.display(),
            ));
        }
    } else {
        if let Some((ts, count, full_text)) = reasoning_delta {
            let ts_str = format_timestamp(ts);
            let label = if count == 1 { "delta" } else { "deltas" };
            lines.push(format!("{ts_str} Thinking: {full_text} ({count} {label})"));
        }

        if let Some((ts, count, full_text)) = message_delta {
            let ts_str = format_timestamp(ts);
            if count <= 1 {
                lines.push(format!(
                    "{ts_str} Assistant (typing): {full_text} (1 chunk)"
                ));
            } else {
                lines.push(format!(
                    "{ts_str} Assistant (typing): {full_text} ({count} chunks)"
                ));
            }
        }

        if let Some((ts, msg)) = last_assistant_message {
            let ts_str = format_timestamp(ts);
            lines.push(format!("{ts_str} Assistant: {msg}"));
        }

        if let Some((ts, summary)) = reasoning_summary {
            let ts_str = format_timestamp(ts);
            lines.push(format!("{ts_str} Thinking: {summary}"));
        }

        if let Some(ts) = task_complete_ts {
            let ts_str = format_timestamp(ts);
            lines.push(format!("{ts_str} Task complete"));
        }
    }

    (activity, lines)
}

/// Render a single paged view over a log window, returning both the rendered
/// lines and cursor information for paging.
#[allow(clippy::too_many_arguments)]
pub fn render_logs_page(
    session_id: ConversationId,
    logs: &[LogEntry],
    earliest_ms: Option<i64>,
    latest_ms: Option<i64>,
    returned: usize,
    total_available: usize,
    more_available: bool,
    max_lines: usize,
    direction: PageDirection,
) -> RenderedPage {
    let max_lines = max_lines.max(1);
    let (status, full_lines) = render_logs_lines(
        session_id,
        logs,
        earliest_ms,
        latest_ms,
        returned,
        total_available,
        more_available,
    );

    if full_lines.is_empty() {
        return RenderedPage {
            lines: Vec::new(),
            status,
            first_timestamp_ms: earliest_ms,
            last_timestamp_ms: latest_ms,
            has_more_before: false,
            has_more_after: false,
        };
    }

    // Preserve the header, page the body.
    let mut iter = full_lines.into_iter();
    let mut header = iter.next().unwrap_or_default();
    let body: Vec<String> = iter.collect();
    let body_len = body.len();

    let trimmed_body: Vec<String> = if body_len <= max_lines {
        body
    } else {
        match direction {
            PageDirection::Backward => body[body_len - max_lines..].to_vec(),
            PageDirection::Forward => body[..max_lines].to_vec(),
        }
    };

    let mut lines = Vec::with_capacity(1 + trimmed_body.len());
    lines.push(header.clone());
    lines.extend(trimmed_body);

    let first_timestamp_ms = earliest_ms.or_else(|| logs.first().map(|e| e.timestamp_ms));
    let last_timestamp_ms = latest_ms.or_else(|| logs.last().map(|e| e.timestamp_ms));

    let has_more_before =
        matches!(direction, PageDirection::Backward) && total_available > returned;
    let has_more_after = matches!(direction, PageDirection::Forward) && more_available;

    // Extend the header with explicit paging hints so callers can tell
    // whether there is older history or whether this page is at the
    // latest known point in time, without reasoning about event
    // counts. We deliberately avoid talking about "newer logs" to
    // prevent confusion while the subagent is still working.
    let older = if has_more_before { "true" } else { "false" };
    let at_latest = if has_more_after { "false" } else { "true" };
    header.push_str(&format!(" • older_logs={older} • at_latest={at_latest}"));

    if let Some(first) = lines.first_mut() {
        *first = header;
    }

    RenderedPage {
        lines,
        status,
        first_timestamp_ms,
        last_timestamp_ms,
        has_more_before,
        has_more_after,
    }
}

/// Render a human-friendly transcript for a subagent_logs payload.
///
/// This is intentionally conservative and focuses on the common shapes we
/// currently exercise in tests: simple assistant replies, reasoning summaries,
/// token counts, and exec_command_begin events.
pub fn render_logs_as_text(
    session_id: ConversationId,
    logs: &[LogEntry],
    earliest_ms: Option<i64>,
    latest_ms: Option<i64>,
    returned: usize,
    total_available: usize,
    more_available: bool,
) -> String {
    let page = render_logs_page(
        session_id,
        logs,
        earliest_ms,
        latest_ms,
        returned,
        total_available,
        more_available,
        usize::MAX,
        PageDirection::Backward,
    );

    page.lines.join("\n")
}

/// Variant of `render_logs_as_text` that applies a `max_lines` budget and a
/// paging direction. This is intended for UIs that want a bounded transcript
/// (for example, the last 30 lines) while still reusing the same aggregation
/// logic as the full-text renderer.
#[allow(clippy::too_many_arguments)]
pub fn render_logs_as_text_with_max_lines(
    session_id: ConversationId,
    logs: &[LogEntry],
    earliest_ms: Option<i64>,
    latest_ms: Option<i64>,
    returned: usize,
    total_available: usize,
    more_available: bool,
    max_lines: usize,
    direction: PageDirection,
) -> String {
    let page = render_logs_page(
        session_id,
        logs,
        earliest_ms,
        latest_ms,
        returned,
        total_available,
        more_available,
        max_lines,
        direction,
    );

    page.lines.join("\n")
}

fn map_manager_error(
    err: SubagentManagerError,
    agent_id_hint: Option<AgentId>,
) -> FunctionCallError {
    match err {
        SubagentManagerError::NotFound => {
            if let Some(agent_id) = agent_id_hint {
                FunctionCallError::RespondToModel(format!(
                    "agent_id {agent_id} is no longer active; refresh subagent_list"
                ))
            } else {
                FunctionCallError::RespondToModel("unknown session id".to_string())
            }
        }
        SubagentManagerError::LaunchFailed(message) => {
            FunctionCallError::RespondToModel(format!("subagent launch failed: {message}"))
        }
        SubagentManagerError::LimitReached { limit } => FunctionCallError::RespondToModel(format!(
            "subagent limit reached: at most {limit} active subagents per session"
        )),
        SubagentManagerError::AwaitTimedOut {
            session_id,
            agent_id,
            timeout_ms,
        } => FunctionCallError::RespondToModel(format!(
            "timed out after {timeout_ms} ms waiting for agent_id {agent_id} (session {session_id})"
        )),
        SubagentManagerError::InvalidPruneRequest(message) => {
            FunctionCallError::RespondToModel(message)
        }
        SubagentManagerError::SandboxOverrideForbidden { requested, parent } => {
            FunctionCallError::RespondToModel(format!(
                "sandbox_mode {requested:?} not permitted; parent session runs in {parent:?}"
            ))
        }
        SubagentManagerError::AgentIdMismatch {
            session_id,
            agent_id,
        } => FunctionCallError::RespondToModel(format!(
            "agent_id {agent_id} mismatch for session {session_id}: refresh subagent_list before sending"
        )),
    }
}

fn normalize_label(label: Option<String>) -> Option<String> {
    label
        .map(|s| collapse_whitespace(s.trim()))
        .filter(|s| !s.is_empty())
}

fn summarize_prompt(prompt: &str) -> Option<String> {
    let collapsed = collapse_whitespace(prompt.trim());
    if collapsed.is_empty() {
        return None;
    }
    let mut summary = collapsed.chars().take(80).collect::<String>();
    if collapsed.chars().count() > summary.chars().count() {
        summary.push('…');
    }
    Some(summary)
}

fn summarize_optional_prompt(prompt: &Option<String>) -> Option<String> {
    prompt.as_deref().and_then(summarize_prompt)
}

fn resolve_await_timeout(timeout_s: Option<u64>) -> Result<Duration, FunctionCallError> {
    let raw = timeout_s.unwrap_or(MAX_AWAIT_TIMEOUT_SECS);
    let bounded = if raw == 0 {
        MAX_AWAIT_TIMEOUT_SECS
    } else {
        raw
    };
    if bounded < 300 {
        return Err(FunctionCallError::RespondToModel(
            "subagent_await timeout must be at least 300 seconds; prefer 5–30 minutes unless you have nothing else to do meanwhile.".to_string(),
        ));
    }
    if bounded > MAX_AWAIT_TIMEOUT_SECS {
        return Err(FunctionCallError::RespondToModel(format!(
            "subagent_await timeout_secs ({bounded}s) exceeds the 30-minute limit ({MAX_AWAIT_TIMEOUT_SECS}s). Use a shorter timeout or omit it to use the default."
        )));
    }
    Ok(Duration::from_secs(bounded))
}

fn require_agent_session(
    registry: &HashMap<AgentId, SubagentMetadata>,
    agent_id: AgentId,
) -> Result<ConversationId, FunctionCallError> {
    registry
        .get(&agent_id)
        .map(|m| m.session_id)
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(format!(
                "agent_id {agent_id} not found; refresh subagent_list"
            ))
        })
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn completion_status(completion: &SubagentCompletion) -> &'static str {
    match completion {
        SubagentCompletion::Completed { .. } => "completed",
        SubagentCompletion::Canceled { .. } => "canceled",
        SubagentCompletion::Failed { .. } => "failed",
    }
}

#[derive(Deserialize)]
struct LogsArgs {
    agent_id: AgentId,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    max_bytes: Option<usize>,
    #[serde(default)]
    since_ms: Option<i64>,
    #[serde(default)]
    before_ms: Option<i64>,
}

fn apply_log_window(
    entries: Vec<LogEntry>,
    limit: usize,
    max_bytes: Option<usize>,
) -> (Vec<LogEntry>, bool) {
    let total = entries.len();
    let start = total.saturating_sub(limit);
    let mut window = entries[start..].to_vec();

    let mut truncated_by_bytes = false;
    if let Some(cap) = max_bytes
        && cap > 0
    {
        let mut used: usize = 2; // for []
        let mut keep: Vec<LogEntry> = Vec::new();
        for (idx, entry) in window.iter().enumerate() {
            if let Ok(s) = serde_json::to_string(entry) {
                let extra = s.len() + if idx == 0 { 0 } else { 1 };
                if used + extra > cap {
                    truncated_by_bytes = true;
                    break;
                }
                used += extra;
                keep.push(entry.clone());
            } else {
                truncated_by_bytes = true;
                break;
            }
        }
        window = keep;
    }
    (window, truncated_by_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::AgentId;
    use codex_protocol::ConversationId;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::Event;
    use codex_protocol::protocol::EventMsg;
    use std::time::SystemTime;

    fn make_event(message: &str, ts: i64) -> LogEntry {
        LogEntry {
            timestamp_ms: ts,
            event: Event {
                id: format!("e-{ts}"),
                msg: EventMsg::AgentMessage(AgentMessageEvent {
                    message: message.to_string(),
                }),
            },
        }
    }

    #[test]
    fn render_logs_page_trims_tail_for_backward() {
        let entries = vec![
            make_event("one", 1),
            make_event("two", 2),
            make_event("three", 3),
            make_event("four", 4),
        ];
        let earliest_ms = entries.first().map(|e| e.timestamp_ms);
        let latest_ms = entries.last().map(|e| e.timestamp_ms);

        let page = render_logs_page(
            ConversationId::new(),
            &entries,
            earliest_ms,
            latest_ms,
            entries.len(),
            entries.len() + 2, // pretend there are older events outside the window
            true,
            2,
            PageDirection::Backward,
        );

        // Header + a single assistant line summarizing the tail.
        assert_eq!(page.lines.len(), 2);
        assert!(page.lines[1].contains("four"));
        assert!(page.has_more_before);
        assert!(!page.has_more_after);
    }

    #[test]
    fn render_logs_page_trims_head_for_forward() {
        let entries = vec![
            make_event("one", 1),
            make_event("two", 2),
            make_event("three", 3),
            make_event("four", 4),
        ];
        let earliest_ms = entries.first().map(|e| e.timestamp_ms);
        let latest_ms = entries.last().map(|e| e.timestamp_ms);

        let page = render_logs_page(
            ConversationId::new(),
            &entries,
            earliest_ms,
            latest_ms,
            entries.len(),
            entries.len(),
            false,
            2,
            PageDirection::Forward,
        );

        // Header + a single assistant line summarizing the window.
        assert_eq!(page.lines.len(), 2);
        assert!(page.lines[1].contains("four"));
        assert!(!page.has_more_before);
        assert!(!page.has_more_after);
    }

    #[test]
    fn apply_log_window_respects_limit_latest() {
        let entries = vec![
            make_event("a", 1),
            make_event("b", 2),
            make_event("c", 3),
            make_event("d", 4),
        ];
        let (window, truncated) = apply_log_window(entries, 2, None);
        assert!(!truncated);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].timestamp_ms, 3);
        assert_eq!(window[1].timestamp_ms, 4);
    }

    #[test]
    fn apply_log_window_respects_byte_cap() {
        let entries = vec![
            make_event("short", 1),
            make_event("also short", 2),
            make_event("this is a bit longer message", 3),
        ];
        // Compute a cap that fits exactly the first item but not two.
        let first_size = serde_json::to_string(&entries[0]).unwrap().len() + 2; // +2 for []
        let cap = first_size + 1; // allow first, not second
        let (window, truncated) = apply_log_window(entries.clone(), 10, Some(cap));
        assert!(truncated);
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].timestamp_ms, 1);

        // Larger cap computed to fit all entries.
        let total_estimate: usize = entries
            .iter()
            .map(|e| serde_json::to_string(e).unwrap().len())
            .sum::<usize>()
            + (entries.len().saturating_sub(1)) // commas
            + 2; // brackets
        let (window2, truncated2) = apply_log_window(entries, 10, Some(total_estimate + 10));
        assert!(!truncated2);
        assert_eq!(window2.len(), 3);
        assert_eq!(window2[0].timestamp_ms, 1);
        assert_eq!(window2[2].timestamp_ms, 3);
    }

    #[test]
    fn summarize_optional_prompt_handles_none() {
        assert!(summarize_optional_prompt(&None).is_none());
        let prompt = Some("explain tests".to_string());
        assert_eq!(
            summarize_optional_prompt(&prompt),
            Some("explain tests".to_string())
        );
    }

    #[test]
    fn resolve_timeout_defaults_to_max() {
        let duration = resolve_await_timeout(None).unwrap();
        assert_eq!(duration.as_secs(), MAX_AWAIT_TIMEOUT_SECS);
    }

    #[test]
    fn resolve_timeout_accepts_zero_as_max() {
        let duration = resolve_await_timeout(Some(0)).unwrap();
        assert_eq!(duration.as_secs(), MAX_AWAIT_TIMEOUT_SECS);
    }

    #[test]
    fn resolve_timeout_rejects_small_values() {
        let err = resolve_await_timeout(Some(299)).unwrap_err();
        match err {
            FunctionCallError::RespondToModel(msg) => {
                assert!(msg.contains("300"), "unexpected message: {msg}");
            }
            other => panic!("expected RespondToModel error, got {other:?}"),
        }
    }

    #[test]
    fn resolve_timeout_rejects_excessive_values() {
        let err = resolve_await_timeout(Some(MAX_AWAIT_TIMEOUT_SECS + 1)).unwrap_err();
        match err {
            FunctionCallError::RespondToModel(msg) => {
                assert!(msg.contains("30-minute"), "unexpected message: {msg}");
            }
            other => panic!("expected RespondToModel error, got {other:?}"),
        }
    }

    fn _metadata(
        session_id: ConversationId,
        parent_session_id: Option<ConversationId>,
        agent_id: AgentId,
    ) -> SubagentMetadata {
        SubagentMetadata {
            agent_id,
            parent_agent_id: Some(0),
            session_id,
            parent_session_id,
            origin: SubagentOrigin::Spawn,
            initial_message_count: 0,
            status: SubagentStatus::Running,
            created_at: SystemTime::now(),
            created_at_ms: 0,
            session_key: session_id.to_string(),
            label: None,
            summary: None,
            reasoning_header: None,
            pending_messages: 0,
            pending_interrupts: 0,
        }
    }
}
