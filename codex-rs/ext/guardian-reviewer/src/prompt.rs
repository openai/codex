//! Composes synchronous review prompts from the host's captured context sections.
//! Full/delta selection, empty-transcript text, and cursor accounting preserve the existing layout.

use codex_guardian_context::CollectedContext;
use codex_guardian_context::ComposedContext;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::TranscriptCursor as GuardianTranscriptCursor;
use codex_guardian_context::TranscriptMode as GuardianPromptMode;
use codex_guardian_context::TranscriptSelection;

pub const GUARDIAN_TRANSCRIPT_START: &str = ">>> TRANSCRIPT START\n";

pub struct GuardianPromptItems {
    pub context: ComposedContext,
    pub transcript_cursor: GuardianTranscriptCursor,
    pub node_repl_evidence_sequence: u64,
}

pub fn compose_guardian_prompt(
    sections: CollectedContext,
    profile: &ContextProfile,
    mode: GuardianPromptMode,
    history_version: u64,
    session_id: &str,
    node_repl_evidence_sequence: u64,
) -> anyhow::Result<GuardianPromptItems> {
    let (offset, presentation, transcript_cursor) =
        select_transcript(&sections, mode, history_version, session_id);
    let context =
        codex_guardian_context::prepare_sync_context(sections, profile, offset, presentation)?;
    Ok(GuardianPromptItems {
        context,
        transcript_cursor,
        node_repl_evidence_sequence,
    })
}

/// Prepares and composes any sync request after the host resolves reviewer/model facts.
pub async fn compose_guardian_prompt_with_preparation(
    sections: CollectedContext,
    profile: &ContextProfile,
    mode: GuardianPromptMode,
    history_version: u64,
    session_id: &str,
    node_repl_evidence_sequence: u64,
    preparation: codex_guardian_context::SyncPreparation<'_>,
) -> anyhow::Result<GuardianPromptItems> {
    let (offset, presentation, transcript_cursor) =
        select_transcript(&sections, mode, history_version, session_id);
    let context = codex_guardian_context::prepare_sync_context_with_facts(
        sections,
        profile,
        offset,
        presentation,
        preparation,
    )
    .await?;
    Ok(GuardianPromptItems {
        context,
        transcript_cursor,
        node_repl_evidence_sequence,
    })
}

fn select_transcript<'a>(
    sections: &CollectedContext,
    mode: GuardianPromptMode,
    history_version: u64,
    session_id: &'a str,
) -> (usize, ContextPresentation<'a>, GuardianTranscriptCursor) {
    let (selection, transcript_cursor) =
        mode.select(sections.transcript_entries(), history_version);
    let (offset, presentation) = match selection {
        TranscriptSelection::Full(_) => (0, ContextPresentation::SyncFull { session_id }),
        TranscriptSelection::Delta { offset, .. } => {
            (offset, ContextPresentation::SyncDelta { session_id })
        }
    };
    (offset, presentation, transcript_cursor)
}

pub fn guardian_truncate_text(content: &str, token_cap: usize) -> (String, bool) {
    (
        codex_guardian_context::truncate_text(content, token_cap),
        content.len() > codex_utils_output_truncation::approx_bytes_for_tokens(token_cap),
    )
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
