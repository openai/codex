//! V2 installation entry type.

use super::scope::InstallScope;
use serde::Deserialize;
use serde::Serialize;

/// V2 installation entry for a plugin.
///
/// Each plugin can have multiple entries at different scopes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallEntryV2 {
    /// Installation scope.
    pub scope: InstallScope,

    /// Project path (required for project/local scopes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,

    /// Absolute path to the versioned plugin directory.
    pub install_path: String,

    /// Installed version (semver or git SHA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// ISO 8601 timestamp of installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<String>,

    /// ISO 8601 timestamp of last update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,

    /// Git commit SHA for git-based plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_commit_sha: Option<String>,

    /// True if plugin is in marketplace directory (local development).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_local: Option<bool>,

    /// Original source for updates (github:owner/repo, npm:package, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl InstallEntryV2 {
    /// Create a new installation entry.
    pub fn new(scope: InstallScope, install_path: String) -> Self {
        Self {
            scope,
            project_path: None,
            install_path,
            version: None,
            installed_at: Some(chrono::Utc::now().to_rfc3339()),
            last_updated: None,
            git_commit_sha: None,
            is_local: None,
            source: None,
        }
    }

    /// Create a new entry with project path.
    pub fn with_project_path(mut self, path: impl Into<String>) -> Self {
        self.project_path = Some(path.into());
        self
    }

    /// Set the version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the git commit SHA.
    pub fn with_git_sha(mut self, sha: impl Into<String>) -> Self {
        self.git_commit_sha = Some(sha.into());
        self
    }

    /// Mark as local development.
    pub fn with_is_local(mut self, is_local: bool) -> Self {
        self.is_local = Some(is_local);
        self
    }

    /// Set the original source for updates.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Update the last_updated timestamp to now.
    pub fn touch(&mut self) {
        self.last_updated = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Check if this entry matches the given scope and optional project path.
    pub fn matches(&self, scope: InstallScope, project_path: Option<&str>) -> bool {
        if self.scope != scope {
            return false;
        }

        match (
            scope.requires_project_path(),
            project_path,
            &self.project_path,
        ) {
            // Scope requires project path - both must match
            (true, Some(query_path), Some(entry_path)) => query_path == entry_path,
            // Scope requires project path but none provided or stored
            (true, _, _) => false,
            // Scope doesn't require project path
            (false, _, _) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_matches_user_scope() {
        let entry = InstallEntryV2::new(InstallScope::User, "/path/to/plugin".to_string());

        assert!(entry.matches(InstallScope::User, None));
        assert!(entry.matches(InstallScope::User, Some("/any/path")));
        assert!(!entry.matches(InstallScope::Project, None));
    }

    #[test]
    fn test_entry_matches_project_scope() {
        let entry = InstallEntryV2::new(InstallScope::Project, "/path/to/plugin".to_string())
            .with_project_path("/my/project");

        assert!(entry.matches(InstallScope::Project, Some("/my/project")));
        assert!(!entry.matches(InstallScope::Project, Some("/other/project")));
        assert!(!entry.matches(InstallScope::Project, None));
        assert!(!entry.matches(InstallScope::User, None));
    }

    #[test]
    fn test_entry_serialization() {
        let entry = InstallEntryV2::new(InstallScope::User, "/path/to/plugin".to_string())
            .with_version("1.0.0")
            .with_git_sha("abc123");

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"scope\":\"user\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
        assert!(json.contains("\"gitCommitSha\":\"abc123\""));

        let parsed: InstallEntryV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.scope, InstallScope::User);
        assert_eq!(parsed.version, Some("1.0.0".to_string()));
    }
}
