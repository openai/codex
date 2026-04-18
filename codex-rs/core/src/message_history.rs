use crate::config::Config;
pub use codex_message_history::HistoryEntry;
use codex_message_history::MessageHistoryConfig;
use codex_protocol::ThreadId;
use std::io::Result;

fn message_history_config(config: &Config) -> MessageHistoryConfig {
    MessageHistoryConfig {
        codex_home: config.codex_home.clone(),
        persistence: config.history.persistence,
        max_bytes: config.history.max_bytes,
    }
}

pub async fn append_entry(text: &str, conversation_id: &ThreadId, config: &Config) -> Result<()> {
    codex_message_history::append_entry(text, conversation_id, &message_history_config(config))
        .await
}

pub async fn history_metadata(config: &Config) -> (u64, usize) {
    codex_message_history::history_metadata(&message_history_config(config)).await
}

pub fn lookup(log_id: u64, offset: usize, config: &Config) -> Option<HistoryEntry> {
    codex_message_history::lookup(log_id, offset, &message_history_config(config))
}
