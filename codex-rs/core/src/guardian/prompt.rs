use codex_extension_api::ConversationHistorySnapshot;
use codex_guardian_context::Budgeted;
use codex_guardian_context::CollectedContext;
use codex_guardian_context::ComposedContext;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
#[cfg(test)]
use codex_guardian_context::ConversationTranscriptEntry;
use codex_guardian_context::GuardianRootMessage;
use codex_guardian_context::PermissionContext;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::SectionError;
use codex_guardian_context::SectionHistory;
use codex_guardian_context::SectionInput;
use codex_guardian_context::default_registry;
use codex_protocol::models::ResponseItem;

use crate::context::ContextualUserFragment;
use crate::context::GuardianReviewEvidence;
use crate::context::GuardianToolDescriptions;
use crate::context::NodeReplReviewEvidence;
use crate::context::NodeReplReviewEvidenceMode;
use crate::context::node_repl_review_evidence_mode;
use crate::event_mapping::is_contextual_user_message_content;
use crate::session::session::Session;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use codex_utils_output_truncation::truncate_text;

use super::ApprovalRequestReasons;
use super::GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS;
use super::GUARDIAN_MAX_TOOL_ENTRY_TOKENS;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use super::approval_request::format_guardian_action_pretty;

const GUARDIAN_MAX_APPROVAL_REASON_TOKENS: usize = 512;
pub(super) const GUARDIAN_TRANSCRIPT_START: &str = ">>> TRANSCRIPT START\n";

pub(crate) struct GuardianPromptItems {
    pub(crate) context: ComposedContext,
    pub(crate) transcript_cursor: GuardianTranscriptCursor,
    pub(crate) node_repl_evidence_sequence: u64,
}

/// Points to the end of the transcript that the guardian has already reviewed.
/// The saved count is only reusable when `parent_history_version` still matches.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GuardianTranscriptCursor {
    pub(crate) parent_history_version: u64,
    pub(crate) transcript_entry_count: usize,
}

pub(crate) enum GuardianPromptMode {
    Full,
    Delta { cursor: GuardianTranscriptCursor },
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
    build_guardian_prompt_items_with_parent_turn(
        session,
        session.conversation_history_snapshot().await.as_ref(),
        /*parent_context*/ None,
        ApprovalRequestReasons {
            approval: None,
            retry: retry_reason,
        },
        request,
        mode,
        /*reviewed_node_repl_evidence_sequence*/ 0,
    )
    .await
}

pub(crate) async fn build_guardian_prompt_items_with_parent_turn(
    session: &Session,
    history: &dyn ConversationHistorySnapshot,
    parent_context: Option<&GuardianReviewContext>,
    reasons: ApprovalRequestReasons,
    request: GuardianApprovalRequest,
    mode: GuardianPromptMode,
    reviewed_node_repl_evidence_sequence: u64,
) -> anyhow::Result<GuardianPromptItems> {
    let evidence_mode = parent_context
        .map(|context| node_repl_review_evidence_mode(context.turn()))
        .unwrap_or(NodeReplReviewEvidenceMode::Disabled);
    let node_repl_transcripts_enabled = evidence_mode != NodeReplReviewEvidenceMode::Disabled;
    let node_repl_result_token_limit = if node_repl_transcripts_enabled {
        GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS
    } else {
        GUARDIAN_MAX_TOOL_ENTRY_TOKENS
    };
    let root_authorization = session
        .services
        .agent_control
        .root_user_authorization(session.thread_id)
        .await
        .map(|snapshot| snapshot.messages);
    let trusted_user_inputs = session
        .services
        .thread_extension_data
        .get_or_init(GuardianReviewEvidence::default)
        .user_input_snapshot(history)
        .fragments;
    let planned_action_json = format_guardian_action_pretty(&request)?;
    let planned_action = PlannedAction {
        json: planned_action_json,
        tool_descriptions: if let GuardianApprovalRequest::McpToolCall {
            tool_description,
            connector_description,
            ..
        } = &request
        {
            GuardianToolDescriptions::new(
                tool_description.as_deref(),
                connector_description.as_deref(),
            )
            .map(|descriptions| descriptions.render())
        } else {
            None
        },
        kind: match &request {
            GuardianApprovalRequest::NetworkAccess { trigger, .. } => PlannedActionKind::Network {
                has_trigger: trigger.is_some(),
            },
            GuardianApprovalRequest::WriteStdin { .. } => PlannedActionKind::TerminalInput,
            #[cfg(unix)]
            GuardianApprovalRequest::Execve { .. } => PlannedActionKind::Command,
            GuardianApprovalRequest::ExecCommand { .. }
            | GuardianApprovalRequest::ApplyPatch { .. }
            | GuardianApprovalRequest::McpToolCall { .. }
            | GuardianApprovalRequest::RequestPermissions { .. } => PlannedActionKind::Command,
        },
        reason: reasons.retry.or(reasons.approval).map(|reason| {
            truncate_text(
                &reason,
                TruncationPolicy::Tokens(GUARDIAN_MAX_APPROVAL_REASON_TOKENS),
            )
        }),
    };
    let permissions = parent_context.map(parent_turn_permissions);
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
    let sections = collect_guardian_context(
        &GuardianReviewHistory(history),
        node_repl_result_token_limit,
        root_authorization.as_deref().unwrap_or_default(),
        &trusted_user_inputs,
        Some(&planned_action),
        permissions.as_ref(),
        node_repl_context.as_ref(),
    )?;
    let transcript_entries = sections.transcript_entries();
    let transcript_cursor = GuardianTranscriptCursor {
        parent_history_version: history.review_history_version(),
        transcript_entry_count: transcript_entries.len(),
    };

    let prompt_shape = match mode {
        GuardianPromptMode::Full => GuardianPromptShape::Full,
        GuardianPromptMode::Delta { cursor } => {
            if cursor.parent_history_version == transcript_cursor.parent_history_version
                && cursor.transcript_entry_count <= transcript_cursor.transcript_entry_count
            {
                GuardianPromptShape::Delta {
                    already_seen_entry_count: cursor.transcript_entry_count,
                }
            } else {
                GuardianPromptShape::Full
            }
        }
    };
    let session_id = session.thread_id.to_string();
    let (transcript_entries, offset, placeholder, presentation) = match prompt_shape {
        GuardianPromptShape::Full => (
            transcript_entries,
            0,
            "<no retained transcript entries>",
            ContextPresentation::SyncFull {
                session_id: &session_id,
            },
        ),
        GuardianPromptShape::Delta {
            already_seen_entry_count,
        } => (
            &transcript_entries[already_seen_entry_count..],
            already_seen_entry_count,
            "<no retained transcript delta entries>",
            ContextPresentation::SyncDelta {
                session_id: &session_id,
            },
        ),
    };
    let profile = ContextProfile::synchronous();
    let mut transcript = profile.render_transcript(transcript_entries, offset);
    if transcript_entries.is_empty() {
        transcript
            .items
            .push(Budgeted::required(placeholder.to_owned()));
    }
    let context = sections.compose(presentation, transcript)?;
    Ok(GuardianPromptItems {
        context,
        transcript_cursor,
        node_repl_evidence_sequence,
    })
}

fn parent_turn_permissions(context: &GuardianReviewContext) -> PermissionContext {
    let turn = context.turn();
    let environment = context.environments().primary();
    #[allow(deprecated)]
    let cwd = environment
        .and_then(|environment| environment.cwd().to_abs_path().ok())
        .unwrap_or_else(|| turn.cwd.clone());
    let permission_profile = context
        .environments()
        .permission_profile_or_else(|| turn.permission_profile());
    let file_system_policy = permission_profile.file_system_sandbox_policy();
    PermissionContext {
        denied_paths: file_system_policy
            .get_unreadable_roots_with_cwd(&cwd)
            .into_iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect(),
        denied_globs: file_system_policy.get_unreadable_globs_with_cwd(&cwd),
    }
}

enum GuardianPromptShape {
    Full,
    Delta { already_seen_entry_count: usize },
}

/// Exercises the sync profile through the host's existing transcript tests.
#[cfg(test)]
pub(crate) fn render_guardian_transcript_entries(
    entries: &[ConversationTranscriptEntry],
) -> (Vec<String>, Option<String>) {
    let mut transcript =
        ContextProfile::synchronous().render_transcript(entries, /*entry_number_offset*/ 0);
    if entries.is_empty() {
        transcript.items.push(Budgeted::required(
            "<no retained transcript entries>".to_owned(),
        ));
    }
    (
        transcript
            .items
            .into_iter()
            .map(|item| item.content)
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
/// Per-entry truncation happens during collection, using the current review's
/// Node REPL cap; the cursor still counts every non-empty evidence entry.
pub(super) fn collect_guardian_context(
    history: &dyn SectionHistory,
    node_repl_result_token_limit: usize,
    root_conversation: &[GuardianRootMessage],
    trusted_user_answers: &[String],
    planned_action: Option<&PlannedAction>,
    permissions: Option<&PermissionContext>,
    node_repl: Option<&codex_guardian_context::NodeReplContext<'_>>,
) -> Result<CollectedContext, SectionError> {
    let mut profile = ContextProfile::synchronous();
    profile.transcript.entry_limits.node_repl_output_tokens = node_repl_result_token_limit;
    default_registry().prepare(&SectionInput {
        target: profile.target,
        history: &FilteredGuardianHistory(history),
        transcript: &profile.transcript,
        root_conversation,
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

struct GuardianReviewHistory<'a>(&'a dyn ConversationHistorySnapshot);

impl SectionHistory for GuardianReviewHistory<'_> {
    fn retained_context(&self) -> Option<&codex_history::RetainedContext> {
        self.0.retained_context()
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        self.0.review_items()
    }
}

struct FilteredGuardianHistory<'a>(&'a dyn SectionHistory);

impl SectionHistory for FilteredGuardianHistory<'_> {
    fn retained_context(&self) -> Option<&codex_history::RetainedContext> {
        self.0.retained_context()
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.0.items().filter(|item| {
            !matches!(
                item,
                ResponseItem::Message { role, content, .. }
                    if role == "user" && is_contextual_user_message_content(content)
            )
        }))
    }
}

pub(crate) fn guardian_truncate_text(content: &str, token_cap: usize) -> (String, bool) {
    (
        codex_guardian_context::truncate_text(content, token_cap),
        content.len() > approx_bytes_for_tokens(token_cap),
    )
}

use codex_guardian_reviewer::guardian_output_contract_prompt;

pub(crate) const BUNDLED_GUARDIAN_POLICY: &str = include_str!("../../assets/guardian/policy.md");
pub(crate) const BUNDLED_GUARDIAN_POLICY_TEMPLATE: &str =
    include_str!("../../assets/guardian/policy_template.md");
const TENANT_POLICY_CONFIG_PLACEHOLDER: &str = "{{ tenant_policy_config }}";

/// Guardian policy prompt.
///
/// Keep the bundled fallback in a dedicated markdown file so reviewers can
/// audit prompt changes directly without diffing through code. The output
/// contract is appended from code so it stays near `guardian_output_schema()`.
///
/// The template is intentionally separated from the default tenant policy
/// configuration so workspace-managed overrides can keep the configurable
/// section narrower than the full policy.
pub(super) fn guardian_policy_prompt_with_config_and_template(
    tenant_policy_config: &str,
    policy_template: &str,
) -> String {
    let template = policy_template.trim_end();
    let prompt = template.replace(
        TENANT_POLICY_CONFIG_PLACEHOLDER,
        tenant_policy_config.trim(),
    );
    format!("{prompt}\n\n{}\n", guardian_output_contract_prompt())
}
