//! Converts completed audio transcripts into ordinary, source-attributed conversation evidence.
//! Transcripts participate in the same delta selection and budgets as other messages and tool results.

use crate::ConversationTranscriptEntry;
use crate::ConversationTranscriptEntryKind;
use crate::ConversationTranscriptOptions;
use crate::TranscriptContent;

/// Text captured from a recording and paired with its original source by the host.
#[derive(Clone, Copy)]
pub enum AudioTranscript<'a> {
    /// Speech from audio submitted on a user turn.
    UserInput { turn_id: &'a str, text: &'a str },
    /// Speech from audio returned by a tool call.
    ToolOutput { call_id: &'a str, text: &'a str },
}

pub(crate) fn collect_audio_transcript(
    transcript: AudioTranscript<'_>,
    options: &ConversationTranscriptOptions,
) -> Option<ConversationTranscriptEntry> {
    let (kind, text) = match transcript {
        AudioTranscript::UserInput { turn_id, text } => (
            ConversationTranscriptEntryKind::UserAudioTranscript(format!(
                "audio transcript (recording submitted on user turn {turn_id:?})"
            )),
            text,
        ),
        AudioTranscript::ToolOutput { call_id, text } if options.include_tool_outputs => (
            ConversationTranscriptEntryKind::ToolAudioTranscript(format!(
                "tool audio transcript (call {call_id:?})"
            )),
            text,
        ),
        AudioTranscript::ToolOutput { .. } => return None,
    };
    if text.trim().is_empty() {
        return None;
    }

    Some(ConversationTranscriptEntry {
        kind,
        original_bytes: text.len(),
        content: TranscriptContent::Text(text.to_owned()),
        // A transcription is distinct evidence from the original user message.
        retained_source: None,
    })
}
