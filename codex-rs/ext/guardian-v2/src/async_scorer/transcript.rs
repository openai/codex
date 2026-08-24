use std::collections::HashMap;
use std::collections::VecDeque;

use codex_extension_api::ResponseItem;
pub(crate) use codex_features::GuardianV2TranscriptSource as TranscriptSource;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::plaintext_agent_message_content;
use codex_protocol::protocol::TruncationPolicy;

use super::truncation::TruncationObservation;

pub(crate) const MAX_MESSAGE_ENTRY_TOKENS: usize = 2_000;
pub(crate) const MAX_TOOL_ENTRY_TOKENS: usize = 1_000;
pub(crate) const MAX_MESSAGE_TRANSCRIPT_TOKENS: usize = 10_000;
pub(crate) const MAX_TOOL_TRANSCRIPT_TOKENS: usize = 10_000;
pub(crate) const MAX_RECENT_NON_USER_ENTRIES: usize = 40;
const MAX_TRANSCRIPT_IMAGES: usize = 4;
const MAX_TRANSCRIPT_IMAGE_BYTES: usize = 8 * 1024 * 1024;
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
    original_bytes: usize,
    retained_bytes: usize,
}

pub(crate) struct RenderedTranscript {
    pub(crate) entries: Vec<String>,
    pub(crate) truncations: Vec<TruncationObservation>,
}

pub(crate) struct RenderedImages {
    pub(crate) images: Vec<ContentItem>,
    pub(crate) omitted_bytes: usize,
}

fn retained_tokens(
    entries: &[TranscriptEntry],
    retained: &VecDeque<usize>,
    kind: TranscriptEntryKind,
) -> usize {
    retained
        .iter()
        .filter_map(|&index| {
            let entry = &entries[index];
            (entry.kind == kind).then_some(entry.tokens)
        })
        .sum()
}

fn evict_oldest_matching(
    entries: &[TranscriptEntry],
    retained: &mut VecDeque<usize>,
    count: usize,
    mut matches: impl FnMut(TranscriptEntryKind) -> bool,
) {
    let mut remaining = count;
    retained.retain(|&index| {
        if remaining > 0 && matches(entries[index].kind) {
            remaining -= 1;
            false
        } else {
            true
        }
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptConfig {
    pub(crate) sources: Vec<TranscriptSource>,
    pub(crate) include_images: bool,
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
            include_images: false,
            max_message_entry_tokens: MAX_MESSAGE_ENTRY_TOKENS,
            max_tool_entry_tokens: MAX_TOOL_ENTRY_TOKENS,
            max_message_transcript_tokens: MAX_MESSAGE_TRANSCRIPT_TOKENS,
            max_tool_transcript_tokens: MAX_TOOL_TRANSCRIPT_TOKENS,
            max_recent_non_user_entries: MAX_RECENT_NON_USER_ENTRIES,
        }
    }
}

impl TranscriptConfig {
    pub(crate) fn images<'a>(
        &self,
        items: impl IntoIterator<Item = &'a ResponseItem>,
        node_repl_images: impl IntoIterator<Item = ContentItem>,
    ) -> RenderedImages {
        if !self.include_images {
            return RenderedImages {
                images: Vec::new(),
                omitted_bytes: 0,
            };
        }

        let mut images = VecDeque::new();
        let mut image_bytes = 0usize;
        let mut omitted_bytes = 0usize;
        let mut include_image = |image_url: &str, detail: Option<ImageDetail>| {
            if image_url.len() > MAX_TRANSCRIPT_IMAGE_BYTES {
                omitted_bytes = omitted_bytes.saturating_add(image_url.len());
                return;
            }
            while images.len() >= MAX_TRANSCRIPT_IMAGES
                || image_bytes + image_url.len() > MAX_TRANSCRIPT_IMAGE_BYTES
            {
                let Some(ContentItem::InputImage { image_url, .. }) = images.pop_front() else {
                    break;
                };
                image_bytes -= image_url.len();
                omitted_bytes = omitted_bytes.saturating_add(image_url.len());
            }
            image_bytes += image_url.len();
            images.push_back(ContentItem::InputImage {
                image_url: image_url.to_owned(),
                detail,
            });
        };

        for item in items {
            match item {
                ResponseItem::Message { role, content, .. }
                    if matches!(role.as_str(), "user" | "assistant") =>
                {
                    for item in content {
                        if let ContentItem::InputImage { image_url, detail } = item {
                            include_image(image_url, *detail);
                        }
                    }
                }
                ResponseItem::FunctionCallOutput { output, .. }
                | ResponseItem::CustomToolCallOutput { output, .. }
                    if self.sources.contains(&TranscriptSource::ToolOutputs) =>
                {
                    if let Some(content) = output.content_items() {
                        for item in content {
                            if let FunctionCallOutputContentItem::InputImage { image_url, detail } =
                                item
                            {
                                include_image(image_url, *detail);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if self.sources.contains(&TranscriptSource::ToolOutputs) {
            for image in node_repl_images {
                if let ContentItem::InputImage { image_url, detail } = image {
                    include_image(&image_url, detail);
                }
            }
        }

        RenderedImages {
            images: images.into_iter().collect(),
            omitted_bytes,
        }
    }

    pub(crate) fn build<'a>(
        &self,
        items: impl IntoIterator<Item = &'a ResponseItem>,
    ) -> RenderedTranscript {
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
                    call_id: Some(call_id),
                    output,
                    ..
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
                ResponseItem::FunctionCallOutput {
                    call_id: None,
                    name,
                    namespace,
                    output,
                    ..
                } => {
                    if !self.sources.contains(&TranscriptSource::ToolOutputs) {
                        continue;
                    }
                    let output = output
                        .body
                        .to_text()
                        .unwrap_or_else(|| "[non-text output]".into());
                    let role = match (name.as_deref(), namespace.as_deref()) {
                        (Some(name), Some(namespace)) => {
                            format!("tool {namespace}.{name} result")
                        }
                        (Some(name), None) => format!("tool {name} result"),
                        (None, _) => "tool result".to_owned(),
                    };
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
            let original_bytes = text.len();
            let text = truncate_entry(&text, token_cap);
            let retained_bytes = text.len();
            let entry_number = entries.len() + 1;
            let text = format!("[{entry_number}] {role}: {text}\n");
            let tokens = TruncationPolicy::Bytes(text.len()).token_budget();
            entries.push(TranscriptEntry {
                kind,
                text,
                tokens,
                original_bytes,
                retained_bytes,
            });
        }

        let mut included = vec![false; entries.len()];
        let mut message_tokens = 0;
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

        let available_message_tokens = self
            .max_message_transcript_tokens
            .saturating_sub(message_tokens);
        let mut retained_non_user = VecDeque::new();

        // Replay the transcript as an append-only bounded buffer. When a new entry would overflow
        // a budget, remove half of the existing entries in that pool before appending it. This
        // keeps the retained prefix stable between overflow points without storing sampler state.
        for (index, entry) in entries.iter().enumerate() {
            if entry.kind == TranscriptEntryKind::User {
                continue;
            }

            let token_budget = match entry.kind {
                TranscriptEntryKind::Tool => self.max_tool_transcript_tokens,
                TranscriptEntryKind::Message => available_message_tokens,
                TranscriptEntryKind::User => unreachable!("user entries were selected separately"),
            };

            // An entry that cannot fit on its own must not evict retained evidence.
            if entry.tokens > token_budget || self.max_recent_non_user_entries == 0 {
                continue;
            }

            if retained_tokens(&entries, &retained_non_user, entry.kind)
                .saturating_add(entry.tokens)
                > token_budget
            {
                let retained_kind_count = retained_non_user
                    .iter()
                    .filter(|&&retained_index| entries[retained_index].kind == entry.kind)
                    .count();
                evict_oldest_matching(
                    &entries,
                    &mut retained_non_user,
                    retained_kind_count.div_ceil(2),
                    |kind| kind == entry.kind,
                );
                while retained_tokens(&entries, &retained_non_user, entry.kind)
                    .saturating_add(entry.tokens)
                    > token_budget
                {
                    evict_oldest_matching(
                        &entries,
                        &mut retained_non_user,
                        /*count*/ 1,
                        |kind| kind == entry.kind,
                    );
                }
            }

            if retained_non_user.len().saturating_add(1) > self.max_recent_non_user_entries {
                let entries_to_evict = retained_non_user.len().div_ceil(2);
                evict_oldest_matching(&entries, &mut retained_non_user, entries_to_evict, |_| true);
            }

            retained_non_user.push_back(index);
        }

        for index in retained_non_user {
            included[index] = true;
        }

        let mut truncations = Vec::new();
        let entries = entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let component = match entry.kind {
                    TranscriptEntryKind::User => "transcript_user",
                    TranscriptEntryKind::Message => "transcript_message",
                    TranscriptEntryKind::Tool => "transcript_tool",
                };
                let retained_bytes = if included[index] {
                    entry.retained_bytes
                } else {
                    0
                };
                if entry.original_bytes > retained_bytes {
                    truncations.push(TruncationObservation {
                        component,
                        original_bytes: entry.original_bytes,
                        retained_bytes,
                    });
                }
                included[index].then_some(entry.text)
            })
            .collect();

        RenderedTranscript {
            entries,
            truncations,
        }
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
