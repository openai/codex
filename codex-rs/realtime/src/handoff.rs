use codex_protocol::protocol::RealtimeHandoffRequested;

pub const REALTIME_USER_TEXT_PREFIX: &str = "[USER] ";
pub const REALTIME_BACKEND_TEXT_PREFIX: &str = "[BACKEND] ";

pub fn prefix_realtime_v2_text(text: String, prefix: &str) -> String {
    if text.is_empty() || text.starts_with(prefix) {
        return text;
    }
    format!("{prefix}{text}")
}

fn realtime_transcript_delta_from_handoff(handoff: &RealtimeHandoffRequested) -> Option<String> {
    let active_transcript = handoff
        .active_transcript
        .iter()
        .map(|entry| format!("{role}: {text}", role = entry.role, text = entry.text))
        .collect::<Vec<_>>()
        .join("\n");
    (!active_transcript.is_empty()).then_some(active_transcript)
}

pub fn realtime_text_from_handoff_request(handoff: &RealtimeHandoffRequested) -> Option<String> {
    (!handoff.input_transcript.is_empty())
        .then_some(handoff.input_transcript.clone())
        .or_else(|| realtime_transcript_delta_from_handoff(handoff))
}

pub fn realtime_delegation_from_handoff(handoff: &RealtimeHandoffRequested) -> Option<String> {
    let input = realtime_text_from_handoff_request(handoff)?;
    Some(wrap_realtime_delegation_input(
        &input,
        realtime_transcript_delta_from_handoff(handoff).as_deref(),
    ))
}

pub fn wrap_realtime_delegation_input(input: &str, transcript_delta: Option<&str>) -> String {
    let input = escape_xml_text(input);
    if let Some(transcript_delta) = transcript_delta.filter(|text| !text.is_empty()) {
        let transcript_delta = escape_xml_text(transcript_delta);
        return format!(
            "<realtime_delegation>\n  <input>{input}</input>\n  <transcript_delta>{transcript_delta}</transcript_delta>\n</realtime_delegation>"
        );
    }

    format!("<realtime_delegation>\n  <input>{input}</input>\n</realtime_delegation>")
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
