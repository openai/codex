//! Recognizes Guardian session sources and scopes legacy reviewer cache keys to the parent thread.

use codex_protocol::ThreadId;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

pub const GUARDIAN_REVIEWER_NAME: &str = "guardian";

pub fn is_basic_session_source(session_source: &SessionSource) -> bool {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::Other(label)) => label == GUARDIAN_REVIEWER_NAME,
        SessionSource::Internal(InternalSessionSource::Guardian) => true,
        _ => false,
    }
}

pub fn prompt_cache_key_override_for_review_session(
    session_source: &SessionSource,
    parent_thread_id: Option<ThreadId>,
) -> Option<String> {
    let SessionSource::SubAgent(SubAgentSource::Other(name)) = session_source else {
        return None;
    };
    if name != GUARDIAN_REVIEWER_NAME {
        return None;
    }
    let parent_thread_id = parent_thread_id?;
    Some(format!("guardian:{parent_thread_id}"))
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
