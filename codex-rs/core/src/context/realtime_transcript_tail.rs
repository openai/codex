use super::ContextualUserFragment;
use crate::realtime_context::truncate_realtime_text_to_token_budget;

const REALTIME_TRANSCRIPT_TAIL_OPEN_TAG: &str = "<realtime_delegation>";
const REALTIME_TRANSCRIPT_TAIL_CLOSE_TAG: &str = "</realtime_delegation>";
const REALTIME_TRANSCRIPT_TAIL_TOKEN_BUDGET: usize = 8_000;
const REALTIME_TRANSCRIPT_TAIL_INSTRUCTIONS: &str = "The user just ended their realtime session. Here is the remaining handoff/transcript tail. You probably do not have to do anything; acknowledge the handoff unless the transcript itself asks for something.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RealtimeTranscriptTail {
    transcript_delta: String,
}

impl RealtimeTranscriptTail {
    pub(crate) fn new(transcript_delta: impl AsRef<str>) -> Self {
        Self {
            transcript_delta: truncate_realtime_text_to_token_budget(
                transcript_delta.as_ref(),
                REALTIME_TRANSCRIPT_TAIL_TOKEN_BUDGET,
            ),
        }
    }
}

impl ContextualUserFragment for RealtimeTranscriptTail {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            REALTIME_TRANSCRIPT_TAIL_OPEN_TAG,
            REALTIME_TRANSCRIPT_TAIL_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        let instructions = escape_xml_text(REALTIME_TRANSCRIPT_TAIL_INSTRUCTIONS);
        let transcript_delta = escape_xml_text(&self.transcript_delta);
        format!(
            "\n  <input>{instructions}</input>\n  <transcript_delta>{transcript_delta}</transcript_delta>\n"
        )
    }
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "realtime_transcript_tail_tests.rs"]
mod tests;
