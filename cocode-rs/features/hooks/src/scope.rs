//! Hook scope for priority ordering.
//!
//! Hooks are executed in scope priority order when multiple hooks match the
//! same event.

use serde::{Deserialize, Serialize};

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
}
