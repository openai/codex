use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use http::HeaderValue;

pub fn build_session_headers(session_id: Option<String>, thread_id: Option<String>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(id) = session_id {
        insert_header(&mut headers, "session_id", &id);
    }
    if let Some(id) = thread_id {
        insert_header(&mut headers, "thread_id", &id);
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
