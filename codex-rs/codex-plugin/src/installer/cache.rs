//! Plugin cache management.

use crate::error::Result;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

/// Get the cache directory for plugins.
pub fn get_cache_dir(codex_home: &Path) -> PathBuf {
    codex_home.join("plugins").join("cache")
}

/// Get the versioned path for a plugin installation.
pub fn get_plugin_version_path(
    cache_dir: &Path,
    marketplace: &str,
    plugin_name: &str,
    version: &str,
) -> PathBuf {
    cache_dir.join(marketplace).join(plugin_name).join(version)
}

/// List installed versions of a plugin.
pub async fn list_plugin_versions(
    cache_dir: &Path,
    marketplace: &str,
    plugin_name: &str,
) -> Result<Vec<String>> {
    let plugin_dir = cache_dir.join(marketplace).join(plugin_name);

    if !plugin_dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    let mut entries = fs::read_dir(&plugin_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                versions.push(name.to_string());
            }
        }
    }

    Ok(versions)
}

/// Clean up old versions of a plugin, keeping only the N most recent.
pub async fn cleanup_old_versions(
    cache_dir: &Path,
    marketplace: &str,
    plugin_name: &str,
    keep_count: usize,
) -> Result<usize> {
    let plugin_dir = cache_dir.join(marketplace).join(plugin_name);

    if !plugin_dir.exists() {
        return Ok(0);
    }

    let versions = list_plugin_versions(cache_dir, marketplace, plugin_name).await?;

    if versions.len() <= keep_count {
        return Ok(0);
    }

    // Sort by modification time (newest first)
    let mut versioned_paths: Vec<_> = versions.iter().map(|v| plugin_dir.join(v)).collect();

    versioned_paths.sort_by(|a, b| {
        let a_time = std::fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let b_time = std::fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        b_time.cmp(&a_time)
    });

    // Remove old versions
    let mut removed = 0;
    for path in versioned_paths.iter().skip(keep_count) {
        if path.exists() {
            fs::remove_dir_all(path).await?;
            debug!("Removed old version at {}", path.display());
            removed += 1;
        }
    }

    Ok(removed)
}

/// Get total cache size in bytes.
pub async fn get_cache_size(cache_dir: &Path) -> Result<u64> {
    if !cache_dir.exists() {
        return Ok(0);
    }

    fn dir_size_sync(path: &Path) -> std::io::Result<u64> {
        let mut size = 0u64;
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                size += dir_size_sync(&entry.path())?;
            } else {
                size += metadata.len();
            }
        }
        Ok(size)
    }

    let total_size = dir_size_sync(cache_dir).unwrap_or(0);

    Ok(total_size)
}

/// Clear the entire plugin cache.
pub async fn clear_cache(cache_dir: &Path) -> Result<()> {
    if cache_dir.exists() {
        fs::remove_dir_all(cache_dir).await?;
        debug!("Cleared plugin cache at {}", cache_dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_list_plugin_versions() {
        let cache_dir = tempdir().unwrap();
        let plugin_dir = cache_dir.path().join("mp").join("plugin");

        std::fs::create_dir_all(plugin_dir.join("1.0.0")).unwrap();
        std::fs::create_dir_all(plugin_dir.join("1.1.0")).unwrap();
        std::fs::create_dir_all(plugin_dir.join("2.0.0")).unwrap();

        let versions = list_plugin_versions(cache_dir.path(), "mp", "plugin")
            .await
            .unwrap();

        assert_eq!(versions.len(), 3);
    }

    #[tokio::test]
    async fn test_cleanup_old_versions() {
        let cache_dir = tempdir().unwrap();
        let plugin_dir = cache_dir.path().join("mp").join("plugin");

        // Create versions with different timestamps
        for v in &["1.0.0", "1.1.0", "2.0.0", "3.0.0"] {
            std::fs::create_dir_all(plugin_dir.join(v)).unwrap();
            std::fs::write(plugin_dir.join(v).join("marker"), v).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let removed = cleanup_old_versions(cache_dir.path(), "mp", "plugin", 2)
            .await
            .unwrap();

        assert_eq!(removed, 2);

        let remaining = list_plugin_versions(cache_dir.path(), "mp", "plugin")
            .await
            .unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn test_get_cache_size() {
        let cache_dir = tempdir().unwrap();
        let plugin_dir = cache_dir.path().join("mp").join("plugin").join("1.0.0");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("file.txt"), "hello world").unwrap();

        let size = get_cache_size(cache_dir.path()).await.unwrap();
        assert!(size > 0);
    }
}
