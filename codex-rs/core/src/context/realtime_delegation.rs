use super::ContextualUserFragment;

const MAX_REALTIME_DELEGATION_FIELD_BYTES: usize = 4 * 1024;
const TRUNCATION_MARKER: &str = "…";

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
        let input = escape_xml_text_bounded(self.input, Retain::Start);
        let source = match self.source {
            RealtimeDelegationSource::Handoff => "",
            RealtimeDelegationSource::TranscriptTailFlush => {
                "  <source>transcript_tail_flush</source>\n"
            }
        };
        if let Some(transcript_delta) = self.transcript_delta.filter(|text| !text.is_empty()) {
            let transcript_delta = escape_xml_text_bounded(transcript_delta, Retain::End);
            return format!(
                "\n{source}  <input>{input}</input>\n  <transcript_delta>{transcript_delta}</transcript_delta>\n"
            );
        }

        format!("\n{source}  <input>{input}</input>\n")
    }
}

#[derive(Clone, Copy)]
enum Retain {
    Start,
    End,
}

fn escape_xml_text_bounded(input: &str, retain: Retain) -> String {
    let escaped = escape_xml_text(input);
    if escaped.len() <= MAX_REALTIME_DELEGATION_FIELD_BYTES {
        return escaped;
    }
    let retained_bytes = MAX_REALTIME_DELEGATION_FIELD_BYTES - TRUNCATION_MARKER.len();
    match retain {
        Retain::Start => {
            let mut end = retained_bytes;
            while !escaped.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}{TRUNCATION_MARKER}", &escaped[..end])
        }
        Retain::End => {
            let mut start = escaped.len() - retained_bytes;
            while !escaped.is_char_boundary(start) {
                start += 1;
            }
            format!("{TRUNCATION_MARKER}{}", &escaped[start..])
        }
    }
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
