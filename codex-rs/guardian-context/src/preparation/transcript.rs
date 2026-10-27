//! Resolved Guardian evidence profiles, transcript admission and retention.
//! Sync keeps recent entries; async protects approvals/final answers and evicts
//! in cacheable chunks. Hosts still own history snapshots and delivery cursors.
//! User messages and manual approvals stay complete until whole-request admission.
//! They may be shortened with explicit markers only during budget recovery.
//! The host selects line or JSON rendering; JSON keeps embedded headers inside text.

use codex_protocol::protocol::TruncationPolicy;
use std::collections::HashMap;

use crate::BudgetPriority;
use crate::Budgeted;
use crate::ContextTarget;
use crate::ConversationTranscriptConfig;
use crate::ConversationTranscriptEntry;
use crate::ConversationTranscriptEntryKind;
use crate::ConversationTranscriptOptions;
use crate::PreparedTranscript;
use crate::Retention;
use crate::TranscriptContent;
use crate::TranscriptEntryLimits;
use crate::TranscriptFormat;
use crate::TranscriptRetentionConfig;
use crate::TruncationObservation;
use crate::transcript_record::TranscriptRecord;

use self::window::TranscriptWindow;
mod window;

const MIN_RECENT_TOOL_ENTRIES: usize = 5;

/// Request-local policy resolved from the consumer's model and configuration.
/// Registry scope follows `target`; source flags and caps apply before retention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextProfile {
    pub target: ContextTarget,
    pub transcript_format: TranscriptFormat,
    pub transcript: ConversationTranscriptConfig,
    pub retention: TranscriptRetentionConfig,
    pub include_images: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TranscriptEntryKind {
    User,
    ManualApproval,
    ProtectedMessage,
    Message,
    Tool,
}

struct TranscriptEntry {
    kind: TranscriptEntryKind,
    content: TranscriptContent,
    tokens: usize,
    original_bytes: usize,
    retained_bytes: usize,
    source: Option<codex_history::RetainedSource>,
}

impl ContextProfile {
    /// Existing synchronous defaults before the host resolves the REPL output cap.
    pub const fn synchronous() -> Self {
        Self {
            target: ContextTarget::Sync,
            transcript_format: TranscriptFormat::Line,
            transcript: ConversationTranscriptConfig {
                options: ConversationTranscriptOptions {
                    include_tool_calls: true,
                    include_tool_outputs: true,
                    include_reasoning: false,
                    native_multi_agent_namespace: None,
                },
                entry_limits: TranscriptEntryLimits {
                    message_tokens: 5_000,
                    tool_tokens: 1_000,
                    node_repl_output_tokens: 1_000,
                },
            },
            retention: TranscriptRetentionConfig {
                max_message_transcript_tokens: 20_000,
                max_tool_transcript_tokens: 10_000,
                max_recent_non_user_entries: 40,
            },
            include_images: false,
        }
    }

    /// Existing asynchronous defaults before model and local overrides.
    pub const fn asynchronous() -> Self {
        Self {
            target: ContextTarget::Async,
            transcript_format: TranscriptFormat::Line,
            transcript: ConversationTranscriptConfig {
                options: ConversationTranscriptOptions {
                    include_tool_calls: true,
                    include_tool_outputs: true,
                    include_reasoning: false,
                    native_multi_agent_namespace: None,
                },
                entry_limits: TranscriptEntryLimits {
                    message_tokens: 2_000,
                    tool_tokens: 1_000,
                    node_repl_output_tokens: 1_000,
                },
            },
            retention: TranscriptRetentionConfig {
                max_message_transcript_tokens: 10_000,
                max_tool_transcript_tokens: 10_000,
                max_recent_non_user_entries: 40,
            },
            include_images: true,
        }
    }

    /// Prepares structured, bounded entries without advancing the host's full/delta cursor.
    /// The host supplies the slice and original offset; empty placeholders depend
    /// on its full/delta presentation and are supplied after selection.
    pub fn prepare_transcript(
        &self,
        transcript_entries: &[ConversationTranscriptEntry],
        entry_number_offset: usize,
    ) -> PreparedTranscript {
        let entries = transcript_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let kind = match &entry.kind {
                    ConversationTranscriptEntryKind::User => TranscriptEntryKind::User,
                    ConversationTranscriptEntryKind::Developer => {
                        TranscriptEntryKind::ManualApproval
                    }
                    ConversationTranscriptEntryKind::ProtectedAssistant => {
                        TranscriptEntryKind::ProtectedMessage
                    }
                    ConversationTranscriptEntryKind::Assistant
                    | ConversationTranscriptEntryKind::Reasoning => TranscriptEntryKind::Message,
                    ConversationTranscriptEntryKind::ToolCall(_)
                    | ConversationTranscriptEntryKind::ToolOutput(_)
                    | ConversationTranscriptEntryKind::NodeReplToolOutput(_) => {
                        TranscriptEntryKind::Tool
                    }
                };
                let number = index + entry_number_offset + 1;
                let suffix = match self.target {
                    ContextTarget::Sync => "",
                    ContextTarget::Async => "\n",
                };
                let retained_source = entry.retained_source.as_ref().filter(|source| {
                    entry.kind == ConversationTranscriptEntryKind::User
                        && (self.target == ContextTarget::Async
                            || source.rendered_bytes
                                <= TruncationPolicy::Tokens(
                                    crate::retained_instructions::MAX_INSTRUCTION_TOKENS,
                                )
                                .byte_budget())
                });
                let prepare_text = |text: &str| {
                    let record = TranscriptRecord::new(
                        &entry.kind,
                        number,
                        text.to_owned(),
                        retained_source.map(|retained| retained.order.clone()),
                        self.transcript_format,
                        suffix,
                    );
                    let tokens =
                        TruncationPolicy::Bytes(record.rendered().text_bytes).token_budget();
                    (TranscriptContent::Record(record), tokens, text.len())
                };
                let content = match self.target {
                    ContextTarget::Sync => {
                        prepare_sync_content(entry, &self.transcript.entry_limits)
                    }
                    ContextTarget::Async => std::borrow::Cow::Borrowed(&entry.content),
                };
                let (content, tokens, retained_bytes) = match content.as_ref() {
                    TranscriptContent::Text(text) => prepare_text(text),
                    TranscriptContent::ScheduledMessage(message) => prepare_text(&message.text),
                    TranscriptContent::Record(record) => prepare_text(record.text()),
                    TranscriptContent::NativeMessage(message) => (
                        TranscriptContent::NativeMessage(message.clone()),
                        crate::native_tool_call::estimate_tokens(message),
                        entry.original_bytes,
                    ),
                };
                TranscriptEntry {
                    kind,
                    tokens,
                    content,
                    original_bytes: entry.original_bytes,
                    retained_bytes,
                    source: retained_source.map(|retained| retained.source.clone()),
                }
            })
            .collect::<Vec<_>>();
        let mut included = vec![false; entries.len()];
        let mut authorization_tokens = 0usize;
        for (index, entry) in entries.iter().enumerate() {
            if matches!(
                entry.kind,
                TranscriptEntryKind::User | TranscriptEntryKind::ManualApproval
            ) {
                included[index] = true;
                authorization_tokens = authorization_tokens.saturating_add(entry.tokens);
            }
        }
        match self.target {
            ContextTarget::Sync => {
                let mut message_tokens = authorization_tokens;
                let mut tool_tokens = 0usize;
                let mut retained_non_user_entries = 0;
                for (index, entry) in entries.iter().enumerate().rev() {
                    if matches!(
                        entry.kind,
                        TranscriptEntryKind::User | TranscriptEntryKind::ManualApproval
                    ) || retained_non_user_entries >= self.retention.max_recent_non_user_entries
                    {
                        continue;
                    }
                    let (tokens, limit) = if entry.kind == TranscriptEntryKind::Tool {
                        (&mut tool_tokens, self.retention.max_tool_transcript_tokens)
                    } else {
                        (
                            &mut message_tokens,
                            self.retention.max_message_transcript_tokens,
                        )
                    };
                    if tokens.saturating_add(entry.tokens) <= limit {
                        included[index] = true;
                        retained_non_user_entries += 1;
                        *tokens += entry.tokens;
                    }
                }
            }
            ContextTarget::Async => {
                let available = self
                    .retention
                    .max_message_transcript_tokens
                    .saturating_sub(authorization_tokens);
                let mut window = TranscriptWindow::new(&entries, &self.retention, available);
                for index in 0..entries.len() {
                    window.insert(index);
                }
                for index in window.into_indices() {
                    included[index] = true;
                }
            }
        }
        let omission_note = (self.target == ContextTarget::Sync
            && included.iter().any(|included| !included))
        .then(|| crate::transcript::TRANSCRIPT_OMISSION_NOTICE.to_owned());
        let mut truncations = Vec::new();
        let mut items: Vec<_> = entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let component = match entry.kind {
                    TranscriptEntryKind::User => "transcript_user",
                    TranscriptEntryKind::ManualApproval
                    | TranscriptEntryKind::ProtectedMessage
                    | TranscriptEntryKind::Message => "transcript_message",
                    TranscriptEntryKind::Tool => "transcript_tool",
                };
                let retained_bytes = if included[index] {
                    entry.retained_bytes
                } else {
                    0
                };
                if self.target == ContextTarget::Async && entry.original_bytes > retained_bytes {
                    truncations.push(TruncationObservation {
                        component,
                        original_bytes: entry.original_bytes,
                        retained_bytes,
                    });
                }
                if included[index] {
                    Some(match entry.kind {
                        TranscriptEntryKind::User | TranscriptEntryKind::ManualApproval => {
                            if let Some(source) = entry.source {
                                let mut item = Budgeted::required(entry.content);
                                item.source = Some(source);
                                item
                            } else {
                                Budgeted::historical(entry.content)
                            }
                        }
                        TranscriptEntryKind::ProtectedMessage => Budgeted::required(entry.content),
                        TranscriptEntryKind::Message => {
                            Budgeted::optional(entry.content, BudgetPriority::Commentary)
                        }
                        TranscriptEntryKind::Tool => {
                            Budgeted::optional(entry.content, BudgetPriority::Tool)
                        }
                    })
                } else {
                    None
                }
            })
            .collect();
        // Keep the newest tool evidence even when the aggregate allowance is tight.
        for item in items
            .iter_mut()
            .rev()
            .filter(|item| item.retention == Retention::Optional(BudgetPriority::Tool))
            .take(MIN_RECENT_TOOL_ENTRIES)
        {
            item.retention = Retention::Required;
        }
        PreparedTranscript {
            items,
            omission_note,
            truncations,
        }
    }
}

/// Bounds sync content before rendering or calculating retention costs.
fn prepare_sync_content<'a>(
    entry: &'a crate::ConversationTranscriptEntry,
    limits: &crate::TranscriptEntryLimits,
) -> std::borrow::Cow<'a, TranscriptContent> {
    let text = match &entry.content {
        TranscriptContent::Text(text) => text.as_str(),
        TranscriptContent::ScheduledMessage(message) => message.text.as_str(),
        TranscriptContent::Record(record) => record.text(),
        TranscriptContent::NativeMessage(_) => {
            // Native ciphertext stays whole, or leaves an omission at the same position.
            let limit = codex_protocol::protocol::TruncationPolicy::Tokens(limits.message_tokens)
                .byte_budget();
            return if entry.original_bytes > limit {
                std::borrow::Cow::Owned(TranscriptContent::Text(
                    crate::transcript::TRANSCRIPT_OMISSION_NOTICE.to_owned(),
                ))
            } else {
                std::borrow::Cow::Borrowed(&entry.content)
            };
        }
    };
    let tokens = match &entry.kind {
        ConversationTranscriptEntryKind::ToolCall(_)
        | ConversationTranscriptEntryKind::ToolOutput(_) => limits.tool_tokens,
        ConversationTranscriptEntryKind::Assistant
        | ConversationTranscriptEntryKind::ProtectedAssistant
        | ConversationTranscriptEntryKind::Reasoning => limits.message_tokens,
        ConversationTranscriptEntryKind::NodeReplToolOutput(_) => limits.node_repl_output_tokens,
        ConversationTranscriptEntryKind::User | ConversationTranscriptEntryKind::Developer => {
            return std::borrow::Cow::Borrowed(&entry.content);
        }
    };
    std::borrow::Cow::Owned(TranscriptContent::Text(crate::truncate_text(text, tokens)))
}

#[derive(Clone, PartialEq)]
pub(crate) struct HeartbeatInstruction {
    automation_id: String,
    timestamp: String,
    instructions: String,
}

impl From<codex_history::Heartbeat<'_>> for HeartbeatInstruction {
    fn from(value: codex_history::Heartbeat<'_>) -> Self {
        Self {
            automation_id: value.automation_id.to_owned(),
            timestamp: value.timestamp.to_owned(),
            instructions: value.instructions.to_owned(),
        }
    }
}

#[derive(Default)]
pub(crate) struct HeartbeatReferences {
    versions: HashMap<String, (String, usize)>,
}

impl HeartbeatReferences {
    /// Returns whether the representation no longer proves complete source delivery.
    pub(crate) fn prepare(
        &mut self,
        heartbeat: Option<&HeartbeatInstruction>,
        text: &mut String,
        number: usize,
    ) -> bool {
        let Some(heartbeat) = heartbeat else {
            self.versions.clear();
            return false;
        };
        match self.versions.get(&heartbeat.automation_id) {
            Some((instructions, previous)) if *instructions == heartbeat.instructions => {
                let automation_id = &heartbeat.automation_id;
                let timestamp = &heartbeat.timestamp;
                *text = format!(
                    "Scheduled automation {automation_id} ran at {timestamp}. Instructions unchanged from transcript entry [{previous}]; this is a replay of that earlier instruction, not a new human instruction. If the referenced instructions are unavailable, do not infer authorization from this reference."
                );
                true
            }
            _ => {
                self.versions.insert(
                    heartbeat.automation_id.clone(),
                    (heartbeat.instructions.clone(), number),
                );
                false
            }
        }
    }
}

/// Runs over the complete collected transcript so delta references keep their old targets.
pub(super) fn prepare_sync_heartbeats(context: &mut crate::CollectedContext) {
    for section in &mut context.sections {
        let crate::ContextSection::ConversationTranscript { items } = section else {
            continue;
        };
        let mut references = HeartbeatReferences::default();
        for (index, entry) in items.iter_mut().enumerate() {
            let crate::TranscriptContent::ScheduledMessage(message) = &mut entry.content else {
                continue;
            };
            if references.prepare(message.heartbeat.as_ref(), &mut message.text, index + 1) {
                entry.retained_source = None;
            }
            entry.content = crate::TranscriptContent::Text(std::mem::take(&mut message.text));
        }
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
