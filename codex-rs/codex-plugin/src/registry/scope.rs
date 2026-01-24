//! Installation scope definitions.

use serde::Deserialize;
use serde::Serialize;

/// Installation scope for plugins.
///
/// Scopes determine where a plugin is installed and its priority
/// when resolving which installation to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    /// Enterprise/policy managed installations.
    /// Lowest priority, cannot be overridden by users in some configurations.
    Managed,

    /// User-level installations in ~/.codex/plugins/.
    /// Default scope for most installations.
    User,

    /// Project-specific installations in .codex/plugins/.
    /// Requires project_path to be set.
    Project,

    /// Local development installations.
    /// Used for plugin development, requires project_path.
    Local,
}

impl InstallScope {
    /// Returns true if this scope requires a project path.
    pub fn requires_project_path(&self) -> bool {
        matches!(self, InstallScope::Project | InstallScope::Local)
    }

    /// Returns the resolution priority (higher = more specific).
    /// Used when resolving which installation to use.
    pub fn priority(&self) -> i32 {
        match self {
            InstallScope::Managed => 0,
            InstallScope::User => 1,
            InstallScope::Project => 2,
            InstallScope::Local => 3,
        }
    }

    /// Returns all scopes in resolution order (highest priority first).
    pub fn resolution_order() -> &'static [InstallScope] {
        &[
            InstallScope::Local,
            InstallScope::Project,
            InstallScope::User,
            InstallScope::Managed,
        ]
    }
}

impl Default for InstallScope {
    fn default() -> Self {
        InstallScope::User
    }
}

impl std::fmt::Display for InstallScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallScope::Managed => write!(f, "managed"),
            InstallScope::User => write!(f, "user"),
            InstallScope::Project => write!(f, "project"),
            InstallScope::Local => write!(f, "local"),
        }
    }
}

impl std::str::FromStr for InstallScope {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "managed" => Ok(InstallScope::Managed),
            "user" => Ok(InstallScope::User),
            "project" => Ok(InstallScope::Project),
            "local" => Ok(InstallScope::Local),
            _ => Err(format!("Invalid scope: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_requires_project_path() {
        assert!(!InstallScope::Managed.requires_project_path());
        assert!(!InstallScope::User.requires_project_path());
        assert!(InstallScope::Project.requires_project_path());
        assert!(InstallScope::Local.requires_project_path());
    }

    #[test]
    fn test_scope_priority() {
        assert!(InstallScope::Local.priority() > InstallScope::Project.priority());
        assert!(InstallScope::Project.priority() > InstallScope::User.priority());
        assert!(InstallScope::User.priority() > InstallScope::Managed.priority());
    }

    #[test]
    fn test_scope_from_str() {
        assert_eq!("user".parse::<InstallScope>().unwrap(), InstallScope::User);
        assert_eq!(
            "PROJECT".parse::<InstallScope>().unwrap(),
            InstallScope::Project
        );
        assert!("invalid".parse::<InstallScope>().is_err());
    }
}
