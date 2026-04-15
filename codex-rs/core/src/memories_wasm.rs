use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::memory_citation::MemoryCitation;

use crate::codex::Session;
use crate::config::Config;

pub(crate) mod citations {
    use super::*;

    pub fn parse_memory_citation(_citations: Vec<String>) -> Option<MemoryCitation> {
        None
    }

    pub fn get_thread_id_from_citations(_citations: Vec<String>) -> Vec<ThreadId> {
        Vec::new()
    }
}

pub(crate) mod prompts {
    use super::*;

    pub(crate) async fn build_memory_tool_developer_instructions(
        _codex_home: &Path,
    ) -> Option<String> {
        None
    }
}

pub(crate) mod usage {
    use crate::tools::context::ToolInvocation;

    pub(crate) async fn emit_metric_for_tool_read(_invocation: &ToolInvocation, _success: bool) {}
}

pub(crate) fn start_memories_startup_task(
    _sess: &Arc<Session>,
    _config: Arc<Config>,
    _session_source: &codex_protocol::protocol::SessionSource,
) {
}

pub(crate) async fn clear_memory_root_contents(memory_root: &Path) -> std::io::Result<()> {
    crate::async_fs::create_dir_all(memory_root).await
}

pub fn memory_root(codex_home: &Path) -> PathBuf {
    codex_home.join("memories")
}
