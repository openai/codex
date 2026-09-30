use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ResponseItem;
pub(crate) use codex_features::GuardianV2TranscriptSource as TranscriptSource;
use codex_guardian_context::CollectedContext;
use codex_guardian_context::ComposedContext;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::ConversationTranscriptConfig;
use codex_guardian_context::ConversationTranscriptOptions;
use codex_guardian_context::GuardianRootMessage;
#[cfg(test)]
use codex_guardian_context::MANUAL_APPROVAL_DEVELOPER_PREFIX;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PreviousReviews;
use codex_guardian_context::SectionError;
use codex_guardian_context::SectionHistory;
use codex_guardian_context::SectionInput;
use codex_guardian_context::TranscriptCursor;
use codex_guardian_context::TranscriptEntryLimits;
use codex_guardian_context::TranscriptImageInput;
use codex_guardian_context::TranscriptMode;
use codex_guardian_context::TranscriptRetentionConfig;
use codex_guardian_context::TranscriptSelection;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::default_registry;

pub(crate) const MAX_MESSAGE_ENTRY_TOKENS: usize = ContextProfile::asynchronous()
    .transcript
    .entry_limits
    .message_tokens;
pub(crate) const MAX_TOOL_ENTRY_TOKENS: usize = ContextProfile::asynchronous()
    .transcript
    .entry_limits
    .tool_tokens;
pub(crate) const MAX_MESSAGE_TRANSCRIPT_TOKENS: usize = ContextProfile::asynchronous()
    .retention
    .max_message_transcript_tokens;
pub(crate) const MAX_TOOL_TRANSCRIPT_TOKENS: usize = ContextProfile::asynchronous()
    .retention
    .max_tool_transcript_tokens;
pub(crate) const MAX_RECENT_NON_USER_ENTRIES: usize = ContextProfile::asynchronous()
    .retention
    .max_recent_non_user_entries;
/// Host snapshot and evidence borrowed for a single section collection.
pub(crate) struct ContextInput<'a> {
    pub(crate) target: ContextTarget,
    pub(crate) history: &'a dyn ConversationHistorySnapshot,
    pub(crate) root_conversation: &'a [GuardianRootMessage],
    pub(crate) trusted_user_answers: &'a [String],
    pub(crate) planned_action: Option<&'a PlannedAction>,
    pub(crate) permissions: Option<&'a codex_guardian_context::PermissionContext>,
    pub(crate) previous_reviews: Option<&'a PreviousReviews>,
    pub(crate) trusted_tool: Option<&'a TrustedTool>,
    pub(crate) trusted_skill_paths: &'a [String],
    pub(crate) node_repl_images: Option<&'a [codex_protocol::models::ContentItem]>,
}

pub(crate) type RenderedContext = ComposedContext;

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
            include_images: ContextProfile::asynchronous().include_images,
            max_message_entry_tokens: MAX_MESSAGE_ENTRY_TOKENS,
            max_tool_entry_tokens: MAX_TOOL_ENTRY_TOKENS,
            max_message_transcript_tokens: MAX_MESSAGE_TRANSCRIPT_TOKENS,
            max_tool_transcript_tokens: MAX_TOOL_TRANSCRIPT_TOKENS,
            max_recent_non_user_entries: MAX_RECENT_NON_USER_ENTRIES,
        }
    }
}

impl TranscriptConfig {
    pub(crate) fn build_context(
        &self,
        input: ContextInput<'_>,
    ) -> Result<RenderedContext, SectionError> {
        let (mut context, _) = self.collect_context(input)?.compose(/*cursor*/ None, &[])?;
        context.deduplicate_transcript_instructions();
        Ok(context)
    }

    pub(super) fn collect_context(
        &self,
        input: ContextInput<'_>,
    ) -> Result<CollectedTranscript, SectionError> {
        let ContextInput {
            target,
            history,
            root_conversation,
            trusted_user_answers,
            planned_action,
            permissions,
            previous_reviews,
            trusted_tool,
            trusted_skill_paths,
            node_repl_images,
        } = input;
        let history_version = history.review_history_version();
        let history = SnapshotHistory(history);
        let profile = ContextProfile {
            target: ContextTarget::Async,
            include_images: self.include_images,
            retention: TranscriptRetentionConfig {
                max_message_transcript_tokens: self.max_message_transcript_tokens,
                max_tool_transcript_tokens: self.max_tool_transcript_tokens,
                max_recent_non_user_entries: self.max_recent_non_user_entries,
            },
            transcript: ConversationTranscriptConfig {
                options: ConversationTranscriptOptions {
                    include_tool_calls: self.sources.contains(&TranscriptSource::ToolCalls),
                    include_tool_outputs: self.sources.contains(&TranscriptSource::ToolOutputs),
                    include_reasoning: self.sources.contains(&TranscriptSource::Reasoning),
                },
                entry_limits: TranscriptEntryLimits {
                    message_tokens: self.max_message_entry_tokens,
                    tool_tokens: self.max_tool_entry_tokens,
                    node_repl_output_tokens: self.max_tool_entry_tokens,
                },
            },
        };
        let context = default_registry().prepare(&SectionInput {
            target,
            history: &history,
            transcript: &profile.transcript,
            root_conversation,
            trusted_user_answers,
            planned_action,
            permissions,
            previous_reviews,
            trusted_tool,
            trusted_skill_paths,
            images: node_repl_images.map(|node_repl_images| TranscriptImageInput {
                enabled: profile.include_images,
                include_tool_outputs: profile.transcript.options.include_tool_outputs,
                node_repl_images,
            }),
            node_repl: None,
        })?;
        Ok(CollectedTranscript {
            context,
            profile,
            history_version,
        })
    }
}

/// Captured evidence stays typed until the owning conversation selects its delta.
#[derive(Clone)]
pub(super) struct CollectedTranscript {
    context: CollectedContext,
    profile: ContextProfile,
    history_version: u64,
}

impl CollectedTranscript {
    pub(super) fn select(
        &self,
        cursor: Option<TranscriptCursor>,
    ) -> (TranscriptSelection<'_>, TranscriptCursor) {
        let mode = cursor.map_or(TranscriptMode::Full, |cursor| TranscriptMode::Delta {
            cursor,
        });
        mode.select(self.context.transcript_entries(), self.history_version)
    }

    pub(super) fn compose(
        mut self,
        cursor: Option<TranscriptCursor>,
        reviewer_history: &[codex_history::ResponseItemEnvelope],
    ) -> Result<(ComposedContext, TranscriptCursor), SectionError> {
        self.context.retain_new_reviews(reviewer_history);
        let (selection, next) = self.select(cursor);
        let (entries, offset, presentation) = match selection {
            TranscriptSelection::Full(entries) => (entries, 0, ContextPresentation::Async),
            TranscriptSelection::Delta { entries, offset } => {
                (entries, offset, ContextPresentation::AsyncDelta)
            }
        };
        let transcript = self.profile.render_transcript(entries, offset);
        let context = self.context.compose(presentation, transcript)?;
        Ok((context, next))
    }
}

struct SnapshotHistory<'a>(&'a dyn ConversationHistorySnapshot);

impl SectionHistory for SnapshotHistory<'_> {
    fn retained_context(&self) -> Option<&codex_history::RetainedContext> {
        self.0.retained_context()
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.items_with_sources().map(|(item, _)| item))
    }

    fn items_with_sources(
        &self,
    ) -> Box<dyn Iterator<Item = (&ResponseItem, Option<&codex_history::RetainedSource>)> + Send + '_>
    {
        self.0.review_items_with_sources()
    }

    fn render_retained_assistant(
        &self,
        message: &codex_history::RetainedUserMessage,
    ) -> Option<GuardianRootMessage> {
        codex_core::context::render_retained_assistant_context(message)
            .map(GuardianRootMessage::Assistant)
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
