//! Global hook settings.

use serde::Deserialize;
use serde::Serialize;

/// Global settings that control hook behavior.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookSettings {
    /// Disable all hooks globally.
    #[serde(default)]
    pub disable_all_hooks: bool,

    /// Only allow hooks from managed (policy/plugin) sources.
    #[serde(default)]
    pub allow_managed_hooks_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let settings = HookSettings::default();
        assert!(!settings.disable_all_hooks);
        assert!(!settings.allow_managed_hooks_only);
    }

    #[test]
    fn test_serde_defaults() {
        let json = "{}";
        let settings: HookSettings = serde_json::from_str(json).expect("deserialize");
        assert!(!settings.disable_all_hooks);
        assert!(!settings.allow_managed_hooks_only);
    }

    #[test]
    fn test_serde_roundtrip() {
        let settings = HookSettings {
            disable_all_hooks: true,
            allow_managed_hooks_only: true,
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        let parsed: HookSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.disable_all_hooks, settings.disable_all_hooks);
        assert_eq!(
            parsed.allow_managed_hooks_only,
            settings.allow_managed_hooks_only
        );
    }
}
