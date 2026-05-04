use std::path::Path;

pub async fn clear_memory_roots_contents(codex_home: &Path) -> std::io::Result<()> {
    for memory_root in [
        codex_home.join("memories"),
        codex_home.join("memories_extensions"),
    ] {
        clear_memory_root_contents(memory_root.as_path()).await?;
    }

    Ok(())
}

pub(crate) async fn clear_memory_root_contents(memory_root: &Path) -> std::io::Result<()> {
    match tokio::fs::symlink_metadata(memory_root).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "refusing to clear symlinked memory root {}",
                    memory_root.display()
                ),
            ));
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }

    tokio::fs::create_dir_all(memory_root).await?;

    let mut entries = tokio::fs::read_dir(memory_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = entry.file_type().await?;
        if file_type.is_dir() {
            tokio::fs::remove_dir_all(path).await?;
        } else {
            tokio::fs::remove_file(path).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn clear_memory_root_contents_preserves_root_directory() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("memories");
        let nested_dir = root.join("rollout_summaries");
        tokio::fs::create_dir_all(&nested_dir)
            .await
            .expect("create rollout summaries dir");
        tokio::fs::write(root.join("MEMORY.md"), "stale memory index\n")
            .await
            .expect("write memory index");
        tokio::fs::write(nested_dir.join("rollout.md"), "stale rollout\n")
            .await
            .expect("write rollout summary");

        clear_memory_root_contents(&root)
            .await
            .expect("clear memory root contents");

        assert!(
            tokio::fs::try_exists(&root)
                .await
                .expect("check memory root existence"),
            "memory root should still exist after clearing contents"
        );
        let mut entries = tokio::fs::read_dir(&root)
            .await
            .expect("read memory root after clear");
        assert!(
            entries
                .next_entry()
                .await
                .expect("read next entry")
                .is_none(),
            "memory root should be empty after clearing contents"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn clear_memory_root_contents_rejects_symlinked_root() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("outside");
        tokio::fs::create_dir_all(&target)
            .await
            .expect("create symlink target dir");
        let target_file = target.join("keep.txt");
        tokio::fs::write(&target_file, "keep\n")
            .await
            .expect("write target file");

        let root = dir.path().join("memories");
        std::os::unix::fs::symlink(&target, &root).expect("create memory root symlink");

        let err = clear_memory_root_contents(&root)
            .await
            .expect_err("symlinked memory root should be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            tokio::fs::try_exists(&target_file)
                .await
                .expect("check target file existence"),
            "rejecting a symlinked memory root should not delete the symlink target"
        );
    }
}
