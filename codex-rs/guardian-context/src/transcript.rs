//! Collects bounded conversation evidence before consumer-specific rendering.
//!
//! Both Guardian consumers receive the same role and tool-source attribution,
//! with complete user messages and capped non-user entries. Resolved context profiles
//! apply aggregate retention after the host selects its full/delta slice. Tool outputs with a
//! call ID retain their generic label when the call is unavailable. Outputs
//! without a call ID require an explicit name.

use std::collections::HashMap;

use codex_protocol::mcp::is_node_repl_backed_tool;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::plaintext_agent_message_content;
use codex_protocol::protocol::InterAgentCommunication;

use crate::ContextSection;
use crate::ConversationTranscriptEntry;
use crate::ConversationTranscriptEntryKind;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionHistory;
use crate::SectionInput;
use crate::SectionScope;
use crate::truncate_text;

/// Trusted developer marker that preserves an explicit manual action approval.
pub const MANUAL_APPROVAL_DEVELOPER_PREFIX: &str =
    "The user has manually approved a specific action that was previously `Rejected`.";

/// Evidence sources included alongside user and assistant conversation messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConversationTranscriptOptions {
    /// Includes submitted function, custom-tool, shell, and web-search calls.
    pub include_tool_calls: bool,
    /// Includes function and custom-tool outputs.
    pub include_tool_outputs: bool,
    /// Includes plaintext reasoning summaries and reasoning content.
    pub include_reasoning: bool,
}

impl Default for ConversationTranscriptOptions {
    fn default() -> Self {
        Self {
            include_tool_calls: true,
            include_tool_outputs: true,
            include_reasoning: false,
        }
    }
}

/// Per-entry caps resolved by the caller for the current review.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptEntryLimits {
    /// Cap for assistant and plaintext reasoning entries; user and manual approvals stay complete.
    pub message_tokens: usize,
    /// Cap for tool calls and ordinary tool outputs.
    pub tool_tokens: usize,
    /// Cap for Node REPL-backed outputs, which sync may retain at a larger size.
    pub node_repl_output_tokens: usize,
}

/// Aggregate limits for retaining rendered transcript entries.
///
/// Context profiles apply the sync or async selection rules using these limits.
/// User messages and manual approvals survive these soft limits; the complete
/// request budget can shorten them with markers after other recovery is exhausted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptRetentionConfig {
    /// Budget for rendered user, developer, assistant, and reasoning entries.
    pub max_message_transcript_tokens: usize,
    /// Separate budget for rendered tool calls and results.
    pub max_tool_transcript_tokens: usize,
    /// Maximum retained entries other than user messages and manual approvals.
    pub max_recent_non_user_entries: usize,
}

/// Evidence sources and per-entry limits supplied on each collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConversationTranscriptConfig {
    /// Evidence sources to include in this request.
    pub options: ConversationTranscriptOptions,
    /// Per-entry limits applied before accumulating transcript text.
    pub entry_limits: TranscriptEntryLimits,
}

/// Shared contributor that extracts parent-conversation evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConversationTranscriptSection;

impl SectionContributor for ConversationTranscriptSection {
    fn scope(&self) -> SectionScope {
        SectionScope::Shared
    }

    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        Ok(Some(ContextSection::ConversationTranscript {
            items: collect_transcript(input.history, input.transcript),
        }))
    }
}

/// Extracts bounded transcript entries without composing other context sections.
///
/// Entries preserve conversation order and role/tool attribution. Non-user
/// limits apply during collection; context profiles own aggregate retention.
pub fn collect_transcript(
    history: &dyn SectionHistory,
    config: &ConversationTranscriptConfig,
) -> Vec<ConversationTranscriptEntry> {
    let mut entries = Vec::new();
    let mut tool_names_by_call_id = HashMap::new();

    for item in history.items() {
        let (kind, text) = match item {
            ResponseItem::Message {
                role,
                content,
                phase,
                ..
            } => {
                let text = content
                    .iter()
                    .filter_map(|item| match item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text }
                            if !text.is_empty() =>
                        {
                            Some(text.as_str())
                        }
                        ContentItem::InputText { .. }
                        | ContentItem::OutputText { .. }
                        | ContentItem::InputImage { .. }
                        | ContentItem::InputAudio { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let kind = match role.as_str() {
                    "user" => ConversationTranscriptEntryKind::User,
                    "developer" if text.starts_with(MANUAL_APPROVAL_DEVELOPER_PREFIX) => {
                        ConversationTranscriptEntryKind::Developer
                    }
                    "assistant"
                        if matches!(phase, None | Some(MessagePhase::FinalAnswer))
                            && !InterAgentCommunication::is_message_content(content) =>
                    {
                        ConversationTranscriptEntryKind::ProtectedAssistant
                    }
                    "assistant" => ConversationTranscriptEntryKind::Assistant,
                    _ => continue,
                };
                (kind, text)
            }
            ResponseItem::AgentMessage {
                author, content, ..
            } => {
                let Some(text) = plaintext_agent_message_content(content) else {
                    continue;
                };
                (
                    ConversationTranscriptEntryKind::Assistant,
                    format!("Agent message from {author}:\n{text}"),
                )
            }
            ResponseItem::FunctionCall {
                name,
                namespace,
                arguments,
                call_id,
                ..
            }
            | ResponseItem::CustomToolCall {
                name,
                namespace,
                input: arguments,
                call_id,
                ..
            } => {
                tool_names_by_call_id
                    .insert(call_id.as_str(), (name.as_str(), namespace.as_deref()));
                if !config.options.include_tool_calls {
                    continue;
                }
                (
                    ConversationTranscriptEntryKind::ToolCall(format!("tool {name} call")),
                    arguments.clone(),
                )
            }
            ResponseItem::FunctionCallOutput {
                call_id: Some(call_id),
                output,
                ..
            }
            | ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } => {
                if !config.options.include_tool_outputs {
                    continue;
                }
                let kind = match tool_names_by_call_id.get(call_id.as_str()) {
                    Some((name, namespace)) if is_node_repl_backed_tool(name, *namespace) => {
                        ConversationTranscriptEntryKind::NodeReplToolOutput(format!(
                            "tool {name} result"
                        ))
                    }
                    Some((name, _)) => {
                        ConversationTranscriptEntryKind::ToolOutput(format!("tool {name} result"))
                    }
                    None => ConversationTranscriptEntryKind::ToolOutput("tool result".to_string()),
                };
                let Some(text) = output.body.to_text() else {
                    continue;
                };
                (kind, text)
            }
            ResponseItem::FunctionCallOutput {
                call_id: None,
                name: Some(name),
                namespace,
                output,
                ..
            } => {
                if !config.options.include_tool_outputs {
                    continue;
                }
                let role = match namespace {
                    Some(namespace) => format!("tool {namespace}.{name} result"),
                    None => format!("tool {name} result"),
                };
                (
                    ConversationTranscriptEntryKind::ToolOutput(role),
                    output
                        .body
                        .to_text()
                        .unwrap_or_else(|| "[non-text output]".into()),
                )
            }
            ResponseItem::Reasoning {
                summary, content, ..
            } => {
                if !config.options.include_reasoning {
                    continue;
                }
                let text = summary
                    .iter()
                    .map(|item| match item {
                        ReasoningItemReasoningSummary::SummaryText { text } => text.as_str(),
                    })
                    .chain(content.iter().flatten().map(|item| match item {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => text.as_str(),
                    }))
                    .filter(|text| !text.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                (ConversationTranscriptEntryKind::Reasoning, text)
            }
            ResponseItem::LocalShellCall { action, .. } => {
                if !config.options.include_tool_calls {
                    continue;
                }
                let Ok(text) = serde_json::to_string(action) else {
                    continue;
                };
                (
                    ConversationTranscriptEntryKind::ToolCall("tool shell call".to_string()),
                    text,
                )
            }
            ResponseItem::WebSearchCall { action, .. } => {
                if !config.options.include_tool_calls {
                    continue;
                }
                let Some(action) = action else {
                    continue;
                };
                let Ok(text) = serde_json::to_string(action) else {
                    continue;
                };
                (
                    ConversationTranscriptEntryKind::ToolCall("tool web_search call".to_string()),
                    text,
                )
            }
            ResponseItem::FunctionCallOutput {
                call_id: None,
                name: None,
                ..
            }
            | ResponseItem::AdditionalTools { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::ConfigurationUpdate { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => continue,
        };

        if text.trim().is_empty() {
            continue;
        }
        let original_bytes = text.len();
        let text = match &kind {
            ConversationTranscriptEntryKind::User | ConversationTranscriptEntryKind::Developer => {
                text
            }
            ConversationTranscriptEntryKind::Assistant
            | ConversationTranscriptEntryKind::ProtectedAssistant
            | ConversationTranscriptEntryKind::Reasoning => {
                truncate_text(&text, config.entry_limits.message_tokens)
            }
            ConversationTranscriptEntryKind::ToolCall(_)
            | ConversationTranscriptEntryKind::ToolOutput(_) => {
                truncate_text(&text, config.entry_limits.tool_tokens)
            }
            ConversationTranscriptEntryKind::NodeReplToolOutput(_) => {
                truncate_text(&text, config.entry_limits.node_repl_output_tokens)
            }
        };
        entries.push(ConversationTranscriptEntry {
            kind,
            text,
            original_bytes,
        });
    }

    entries
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
