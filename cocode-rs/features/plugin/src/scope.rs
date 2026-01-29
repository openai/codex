//! Plugin scope definitions.
//!
//! Plugins are discovered from multiple scopes in priority order.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The scope from which a plugin was loaded.
///
/// Scopes are ordered by priority (higher scopes override lower ones):
/// 1. Project - `.cocode/plugins/` in the project directory
/// 2. User - `~/.config/cocode/plugins/`
/// 3. Managed - System-installed plugins
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PluginScope {
    /// System-installed (lowest priority).
    Managed,

    /// User-global plugins.
    User,

    /// Project-local plugins (highest priority).
    Project,
}

impl PluginScope {
    /// Get the default directory for this scope.
    pub fn default_dir(&self) -> Option<PathBuf> {
        match self {
            Self::Managed => {
                // Platform-specific system plugin directory
                #[cfg(target_os = "macos")]
                {
                    Some(PathBuf::from("/usr/local/share/cocode/plugins"))
                }
                #[cfg(target_os = "linux")]
                {
                    Some(PathBuf::from("/usr/share/cocode/plugins"))
                }
                #[cfg(target_os = "windows")]
                {
                    std::env::var("PROGRAMDATA")
                        .ok()
                        .map(|p| PathBuf::from(p).join("cocode").join("plugins"))
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
                {
                    None
                }
            }
            Self::User => dirs::config_dir().map(|p| p.join("cocode").join("plugins")),
            Self::Project => {
                // Project scope depends on the current working directory
                None
            }
        }
    }

    /// Get the priority of this scope (higher = more specific).
    pub fn priority(&self) -> i32 {
        match self {
            Self::Managed => 0,
            Self::User => 1,
            Self::Project => 2,
        }
    }
}

impl std::fmt::Display for PluginScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Managed => write!(f, "managed"),
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_priority() {
        assert!(PluginScope::Project.priority() > PluginScope::User.priority());
        assert!(PluginScope::User.priority() > PluginScope::Managed.priority());
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(PluginScope::Managed.to_string(), "managed");
        assert_eq!(PluginScope::User.to_string(), "user");
        assert_eq!(PluginScope::Project.to_string(), "project");
    }

    #[test]
    fn test_scope_ordering() {
        let mut scopes = vec![
            PluginScope::Project,
            PluginScope::Managed,
            PluginScope::User,
        ];
        scopes.sort();
        assert_eq!(
            scopes,
            vec![
                PluginScope::Managed,
                PluginScope::User,
                PluginScope::Project
            ]
        );
    }
}
