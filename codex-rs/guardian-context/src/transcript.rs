//! Collects original conversation evidence before consumer-specific preparation.
//!
//! Both Guardian consumers receive the same role and tool-source attribution,
//! with generated compaction summaries attributed to the assistant, complete user
//! messages. Registry collection preserves originals for both consumers; resolved profiles
//! apply aggregate retention after the host selects its full/delta slice. Tool outputs with a
//! call ID retain their generic label when the call is unavailable. Outputs
//! without a call ID require an explicit name.

use codex_history::RetainedContextEntry;
use std::collections::HashMap;

use codex_protocol::mcp::is_node_repl_backed_tool;
use codex_protocol::models::AgentMessageInputContent;
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
use crate::PlannedAction;
use crate::PlannedActionKind;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionHistory;
use crate::SectionInput;
use crate::SectionScope;
use crate::TranscriptContent;

pub(crate) const TRANSCRIPT_OMISSION_NOTICE: &str = "Some conversation entries were omitted.";

/// Trusted developer marker that preserves an explicit manual action approval.
pub const MANUAL_APPROVAL_DEVELOPER_PREFIX: &str =
    "The user has manually approved a specific action that was previously `Rejected`.";

/// Copied approval messages are model context, not a child's trusted approval evidence.
/// Filter before dropping envelope provenance, including when replaying review checkpoints.
pub fn is_inherited_manual_approval(envelope: &codex_history::ResponseItemEnvelope) -> bool {
    envelope
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.inherited_user_message)
        && matches!(&envelope.item, ResponseItem::Message { role, content, .. }
            if role == "developer" && content.iter().any(|part|
                matches!(part, ContentItem::InputText { text } | ContentItem::OutputText { text , .. }
                    if text.starts_with(MANUAL_APPROVAL_DEVELOPER_PREFIX))))
}

/// Evidence sources included alongside user and assistant conversation messages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTranscriptOptions {
    /// Includes submitted function, custom-tool, shell, and web-search calls.
    pub include_tool_calls: bool,
    /// Includes function and custom-tool outputs.
    pub include_tool_outputs: bool,
    /// Includes plaintext reasoning summaries and reasoning content.
    pub include_reasoning: bool,
    /// Preserve calls in this MAv2 namespace for synchronous backend decryption.
    /// `None` keeps the existing text transcript; `functions` selects the default namespace.
    pub native_multi_agent_namespace: Option<String>,
}

impl Default for ConversationTranscriptOptions {
    fn default() -> Self {
        Self {
            include_tool_calls: true,
            include_tool_outputs: true,
            include_reasoning: false,
            native_multi_agent_namespace: None,
        }
    }
}

/// Per-entry caps resolved by the caller for the current review.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptEntryLimits {
    /// Profile cap for assistant and plaintext reasoning; user/approval policy is applied separately.
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTranscriptConfig {
    /// Evidence sources to include in this request.
    pub options: ConversationTranscriptOptions,
    /// Request-time entry limits applied during preparation.
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
        let proposed_call_id = if let Some(PlannedAction {
            kind: PlannedActionKind::ToolCall(call),
            ..
        }) = input.planned_action
            && let ResponseItem::FunctionCall { call_id, .. } = call.as_ref()
        {
            Some(call_id.as_str())
        } else {
            None
        };
        Ok(Some(ContextSection::ConversationTranscript {
            items: collect_transcript_for_review(
                input.history,
                input.transcript,
                proposed_call_id,
                input.is_selected_exec_output,
            ),
        }))
    }
}

/// Extracts original transcript entries without composing other context sections.
///
/// Entries preserve conversation order and role/tool attribution. Context
/// preparation applies entry limits and aggregate retention.
pub fn collect_transcript(
    history: &dyn SectionHistory,
    config: &ConversationTranscriptConfig,
) -> Vec<ConversationTranscriptEntry> {
    collect_transcript_for_review(
        history, config, /*proposed_call_id*/ None, /*is_selected_exec_output*/ None,
    )
}

fn collect_transcript_for_review(
    history: &dyn SectionHistory,
    config: &ConversationTranscriptConfig,
    proposed_call_id: Option<&str>,
    is_selected_exec_output: Option<&crate::SelectedExecOutputPredicate<'_>>,
) -> Vec<ConversationTranscriptEntry> {
    let mut entries = Vec::new();
    let mut tool_names_by_call_id = HashMap::new();
    // Positional legacy labels cannot establish reusable delivery proof, including
    // through transcript copies. The separate retained section stays authoritative.
    let retained_context = history
        .retained_context()
        .filter(|context| !crate::retained_instructions::has_legacy_order(context));

    for (item, source) in history.items_with_sources() {
        let (kind, text) = match item {
            ResponseItem::Message {
                role,
                content,
                phase,
                internal_chat_message_metadata_passthrough,
                ..
            } => {
                let text = content
                    .iter()
                    .filter_map(|item| match item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text, .. }
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
                    // Local compaction uses the user wire role for model context, but
                    // its generated summary is not user authorization.
                    "user"
                        if internal_chat_message_metadata_passthrough
                            .as_ref()
                            .and_then(|metadata| metadata.content_item_kinds.as_ref())
                            .is_some_and(|kinds| {
                                kinds.iter().any(|kind| kind.0 == "compaction.summary")
                            }) =>
                    {
                        ConversationTranscriptEntryKind::Assistant
                    }
                    "user" => ConversationTranscriptEntryKind::User,
                    "developer" if text.starts_with(MANUAL_APPROVAL_DEVELOPER_PREFIX) => {
                        ConversationTranscriptEntryKind::Developer
                    }
                    "assistant"
                        if matches!(
                            phase,
                            None | Some(MessagePhase::PartialAnswer | MessagePhase::FinalAnswer)
                        ) && !InterAgentCommunication::is_message_content(content) =>
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
                if content
                    .iter()
                    .any(|part| matches!(part, AgentMessageInputContent::EncryptedContent { .. }))
                {
                    // Preserve ciphertext whole; preparation charges serialized bytes
                    // and decides whether it fits the request's entry limits.
                    let bytes = crate::budget::ByteCount::item(item);
                    entries.push(ConversationTranscriptEntry {
                        kind: ConversationTranscriptEntryKind::Assistant,
                        content: TranscriptContent::NativeMessage(Box::new(item.clone())),
                        original_bytes: bytes,
                        retained_source: None,
                    });
                    continue;
                }
                let Some(text) = plaintext_agent_message_content(content) else {
                    continue;
                };
                (
                    ConversationTranscriptEntryKind::Assistant,
                    format!("Agent message from {author}:\n{text}"),
                )
            }
            // The proposed call is delivered separately; preserve this cursor position.
            ResponseItem::FunctionCall { name, call_id, .. }
                if config.options.include_tool_calls
                    && proposed_call_id == Some(call_id.as_str()) =>
            {
                (
                    ConversationTranscriptEntryKind::ToolCall(format!("tool {name} call")),
                    TRANSCRIPT_OMISSION_NOTICE.to_owned(),
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
                let kind = ConversationTranscriptEntryKind::ToolCall(format!("tool {name} call"));
                if matches!(item, ResponseItem::FunctionCall { .. })
                    && config
                        .options
                        .native_multi_agent_namespace
                        .as_deref()
                        .is_some_and(|configured| {
                            namespace
                                .as_deref()
                                .filter(|namespace| !namespace.is_empty())
                                .unwrap_or("functions")
                                == configured
                        })
                    && matches!(
                        name.as_str(),
                        "spawn_agent" | "send_message" | "followup_task" | "interrupt_agent"
                    )
                {
                    entries.push(ConversationTranscriptEntry {
                        kind,
                        content: TranscriptContent::NativeMessage(Box::new(item.clone())),
                        original_bytes: crate::budget::ByteCount::item(item),
                        retained_source: None,
                    });
                    continue;
                }
                (kind, arguments.clone())
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
                    // The recorder can still verify an exec result after its call leaves review history.
                    None
                    | Some((
                        "exec",
                        None | Some("" | codex_protocol::DEFAULT_FUNCTION_NAMESPACE),
                    )) if matches!(item, ResponseItem::CustomToolCallOutput { .. })
                        && is_selected_exec_output.is_some_and(|select| select(item)) =>
                    {
                        ConversationTranscriptEntryKind::SelectedExecOutput
                    }
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
        let scheduled = codex_history::UserInputOrigin::from_message(item)
            == codex_history::UserInputOrigin::Heartbeat;
        let heartbeat = codex_history::Heartbeat::from_message(item).map(Into::into);
        entries.push(ConversationTranscriptEntry {
            retained_source: retained_context.and_then(|context| {
                let source = source.filter(|source| {
                    source.complete
                        && kind == ConversationTranscriptEntryKind::User
                        && source.id.role == codex_history::RetainedSourceRole::User
                        && Some(source.id.message_id.as_str())
                            == item.id().map(codex_protocol::ResponseItemId::as_str)
                        && source.id.turn_id == item.turn_id().unwrap_or_default()
                })?;
                crate::retained_instructions::source_order_labels(context).find_map(
                    |(order, entry)| {
                        if context.source(entry).as_ref() != Some(source) {
                            return None;
                        }
                        let RetainedContextEntry::UserMessage(message) = entry else {
                            return None;
                        };
                        let rendered = format!(
                            "Retained source order: {order}\n{}",
                            crate::GuardianRootMessage::User(message.text.clone()).render()
                        );
                        Some(crate::RetainedTranscriptSource {
                            rendered_bytes: rendered.len(),
                            order,
                            source: source.clone(),
                        })
                    },
                )
            }),
            kind,
            content: if scheduled {
                TranscriptContent::ScheduledMessage(crate::entry::ScheduledTranscriptMessage {
                    text,
                    heartbeat,
                })
            } else {
                TranscriptContent::Text(text)
            },
            original_bytes,
        });
    }

    entries
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
