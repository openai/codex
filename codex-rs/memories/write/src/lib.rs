//! Write-path helpers for Codex memories.
//!
//! This crate owns the file-backed memory artifact helpers, Phase 1 and Phase
//! 2 prompt rendering, extension pruning, and workspace diffing. Runtime
//! orchestration for Phase 1 and Phase 2 remains in `codex-core`.

mod control;
mod extensions;
mod prompts;
mod storage;
pub mod workspace;

use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;

pub use control::clear_memory_roots_contents;
pub use extensions::prune_old_extension_resources;
pub use prompts::build_consolidation_prompt;
pub use prompts::build_stage_one_input_message;
pub use storage::rebuild_raw_memories_file_from_memories;
pub use storage::rollout_summary_file_stem;
pub use storage::sync_rollout_summaries_from_memories;

/// Prompt used for phase 1 extraction.
pub const STAGE_ONE_PROMPT: &str = include_str!("../templates/memories/stage_one_system.md");

/// Fallback stage-1 rollout truncation limit (tokens) when model metadata
/// does not include a valid context window.
pub const DEFAULT_STAGE_ONE_ROLLOUT_TOKEN_LIMIT: usize = 150_000;

/// Portion of the model effective input window reserved for the stage-1
/// rollout input.
///
/// Keeping this below 100% leaves room for system instructions, prompt framing,
/// and model output.
pub const STAGE_ONE_CONTEXT_WINDOW_PERCENT: i64 = 70;

mod artifacts {
    pub(super) const EXTENSIONS_SUBDIR: &str = "extensions";
    pub(super) const ROLLOUT_SUMMARIES_SUBDIR: &str = "rollout_summaries";
    pub(super) const RAW_MEMORIES_FILENAME: &str = "raw_memories.md";
}

pub fn memory_root(codex_home: &AbsolutePathBuf) -> AbsolutePathBuf {
    codex_home.join("memories")
}

pub fn rollout_summaries_dir(root: &Path) -> PathBuf {
    root.join(artifacts::ROLLOUT_SUMMARIES_SUBDIR)
}

pub fn memory_extensions_root(root: &Path) -> PathBuf {
    root.join(artifacts::EXTENSIONS_SUBDIR)
}

pub fn raw_memories_file(root: &Path) -> PathBuf {
    root.join(artifacts::RAW_MEMORIES_FILENAME)
}

pub async fn ensure_layout(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(rollout_summaries_dir(root)).await
}
