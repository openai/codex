//! Export session rollout as a single JSON file for training/RL workflows.

use std::io::Error as IoError;
use std::path::Path;

use codex_protocol::models::ContentItem;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use serde::Serialize;
use serde_json::Value;

/// RL Training export format - structured for reinforcement learning workflows.
#[derive(Clone, Serialize)]
pub struct RLTrainingExport {
    /// Session identifier
    pub session_id: String,
    /// Model used for this session
    pub model: Option<String>,
    /// Working directory
    pub cwd: String,
    /// The original user prompt
    pub prompt: Option<String>,
    /// System/agent instructions (from AGENTS.md, etc.)
    pub system_instructions: Option<String>,
    /// The full trajectory of interactions
    pub trajectory: Vec<TrajectoryStep>,
    /// Final assistant response (if any)
    pub final_response: Option<String>,
    /// Whether the session completed successfully
    pub success: bool,
    /// Source rollout file path
    pub source_path: String,
    /// Total number of raw rollout items
    pub raw_item_count: usize,
}

/// A single step in the trajectory
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrajectoryStep {
    /// User message
    UserMessage { content: String, timestamp: String },
    /// Assistant message
    AssistantMessage { content: String, timestamp: String },
    /// Assistant reasoning/thinking
    Reasoning { content: String, timestamp: String },
    /// Tool/function call
    ToolCall {
        name: String,
        arguments: Value,
        call_id: Option<String>,
        timestamp: String,
    },
    /// Tool/function output
    ToolOutput {
        call_id: String,
        output: String,
        timestamp: String,
    },
    /// Shell command execution
    ShellCommand {
        command: Vec<String>,
        call_id: Option<String>,
        timestamp: String,
    },
}

/// Read a JSONL rollout file and parse it into a vector of RolloutLine.
pub fn read_rollout_jsonl(path: &Path) -> std::io::Result<Vec<RolloutLine>> {
    let text = std::fs::read_to_string(path)?;
    parse_rollout_jsonl(&text)
}

/// Parse JSONL text into a vector of RolloutLine.
fn parse_rollout_jsonl(text: &str) -> std::io::Result<Vec<RolloutLine>> {
    let mut items = Vec::new();

    for (line_num, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<RolloutLine>(line) {
            Ok(rollout_line) => {
                items.push(rollout_line);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse rollout line {} as RolloutLine: {e}",
                    line_num + 1
                );
                // Continue parsing other lines
            }
        }
    }

    Ok(items)
}

/// Extract text content from a message's content items
fn extract_text_content(content: &[ContentItem]) -> String {
    content
        .iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text } => Some(text.clone()),
            ContentItem::OutputText { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Extract text from reasoning content items
fn extract_reasoning_content(content: &Option<Vec<ReasoningItemContent>>) -> String {
    match content {
        Some(items) => items
            .iter()
            .filter_map(|item| match item {
                ReasoningItemContent::ReasoningText { text } => Some(text.clone()),
                ReasoningItemContent::Text { text } => Some(text.clone()),
            })
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

/// Convert rollout items to RL training format
fn build_rl_export(source_path: &Path, items: Vec<RolloutLine>) -> RLTrainingExport {
    let raw_item_count = items.len();
    let mut session_id = String::new();
    let mut model: Option<String> = None;
    let mut cwd = String::new();
    let mut prompt: Option<String> = None;
    let mut system_instructions: Option<String> = None;
    let mut trajectory: Vec<TrajectoryStep> = Vec::new();
    let mut final_response: Option<String> = None;
    let mut success = true;

    for rollout_line in items {
        let timestamp = rollout_line.timestamp.clone();

        match rollout_line.item {
            RolloutItem::SessionMeta(meta) => {
                session_id = meta.meta.id.to_string();
                cwd = meta.meta.cwd.display().to_string();
                if let Some(instructions) = meta.meta.instructions {
                    system_instructions = Some(instructions);
                }
            }
            RolloutItem::TurnContext(ctx) => {
                model = Some(ctx.model);
                if ctx.user_instructions.is_some() && system_instructions.is_none() {
                    system_instructions = ctx.user_instructions;
                }
            }
            RolloutItem::ResponseItem(item) => match item {
                ResponseItem::Message { role, content, .. } => {
                    let text = extract_text_content(&content);
                    if text.is_empty() {
                        continue;
                    }

                    if role == "user" {
                        // Check if this looks like the actual prompt (not system context)
                        if !text.starts_with('<') && !text.starts_with('#') && prompt.is_none() {
                            prompt = Some(text.clone());
                        }
                        trajectory.push(TrajectoryStep::UserMessage {
                            content: text,
                            timestamp: timestamp.clone(),
                        });
                    } else if role == "assistant" {
                        final_response = Some(text.clone());
                        trajectory.push(TrajectoryStep::AssistantMessage {
                            content: text,
                            timestamp: timestamp.clone(),
                        });
                    }
                }
                ResponseItem::Reasoning { content, .. } => {
                    let text = extract_reasoning_content(&content);
                    if !text.is_empty() {
                        trajectory.push(TrajectoryStep::Reasoning {
                            content: text,
                            timestamp: timestamp.clone(),
                        });
                    }
                }
                ResponseItem::FunctionCall {
                    name,
                    arguments,
                    call_id,
                    ..
                } => {
                    let args: Value =
                        serde_json::from_str(&arguments).unwrap_or(Value::String(arguments));
                    trajectory.push(TrajectoryStep::ToolCall {
                        name,
                        arguments: args,
                        call_id: Some(call_id),
                        timestamp: timestamp.clone(),
                    });
                }
                ResponseItem::FunctionCallOutput {
                    call_id, output, ..
                } => {
                    trajectory.push(TrajectoryStep::ToolOutput {
                        call_id,
                        output: output.content,
                        timestamp: timestamp.clone(),
                    });
                }
                ResponseItem::LocalShellCall {
                    call_id, action, ..
                } => {
                    let command = match action {
                        LocalShellAction::Exec(exec_action) => exec_action.command,
                    };
                    trajectory.push(TrajectoryStep::ShellCommand {
                        command,
                        call_id,
                        timestamp: timestamp.clone(),
                    });
                }
                ResponseItem::CustomToolCall {
                    name,
                    input,
                    call_id,
                    ..
                } => {
                    let args: Value = serde_json::from_str(&input).unwrap_or(Value::String(input));
                    trajectory.push(TrajectoryStep::ToolCall {
                        name,
                        arguments: args,
                        call_id: Some(call_id),
                        timestamp: timestamp.clone(),
                    });
                }
                ResponseItem::CustomToolCallOutput {
                    call_id, output, ..
                } => {
                    trajectory.push(TrajectoryStep::ToolOutput {
                        call_id,
                        output,
                        timestamp: timestamp.clone(),
                    });
                }
                _ => {}
            },
            RolloutItem::EventMsg(ev) => {
                if let EventMsg::Error(_) = ev {
                    success = false;
                }
            }
            _ => {}
        }
    }

    RLTrainingExport {
        session_id,
        model,
        cwd,
        prompt,
        system_instructions,
        trajectory,
        final_response,
        success,
        source_path: source_path.display().to_string(),
        raw_item_count,
    }
}

/// Export a rollout JSONL file to a single JSON file formatted for RL training.
/// Returns the number of trajectory steps exported.
pub fn export_rollout_to_json(source_path: &Path, output_path: &Path) -> std::io::Result<usize> {
    let items = read_rollout_jsonl(source_path)?;
    let export = build_rl_export(source_path, items);
    let step_count = export.trajectory.len();

    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| IoError::other(format!("Failed to serialize rollout: {e}")))?;

    std::fs::write(output_path, json)?;

    Ok(step_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_empty_jsonl() {
        let items = parse_rollout_jsonl("").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn parse_whitespace_only_jsonl() {
        let items = parse_rollout_jsonl("  \n  \n  ").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn parse_valid_session_meta() {
        // Uses a valid UUID format for the id field
        let jsonl = r#"{"timestamp":"2025-01-03T12:00:00.000Z","type":"session_meta","payload":{"id":"550e8400-e29b-41d4-a716-446655440000","timestamp":"2025-01-03T12:00:00.000Z","cwd":"/tmp","originator":"test","cli_version":"1.0.0","source":"exec","git":null}}"#;
        let items = parse_rollout_jsonl(jsonl).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn skips_invalid_lines() {
        let jsonl = "not valid json\n{\"also\":\"invalid as rollout line\"}";
        let items = parse_rollout_jsonl(jsonl).unwrap();
        assert!(items.is_empty());
    }
}
