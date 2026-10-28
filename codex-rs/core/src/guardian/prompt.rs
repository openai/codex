use codex_extension_api::ConversationHistorySnapshot;
use codex_features::Feature;
#[cfg(test)]
use codex_guardian_context::Budgeted;
use codex_guardian_context::CollectedContext;
use codex_guardian_context::ContextProfile;
#[cfg(test)]
use codex_guardian_context::ConversationTranscriptEntry;
use codex_guardian_context::GuardianRootMessage;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::SectionError;
use codex_guardian_context::SectionHistory;
use codex_guardian_context::SectionInput;
#[cfg(test)]
pub(crate) use codex_guardian_context::TranscriptCursor as GuardianTranscriptCursor;
pub(crate) use codex_guardian_context::TranscriptMode as GuardianPromptMode;
use codex_guardian_context::default_registry;
#[cfg(test)]
use codex_guardian_reviewer::GuardianPromptItems;
use codex_protocol::models::ResponseItem;

use crate::context::GuardianPermissionContext;
use crate::context::NodeReplReviewEvidence;
use crate::context::NodeReplReviewEvidenceMode;
use crate::context::is_guardian_context_message;
use crate::context::node_repl_review_evidence_mode;
use crate::session::session::Session;

use super::ApprovalRequestReasons;
use super::GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS;
use super::GUARDIAN_MAX_TOOL_ENTRY_TOKENS;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;

// Bound both JSON and permission evidence without restricting manual approvals.
const MAX_GUARDIAN_ENVIRONMENT_ID_BYTES: usize = 256;
pub(super) use codex_guardian_reviewer::GUARDIAN_TRANSCRIPT_START;

/// Selects transcript evidence using the checkpoint already loaded by the reviewer.
pub(crate) enum GuardianTranscriptHistory<'a> {
    Retained(&'a dyn ConversationHistorySnapshot),
    LoadedParentCheckpoint(&'a dyn ConversationHistorySnapshot),
}

/// Builds the guardian user content items from:
/// - a compact transcript for authorization and local context
/// - the exact action JSON being proposed for approval
///
/// The fixed guardian policy lives in the review session developer message.
/// Split the variable request into separate user content items so the
/// Responses request snapshot shows clear boundaries while preserving exact
/// prompt text through trailing newlines.
#[cfg(test)]
pub(crate) async fn build_guardian_prompt_items(
    session: &Session,
    retry_reason: Option<String>,
    request: GuardianApprovalRequest,
    mode: GuardianPromptMode,
) -> anyhow::Result<GuardianPromptItems> {
    let collected = collect_guardian_prompt_items_with_parent_turn(
        session,
        GuardianTranscriptHistory::Retained(session.conversation_history_snapshot().await.as_ref()),
        /*parent_context*/ None,
        ApprovalRequestReasons {
            approval: None,
            retry: retry_reason,
        },
        request,
        mode,
        /*reviewed_node_repl_evidence_sequence*/ 0,
    )
    .await?;
    codex_guardian_reviewer::compose_guardian_prompt(
        collected.sections,
        &collected.profile,
        collected.mode,
        collected.history_version,
        &collected.session_id,
        collected.node_repl_evidence_sequence,
    )
}

/// Captured request sources and host cursor facts, before request-size preparation.
pub(super) struct CollectedGuardianPrompt {
    pub(super) sections: CollectedContext,
    pub(super) profile: ContextProfile,
    pub(super) mode: GuardianPromptMode,
    pub(super) history_version: u64,
    pub(super) session_id: String,
    pub(super) node_repl_evidence_sequence: u64,
}

pub(super) async fn collect_guardian_prompt_items_with_parent_turn(
    session: &Session,
    transcript_history: GuardianTranscriptHistory<'_>,
    parent_context: Option<&GuardianReviewContext>,
    reasons: ApprovalRequestReasons,
    request: GuardianApprovalRequest,
    mode: GuardianPromptMode,
    reviewed_node_repl_evidence_sequence: u64,
) -> anyhow::Result<CollectedGuardianPrompt> {
    let (history, use_parent_checkpoint) = match transcript_history {
        GuardianTranscriptHistory::Retained(history) => (history, false),
        GuardianTranscriptHistory::LoadedParentCheckpoint(history) => (history, true),
    };
    if request
        .target_environment_id()
        .is_some_and(|id| id.len() > MAX_GUARDIAN_ENVIRONMENT_ID_BYTES)
    {
        anyhow::bail!("approval environment id exceeds Guardian's 256-byte limit");
    }
    let evidence_mode = parent_context
        .map(|context| node_repl_review_evidence_mode(context.turn()))
        .unwrap_or(NodeReplReviewEvidenceMode::Disabled);
    let node_repl_transcripts_enabled = evidence_mode != NodeReplReviewEvidenceMode::Disabled;
    let node_repl_result_token_limit = if node_repl_transcripts_enabled {
        GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS
    } else {
        GUARDIAN_MAX_TOOL_ENTRY_TOKENS
    };
    let config = session.get_config().await;
    let root_authorization = session
        .services
        .agent_control
        .get_guardian_package(session.thread_id)
        .await
        .and_then(|snapshot| snapshot.request_candidates);
    let trusted_user_inputs = history
        .retained_context()
        .map(codex_guardian_context::collect_verified_answer_candidates)
        .unwrap_or_default();
    let planned_action = codex_guardian_reviewer::build_guardian_planned_action(
        &request,
        reasons.retry.or(reasons.approval),
    )?;
    let permissions = parent_context
        .map(|context| {
            super::permissions::for_environment(context, request.target_environment_id())
        })
        .transpose()?;
    let node_repl_snapshot = if node_repl_transcripts_enabled {
        session
            .services
            .thread_extension_data
            .get::<NodeReplReviewEvidence>()
            .and_then(|evidence| evidence.snapshot_since(reviewed_node_repl_evidence_sequence))
    } else {
        None
    };
    let node_repl_context = node_repl_snapshot
        .as_ref()
        .map(|snapshot| snapshot.context(evidence_mode));
    let node_repl_evidence_sequence = node_repl_snapshot
        .as_ref()
        .map_or(reviewed_node_repl_evidence_sequence, |snapshot| {
            snapshot.sequence
        });
    let mut profile = ContextProfile::synchronous();
    profile.transcript.entry_limits.node_repl_output_tokens = node_repl_result_token_limit;
    profile.transcript.options.native_multi_agent_namespace = parent_context
        .is_some_and(|context| {
            let turn = context.turn();
            turn.config
                .features
                .enabled(Feature::GuardianNativeMultiAgentHistory)
                && turn.multi_agent_version == codex_protocol::protocol::MultiAgentVersion::V2
        })
        .then(|| {
            config
                .multi_agent_v2
                .tool_namespace
                .clone()
                .filter(|namespace| !namespace.is_empty())
                .unwrap_or_else(|| "functions".to_owned())
        });
    profile.transcript_format = match parent_context {
        Some(context) => context.turn().config.guardian_transcript_mode,
        None => config.guardian_transcript_mode,
    };
    let sections = collect_guardian_context(
        &GuardianReviewHistory {
            history,
            use_parent_checkpoint,
        },
        &profile,
        root_authorization.as_ref(),
        &trusted_user_inputs,
        Some(&planned_action),
        permissions.as_ref(),
        node_repl_context.as_ref(),
    )?;
    Ok(CollectedGuardianPrompt {
        sections,
        profile,
        mode,
        history_version: if use_parent_checkpoint {
            history.history_version()
        } else {
            history.review_history_version()
        },
        session_id: session.thread_id.to_string(),
        node_repl_evidence_sequence,
    })
}

/// Exercises the sync profile through the host's existing transcript tests.
#[cfg(test)]
pub(crate) fn render_guardian_transcript_entries(
    entries: &[ConversationTranscriptEntry],
    node_repl_result_token_limit: usize,
) -> (Vec<String>, Option<String>) {
    let mut profile = ContextProfile::synchronous();
    profile.transcript.entry_limits.node_repl_output_tokens = node_repl_result_token_limit;
    let mut transcript = profile.prepare_transcript(entries, /*entry_number_offset*/ 0);
    if entries.is_empty() {
        transcript.items.push(Budgeted::required(
            codex_guardian_context::TranscriptContent::Text(
                "<no retained transcript entries>".to_owned(),
            ),
        ));
    }
    (
        transcript
            .items
            .into_iter()
            .map(|item| match item.content {
                codex_guardian_context::TranscriptContent::Text(text) => text,
                codex_guardian_context::TranscriptContent::Record(record) => record.to_string(),
                codex_guardian_context::TranscriptContent::ScheduledMessage(_)
                | codex_guardian_context::TranscriptContent::NativeMessage(_) => {
                    panic!("expected text transcript")
                }
            })
            .collect(),
        transcript.omission_note,
    )
}

/// Retains the human-readable conversation plus recent tool call / result
/// evidence for guardian review and skips synthetic contextual scaffolding that
/// would just add noise because the guardian reviewer already gets the normal
/// inherited top-level context from session startup.
///
/// Keep both tool calls and tool results here. The reviewer often needs the
/// agent's exact queried path / arguments as well as the returned evidence to
/// decide whether the pending approval is justified.
/// Per-entry truncation happens in sync preparation after cursor selection.
/// The cursor counts every non-empty evidence entry.
pub(super) fn collect_guardian_context(
    history: &dyn SectionHistory,
    profile: &ContextProfile,
    root_conversation_candidates: Option<&codex_guardian_context::RootConversationInput>,
    trusted_user_answers: &[codex_guardian_context::TrustedUserAnswer],
    planned_action: Option<&PlannedAction>,
    permissions: Option<&GuardianPermissionContext>,
    node_repl: Option<&codex_guardian_context::NodeReplContext<'_>>,
) -> Result<CollectedContext, SectionError> {
    default_registry().prepare(&SectionInput {
        target: profile.target,
        history: &FilteredGuardianHistory(history),
        transcript: &profile.transcript,
        root_conversation: &[],
        root_conversation_candidates,
        trusted_user_answers,
        planned_action,
        permissions,
        previous_reviews: None,
        trusted_tool: None,
        trusted_skill_paths: &[],
        images: None,
        node_repl,
    })
}

struct GuardianReviewHistory<'a> {
    history: &'a dyn ConversationHistorySnapshot,
    use_parent_checkpoint: bool,
}

impl SectionHistory for GuardianReviewHistory<'_> {
    fn retained_context(&self) -> Option<&codex_history::RetainedContext> {
        self.history.retained_context()
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.items_with_sources().map(|(item, _)| item))
    }

    fn items_with_sources(
        &self,
    ) -> Box<dyn Iterator<Item = (&ResponseItem, Option<&codex_history::RetainedSource>)> + Send + '_>
    {
        if self.use_parent_checkpoint && !self.history.uses_parent_context_for_review() {
            // The checkpoint replaces the legacy backup. Preserve the current
            // parent window without claiming complete legacy source metadata.
            Box::new(self.history.items().map(|item| (item, None)))
        } else {
            self.history.review_items_with_sources()
        }
    }

    fn render_original_assistant(
        &self,
        message: &codex_history::RetainedUserMessage,
    ) -> Option<GuardianRootMessage> {
        crate::context::render_original_assistant_context(message)
            .map(GuardianRootMessage::Assistant)
    }

    fn render_retained_assistant(
        &self,
        message: &codex_history::RetainedUserMessage,
    ) -> Option<GuardianRootMessage> {
        crate::context::render_retained_assistant_context(message)
            .map(GuardianRootMessage::Assistant)
    }
}

struct FilteredGuardianHistory<'a>(&'a dyn SectionHistory);

impl SectionHistory for FilteredGuardianHistory<'_> {
    fn items_with_sources(
        &self,
    ) -> Box<dyn Iterator<Item = (&ResponseItem, Option<&codex_history::RetainedSource>)> + Send + '_>
    {
        Box::new(
            self.0
                .items_with_sources()
                .filter(|(item, _)| !is_guardian_context_message(item)),
        )
    }

    fn retained_context(&self) -> Option<&codex_history::RetainedContext> {
        self.0.retained_context()
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.items_with_sources().map(|(item, _)| item))
    }

    fn render_original_assistant(
        &self,
        message: &codex_history::RetainedUserMessage,
    ) -> Option<GuardianRootMessage> {
        self.0.render_original_assistant(message)
    }

    fn render_retained_assistant(
        &self,
        message: &codex_history::RetainedUserMessage,
    ) -> Option<GuardianRootMessage> {
        self.0.render_retained_assistant(message)
    }
}
