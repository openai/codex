//! Rollout module: persistence and discovery of session rollout files.

pub(crate) const SESSIONS_SUBDIR: &str = "sessions";

pub mod list;
pub mod recorder;

pub use recorder::RolloutRecorder;
pub use recorder::SessionStateSnapshot;

#[allow(dead_code)]
impl RolloutRecorder {
    /// List conversations (rollout files) under the provided Codex home directory.
    pub async fn list_conversations(
        codex_home: &std::path::Path,
        page_size: usize,
        cursor: Option<&str>,
    ) -> std::io::Result<crate::rollout::list::ConversationsPage> {
        list::get_conversations(codex_home, page_size, cursor).await
    }
}
