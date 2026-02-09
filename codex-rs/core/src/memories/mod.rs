mod phase_one;
mod prompts;
mod rollout;
mod selection;
mod storage;
mod types;

#[cfg(test)]
mod tests;

use crate::path_utils::normalize_for_path_comparison;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
use std::path::PathBuf;

/// Subagent source label used to identify consolidation tasks.
pub(crate) const MEMORY_CONSOLIDATION_SUBAGENT_LABEL: &str = "memory_consolidation";
/// Maximum number of rollout candidates processed per startup pass.
pub(crate) const MAX_ROLLOUTS_PER_STARTUP: usize = 8;
/// Concurrency cap for startup memory extraction and consolidation scheduling.
pub(crate) const PHASE_ONE_CONCURRENCY_LIMIT: usize = MAX_ROLLOUTS_PER_STARTUP;
/// Maximum number of recent raw memories retained per working directory.
pub(crate) const MAX_RAW_MEMORIES_PER_CWD: usize = 10;
/// Lease duration (seconds) for per-cwd consolidation locks.
pub(crate) const CONSOLIDATION_LOCK_LEASE_SECONDS: i64 = 600;

const MEMORY_SUBDIR: &str = "memory";
const RAW_MEMORIES_SUBDIR: &str = "raw_memories";
const MEMORY_SUMMARY_FILENAME: &str = "memory_summary.md";
const MEMORY_REGISTRY_FILENAME: &str = "MEMORY.md";
const LEGACY_CONSOLIDATED_FILENAME: &str = "consolidated.md";
const SKILLS_SUBDIR: &str = "skills";

pub(crate) use phase_one::RAW_MEMORY_PROMPT;
pub(crate) use phase_one::parse_stage_one_output;
pub(crate) use phase_one::stage_one_output_schema;
pub(crate) use prompts::build_consolidation_prompt;
pub(crate) use prompts::build_stage_one_input_message;
#[cfg(test)]
pub(crate) use rollout::StageOneResponseItemKinds;
pub(crate) use rollout::StageOneRolloutFilter;
pub(crate) use rollout::serialize_filtered_rollout_response_items;
pub(crate) use selection::select_rollout_candidates_from_db;
pub(crate) use storage::prune_to_recent_memories_and_rebuild_summary;
pub(crate) use storage::wipe_consolidation_outputs;
pub(crate) use storage::write_raw_memory;
pub(crate) use types::RolloutCandidate;

/// Returns the on-disk memory root directory for a given working directory.
///
/// The cwd is normalized and hashed into a deterministic bucket under
/// `<codex_home>/memories/<hash>/memory`.
pub(crate) fn memory_root_for_cwd(codex_home: &Path, cwd: &Path) -> PathBuf {
    let bucket = memory_bucket_for_cwd(cwd);
    codex_home.join("memories").join(bucket).join(MEMORY_SUBDIR)
}

fn raw_memories_dir(root: &Path) -> PathBuf {
    root.join(RAW_MEMORIES_SUBDIR)
}

fn memory_summary_file(root: &Path) -> PathBuf {
    root.join(MEMORY_SUMMARY_FILENAME)
}

/// Ensures the phase-1 memory directory layout exists for the given root.
pub(crate) async fn ensure_layout(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(raw_memories_dir(root)).await
}

fn memory_bucket_for_cwd(cwd: &Path) -> String {
    let normalized = normalize_for_path_comparison(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let normalized = normalized.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}
