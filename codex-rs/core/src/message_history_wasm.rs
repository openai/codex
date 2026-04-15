use crate::config::Config;
use codex_protocol::ThreadId;

pub struct HistoryEntry {
    pub session_id: String,
    pub ts: u64,
    pub text: String,
}

pub async fn append_entry(
    _text: &str,
    _conversation_id: &ThreadId,
    _config: &Config,
) -> crate::error::Result<()> {
    Ok(())
}

pub async fn history_metadata(_config: &Config) -> (u64, usize) {
    (0, 0)
}

pub fn lookup(_log_id: u64, _offset: usize, _config: &Config) -> Option<HistoryEntry> {
    None
}
