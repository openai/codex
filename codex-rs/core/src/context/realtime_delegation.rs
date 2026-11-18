use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;
use codex_protocol::realtime::RealtimeTranscriptRole;

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
    preserve_transcript: bool,
}

impl<'a> RealtimeDelegation<'a> {
    pub(crate) const FIELD_BYTES: usize = 4 * 1024;

    /// Render and attribute the same fields, so omitted transcript rows
    /// cannot accidentally appear in the accepted source list.
    pub(crate) fn from_events(
        input: &str,
        entries: &[(Option<&str>, RealtimeTranscriptRole, &str)],
        source: RealtimeDelegationSource,
        preserve_transcript: bool,
    ) -> Option<(String, Vec<String>)> {
        if input.is_empty() && entries.is_empty() {
            return None;
        }
        let mut transcript = String::new();
        let mut ranges = Vec::new();
        let mut escaped_len = 0;
        for (id, role, text) in entries {
            if !transcript.is_empty() {
                transcript.push('\n');
                escaped_len += 1;
            }
            let role = match role {
                RealtimeTranscriptRole::User => "user",
                RealtimeTranscriptRole::Assistant => "assistant",
            };
            let line = format!("{role}: {text}");
            let start = escaped_len;
            escaped_len += escape_xml_text(&line).len();
            transcript.push_str(&line);
            ranges.push((*id, start..escaped_len));
        }
        let escaped_transcript = escape_xml_text(&transcript);
        let delta_start = if preserve_transcript {
            0
        } else {
            retained_range(&escaped_transcript, Retain::End).start
        };
        let input_end = retained_range(&escaped_transcript, Retain::Start).end;
        let input_source = entries
            .iter()
            .rfind(|(_, role, text)| {
                *role == RealtimeTranscriptRole::User && text.trim() == input.trim()
            })
            .and_then(|(id, _, _)| *id);
        let sources = ranges
            .into_iter()
            .filter(|(id, range)| {
                range.end > delta_start
                    || (input.is_empty() && range.start < input_end)
                    || (id.is_some() && *id == input_source)
            })
            .filter_map(|(id, _)| id.map(str::to_string))
            .collect();
        let input = if input.is_empty() { &transcript } else { input };
        Some((
            RealtimeDelegation::new(input, Some(&transcript), source, preserve_transcript)
                .render()
                .into_text()?,
            sources,
        ))
    }

    pub(crate) fn new(
        input: &'a str,
        transcript_delta: Option<&'a str>,
        source: RealtimeDelegationSource,
        preserve_transcript: bool,
    ) -> Self {
        Self {
            input,
            transcript_delta,
            source,
            preserve_transcript,
        }
    }
}

impl ContextualUserFragment for RealtimeDelegation<'_> {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("realtime_conversation.delegation".to_string())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<realtime_delegation>", "</realtime_delegation>")
    }

    fn body(&self) -> ContentItem {
        let input = escape_xml_text_bounded(self.input, Retain::Start);
        let source = match self.source {
            RealtimeDelegationSource::Handoff => "",
            RealtimeDelegationSource::TranscriptTailFlush => {
                "  <source>transcript_tail_flush</source>\n"
            }
        };
        if let Some(transcript_delta) = self.transcript_delta.filter(|text| !text.is_empty()) {
            let transcript_delta = if self.preserve_transcript {
                escape_xml_text(transcript_delta)
            } else {
                escape_xml_text_bounded(transcript_delta, Retain::End)
            };
            return input_text(format!(
                "\n{source}  <input>{input}</input>\n  <transcript_delta>{transcript_delta}</transcript_delta>\n"
            ));
        }

        input_text(format!("\n{source}  <input>{input}</input>\n"))
    }
}

#[derive(Clone, Copy)]
enum Retain {
    Start,
    End,
}

fn escape_xml_text_bounded(input: &str, retain: Retain) -> String {
    let escaped = escape_xml_text(input);
    if escaped.len() <= RealtimeDelegation::FIELD_BYTES {
        return escaped;
    }
    let range = retained_range(&escaped, retain);
    match retain {
        Retain::Start => format!("{}{TRUNCATION_MARKER}", &escaped[range]),
        Retain::End => format!("{TRUNCATION_MARKER}{}", &escaped[range]),
    }
}

fn retained_range(escaped: &str, retain: Retain) -> std::ops::Range<usize> {
    if escaped.len() <= RealtimeDelegation::FIELD_BYTES {
        return 0..escaped.len();
    }
    let retained_bytes = RealtimeDelegation::FIELD_BYTES - TRUNCATION_MARKER.len();
    match retain {
        Retain::Start => {
            let mut end = retained_bytes;
            while !escaped.is_char_boundary(end) {
                end -= 1;
            }
            0..end
        }
        Retain::End => {
            let mut start = escaped.len() - retained_bytes;
            while !escaped.is_char_boundary(start) {
                start += 1;
            }
            start..escaped.len()
        }
    }
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "realtime_delegation_tests.rs"]
mod tests;
