use crate::path_utils::normalize_for_path_comparison;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
use std::path::PathBuf;

use super::scope::MEMORY_SCOPE_KEY_USER;

pub(super) const MEMORY_SUBDIR: &str = "memory";
pub(super) const ROLLOUT_SUMMARIES_SUBDIR: &str = "rollout_summaries";
pub(super) const RAW_MEMORIES_FILENAME: &str = "raw_memories.md";
pub(super) const MEMORY_REGISTRY_FILENAME: &str = "MEMORY.md";
pub(super) const LEGACY_CONSOLIDATED_FILENAME: &str = "consolidated.md";
pub(super) const SKILLS_SUBDIR: &str = "skills";

const CWD_MEMORY_BUCKET_HEX_LEN: usize = 16;

/// Returns the on-disk memory root directory for a given working directory.
///
/// The cwd is normalized and hashed into a deterministic bucket under
/// `<codex_home>/memories/<hash>/memory`.
pub(super) fn memory_root_for_cwd(codex_home: &Path, cwd: &Path) -> PathBuf {
    let bucket = memory_bucket_for_cwd(cwd);
    codex_home.join("memories").join(bucket).join(MEMORY_SUBDIR)
}

/// Returns the on-disk user-shared memory root directory.
pub(super) fn memory_root_for_user(codex_home: &Path) -> PathBuf {
    codex_home
        .join("memories")
        .join(MEMORY_SCOPE_KEY_USER)
        .join(MEMORY_SUBDIR)
}

pub(super) fn rollout_summaries_dir(root: &Path) -> PathBuf {
    root.join(ROLLOUT_SUMMARIES_SUBDIR)
}

pub(super) fn raw_memories_file(root: &Path) -> PathBuf {
    root.join(RAW_MEMORIES_FILENAME)
}

/// Ensures the phase-1 memory directory layout exists for the given root.
pub(super) async fn ensure_layout(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(rollout_summaries_dir(root)).await
}

fn memory_bucket_for_cwd(cwd: &Path) -> String {
    let normalized = normalize_cwd_for_memory(cwd);
    let normalized = normalized.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let full_hash = format!("{:x}", hasher.finalize());
    full_hash[..CWD_MEMORY_BUCKET_HEX_LEN].to_string()
}

fn normalize_cwd_for_memory(cwd: &Path) -> PathBuf {
    normalize_for_path_comparison(cwd).unwrap_or_else(|_| cwd.to_path_buf())
}
