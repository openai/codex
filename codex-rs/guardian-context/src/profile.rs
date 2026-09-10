//! Resolved Guardian evidence profiles and pure transcript retention.
//! Sync keeps recent entries; async protects approvals/final answers and evicts
//! in cacheable chunks. Hosts still own history snapshots and delivery cursors.
//! User messages and manual approvals stay complete until whole-request admission.
//! They may be shortened with explicit markers only during budget recovery.

use codex_protocol::protocol::TruncationPolicy;

use crate::BudgetPriority;
use crate::Budgeted;
use crate::ContextTarget;
use crate::ConversationTranscriptConfig;
use crate::ConversationTranscriptEntry;
use crate::ConversationTranscriptEntryKind;
use crate::ConversationTranscriptOptions;
use crate::RenderedTranscript;
use crate::Retention;
use crate::TranscriptEntryLimits;
use crate::TranscriptRetentionConfig;
use crate::TruncationObservation;

use self::window::TranscriptWindow;
mod window;

const MIN_RECENT_TOOL_ENTRIES: usize = 5;

/// Request-local policy resolved from the consumer's model and configuration.
/// Registry scope follows `target`; source flags and caps apply before retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextProfile {
    pub target: ContextTarget,
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
    text: String,
    tokens: usize,
    original_bytes: usize,
    retained_bytes: usize,
}

impl ContextProfile {
    /// Existing synchronous defaults before the host resolves the REPL output cap.
    pub const fn synchronous() -> Self {
        Self {
            target: ContextTarget::Sync,
            transcript: ConversationTranscriptConfig {
                options: ConversationTranscriptOptions {
                    include_tool_calls: true,
                    include_tool_outputs: true,
                    include_reasoning: false,
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
            transcript: ConversationTranscriptConfig {
                options: ConversationTranscriptOptions {
                    include_tool_calls: true,
                    include_tool_outputs: true,
                    include_reasoning: false,
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

    /// Selects bounded entries without advancing the host's full/delta cursor.
    /// The host supplies the slice and original offset; empty placeholders depend
    /// on its full/delta presentation and are supplied after selection.
    pub fn render_transcript(
        &self,
        transcript_entries: &[ConversationTranscriptEntry],
        entry_number_offset: usize,
    ) -> RenderedTranscript {
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
                let role = entry.kind.role();
                let suffix = match self.target {
                    ContextTarget::Sync => "",
                    ContextTarget::Async => "\n",
                };
                let text = format!("[{number}] {role}: {}{suffix}", entry.text);
                TranscriptEntry {
                    kind,
                    tokens: TruncationPolicy::Bytes(text.len()).token_budget(),
                    text,
                    original_bytes: entry.original_bytes,
                    retained_bytes: entry.text.len(),
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
        .then(|| "Some conversation entries were omitted.".to_owned());
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
                            Budgeted::historical(entry.text)
                        }
                        TranscriptEntryKind::ProtectedMessage => Budgeted::required(entry.text),
                        TranscriptEntryKind::Message => {
                            Budgeted::optional(entry.text, BudgetPriority::Commentary)
                        }
                        TranscriptEntryKind::Tool => {
                            Budgeted::optional(entry.text, BudgetPriority::Tool)
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
        RenderedTranscript {
            items,
            omission_note,
            truncations,
        }
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
