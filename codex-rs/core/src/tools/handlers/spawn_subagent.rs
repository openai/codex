use async_trait::async_trait;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentInvocation;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::SubAgentToolCallActivityEvent;
use codex_protocol::protocol::SubAgentToolCallBeginEvent;
use codex_protocol::protocol::SubAgentToolCallEndEvent;
use codex_protocol::protocol::SubAgentToolCallTokensEvent;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::codex_delegate::run_codex_conversation_one_shot;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub(crate) const SPAWN_SUBAGENT_TOOL_NAME: &str = "spawn_subagent";
pub(crate) const SPAWN_SUBAGENT_LABEL_PREFIX: &str = "spawn_subagent";

const SUBAGENT_DEVELOPER_PROMPT: &str = r#"You are a read-only subagent. You run in a restricted sandbox and must not modify files.

Hard rules:
- Do not ask the user questions.
- Do not propose or perform edits. Do not call apply_patch.
- Do not call spawn_subagent.
- You may explore the repo with read-only commands, but keep it minimal and avoid dumping large files.

Role:
You are a read-only subagent for Codex. Given the user's prompt, use the available tools to research and report back. Do what was asked; nothing more, nothing less.

Strengths:
- Searching for code, configurations, and patterns across large codebases.
- Investigating questions that require exploring multiple files.
- Summarizing findings with concrete evidence (file references + small snippets).

Guidelines:
- Start broad, then narrow down. Try multiple search strategies if the first attempt does not yield results.
- Prefer `rg` for searching; prefer targeted reads of specific files (avoid dumping large files).
- Be thorough, but keep evidence compact: include only the few most relevant snippets (small excerpts).
- Never create or modify files.
- Avoid emojis.
- In the final response, include relevant file paths and small code snippets. Prefer workspace-relative paths."#;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnSubagentArgs {
    description: String,
    prompt: String,
    label: Option<String>,
}

pub(crate) fn parse_spawn_subagent_invocation(
    arguments: &str,
) -> Result<SubAgentInvocation, String> {
    let args: SpawnSubagentArgs = serde_json::from_str(arguments)
        .map_err(|e| format!("failed to parse function arguments: {e:?}"))?;

    let description = normalize_description(&args.description);
    if description.is_empty() {
        return Err("description must be non-empty".to_string());
    }

    let prompt = args.prompt.trim();
    if prompt.is_empty() {
        return Err("prompt must be non-empty".to_string());
    }

    let label = sanitize_label(args.label.as_deref());

    Ok(SubAgentInvocation {
        description,
        label,
        prompt: prompt.to_string(),
    })
}

pub struct SpawnSubagentHandler;

#[async_trait]
impl ToolHandler for SpawnSubagentHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            tool_name,
            ..
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for {tool_name}"
            )));
        };

        let source = turn.client.get_session_source();
        if let SessionSource::SubAgent(_) = source {
            return Err(FunctionCallError::RespondToModel(
                "spawn_subagent is not supported inside subagents".to_string(),
            ));
        }

        let invocation = parse_spawn_subagent_invocation(&arguments)
            .map_err(FunctionCallError::RespondToModel)?;
        let label = invocation.label.clone();
        let subagent_label = format!("{SPAWN_SUBAGENT_LABEL_PREFIX}_{label}");

        let mut cfg = turn.client.config().as_ref().clone();
        cfg.developer_instructions = Some(build_subagent_developer_instructions(
            cfg.developer_instructions.as_deref().unwrap_or_default(),
        ));
        cfg.model = Some(turn.client.get_model());
        cfg.model_reasoning_effort = turn.client.get_reasoning_effort();
        cfg.model_reasoning_summary = turn.client.get_reasoning_summary();

        let mut features = cfg.features.clone();
        features.disable(Feature::ApplyPatchFreeform);
        cfg.features = features;
        cfg.approval_policy =
            crate::config::Constrained::allow_any(codex_protocol::protocol::AskForApproval::Never);
        cfg.sandbox_policy = crate::config::Constrained::allow_any(
            codex_protocol::protocol::SandboxPolicy::ReadOnly,
        );

        session
            .send_event(
                turn.as_ref(),
                EventMsg::SubAgentToolCallBegin(SubAgentToolCallBeginEvent {
                    call_id: call_id.clone(),
                    invocation: invocation.clone(),
                }),
            )
            .await;
        session
            .send_event(
                turn.as_ref(),
                EventMsg::SubAgentToolCallActivity(SubAgentToolCallActivityEvent {
                    call_id: call_id.clone(),
                    activity: "starting".to_string(),
                }),
            )
            .await;

        let started_at = Instant::now();
        let cancel = session
            .turn_cancellation_token(&turn.sub_id)
            .await
            .map_or_else(CancellationToken::new, |token| token.child_token());
        let _cancel_guard = CancelOnDrop::new(cancel.clone());

        let input = vec![UserInput::Text {
            text: invocation.prompt.clone(),
        }];

        let io = match run_codex_conversation_one_shot(
            cfg,
            Arc::clone(&session.services.auth_manager),
            Arc::clone(&session.services.models_manager),
            input,
            Arc::clone(&session),
            Arc::clone(&turn),
            cancel,
            None,
            SubAgentSource::Other(subagent_label),
        )
        .await
        {
            Ok(io) => io,
            Err(err) => {
                let message = format!("failed to start subagent: {err}");
                session
                    .send_event(
                        turn.as_ref(),
                        EventMsg::SubAgentToolCallEnd(SubAgentToolCallEndEvent {
                            call_id: call_id.clone(),
                            invocation: invocation.clone(),
                            duration: started_at.elapsed(),
                            tokens: None,
                            result: Err(message.clone()),
                        }),
                    )
                    .await;
                return Err(FunctionCallError::RespondToModel(message));
            }
        };

        let mut last_agent_message: Option<String> = None;
        let mut last_activity: Option<String> = None;
        let mut tokens: i64 = 0;
        let mut last_reported_tokens: Option<i64> = None;
        let mut last_reported_at = Instant::now();
        while let Ok(event) = io.rx_event.recv().await {
            let Event { id: _, msg } = event;

            if let Some(activity) = activity_for_event(&msg)
                && last_activity.as_deref() != Some(activity.as_str())
            {
                last_activity = Some(activity.clone());
                session
                    .send_event(
                        turn.as_ref(),
                        EventMsg::SubAgentToolCallActivity(SubAgentToolCallActivityEvent {
                            call_id: call_id.clone(),
                            activity,
                        }),
                    )
                    .await;
            }

            match msg {
                EventMsg::TaskComplete(ev) => {
                    last_agent_message = ev.last_agent_message;
                    break;
                }
                EventMsg::TurnAborted(_) => break,
                EventMsg::TokenCount(TokenCountEvent {
                    info: Some(info), ..
                }) => {
                    tokens = tokens.saturating_add(info.last_token_usage.total_tokens.max(0));
                    let now = Instant::now();
                    let should_report =
                        match (last_reported_tokens, last_reported_at.elapsed().as_secs()) {
                            (Some(prev), secs) => {
                                tokens > prev && (tokens - prev >= 250 || secs >= 2)
                            }
                            (None, _) => tokens > 0,
                        };
                    if should_report {
                        session
                            .send_event(
                                turn.as_ref(),
                                EventMsg::SubAgentToolCallTokens(SubAgentToolCallTokensEvent {
                                    call_id: call_id.clone(),
                                    tokens,
                                }),
                            )
                            .await;
                        last_reported_tokens = Some(tokens);
                        last_reported_at = now;
                    }
                }
                _ => {}
            }
        }

        let response = last_agent_message.unwrap_or_default().trim().to_string();
        let tokens = if tokens > 0 { Some(tokens) } else { None };
        let result = Ok(response.clone());
        session
            .send_event(
                turn.as_ref(),
                EventMsg::SubAgentToolCallEnd(SubAgentToolCallEndEvent {
                    call_id,
                    invocation,
                    duration: started_at.elapsed(),
                    tokens,
                    result: result.clone(),
                }),
            )
            .await;

        Ok(ToolOutput::Function {
            content: json!({
                "label": label,
                "response": response,
            })
            .to_string(),
            content_items: None,
            success: Some(true),
        })
    }
}

fn fmt_exec_activity_command(command: &[String]) -> String {
    if command.is_empty() {
        return "shell".to_string();
    }

    let cmd = if let Some((_shell, script)) = crate::parse_command::extract_shell_command(command) {
        let script = script.trim();
        if script.is_empty() {
            "shell".to_string()
        } else {
            script
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }
    } else {
        crate::parse_command::shlex_join(command)
    };

    if cmd.is_empty() {
        "shell".to_string()
    } else {
        cmd
    }
}

fn activity_for_event(msg: &EventMsg) -> Option<String> {
    match msg {
        EventMsg::TaskStarted(_) => Some("starting".to_string()),
        EventMsg::UserMessage(_) => Some("sending prompt".to_string()),
        EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_) => Some("thinking".to_string()),
        EventMsg::AgentMessage(_) | EventMsg::AgentMessageDelta(_) => Some("writing".to_string()),
        EventMsg::ExecCommandBegin(ev) => Some(fmt_exec_activity_command(&ev.command)),
        EventMsg::McpToolCallBegin(ev) => Some(format!(
            "mcp {}/{}",
            ev.invocation.server.trim(),
            ev.invocation.tool.trim()
        )),
        EventMsg::WebSearchBegin(_) => Some("web_search".to_string()),
        _ => None,
    }
}

fn build_subagent_developer_instructions(existing: &str) -> String {
    let existing = existing.trim();
    if existing.is_empty() {
        return SUBAGENT_DEVELOPER_PROMPT.to_string();
    }
    format!("{SUBAGENT_DEVELOPER_PROMPT}\n\n{existing}")
}

fn sanitize_label(label: Option<&str>) -> String {
    let raw = label.unwrap_or_default().trim();
    let mut sanitized = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            sanitized.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            sanitized.push('_');
        }
    }
    if sanitized.is_empty() {
        return "subagent".to_string();
    }
    const MAX_LEN: usize = 64;
    if sanitized.len() > MAX_LEN {
        sanitized.truncate(MAX_LEN);
    }
    sanitized
}

fn normalize_description(description: &str) -> String {
    description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

struct CancelOnDrop {
    token: CancellationToken,
}

impl CancelOnDrop {
    fn new(token: CancellationToken) -> Self {
        Self { token }
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_requires_description() {
        let err = parse_spawn_subagent_invocation(r#"{"prompt":"hi"}"#).unwrap_err();
        assert!(
            err.contains("description"),
            "expected description error, got: {err}"
        );
    }

    #[test]
    fn parse_normalizes_description_whitespace() {
        let invocation = parse_spawn_subagent_invocation(
            r#"{"description":"  find \n  usage  docs  ","prompt":"  Hello  ","label":"My Label"}"#,
        )
        .expect("parse");

        assert_eq!(invocation.description, "find usage docs");
        assert_eq!(invocation.prompt, "Hello");
        assert_eq!(invocation.label, "my_label");
    }
}
