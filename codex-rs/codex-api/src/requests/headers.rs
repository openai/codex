use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;
use codex_utils_string::to_ascii_json_string;
use http::HeaderMap;
use http::HeaderValue;

/// Correlate both the media call and its observer with the same thread and session tree.
pub fn build_realtime_thread_headers(
    session_id: String,
    thread_id: String,
    thread_source: Option<ThreadSource>,
) -> HeaderMap {
    let mut headers = build_session_headers(Some(session_id), Some(thread_id));
    // Voice calls can span zero or many backing turns. Only the saved thread source applies.
    if let Some(source) = thread_source
        && source.as_str().len() <= 256
        && let Ok(metadata) = to_ascii_json_string(&serde_json::json!({ "thread_source": source }))
        && let Ok(value) = HeaderValue::from_str(&metadata)
    {
        headers.insert("x-codex-turn-metadata", value);
    }
    headers
}

pub fn build_session_headers(session_id: Option<String>, thread_id: Option<String>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(id) = session_id {
        insert_header(&mut headers, "session-id", &id);
    }
    if let Some(id) = thread_id {
        insert_header(&mut headers, "thread-id", &id);
    }
    headers
}

pub(crate) fn subagent_header(source: &Option<SessionSource>) -> Option<String> {
    let SessionSource::SubAgent(sub) = source.as_ref()? else {
        return None;
    };
    match sub {
        codex_protocol::protocol::SubAgentSource::Review => Some("review".to_string()),
        codex_protocol::protocol::SubAgentSource::Compact => Some("compact".to_string()),
        codex_protocol::protocol::SubAgentSource::MemoryConsolidation => {
            Some("memory_consolidation".to_string())
        }
        codex_protocol::protocol::SubAgentSource::ThreadSpawn { .. } => {
            Some("collab_spawn".to_string())
        }
        codex_protocol::protocol::SubAgentSource::Other(label) => Some(label.clone()),
    }
}

pub(crate) fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        name.parse::<http::HeaderName>(),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}
