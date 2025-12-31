//! Subagent delegate execution using Codex::spawn().
//!
//! This module provides the core functionality to run subagents using the
//! full Codex session infrastructure instead of the limited AgentExecutor.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_channel::Sender;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use crate::AuthManager;
use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_conversation_interactive;
use crate::config::Config;
use crate::error::CodexErr;
use crate::models_manager::manager::ModelsManager;
use crate::subagent::SubagentActivityEvent;
use crate::subagent::SubagentConfig;
use crate::subagent::SubagentEventType;
use crate::subagent::SubagentResult;
use crate::subagent::SubagentStatus;
use crate::subagent::TranscriptStore;

/// Run a subagent using the Codex delegate pattern.
///
/// This function spawns a full Codex session with filtered tools based on
/// the SubagentConfig, executes the prompt, and returns the result.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_subagent_delegate(
    config: SubagentConfig,
    prompt: String,
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
    event_sender: Option<Sender<SubagentActivityEvent>>,
    transcript_store: Option<&TranscriptStore>,
    _resume_agent_id: Option<&str>,
) -> Result<SubagentResult, CodexErr> {
    let start_time = Instant::now();
    let agent_id = generate_agent_id(&config.definition.agent_type);
    let agent_type = config.definition.agent_type.clone();
    let max_turns = config.max_turns();
    let max_time = Duration::from_secs(config.max_time_seconds() as u64);

    // Send started event
    if let Some(sender) = &event_sender {
        let _ = sender
            .send(SubagentActivityEvent::started(
                &agent_id,
                &agent_type,
                &format!("Starting {agent_type} agent"),
            ))
            .await;
    }

    // Build the modified config for subagent
    let subagent_codex_config = build_subagent_codex_config(&config)?;

    // Spawn the subagent Codex session
    let codex = run_codex_conversation_interactive(
        subagent_codex_config,
        auth_manager,
        models_manager,
        parent_session,
        parent_ctx,
        cancel_token.clone(),
        None, // TODO: Support resume via InitialHistory
    )
    .await?;

    // Submit the initial prompt
    let input = vec![UserInput::Text {
        text: prompt.clone(),
    }];
    codex
        .submit(Op::UserInput { items: input })
        .await
        .map_err(|e| CodexErr::Fatal(format!("Failed to submit prompt: {e}")))?;

    // Process events until completion
    let result = process_events(
        &codex,
        &agent_id,
        &agent_type,
        max_turns,
        max_time,
        start_time,
        &cancel_token,
        event_sender.as_ref(),
    )
    .await;

    // TODO: Phase 3 - Record to transcript store if provided
    // if let Some(store) = transcript_store {
    //     store.record_result(&agent_id, &prompt, &result);
    // }
    let _ = transcript_store; // Suppress unused warning for now

    Ok(result)
}

/// Build a Codex Config suitable for subagent execution.
fn build_subagent_codex_config(config: &SubagentConfig) -> Result<Config, CodexErr> {
    use crate::tools::spec_ext::ToolFilter;

    let mut codex_config = (*config.base_config).clone();

    // Apply developer instructions from agent definition
    if let Some(instructions) = &config.developer_instructions {
        codex_config.developer_instructions = Some(instructions.clone());
    }

    // Disable user instructions for subagent (agent has its own context)
    codex_config.user_instructions = None;

    // Apply tool filter from agent definition
    // Security tiers (ALWAYS_BLOCKED, NON_BUILTIN_BLOCKED) are applied at construction
    codex_config.ext.tool_filter = Some(ToolFilter::from_agent_definition(&config.definition));

    // Note: Approval policy is handled by codex_delegate which routes
    // approval requests to the parent session

    Ok(codex_config)
}

/// Process events from the subagent Codex session.
#[allow(clippy::too_many_arguments)]
async fn process_events(
    codex: &Codex,
    agent_id: &str,
    agent_type: &str,
    max_turns: i32,
    max_time: Duration,
    start_time: Instant,
    cancel_token: &CancellationToken,
    event_sender: Option<&Sender<SubagentActivityEvent>>,
) -> SubagentResult {
    let mut turns_used = 0;
    let mut total_tokens = 0;
    let mut tool_use_count = 0;
    let mut last_result = String::new();
    let mut completed = false;

    loop {
        // Check timeout
        if start_time.elapsed() > max_time {
            return SubagentResult {
                status: SubagentStatus::Timeout,
                result: format!("Agent timed out after {} seconds", max_time.as_secs()),
                turns_used,
                duration: start_time.elapsed(),
                agent_id: agent_id.to_string(),
                total_tool_use_count: tool_use_count,
                total_duration_ms: start_time.elapsed().as_millis() as i64,
                total_tokens,
                usage: None,
            };
        }

        // Check max turns
        if turns_used >= max_turns {
            return SubagentResult {
                status: SubagentStatus::MaxTurns,
                result: format!("Agent reached max turns limit ({max_turns})"),
                turns_used,
                duration: start_time.elapsed(),
                agent_id: agent_id.to_string(),
                total_tool_use_count: tool_use_count,
                total_duration_ms: start_time.elapsed().as_millis() as i64,
                total_tokens,
                usage: None,
            };
        }

        // Check cancellation
        if cancel_token.is_cancelled() {
            return SubagentResult {
                status: SubagentStatus::Aborted,
                result: "Agent was cancelled".to_string(),
                turns_used,
                duration: start_time.elapsed(),
                agent_id: agent_id.to_string(),
                total_tool_use_count: tool_use_count,
                total_duration_ms: start_time.elapsed().as_millis() as i64,
                total_tokens,
                usage: None,
            };
        }

        // Wait for next event with timeout
        let event = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                return SubagentResult {
                    status: SubagentStatus::Aborted,
                    result: "Agent was cancelled".to_string(),
                    turns_used,
                    duration: start_time.elapsed(),
                    agent_id: agent_id.to_string(),
                    total_tool_use_count: tool_use_count,
                    total_duration_ms: start_time.elapsed().as_millis() as i64,
                    total_tokens,
                    usage: None,
                };
            }
            event = codex.next_event() => {
                match event {
                    Ok(e) => e,
                    Err(_) => {
                        // Channel closed
                        break;
                    }
                }
            }
        };

        // Process event
        match &event.msg {
            EventMsg::TaskStarted(_) => {
                turns_used += 1;
                if let Some(sender) = event_sender {
                    let _ = sender
                        .send(
                            SubagentActivityEvent::new(
                                agent_id,
                                agent_type,
                                SubagentEventType::TurnStart,
                            )
                            .with_data("turn_number", turns_used),
                        )
                        .await;
                }
            }

            EventMsg::ItemCompleted(item_event) => {
                // Check for complete_task tool call
                if let Some(output) = extract_complete_task_output(&item_event.item) {
                    last_result = output;
                    completed = true;
                }

                // Track tool usage
                if is_tool_call(&item_event.item) {
                    tool_use_count += 1;
                }
            }

            EventMsg::TaskComplete(complete_event) => {
                // Extract final message if available
                if let Some(msg) = &complete_event.last_agent_message {
                    if !completed {
                        last_result = msg.clone();
                    }
                }

                // Send completion event
                if let Some(sender) = event_sender {
                    let _ = sender
                        .send(SubagentActivityEvent::completed(
                            agent_id,
                            agent_type,
                            turns_used,
                            start_time.elapsed().as_secs_f32(),
                        ))
                        .await;
                }

                return SubagentResult {
                    // Goal if complete_task was called, otherwise still Goal
                    // (task completed successfully without explicit complete_task)
                    status: SubagentStatus::Goal,
                    result: last_result,
                    turns_used,
                    duration: start_time.elapsed(),
                    agent_id: agent_id.to_string(),
                    total_tool_use_count: tool_use_count,
                    total_duration_ms: start_time.elapsed().as_millis() as i64,
                    total_tokens,
                    usage: None,
                };
            }

            EventMsg::TurnAborted(abort_event) => {
                if let Some(sender) = event_sender {
                    let _ = sender
                        .send(SubagentActivityEvent::error(
                            agent_id,
                            agent_type,
                            &format!("Turn aborted: {:?}", abort_event.reason),
                        ))
                        .await;
                }

                return SubagentResult {
                    status: SubagentStatus::Error,
                    result: format!("Turn aborted: {:?}", abort_event.reason),
                    turns_used,
                    duration: start_time.elapsed(),
                    agent_id: agent_id.to_string(),
                    total_tool_use_count: tool_use_count,
                    total_duration_ms: start_time.elapsed().as_millis() as i64,
                    total_tokens,
                    usage: None,
                };
            }

            EventMsg::StreamError(error_event) => {
                if let Some(sender) = event_sender {
                    let _ = sender
                        .send(SubagentActivityEvent::error(
                            agent_id,
                            agent_type,
                            &format!("Stream error: {}", error_event.message),
                        ))
                        .await;
                }

                return SubagentResult {
                    status: SubagentStatus::Error,
                    result: format!("Stream error: {}", error_event.message),
                    turns_used,
                    duration: start_time.elapsed(),
                    agent_id: agent_id.to_string(),
                    total_tool_use_count: tool_use_count,
                    total_duration_ms: start_time.elapsed().as_millis() as i64,
                    total_tokens,
                    usage: None,
                };
            }

            // Handle raw response items for function call detection
            EventMsg::RawResponseItem(raw_item_event) => {
                // Check for complete_task tool call
                if let Some(output) =
                    extract_complete_task_output_from_response(&raw_item_event.item)
                {
                    last_result = output;
                    completed = true;
                }

                // Track tool usage
                if is_response_item_tool_call(&raw_item_event.item) {
                    tool_use_count += 1;
                }
            }

            // Track token usage
            EventMsg::TokenCount(token_event) => {
                if let Some(info) = &token_event.info {
                    total_tokens = info.total_token_usage.total_tokens as i32;
                }
            }

            // Ignore other events
            _ => {}
        }
    }

    // If we get here, the channel was closed unexpectedly
    SubagentResult {
        status: if completed {
            SubagentStatus::Goal
        } else {
            SubagentStatus::Error
        },
        result: if completed {
            last_result
        } else {
            "Agent session ended unexpectedly".to_string()
        },
        turns_used,
        duration: start_time.elapsed(),
        agent_id: agent_id.to_string(),
        total_tool_use_count: tool_use_count,
        total_duration_ms: start_time.elapsed().as_millis() as i64,
        total_tokens,
        usage: None,
    }
}

/// Extract output from a complete_task tool call if present in a ResponseItem.
fn extract_complete_task_output_from_response(item: &ResponseItem) -> Option<String> {
    if let ResponseItem::FunctionCall {
        name, arguments, ..
    } = item
    {
        if name == "complete_task" {
            // Parse arguments JSON to extract result field
            if let Ok(args) = serde_json::from_str::<serde_json::Value>(arguments) {
                return args
                    .get("result")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
        }
    }
    None
}

/// Check if a ResponseItem is a function call (tool use).
fn is_response_item_tool_call(item: &ResponseItem) -> bool {
    matches!(
        item,
        ResponseItem::FunctionCall { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::CustomToolCall { .. }
    )
}

/// Extract output from a complete_task tool call if present.
/// (TurnItem doesn't contain FunctionCall - use RawResponseItem events instead)
fn extract_complete_task_output(_item: &TurnItem) -> Option<String> {
    // TurnItem doesn't include FunctionCall variants.
    // Detection is handled via RawResponseItem events in process_events().
    None
}

/// Check if a TurnItem is a tool call.
/// (TurnItem doesn't contain FunctionCall - use RawResponseItem events instead)
fn is_tool_call(_item: &TurnItem) -> bool {
    // TurnItem doesn't include FunctionCall variants.
    // Detection is handled via RawResponseItem events in process_events().
    false
}

/// Generate a unique agent ID.
fn generate_agent_id(agent_type: &str) -> String {
    use std::time::SystemTime;
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let random: u32 = rand::random();
    format!("{agent_type}-{timestamp:x}-{random:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_agent_id() {
        let id1 = generate_agent_id("Explore");
        let id2 = generate_agent_id("Explore");
        assert!(id1.starts_with("Explore-"));
        assert!(id2.starts_with("Explore-"));
        // IDs should be unique
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_is_response_item_tool_call() {
        // FunctionCall is a tool call
        let function_call = ResponseItem::FunctionCall {
            id: None,
            name: "Read".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        };
        assert!(is_response_item_tool_call(&function_call));

        // LocalShellCall is a tool call
        let shell_call = ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("call-2".to_string()),
            status: codex_protocol::models::LocalShellStatus::Completed,
            action: codex_protocol::models::LocalShellAction::Exec(
                codex_protocol::models::LocalShellExecAction {
                    command: vec!["ls".to_string()],
                    timeout_ms: None,
                    working_directory: None,
                    env: None,
                    user: None,
                },
            ),
        };
        assert!(is_response_item_tool_call(&shell_call));

        // Message is not a tool call
        let message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![],
        };
        assert!(!is_response_item_tool_call(&message));
    }

    #[test]
    fn test_extract_complete_task_output() {
        // complete_task function call with result
        let complete_task = ResponseItem::FunctionCall {
            id: None,
            name: "complete_task".to_string(),
            arguments: r#"{"result": "Task completed successfully"}"#.to_string(),
            call_id: "call-1".to_string(),
        };
        assert_eq!(
            extract_complete_task_output_from_response(&complete_task),
            Some("Task completed successfully".to_string())
        );

        // complete_task without result field
        let no_result = ResponseItem::FunctionCall {
            id: None,
            name: "complete_task".to_string(),
            arguments: r#"{}"#.to_string(),
            call_id: "call-2".to_string(),
        };
        assert_eq!(extract_complete_task_output_from_response(&no_result), None);

        // Different function call (not complete_task)
        let other_call = ResponseItem::FunctionCall {
            id: None,
            name: "Read".to_string(),
            arguments: r#"{"file_path": "/test.txt"}"#.to_string(),
            call_id: "call-3".to_string(),
        };
        assert_eq!(
            extract_complete_task_output_from_response(&other_call),
            None
        );

        // Non-function-call item
        let message = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![],
        };
        assert_eq!(extract_complete_task_output_from_response(&message), None);
    }
}
