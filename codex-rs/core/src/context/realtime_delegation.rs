use super::ContextualUserFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RealtimeDelegationSource {
    Handoff,
    TranscriptTailFlush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RealtimeDelegation<'a> {
    input: &'a str,
    transcript_delta: Option<&'a str>,
    source: RealtimeDelegationSource,
}

impl<'a> RealtimeDelegation<'a> {
    pub(crate) fn new(
        input: &'a str,
        transcript_delta: Option<&'a str>,
        source: RealtimeDelegationSource,
    ) -> Self {
        Self {
            input,
            transcript_delta,
            source,
        }
    }
}

impl ContextualUserFragment for RealtimeDelegation<'_> {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<realtime_delegation>", "</realtime_delegation>")
    }

    fn body(&self) -> String {
        let input = escape_xml_text(self.input);
        let source = match self.source {
            RealtimeDelegationSource::Handoff => "",
            RealtimeDelegationSource::TranscriptTailFlush => {
                "  <source>transcript_tail_flush</source>\n"
            }
        };
        if let Some(transcript_delta) = self.transcript_delta.filter(|text| !text.is_empty()) {
            let transcript_delta = escape_xml_text(transcript_delta);
            return format!(
                "\n{source}  <input>{input}</input>\n  <transcript_delta>{transcript_delta}</transcript_delta>\n"
            );
        }

        format!("\n{source}  <input>{input}</input>\n")
    }
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
