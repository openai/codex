use crate::context::ContextualUserFragment;
use crate::context::RealtimeTranscriptTail;
use crate::session::session::Session;
use codex_protocol::protocol::RealtimeTranscriptEntry;
use std::sync::Arc;
use tracing::warn;

pub(crate) fn format_realtime_transcript(entries: &[RealtimeTranscriptEntry]) -> Option<String> {
    let transcript = entries
        .iter()
        .map(|entry| format!("{role}: {text}", role = entry.role, text = entry.text))
        .collect::<Vec<_>>()
        .join("\n");
    (!transcript.is_empty()).then_some(transcript)
}

pub(crate) async fn persist_realtime_transcript_tail(
    sess: &Arc<Session>,
    transcript_tail: Vec<RealtimeTranscriptEntry>,
) {
    let Some(transcript_delta) = format_realtime_transcript(&transcript_tail) else {
        return;
    };
    let item = ContextualUserFragment::into(RealtimeTranscriptTail::new(transcript_delta));
    let turn_context = sess.new_default_turn().await;
    sess.record_conversation_items(&turn_context, std::slice::from_ref(&item))
        .await;
    if let Err(err) = sess.flush_rollout().await {
        warn!("failed to flush final realtime transcript tail: {err}");
    }
}
