//! Plugin source handling.

use crate::error::PluginError;
use crate::error::Result;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;
use tracing::debug;
use tracing::info;

/// Plugin source for installation.
#[derive(Debug, Clone)]
pub enum PluginSource {
    /// GitHub repository (owner/repo format).
    GitHub {
        repo: String,
        ref_spec: Option<String>,
    },
    /// Git URL (must end with .git).
    Git {
        url: String,
        ref_spec: Option<String>,
    },
    /// Local filesystem path.
    Local { path: PathBuf },
    /// NPM package.
    Npm {
        package: String,
        version: Option<String>,
        registry: Option<String>,
    },
    /// Python pip package.
    Pip {
        package: String,
        version: Option<String>,
        index_url: Option<String>,
    },
}

impl PluginSource {
    /// Parse a source string.
    ///
    /// Supports:
    /// - `./path/to/plugin` - Local path
    /// - `local:/path/to/plugin` - Local path (explicit)
    /// - `owner/repo` - GitHub shorthand
    /// - `github:owner/repo` - GitHub explicit
    /// - `github:owner/repo@ref` - GitHub with ref
    /// - `https://github.com/owner/repo.git` - Full Git URL
    /// - `npm:@scope/package` or `npm:package` - NPM package
    /// - `npm:package@version` - NPM package with version
    /// - `pip:package` or `pip:package==version` - Python pip package
    pub fn parse(s: &str) -> Result<Self> {
        // Pip package
        if let Some(pkg) = s.strip_prefix("pip:") {
            let (package, version) = if let Some((p, v)) = pkg.split_once("==") {
                (p.to_string(), Some(v.to_string()))
            } else if let Some((p, v)) = pkg.split_once(">=") {
                // Support pip's version specifiers (use as minimum version)
                (p.to_string(), Some(format!(">={v}")))
            } else {
                (pkg.to_string(), None)
            };
            return Ok(Self::Pip {
                package,
                version,
                index_url: None,
            });
        }

        // GitHub explicit (github:owner/repo or github:owner/repo@ref)
        if let Some(gh) = s.strip_prefix("github:") {
            let (repo, ref_spec) = if let Some((r, v)) = gh.rsplit_once('@') {
                (r.to_string(), Some(v.to_string()))
            } else {
                (gh.to_string(), None)
            };
            return Ok(Self::GitHub { repo, ref_spec });
        }

        // Local path explicit (local:/path/to/plugin)
        if let Some(path) = s.strip_prefix("local:") {
            return Ok(Self::Local {
                path: PathBuf::from(path),
            });
        }

        // NPM package
        if let Some(pkg) = s.strip_prefix("npm:") {
            let (package, version) = if pkg.starts_with('@') {
                // Scoped package: @scope/package or @scope/package@version
                // Find the first '/' to identify the scope separator
                if let Some(slash_pos) = pkg.find('/') {
                    // Look for '@' after the scope
                    let after_scope = &pkg[slash_pos + 1..];
                    if let Some(at_pos) = after_scope.find('@') {
                        // @scope/package@version
                        let version_start = slash_pos + 1 + at_pos;
                        (
                            pkg[..version_start].to_string(),
                            Some(pkg[version_start + 1..].to_string()),
                        )
                    } else {
                        // @scope/package (no version)
                        (pkg.to_string(), None)
                    }
                } else {
                    // Invalid scoped package (no slash), treat as whole package
                    (pkg.to_string(), None)
                }
            } else if let Some((p, v)) = pkg.rsplit_once('@') {
                // Non-scoped package with version: package@version
                (p.to_string(), Some(v.to_string()))
            } else {
                // Non-scoped package without version
                (pkg.to_string(), None)
            };
            return Ok(Self::Npm {
                package,
                version,
                registry: None,
            });
        }

        // Local path
        if s.starts_with("./") || s.starts_with('/') || s.starts_with("..") {
            return Ok(Self::Local {
                path: PathBuf::from(s),
            });
        }

        // Git URL
        if s.ends_with(".git") {
            return Ok(Self::Git {
                url: s.to_string(),
                ref_spec: None,
            });
        }

        // GitHub shorthand (owner/repo)
        if s.contains('/') && !s.contains("://") {
            return Ok(Self::GitHub {
                repo: s.to_string(),
                ref_spec: None,
            });
        }

        Err(PluginError::Source(format!(
            "Unable to parse source: {s}. Use ./path, owner/repo, https://...git, or npm:package"
        )))
    }

    /// Create a GitHub source.
    pub fn github(repo: impl Into<String>) -> Self {
        Self::GitHub {
            repo: repo.into(),
            ref_spec: None,
        }
    }

    /// Create a Git source.
    pub fn git(url: impl Into<String>) -> Self {
        Self::Git {
            url: url.into(),
            ref_spec: None,
        }
    }

    /// Create a local source.
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::Local { path: path.into() }
    }

    /// Create an NPM source.
    pub fn npm(package: impl Into<String>) -> Self {
        Self::Npm {
            package: package.into(),
            version: None,
            registry: None,
        }
    }

    /// Create a pip source.
    pub fn pip(package: impl Into<String>) -> Self {
        Self::Pip {
            package: package.into(),
            version: None,
            index_url: None,
        }
    }

    /// Set the ref spec (branch/tag/commit) for Git sources.
    pub fn with_ref(mut self, ref_spec: impl Into<String>) -> Self {
        match &mut self {
            Self::GitHub { ref_spec: r, .. } | Self::Git { ref_spec: r, .. } => {
                *r = Some(ref_spec.into());
            }
            Self::Local { .. } | Self::Npm { .. } | Self::Pip { .. } => {}
        }
        self
    }

    /// Set the version for NPM or pip sources.
    pub fn with_version(mut self, ver: impl Into<String>) -> Self {
        match &mut self {
            Self::Npm { version, .. } | Self::Pip { version, .. } => {
                *version = Some(ver.into());
            }
            Self::GitHub { .. } | Self::Git { .. } | Self::Local { .. } => {}
        }
        self
    }
}

/// Fetch a plugin from a source to a temporary directory.
///
/// Returns (temp_dir, git_sha) where git_sha is Some for git-based sources.
///
/// # Arguments
/// * `codex_home` - Codex home directory (respects `CODEX_HOME` env var)
/// * `source` - Plugin source to fetch from
pub async fn fetch_plugin_source(codex_home: &Path, source: &PluginSource) -> Result<FetchResult> {
    match source {
        PluginSource::Local { path } => fetch_local(path).await,
        PluginSource::GitHub { repo, ref_spec } => fetch_github(repo, ref_spec.as_deref()).await,
        PluginSource::Git { url, ref_spec } => fetch_git(url, ref_spec.as_deref()).await,
        PluginSource::Npm {
            package,
            version,
            registry,
        } => fetch_npm(codex_home, package, version.as_deref(), registry.as_deref()).await,
        PluginSource::Pip {
            package,
            version,
            index_url,
        } => fetch_pip(package, version.as_deref(), index_url.as_deref()).await,
    }
}

/// Result of fetching a plugin source.
#[derive(Debug)]
pub struct FetchResult {
    /// Temporary directory containing the plugin files.
    pub path: PathBuf,
    /// Git commit SHA (for git-based sources).
    pub git_sha: Option<String>,
    /// Version (from package.json for npm, or ref for git).
    pub version: Option<String>,
}

/// Fetch from local path.
async fn fetch_local(path: &PathBuf) -> Result<FetchResult> {
    if !path.exists() {
        return Err(PluginError::Source(format!(
            "Local path does not exist: {}",
            path.display()
        )));
    }

    // Create temp directory and copy
    let temp_dir = std::env::temp_dir().join(format!("codex-plugin-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).await?;

    copy_dir_all(path, &temp_dir).await?;

    debug!(
        "Copied local plugin from {} to {}",
        path.display(),
        temp_dir.display()
    );

    Ok(FetchResult {
        path: temp_dir,
        git_sha: None,
        version: None,
    })
}

/// Fetch from GitHub.
async fn fetch_github(repo: &str, ref_spec: Option<&str>) -> Result<FetchResult> {
    let url = format!("https://github.com/{repo}.git");
    fetch_git(&url, ref_spec).await
}

/// Fetch from Git URL.
async fn fetch_git(url: &str, ref_spec: Option<&str>) -> Result<FetchResult> {
    let temp_dir = std::env::temp_dir().join(format!("codex-plugin-{}", uuid::Uuid::new_v4()));

    let mut cmd = Command::new("git");
    cmd.arg("clone")
        .arg("--depth")
        .arg("1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    if let Some(r) = ref_spec {
        cmd.arg("--branch").arg(r);
    }

    cmd.arg(url).arg(&temp_dir);

    info!("Cloning {} to {}", url, temp_dir.display());

    let output = cmd
        .output()
        .await
        .map_err(|e| PluginError::Git(format!("Failed to execute git clone: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PluginError::Git(format!("Git clone failed: {stderr}")));
    }

    // Extract git commit SHA before removing .git
    let git_sha = extract_git_sha(&temp_dir).await;

    // Remove .git directory
    let git_dir = temp_dir.join(".git");
    if git_dir.exists() {
        let _ = fs::remove_dir_all(&git_dir).await;
    }

    debug!(
        "Cloned {} to {} (sha: {:?})",
        url,
        temp_dir.display(),
        git_sha
    );

    Ok(FetchResult {
        path: temp_dir,
        git_sha,
        version: ref_spec.map(String::from),
    })
}

/// Extract the git commit SHA from a cloned repository.
async fn extract_git_sha(repo_path: &PathBuf) -> Option<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !sha.is_empty() {
            return Some(sha);
        }
    }

    None
}

/// Fetch from NPM.
async fn fetch_npm(
    codex_home: &Path,
    package: &str,
    version: Option<&str>,
    registry: Option<&str>,
) -> Result<FetchResult> {
    let npm_cache_dir = codex_home.join("plugins").join("npm-cache");
    fs::create_dir_all(&npm_cache_dir).await?;

    // Build package spec
    let package_spec = match version {
        Some(v) => format!("{package}@{v}"),
        None => package.to_string(),
    };

    info!("Installing NPM package: {}", package_spec);

    let mut cmd = Command::new("npm");
    cmd.arg("install")
        .arg(&package_spec)
        .arg("--prefix")
        .arg(&npm_cache_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(reg) = registry {
        cmd.arg("--registry").arg(reg);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| PluginError::Npm(format!("Failed to execute npm install: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PluginError::Npm(format!("npm install failed: {stderr}")));
    }

    // Find the installed package in node_modules
    let _package_name = package.split('/').last().unwrap_or(package);
    let package_path = npm_cache_dir.join("node_modules").join(package);

    if !package_path.exists() {
        return Err(PluginError::Npm(format!(
            "Package not found after install: {}",
            package_path.display()
        )));
    }

    // Copy to temp directory (don't use node_modules directly)
    let temp_dir = std::env::temp_dir().join(format!("codex-plugin-{}", uuid::Uuid::new_v4()));
    copy_dir_all(&package_path, &temp_dir).await?;

    // Try to extract version from package.json
    let pkg_version = extract_npm_version(&temp_dir).await;

    debug!(
        "Installed npm package {} to {} (version: {:?})",
        package_spec,
        temp_dir.display(),
        pkg_version
    );

    Ok(FetchResult {
        path: temp_dir,
        git_sha: None,
        version: pkg_version.or_else(|| version.map(String::from)),
    })
}

/// Extract version from package.json.
async fn extract_npm_version(package_path: &PathBuf) -> Option<String> {
    let pkg_json = package_path.join("package.json");
    if let Ok(content) = fs::read_to_string(&pkg_json).await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            return json
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from);
        }
    }
    None
}

/// Fetch from pip (Python package).
async fn fetch_pip(
    package: &str,
    version: Option<&str>,
    index_url: Option<&str>,
) -> Result<FetchResult> {
    let temp_dir = std::env::temp_dir().join(format!("codex-plugin-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).await?;

    // Build package spec
    let package_spec = match version {
        Some(v) if v.starts_with(">=") || v.starts_with("<=") || v.starts_with("~=") => {
            format!("{package}{v}")
        }
        Some(v) => format!("{package}=={v}"),
        None => package.to_string(),
    };

    info!("Installing pip package: {}", package_spec);

    let mut cmd = Command::new("pip");
    cmd.arg("install")
        .arg("--target")
        .arg(&temp_dir)
        .arg("--no-deps") // Don't install dependencies
        .arg("--ignore-installed") // Ignore already installed packages
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(idx) = index_url {
        cmd.arg("--index-url").arg(idx);
    }

    cmd.arg(&package_spec);

    let output = cmd
        .output()
        .await
        .map_err(|e| PluginError::Pip(format!("Failed to execute pip install: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PluginError::Pip(format!("pip install failed: {stderr}")));
    }

    // Try to extract version from installed package metadata
    let pkg_version = extract_pip_version(&temp_dir, package).await;

    debug!(
        "Installed pip package {} to {} (version: {:?})",
        package_spec,
        temp_dir.display(),
        pkg_version
    );

    Ok(FetchResult {
        path: temp_dir,
        git_sha: None,
        version: pkg_version.or_else(|| version.map(|v| v.trim_start_matches("==").to_string())),
    })
}

/// Extract version from pip package metadata.
async fn extract_pip_version(install_path: &PathBuf, package: &str) -> Option<String> {
    // pip installs metadata in <package>-<version>.dist-info/METADATA
    // Try to find the dist-info directory
    let package_normalized = package.replace('-', "_").to_lowercase();

    let mut entries = fs::read_dir(install_path).await.ok()?;
    while let Some(entry) = entries.next_entry().await.ok()? {
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_lowercase();

        if name_str.starts_with(&package_normalized) && name_str.ends_with(".dist-info") {
            // Extract version from directory name: package-version.dist-info
            let without_suffix = name_str.trim_end_matches(".dist-info");
            if let Some(version_start) = without_suffix.rfind('-') {
                return Some(without_suffix[version_start + 1..].to_string());
            }
        }
    }

    None
}

/// Recursively copy directory contents.
async fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    fs::create_dir_all(dst).await?;

    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            // Skip .git and node_modules
            let name = entry.file_name();
            if name == ".git" || name == "node_modules" {
                continue;
            }
            Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
        } else {
            fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_local_source() {
        let source = PluginSource::parse("./path/to/plugin").unwrap();
        assert!(matches!(source, PluginSource::Local { .. }));

        let source = PluginSource::parse("../plugin").unwrap();
        assert!(matches!(source, PluginSource::Local { .. }));

        let source = PluginSource::parse("/absolute/path").unwrap();
        assert!(matches!(source, PluginSource::Local { .. }));
    }

    #[test]
    fn test_parse_github_source() {
        let source = PluginSource::parse("owner/repo").unwrap();
        if let PluginSource::GitHub { repo, .. } = source {
            assert_eq!(repo, "owner/repo");
        } else {
            panic!("Expected GitHub source");
        }
    }

    #[test]
    fn test_parse_git_source() {
        let source = PluginSource::parse("https://github.com/owner/repo.git").unwrap();
        if let PluginSource::Git { url, .. } = source {
            assert_eq!(url, "https://github.com/owner/repo.git");
        } else {
            panic!("Expected Git source");
        }
    }

    #[test]
    fn test_parse_npm_source() {
        // Simple package
        let source = PluginSource::parse("npm:my-plugin").unwrap();
        if let PluginSource::Npm {
            package, version, ..
        } = source
        {
            assert_eq!(package, "my-plugin");
            assert!(version.is_none());
        } else {
            panic!("Expected Npm source");
        }

        // Package with version
        let source = PluginSource::parse("npm:my-plugin@1.0.0").unwrap();
        if let PluginSource::Npm {
            package, version, ..
        } = source
        {
            assert_eq!(package, "my-plugin");
            assert_eq!(version, Some("1.0.0".to_string()));
        } else {
            panic!("Expected Npm source");
        }

        // Scoped package
        let source = PluginSource::parse("npm:@scope/my-plugin").unwrap();
        if let PluginSource::Npm {
            package, version, ..
        } = source
        {
            assert_eq!(package, "@scope/my-plugin");
            assert!(version.is_none());
        } else {
            panic!("Expected Npm source");
        }

        // Scoped package with version
        let source = PluginSource::parse("npm:@scope/my-plugin@2.0.0").unwrap();
        if let PluginSource::Npm {
            package, version, ..
        } = source
        {
            assert_eq!(package, "@scope/my-plugin");
            assert_eq!(version, Some("2.0.0".to_string()));
        } else {
            panic!("Expected Npm source");
        }
    }

    #[tokio::test]
    async fn test_fetch_local() {
        let temp_source = tempfile::tempdir().unwrap();
        std::fs::write(temp_source.path().join("test.txt"), "hello").unwrap();

        let result = fetch_local(&temp_source.path().to_path_buf())
            .await
            .unwrap();
        assert!(result.path.join("test.txt").exists());
        assert!(result.git_sha.is_none());

        // Cleanup
        let _ = std::fs::remove_dir_all(&result.path);
    }
}
