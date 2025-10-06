use std::collections::HashMap;

use async_trait::async_trait;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;

use crate::codex::context_item_category_and_text;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ContextPruneHandler;

#[derive(Deserialize)]
struct Args {
    action: String,
    #[serde(default)]
    indices: Vec<usize>,
    #[serde(default)]
    included: Option<bool>,
    #[serde(default)]
    pinned_tail_turns: Option<usize>,
}

fn usage_totals(items: &[ResponseItem]) -> (HashMap<&'static str, usize>, usize) {
    use crate::protocol::PruneCategory as PC;

    let mut by_category: HashMap<&'static str, usize> = HashMap::new();
    let mut total: usize = 0;
    for item in items {
        let (category, text) = context_item_category_and_text(item);
        let bytes = text.len();
        let key: &'static str = match category {
            PC::ToolOutput => "tool_output",
            PC::ToolCall => "tool_call",
            PC::Reasoning => "reasoning",
            PC::AssistantMessage => "assistant_message",
            PC::UserMessage => "user_message",
            PC::UserInstructions => "user_instructions",
            PC::EnvironmentContext => "environment_context",
        };
        let entry = by_category.entry(key).or_insert(0);
        *entry = entry.saturating_add(bytes);
        total = total.saturating_add(bytes);
    }
    (by_category, total)
}

#[async_trait]
impl ToolHandler for ContextPruneHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            payload, session, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "context_prune handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: Args = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse function arguments: {err:?}"
            ))
        })?;

        match args.action.as_str() {
            "list" => {
                let items = session.compute_context_items().await;
                let content = serde_json::to_string(&items)
                    .unwrap_or_else(|_| "{\"error\":\"failed to serialize items\"}".to_string());
                Ok(ToolOutput::Function {
                    content,
                    success: Some(true),
                })
            }
            "usage" => {
                let items = session.history_snapshot().await;
                let (by_category, total) = usage_totals(&items);
                #[derive(serde::Serialize)]
                struct Usage<'a> {
                    total_bytes: usize,
                    by_category: HashMap<&'a str, usize>,
                }
                let payload = Usage {
                    total_bytes: total,
                    by_category,
                };
                let content = serde_json::to_string(&payload)
                    .unwrap_or_else(|_| "{\"error\":\"failed to serialize usage\"}".to_string());
                Ok(ToolOutput::Function {
                    content,
                    success: Some(true),
                })
            }
            "set_inclusion" => {
                let included = args.included.unwrap_or(true);
                session.set_context_inclusion(&args.indices, included).await;
                let items = session.compute_context_items().await;
                let content = serde_json::to_string(&items).unwrap_or_else(|_| {
                    "{\"error\":\"failed to serialize updated items\"}".to_string()
                });
                Ok(ToolOutput::Function {
                    content,
                    success: Some(true),
                })
            }
            "set_pinned_tail_turns" => {
                let turns = args.pinned_tail_turns.unwrap_or(1);
                session.set_pinned_tail_turns(turns).await;
                let items = session.compute_context_items().await;
                let content = serde_json::to_string(&items).unwrap_or_else(|_| {
                    "{\"error\":\"failed to serialize updated items\"}".to_string()
                });
                Ok(ToolOutput::Function {
                    content,
                    success: Some(true),
                })
            }
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported action: {other}. expected 'list' | 'set_inclusion' | 'set_pinned_tail_turns'"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;

    #[test]
    fn usage_counts_full_tool_output_length() {
        let content = "a".repeat(512);
        let item = ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                content: content.clone(),
                success: Some(true),
            },
        };
        let items = vec![item];
        let (by_category, total) = usage_totals(&items);
        assert_eq!(total, content.len());
        assert_eq!(by_category.get("tool_output"), Some(&content.len()));
    }

    #[test]
    fn usage_counts_full_message_length() {
        let text = "b".repeat(384);
        let item = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText { text: text.clone() }],
        };
        let items = vec![item];
        let (by_category, total) = usage_totals(&items);
        assert_eq!(total, text.len());
        assert_eq!(by_category.get("assistant_message"), Some(&text.len()));
    }
}
