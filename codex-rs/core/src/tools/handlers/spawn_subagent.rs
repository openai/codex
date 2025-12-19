use async_trait::async_trait;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
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
- Respond with a concise, plain-text answer to the prompt."#;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnSubagentArgs {
    prompt: String,
    label: Option<String>,
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

        let args: SpawnSubagentArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;

        let prompt = args.prompt.trim();
        if prompt.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "prompt must be non-empty".to_string(),
            ));
        }

        let label = sanitize_label(args.label.as_deref());
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
        cfg.sandbox_policy = codex_protocol::protocol::SandboxPolicy::ReadOnly;

        let input = vec![UserInput::Text {
            text: prompt.to_string(),
        }];

        let cancel = CancellationToken::new();
        let io = run_codex_conversation_one_shot(
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
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to start subagent: {err}"))
        })?;

        let response = collect_subagent_response(io.rx_event).await;

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

async fn collect_subagent_response(rx_event: async_channel::Receiver<Event>) -> String {
    let mut last_agent_message: Option<String> = None;
    while let Ok(event) = rx_event.recv().await {
        match event.msg {
            EventMsg::TaskComplete(ev) => {
                last_agent_message = ev.last_agent_message;
                break;
            }
            EventMsg::TurnAborted(_) => break,
            _ => {}
        }
    }
    last_agent_message.unwrap_or_default().trim().to_string()
}
