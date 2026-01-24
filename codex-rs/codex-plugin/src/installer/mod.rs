//! Plugin installation management.

mod cache;
mod sources;

pub use cache::*;
pub use sources::*;

use crate::error::PluginError;
use crate::error::Result;
use crate::manifest::load_manifest_from_dir;
use crate::manifest::validate_plugin_name;
use crate::registry::InstallEntryV2;
use crate::registry::InstallScope;
use crate::registry::PluginRegistryV2;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::debug;
use tracing::info;

/// Plugin installer.
pub struct PluginInstaller {
    registry: Arc<PluginRegistryV2>,
    cache_dir: PathBuf,
    /// Codex home directory (for NPM cache etc.)
    codex_home: PathBuf,
}

impl PluginInstaller {
    /// Create a new plugin installer.
    ///
    /// # Arguments
    /// * `registry` - Plugin registry
    /// * `cache_dir` - Cache directory for installed plugins
    /// * `codex_home` - Codex home directory (respects `CODEX_HOME` env var)
    pub fn new(
        registry: Arc<PluginRegistryV2>,
        cache_dir: impl Into<PathBuf>,
        codex_home: impl Into<PathBuf>,
    ) -> Self {
        Self {
            registry,
            cache_dir: cache_dir.into(),
            codex_home: codex_home.into(),
        }
    }

    /// Install a plugin from a source.
    pub async fn install(
        &self,
        source: &PluginSource,
        marketplace: &str,
        scope: InstallScope,
        project_path: Option<&Path>,
    ) -> Result<InstallEntryV2> {
        // Validate scope requirements
        if scope.requires_project_path() && project_path.is_none() {
            return Err(PluginError::ProjectPathRequired(scope.to_string()));
        }

        validate_plugin_name(marketplace)?;

        // Fetch plugin to temporary location
        let fetch_result = fetch_plugin_source(&self.codex_home, source).await?;

        // Load and validate manifest
        let manifest = load_manifest_from_dir(&fetch_result.path).await?;
        let plugin_name = manifest.name.clone();

        // Determine version: manifest > fetch result > "unknown"
        let version = manifest
            .version
            .clone()
            .or(fetch_result.version.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Calculate install path
        let install_path =
            self.get_install_path(marketplace, &plugin_name, &version, scope, project_path);

        // Create install directory and copy files
        if let Some(parent) = install_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Remove existing installation at this path
        if install_path.exists() {
            fs::remove_dir_all(&install_path).await?;
        }

        // Move temp directory to install location
        copy_dir_recursive(&fetch_result.path, &install_path).await?;

        // Clean up temp directory
        let _ = fs::remove_dir_all(&fetch_result.path).await;

        // Create registry entry
        let mut entry = InstallEntryV2::new(scope, install_path.to_string_lossy().to_string())
            .with_version(&version)
            .with_source(source_to_string(source));

        if let Some(pp) = project_path {
            entry = entry.with_project_path(pp.to_string_lossy().to_string());
        }

        // Add git SHA if available
        if let Some(sha) = fetch_result.git_sha {
            entry = entry.with_git_sha(sha);
        }

        // Register in registry
        let plugin_id = format!("{plugin_name}@{marketplace}");
        self.registry.upsert(&plugin_id, entry.clone()).await?;
        self.registry.save().await?;

        info!("Installed plugin {} at scope {}", plugin_id, scope);

        Ok(entry)
    }

    /// Uninstall a plugin at a specific scope.
    pub async fn uninstall(
        &self,
        plugin_id: &str,
        scope: InstallScope,
        project_path: Option<&Path>,
    ) -> Result<()> {
        // Remove from registry
        let entry = self
            .registry
            .remove(plugin_id, scope, project_path)
            .await
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        // Remove files
        let install_path = PathBuf::from(&entry.install_path);
        if install_path.exists() {
            fs::remove_dir_all(&install_path).await?;
            debug!("Removed plugin files at {}", install_path.display());
        }

        self.registry.save().await?;

        info!("Uninstalled plugin {} from scope {}", plugin_id, scope);

        Ok(())
    }

    /// Update a plugin to the latest version.
    ///
    /// Requires that the plugin was installed with source tracking enabled.
    pub async fn update(
        &self,
        plugin_id: &str,
        scope: InstallScope,
        project_path: Option<&Path>,
    ) -> Result<InstallEntryV2> {
        // Get current installation
        let current = self
            .registry
            .get_at_scope(plugin_id, scope, project_path)
            .await
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        // Check if we have source information
        let source_str = current.source.as_ref().ok_or_else(|| {
            PluginError::Update(format!(
                "Cannot update {plugin_id}: no source information stored. Reinstall with source tracking."
            ))
        })?;

        // Parse source string back to PluginSource
        let source = PluginSource::parse(source_str)?;

        // Extract marketplace from plugin_id
        let marketplace = plugin_id.split('@').nth(1).unwrap_or("unknown");

        info!("Updating plugin {} from {}", plugin_id, source_str);

        // Re-fetch from source (this will replace the existing installation)
        let entry = self
            .install(&source, marketplace, scope, project_path)
            .await?;

        // Update the last_updated timestamp
        if let Some(mut updated) = self
            .registry
            .get_at_scope(plugin_id, scope, project_path)
            .await
        {
            updated.touch();
            self.registry.upsert(plugin_id, updated.clone()).await?;
            self.registry.save().await?;
        }

        info!(
            "Updated plugin {} to version {:?}",
            plugin_id, entry.version
        );

        Ok(entry)
    }

    /// Validate a plugin directory without installing.
    pub async fn validate(&self, path: &Path) -> Result<crate::manifest::PluginManifest> {
        load_manifest_from_dir(path).await
    }

    /// Get the install path for a plugin.
    fn get_install_path(
        &self,
        marketplace: &str,
        plugin_name: &str,
        version: &str,
        scope: InstallScope,
        project_path: Option<&Path>,
    ) -> PathBuf {
        match scope {
            InstallScope::Managed | InstallScope::User => self
                .cache_dir
                .join(marketplace)
                .join(plugin_name)
                .join(version),
            InstallScope::Project | InstallScope::Local => project_path
                .expect("Project path required")
                .join(".codex")
                .join("plugins")
                .join(marketplace)
                .join(plugin_name),
        }
    }
}

/// Convert a PluginSource to a string for storage.
fn source_to_string(source: &PluginSource) -> String {
    match source {
        PluginSource::GitHub { repo, ref_spec } => match ref_spec {
            Some(r) => format!("github:{repo}@{r}"),
            None => format!("github:{repo}"),
        },
        PluginSource::Git { url, ref_spec } => match ref_spec {
            Some(r) => format!("{url}@{r}"),
            None => url.clone(),
        },
        PluginSource::Local { path } => {
            format!("local:{}", path.display())
        }
        PluginSource::Npm {
            package,
            version,
            registry,
        } => {
            let mut s = format!("npm:{package}");
            if let Some(v) = version {
                s.push('@');
                s.push_str(v);
            }
            if let Some(r) = registry {
                s.push_str(&format!("?registry={r}"));
            }
            s
        }
        PluginSource::Pip {
            package,
            version,
            index_url,
        } => {
            let mut s = format!("pip:{package}");
            if let Some(v) = version {
                s.push_str("==");
                s.push_str(v);
            }
            if let Some(idx) = index_url {
                s.push_str(&format!("?index_url={idx}"));
            }
            s
        }
    }
}

/// Recursively copy a directory.
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).await?;

    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            // Skip .git directories
            if entry.file_name() == ".git" {
                continue;
            }
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else {
            fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PLUGIN_MANIFEST_DIR;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_install_local() {
        let codex_home = tempdir().unwrap();
        let source_dir = tempdir().unwrap();

        // Create plugin source
        let manifest_dir = source_dir.path().join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0"}"#,
        )
        .unwrap();

        let registry = Arc::new(PluginRegistryV2::new(codex_home.path()));
        let installer = PluginInstaller::new(
            Arc::clone(&registry),
            codex_home.path().join("plugins").join("cache"),
            codex_home.path(),
        );

        let source = PluginSource::Local {
            path: source_dir.path().to_path_buf(),
        };

        let entry = installer
            .install(&source, "test-mp", InstallScope::User, None)
            .await
            .unwrap();

        assert_eq!(entry.scope, InstallScope::User);
        assert_eq!(entry.version, Some("1.0.0".to_string()));
        assert!(registry.is_installed("test-plugin@test-mp").await);
    }

    #[tokio::test]
    async fn test_uninstall() {
        let codex_home = tempdir().unwrap();
        let source_dir = tempdir().unwrap();

        // Create and install plugin
        let manifest_dir = source_dir.path().join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0"}"#,
        )
        .unwrap();

        let registry = Arc::new(PluginRegistryV2::new(codex_home.path()));
        let installer = PluginInstaller::new(
            Arc::clone(&registry),
            codex_home.path().join("plugins").join("cache"),
            codex_home.path(),
        );

        let source = PluginSource::Local {
            path: source_dir.path().to_path_buf(),
        };

        installer
            .install(&source, "test-mp", InstallScope::User, None)
            .await
            .unwrap();

        // Uninstall
        installer
            .uninstall("test-plugin@test-mp", InstallScope::User, None)
            .await
            .unwrap();

        assert!(!registry.is_installed("test-plugin@test-mp").await);
    }

    #[test]
    fn test_source_to_string() {
        // GitHub
        let source = PluginSource::GitHub {
            repo: "owner/repo".to_string(),
            ref_spec: None,
        };
        assert_eq!(source_to_string(&source), "github:owner/repo");

        let source = PluginSource::GitHub {
            repo: "owner/repo".to_string(),
            ref_spec: Some("v1.0.0".to_string()),
        };
        assert_eq!(source_to_string(&source), "github:owner/repo@v1.0.0");

        // NPM
        let source = PluginSource::Npm {
            package: "@scope/pkg".to_string(),
            version: Some("1.0.0".to_string()),
            registry: None,
        };
        assert_eq!(source_to_string(&source), "npm:@scope/pkg@1.0.0");
    }
}
