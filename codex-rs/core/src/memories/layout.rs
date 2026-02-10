use std::path::Path;
use std::path::PathBuf;

pub(super) const ROLLOUT_SUMMARIES_SUBDIR: &str = "rollout_summaries";
pub(super) const RAW_MEMORIES_FILENAME: &str = "raw_memories.md";
pub(super) const MEMORY_REGISTRY_FILENAME: &str = "MEMORY.md";
pub(super) const LEGACY_CONSOLIDATED_FILENAME: &str = "consolidated.md";
pub(super) const SKILLS_SUBDIR: &str = "skills";
const LEGACY_USER_SUBDIR: &str = "user";
const LEGACY_MEMORY_SUBDIR: &str = "memory";

/// Returns the shared on-disk memory root directory.
pub(super) fn memory_root(codex_home: &Path) -> PathBuf {
    codex_home.join("memories")
}

pub(super) fn rollout_summaries_dir(root: &Path) -> PathBuf {
    root.join(ROLLOUT_SUMMARIES_SUBDIR)
}

pub(super) fn raw_memories_file(root: &Path) -> PathBuf {
    root.join(RAW_MEMORIES_FILENAME)
}

/// Migrates legacy user memory contents into the shared root when no shared-root
/// phase artifacts exist yet.
pub(super) async fn migrate_legacy_user_memory_root_if_needed(
    codex_home: &Path,
) -> std::io::Result<()> {
    let root = memory_root(codex_home);
    let legacy = legacy_user_memory_root(codex_home);

    if !tokio::fs::try_exists(&legacy).await? || global_root_has_phase_artifacts(&root).await? {
        return Ok(());
    }

    copy_dir_contents_if_missing(&legacy, &root).await
}

/// Ensures the phase-1 memory directory layout exists for the given root.
pub(super) async fn ensure_layout(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(rollout_summaries_dir(root)).await
}

fn legacy_user_memory_root(codex_home: &Path) -> PathBuf {
    codex_home
        .join("memories")
        .join(LEGACY_USER_SUBDIR)
        .join(LEGACY_MEMORY_SUBDIR)
}

async fn global_root_has_phase_artifacts(root: &Path) -> std::io::Result<bool> {
    if tokio::fs::try_exists(&rollout_summaries_dir(root)).await?
        || tokio::fs::try_exists(&raw_memories_file(root)).await?
        || tokio::fs::try_exists(&root.join(MEMORY_REGISTRY_FILENAME)).await?
        || tokio::fs::try_exists(&root.join(LEGACY_CONSOLIDATED_FILENAME)).await?
        || tokio::fs::try_exists(&root.join(SKILLS_SUBDIR)).await?
    {
        return Ok(true);
    }
    Ok(false)
}

fn copy_dir_contents_if_missing<'a>(
    src_dir: &'a Path,
    dst_dir: &'a Path,
) -> futures::future::BoxFuture<'a, std::io::Result<()>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(dst_dir).await?;
        let mut dir = tokio::fs::read_dir(src_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst_dir.join(entry.file_name());
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                copy_dir_contents_if_missing(&src_path, &dst_path).await?;
            } else if metadata.is_file() && !tokio::fs::try_exists(&dst_path).await? {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
        Ok(())
    })
}
