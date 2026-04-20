use std::path::Path;

pub async fn clear_memory_roots_contents(codex_home: &Path) -> std::io::Result<()> {
    let memory_root = codex_home.join("memories");
    clear_memory_root_contents(&memory_root).await?;

    let legacy_extensions_root = codex_home.join("memories_extensions");
    match tokio::fs::symlink_metadata(&legacy_extensions_root).await {
        Ok(_) => clear_memory_root_contents(&legacy_extensions_root).await,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
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
