use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

/// Apply the `x-openai-subagent` header when the session source indicates a
/// subagent. Returns the original builder unchanged when not applicable.
pub(crate) fn apply_subagent_header(
    mut builder: reqwest::RequestBuilder,
    session_source: Option<&SessionSource>,
) -> reqwest::RequestBuilder {
    if let Some(SessionSource::SubAgent(sub)) = session_source {
        let subagent = if let SubAgentSource::Other(label) = sub {
            label.clone()
        } else {
            serde_json::to_value(sub)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| "other".to_string())
        };
        builder = builder.header("x-openai-subagent", subagent);
    }
    builder
}
