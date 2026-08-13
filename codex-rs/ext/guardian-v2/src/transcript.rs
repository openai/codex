use std::collections::HashMap;

use codex_extension_api::ResponseItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::plaintext_agent_message_content;

// Provisional approximation of an 80k-token transcript budget.
const MAX_TRANSCRIPT_BYTES: usize = 320 * 1024;
const MANUAL_APPROVAL_DEVELOPER_PREFIX: &str =
    "The user has manually approved a specific action that was previously `Rejected`.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptSource {
    ToolCalls,
    ToolOutputs,
    Reasoning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptConfig {
    pub(crate) sources: Vec<TranscriptSource>,
}

impl Default for TranscriptConfig {
    fn default() -> Self {
        Self {
            sources: vec![
                TranscriptSource::ToolCalls,
                TranscriptSource::ToolOutputs,
                TranscriptSource::Reasoning,
            ],
        }
    }
}

impl TranscriptConfig {
    pub(crate) fn build<'a>(&self, items: impl IntoIterator<Item = &'a ResponseItem>) -> String {
        let mut transcript = String::new();
        let mut tool_names_by_call_id = HashMap::new();
        let mut entry_number = 0;

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
            entry_number += 1;
            transcript.push_str(&format!("[{entry_number}] {role}: {text}\n"));

            if transcript.len() > MAX_TRANSCRIPT_BYTES {
                let mut first_retained_byte = transcript.len() - MAX_TRANSCRIPT_BYTES;
                while !transcript.is_char_boundary(first_retained_byte) {
                    first_retained_byte += 1;
                }
                transcript.drain(..first_retained_byte);
            }
        }

        transcript
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
