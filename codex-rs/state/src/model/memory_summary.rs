use codex_protocol::ThreadId;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummary {
    pub thread_id: ThreadId,
    pub cwd: PathBuf,
    pub summary: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummaryLock {
    pub cwd: PathBuf,
    pub owner_id: String,
    pub acquired_at: i64,
    pub expires_at: i64,
}
