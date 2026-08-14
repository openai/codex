use std::collections::HashMap;

use codex_extension_api::ResponseItem;
pub(crate) use codex_features::GuardianV2TranscriptSource as TranscriptSource;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::plaintext_agent_message_content;
use codex_protocol::protocol::TruncationPolicy;

pub(crate) const MAX_MESSAGE_ENTRY_TOKENS: usize = 2_000;
pub(crate) const MAX_TOOL_ENTRY_TOKENS: usize = 1_000;
pub(crate) const MAX_MESSAGE_TRANSCRIPT_TOKENS: usize = 10_000;
pub(crate) const MAX_TOOL_TRANSCRIPT_TOKENS: usize = 10_000;
pub(crate) const MAX_RECENT_NON_USER_ENTRIES: usize = 40;
const MANUAL_APPROVAL_DEVELOPER_PREFIX: &str =
    "The user has manually approved a specific action that was previously `Rejected`.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TranscriptEntryKind {
    User,
    Message,
    Tool,
}

struct TranscriptEntry {
    kind: TranscriptEntryKind,
    text: String,
    tokens: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptConfig {
    pub(crate) sources: Vec<TranscriptSource>,
    pub(crate) max_message_entry_tokens: usize,
    pub(crate) max_tool_entry_tokens: usize,
    pub(crate) max_message_transcript_tokens: usize,
    pub(crate) max_tool_transcript_tokens: usize,
    pub(crate) max_recent_non_user_entries: usize,
}

impl Default for TranscriptConfig {
    fn default() -> Self {
        Self {
            sources: vec![TranscriptSource::ToolCalls, TranscriptSource::ToolOutputs],
            max_message_entry_tokens: MAX_MESSAGE_ENTRY_TOKENS,
            max_tool_entry_tokens: MAX_TOOL_ENTRY_TOKENS,
            max_message_transcript_tokens: MAX_MESSAGE_TRANSCRIPT_TOKENS,
            max_tool_transcript_tokens: MAX_TOOL_TRANSCRIPT_TOKENS,
            max_recent_non_user_entries: MAX_RECENT_NON_USER_ENTRIES,
        }
    }
}

impl TranscriptConfig {
    pub(crate) fn build<'a>(
        &self,
        items: impl IntoIterator<Item = &'a ResponseItem>,
    ) -> Vec<String> {
        let mut entries = Vec::new();
        let mut tool_names_by_call_id = HashMap::new();

        for item in items {
            let (role, text) = match item {
                ResponseItem::Message { role, content, .. } => {
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
                    if text.trim().is_empty() {
                        continue;
                    }
                    let include_message = match role.as_str() {
                        "user" | "assistant" => true,
                        "developer" => text.starts_with(MANUAL_APPROVAL_DEVELOPER_PREFIX),
                        _ => false,
                    };
                    if !include_message {
                        continue;
                    }
                    (role.clone(), text)
                }
                ResponseItem::AgentMessage {
                    author, content, ..
                } => {
                    let Some(text) = plaintext_agent_message_content(content) else {
                        continue;
                    };
                    (
                        "assistant".to_owned(),
                        format!("Agent message from {author}:\n{text}"),
                    )
                }
                ResponseItem::FunctionCall {
                    name,
                    arguments,
                    call_id,
                    ..
                }
                | ResponseItem::CustomToolCall {
                    name,
                    input: arguments,
                    call_id,
                    ..
                } => {
                    tool_names_by_call_id.insert(call_id.as_str(), name.as_str());
                    if !self.sources.contains(&TranscriptSource::ToolCalls) {
                        continue;
                    }
                    (format!("tool {name} call"), arguments.clone())
                }
                ResponseItem::FunctionCallOutput {
                    call_id, output, ..
                }
                | ResponseItem::CustomToolCallOutput {
                    call_id, output, ..
                } => {
                    if !self.sources.contains(&TranscriptSource::ToolOutputs) {
                        continue;
                    }
                    let Some(output) = output.body.to_text() else {
                        continue;
                    };
                    let role = tool_names_by_call_id.get(call_id.as_str()).map_or_else(
                        || "tool result".to_owned(),
                        |name| format!("tool {name} result"),
                    );
                    (role, output)
                }
                ResponseItem::Reasoning {
                    summary, content, ..
                } => {
                    if !self.sources.contains(&TranscriptSource::Reasoning) {
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
                    ("reasoning".to_owned(), text)
                }
                ResponseItem::LocalShellCall { action, .. } => {
                    if !self.sources.contains(&TranscriptSource::ToolCalls) {
                        continue;
                    }
                    let Ok(text) = serde_json::to_string(action) else {
                        continue;
                    };
                    ("tool shell call".to_owned(), text)
                }
                ResponseItem::WebSearchCall { action, .. } => {
                    if !self.sources.contains(&TranscriptSource::ToolCalls) {
                        continue;
                    }
                    let Some(action) = action else {
                        continue;
                    };
                    let Ok(text) = serde_json::to_string(action) else {
                        continue;
                    };
                    ("tool web_search call".to_owned(), text)
                }
                ResponseItem::AdditionalTools { .. }
                | ResponseItem::ImageGenerationCall { .. }
                | ResponseItem::ToolSearchCall { .. }
                | ResponseItem::ToolSearchOutput { .. }
                | ResponseItem::Compaction { .. }
                | ResponseItem::CompactionTrigger { .. }
                | ResponseItem::ContextCompaction { .. }
                | ResponseItem::Other => continue,
            };
            if text.trim().is_empty() {
                continue;
            }

            let kind = match role.as_str() {
                "user" => TranscriptEntryKind::User,
                role if role.starts_with("tool ") => TranscriptEntryKind::Tool,
                _ => TranscriptEntryKind::Message,
            };
            let token_cap = match kind {
                TranscriptEntryKind::Tool => self.max_tool_entry_tokens,
                TranscriptEntryKind::User | TranscriptEntryKind::Message => {
                    self.max_message_entry_tokens
                }
            };
            let text = truncate_entry(&text, token_cap);
            let entry_number = entries.len() + 1;
            let text = format!("[{entry_number}] {role}: {text}\n");
            let tokens = TruncationPolicy::Bytes(text.len()).token_budget();
            entries.push(TranscriptEntry { kind, text, tokens });
        }

        let mut included = vec![false; entries.len()];
        let mut message_tokens = 0;
        let mut tool_tokens = 0;
        let user_indices = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (entry.kind == TranscriptEntryKind::User).then_some(index))
            .collect::<Vec<_>>();

        if let Some(&first_user_index) = user_indices.first() {
            included[first_user_index] = true;
            message_tokens += entries[first_user_index].tokens;
        }

        if let Some(&latest_user_index) = user_indices.last()
            && !included[latest_user_index]
            && message_tokens + entries[latest_user_index].tokens
                <= self.max_message_transcript_tokens
        {
            included[latest_user_index] = true;
            message_tokens += entries[latest_user_index].tokens;
        }

        for &index in user_indices.iter().rev() {
            if included[index]
                || message_tokens + entries[index].tokens > self.max_message_transcript_tokens
            {
                continue;
            }

            included[index] = true;
            message_tokens += entries[index].tokens;
        }

        let mut retained_non_user_entries = 0;
        for (index, entry) in entries.iter().enumerate().rev() {
            if entry.kind == TranscriptEntryKind::User
                || retained_non_user_entries >= self.max_recent_non_user_entries
            {
                continue;
            }

            let fits_budget = match entry.kind {
                TranscriptEntryKind::Tool => {
                    tool_tokens + entry.tokens <= self.max_tool_transcript_tokens
                }
                TranscriptEntryKind::Message => {
                    message_tokens + entry.tokens <= self.max_message_transcript_tokens
                }
                TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
            };
            if !fits_budget {
                continue;
            }

            included[index] = true;
            retained_non_user_entries += 1;
            match entry.kind {
                TranscriptEntryKind::Tool => tool_tokens += entry.tokens,
                TranscriptEntryKind::Message => message_tokens += entry.tokens,
                TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
            }
        }

        entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| included[index].then_some(entry.text))
            .collect()
    }
}

pub(crate) fn truncate_entry(text: &str, max_tokens: usize) -> String {
    let max_bytes = TruncationPolicy::Tokens(max_tokens).byte_budget();
    if text.len() <= max_bytes {
        return text.to_owned();
    }

    let omitted_tokens =
        TruncationPolicy::Bytes(text.len().saturating_sub(max_bytes)).token_budget();
    let marker = format!("<truncated omitted_approx_tokens=\"{omitted_tokens}\" />");
    if max_bytes <= marker.len() {
        return marker;
    }

    let available_bytes = max_bytes - marker.len();
    let prefix_bytes = available_bytes / 2;
    let suffix_bytes = available_bytes - prefix_bytes;

    let mut prefix_end = prefix_bytes;
    while !text.is_char_boundary(prefix_end) {
        prefix_end -= 1;
    }

    let mut suffix_start = text.len() - suffix_bytes;
    while !text.is_char_boundary(suffix_start) {
        suffix_start += 1;
    }

    format!("{}{marker}{}", &text[..prefix_end], &text[suffix_start..])
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
