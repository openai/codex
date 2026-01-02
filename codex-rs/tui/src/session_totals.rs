use codex_core::protocol::TokenUsage;
use codex_protocol::ConversationId;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SessionTotals {
    pub(crate) token_usage: TokenUsage,
    pub(crate) total_duration_ms: i64,
}

pub(crate) fn load(codex_home: &Path, conversation_id: ConversationId) -> Option<SessionTotals> {
    let path = totals_path(codex_home, conversation_id);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub(crate) fn store(
    codex_home: &Path,
    conversation_id: ConversationId,
    totals: &SessionTotals,
) -> std::io::Result<()> {
    let path = totals_path(codex_home, conversation_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(totals)
        .map_err(|e| std::io::Error::other(format!("failed to serialize session totals: {e}")))?;
    std::fs::write(path, format!("{json}\n"))
}

pub(crate) fn format_duration_ms(millis: i64) -> String {
    if millis < 1000 {
        return format!("{millis}ms");
    }
    if millis < 60_000 {
        return format!("{:.2}s", millis as f64 / 1000.0);
    }
    let minutes = millis / 60_000;
    let seconds = (millis % 60_000) / 1000;
    format!("{minutes}m {seconds:02}s")
}

fn totals_path(codex_home: &Path, conversation_id: ConversationId) -> PathBuf {
    codex_home
        .join("session_totals")
        .join(format!("{conversation_id}.json"))
}

pub(crate) fn prefer_higher_token_usage(a: TokenUsage, b: TokenUsage) -> TokenUsage {
    if b.total_tokens >= a.total_tokens {
        b
    } else {
        a
    }
}
