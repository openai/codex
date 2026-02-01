//! Hook scope for priority ordering.
//!
//! Hooks are executed in scope priority order when multiple hooks match the
//! same event.

use serde::Deserialize;
use serde::Serialize;

/// The scope from which a hook originates, which determines its priority.
///
/// Lower numeric order = higher priority. `Policy` hooks always run first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookScope {
    /// Organization-level policy hooks (highest priority).
    Policy = 0,
    /// Plugin-provided hooks.
    Plugin = 1,
    /// Session-level hooks.
    Session = 2,
    /// Skill-level hooks (lowest priority).
    Skill = 3,
}

/// The source of a hook, providing more detail than scope alone.
///
/// This identifies where a hook was registered from, enabling:
/// - Policy enforcement (only managed hooks)
/// - Cleanup when plugins/skills are unloaded
/// - Debugging and logging
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookSource {
    /// Registered by organization policy.
    Policy,

    /// Registered by a plugin.
    Plugin {
        /// The name of the plugin.
        name: String,
    },

    /// Registered for the current session.
    Session,

    /// Registered by a skill.
    Skill {
        /// The name of the skill.
        name: String,
    },
}

impl HookSource {
    /// Returns the scope for this source.
    pub fn scope(&self) -> HookScope {
        match self {
            Self::Policy => HookScope::Policy,
            Self::Plugin { .. } => HookScope::Plugin,
            Self::Session => HookScope::Session,
            Self::Skill { .. } => HookScope::Skill,
        }
    }

    /// Returns `true` if this source is a managed source (Policy or Plugin).
    ///
    /// Managed sources are allowed when `allow_managed_hooks_only` is enabled.
    pub fn is_managed(&self) -> bool {
        matches!(self, Self::Policy | Self::Plugin { .. })
    }

    /// Returns the name associated with this source, if any.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Policy | Self::Session => None,
            Self::Plugin { name } | Self::Skill { name } => Some(name),
        }
    }
}

impl Default for HookSource {
    fn default() -> Self {
        Self::Session
    }
}

impl std::fmt::Display for HookSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Policy => write!(f, "policy"),
            Self::Plugin { name } => write!(f, "plugin:{name}"),
            Self::Session => write!(f, "session"),
            Self::Skill { name } => write!(f, "skill:{name}"),
        }
    }
}

impl HookScope {
    /// Returns the string representation of this scope.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Plugin => "plugin",
            Self::Session => "session",
            Self::Skill => "skill",
        }
    }
}

impl std::fmt::Display for HookScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Ord for HookScope {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as i32).cmp(&(*other as i32))
    }
}

impl PartialOrd for HookScope {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_order() {
        assert!(HookScope::Policy < HookScope::Plugin);
        assert!(HookScope::Plugin < HookScope::Session);
        assert!(HookScope::Session < HookScope::Skill);
    }

    #[test]
    fn test_sorting() {
        let mut scopes = vec![
            HookScope::Skill,
            HookScope::Policy,
            HookScope::Session,
            HookScope::Plugin,
        ];
        scopes.sort();
        assert_eq!(
            scopes,
            vec![
                HookScope::Policy,
                HookScope::Plugin,
                HookScope::Session,
                HookScope::Skill,
            ]
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", HookScope::Policy), "policy");
        assert_eq!(format!("{}", HookScope::Plugin), "plugin");
        assert_eq!(format!("{}", HookScope::Session), "session");
        assert_eq!(format!("{}", HookScope::Skill), "skill");
    }

    #[test]
    fn test_serde_roundtrip() {
        let scope = HookScope::Session;
        let json = serde_json::to_string(&scope).expect("serialize");
        assert_eq!(json, "\"session\"");
        let parsed: HookScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, scope);
    }

    // HookSource tests

    #[test]
    fn test_hook_source_scope() {
        assert_eq!(HookSource::Policy.scope(), HookScope::Policy);
        assert_eq!(
            HookSource::Plugin {
                name: "test".to_string()
            }
            .scope(),
            HookScope::Plugin
        );
        assert_eq!(HookSource::Session.scope(), HookScope::Session);
        assert_eq!(
            HookSource::Skill {
                name: "test".to_string()
            }
            .scope(),
            HookScope::Skill
        );
    }

    #[test]
    fn test_hook_source_is_managed() {
        assert!(HookSource::Policy.is_managed());
        assert!(
            HookSource::Plugin {
                name: "test".to_string()
            }
            .is_managed()
        );
        assert!(!HookSource::Session.is_managed());
        assert!(
            !HookSource::Skill {
                name: "test".to_string()
            }
            .is_managed()
        );
    }

    #[test]
    fn test_hook_source_name() {
        assert!(HookSource::Policy.name().is_none());
        assert_eq!(
            HookSource::Plugin {
                name: "my-plugin".to_string()
            }
            .name(),
            Some("my-plugin")
        );
        assert!(HookSource::Session.name().is_none());
        assert_eq!(
            HookSource::Skill {
                name: "my-skill".to_string()
            }
            .name(),
            Some("my-skill")
        );
    }

    #[test]
    fn test_hook_source_display() {
        assert_eq!(format!("{}", HookSource::Policy), "policy");
        assert_eq!(
            format!(
                "{}",
                HookSource::Plugin {
                    name: "my-plugin".to_string()
                }
            ),
            "plugin:my-plugin"
        );
        assert_eq!(format!("{}", HookSource::Session), "session");
        assert_eq!(
            format!(
                "{}",
                HookSource::Skill {
                    name: "my-skill".to_string()
                }
            ),
            "skill:my-skill"
        );
    }

    #[test]
    fn test_hook_source_default() {
        assert_eq!(HookSource::default(), HookSource::Session);
    }

    #[test]
    fn test_hook_source_serde_roundtrip() {
        let sources = vec![
            HookSource::Policy,
            HookSource::Plugin {
                name: "test-plugin".to_string(),
            },
            HookSource::Session,
            HookSource::Skill {
                name: "test-skill".to_string(),
            },
        ];

        for source in sources {
            let json = serde_json::to_string(&source).expect("serialize");
            let parsed: HookSource = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, source);
        }
    }

    #[test]
    fn test_hook_source_serde_format() {
        let policy = HookSource::Policy;
        let json = serde_json::to_string(&policy).expect("serialize");
        assert!(json.contains("\"type\":\"policy\""));

        let plugin = HookSource::Plugin {
            name: "test".to_string(),
        };
        let json = serde_json::to_string(&plugin).expect("serialize");
        assert!(json.contains("\"type\":\"plugin\""));
        assert!(json.contains("\"name\":\"test\""));
    }
}
