use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;
use tracing::warn;

use crate::Prompt;
use crate::ResponseEvent;
use crate::agent::AgentRegistry;
use crate::codex::Session;
use crate::function_tool::FunctionCallError;
use crate::protocol::AgentBeginEvent;
use crate::protocol::AgentEndEvent;
use crate::protocol::AgentMessageContentDeltaEvent;
use crate::protocol::AgentReasoningSectionBreakEvent;
use crate::protocol::AgentStatus;
use crate::protocol::EventMsg;
use crate::protocol::ReasoningContentDeltaEvent;
use crate::protocol::ReasoningRawContentDeltaEvent;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;

#[derive(Debug, Deserialize)]
struct AgentCall {
    #[serde(alias = "agent", alias = "agent_name", alias = "name")]
    name: String,
    #[serde(
        alias = "task",
        alias = "input",
        alias = "instruction",
        alias = "message"
    )]
    task: String,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    plan_item_id: Option<String>,
}

fn assistant_text(item: &ResponseItem) -> Option<String> {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let mut buf = String::new();
        for part in content {
            if let ContentItem::OutputText { text } = part {
                buf.push_str(text);
            }
        }
        if !buf.is_empty() {
            return Some(buf);
        }
    }
    None
}

fn agent_registry(session: &Session) -> &AgentRegistry {
    session.agent_registry()
}

pub struct AgentHandler;

#[async_trait]
impl ToolHandler for AgentHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(
                    "agent tool invoked with incompatible payload".to_string(),
                ));
            }
        };

        let parsed: AgentCall = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("invalid agent call arguments: {e}"))
        })?;

        let AgentCall {
            name,
            task,
            context,
            plan_item_id,
        } = parsed;

        // Prevent recursion by refusing to run if the tool list already excludes agent tool.
        if matches!(turn.client.get_session_source(), SessionSource::SubAgent(_)) {
            warn!("agent tool recursion detected");
            return Err(FunctionCallError::RespondToModel(
                "Agents cannot spawn other agents".to_string(),
            ));
        }

        let registry = agent_registry(session.as_ref());
        let agent_prompt = registry.get_system_prompt(&name);

        let begin_event = AgentBeginEvent {
            call_id: call_id.clone(),
            agent_name: name.clone(),
            task: task.clone(),
            parent_context: None,
            plan_item_id: plan_item_id.clone(),
        };
        session
            .send_event(turn.as_ref(), EventMsg::AgentBegin(begin_event))
            .await;

        let base_instructions = turn
            .base_instructions
            .clone()
            .unwrap_or_else(|| turn.client.get_model_family().base_instructions);
        let instructions = format!(
            "{base_instructions}\n\nYou are the \"{name}\" agent.\n{agent_prompt}\n\nDo not delegate to other agents. Provide the result directly.",
        );

        let mut task_text = task.clone();
        if let Some(ctx) = context.as_ref().filter(|ctx| !ctx.trim().is_empty()) {
            task_text = format!("{task_text}\n\nContext:\n{ctx}");
        }

        let prompt = Prompt {
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: task_text }],
            }],
            tools: Vec::new(),
            parallel_tool_calls: false,
            base_instructions_override: Some(instructions),
            output_schema: None,
        };

        let mut stream = turn
            .client
            .clone()
            .stream(&prompt)
            .await
            .map_err(|e| FunctionCallError::Fatal(e.to_string()))?;

        let started = Instant::now();
        let mut message = String::new();
        let mut item_id = call_id.clone();
        if item_id.is_empty() {
            item_id = "agent-call".to_string();
        }

        while let Some(event) = stream.next().await {
            let event = event.map_err(|e| FunctionCallError::Fatal(e.to_string()))?;
            match event {
                ResponseEvent::Created => {}
                ResponseEvent::OutputItemDone(item) | ResponseEvent::OutputItemAdded(item) => {
                    if message.is_empty()
                        && let Some(text) = assistant_text(&item)
                    {
                        message.push_str(&text);
                    }
                }
                ResponseEvent::OutputTextDelta(delta) => {
                    message.push_str(&delta);
                    let evt = AgentMessageContentDeltaEvent {
                        thread_id: session.conversation_id().to_string(),
                        turn_id: turn.sub_id.clone(),
                        item_id: item_id.clone(),
                        delta,
                    };
                    session
                        .send_event(turn.as_ref(), EventMsg::AgentMessageContentDelta(evt))
                        .await;
                }
                ResponseEvent::ReasoningSummaryDelta {
                    delta,
                    summary_index,
                } => {
                    let evt = ReasoningContentDeltaEvent {
                        thread_id: session.conversation_id().to_string(),
                        turn_id: turn.sub_id.clone(),
                        item_id: item_id.clone(),
                        delta,
                        summary_index,
                    };
                    session
                        .send_event(turn.as_ref(), EventMsg::ReasoningContentDelta(evt))
                        .await;
                }
                ResponseEvent::ReasoningSummaryPartAdded { summary_index } => {
                    session
                        .send_event(
                            turn.as_ref(),
                            EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
                                item_id: item_id.clone(),
                                summary_index,
                            }),
                        )
                        .await;
                }
                ResponseEvent::ReasoningContentDelta {
                    delta,
                    content_index,
                } => {
                    let evt = ReasoningRawContentDeltaEvent {
                        thread_id: session.conversation_id().to_string(),
                        turn_id: turn.sub_id.clone(),
                        item_id: item_id.clone(),
                        delta,
                        content_index,
                    };
                    session
                        .send_event(turn.as_ref(), EventMsg::ReasoningRawContentDelta(evt))
                        .await;
                }
                ResponseEvent::RateLimits(snapshot) => {
                    session.update_rate_limits(turn.as_ref(), snapshot).await;
                }
                ResponseEvent::Completed {
                    response_id: _,
                    token_usage,
                } => {
                    session
                        .update_token_usage_info(turn.as_ref(), token_usage.as_ref())
                        .await;
                    break;
                }
            }
        }

        if message.is_empty() {
            message = "Agent completed without output.".to_string();
        }

        let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        session
            .send_event(
                turn.as_ref(),
                EventMsg::AgentMessage(crate::protocol::AgentMessageEvent {
                    message: message.clone(),
                }),
            )
            .await;
        session
            .send_event(
                turn.as_ref(),
                EventMsg::AgentEnd(AgentEndEvent {
                    call_id,
                    agent_name: name,
                    summary: message.clone(),
                    status: AgentStatus::Done,
                    duration_ms,
                    plan_item_id,
                }),
            )
            .await;

        Ok(ToolOutput::Function {
            content: message,
            content_items: None,
            success: Some(true),
        })
    }
}
