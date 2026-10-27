//! Composes synchronous review prompts from the host's captured context sections.
//! Full/delta selection, empty-transcript text, and cursor accounting preserve the existing layout.

use codex_guardian_context::CollectedContext;
use codex_guardian_context::ComposedContext;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::TranscriptCursor as GuardianTranscriptCursor;
use codex_guardian_context::TranscriptMode as GuardianPromptMode;
use codex_guardian_context::TranscriptSelection;

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
    let (selection, transcript_cursor) =
        mode.select(sections.transcript_entries(), history_version);
    let (offset, presentation) = match selection {
        TranscriptSelection::Full(_) => (0, ContextPresentation::SyncFull { session_id }),
        TranscriptSelection::Delta { offset, .. } => {
            (offset, ContextPresentation::SyncDelta { session_id })
        }
    };
    let context =
        codex_guardian_context::prepare_sync_context(sections, profile, offset, presentation)?;
    Ok(GuardianPromptItems {
        context,
        transcript_cursor,
        node_repl_evidence_sequence,
    })
}
