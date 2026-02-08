//! Centralized feature flags and metadata.
//!
//! This module defines a small set of toggles that gate experimental and
//! optional behavior across the codebase. Instead of wiring individual
//! booleans through multiple types, call sites consult a single `Features`
//! container attached to `Config`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// High-level lifecycle stage for a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Experimental,
    Beta {
        name: &'static str,
        menu_description: &'static str,
        announcement: &'static str,
    },
    Stable,
    Deprecated,
    Removed,
}

impl Stage {
    pub fn beta_menu_name(self) -> Option<&'static str> {
        match self {
            Stage::Beta { name, .. } => Some(name),
            _ => None,
        }
    }

    pub fn beta_menu_description(self) -> Option<&'static str> {
        match self {
            Stage::Beta {
                menu_description, ..
            } => Some(menu_description),
            _ => None,
        }
    }

    pub fn beta_announcement(self) -> Option<&'static str> {
        match self {
            Stage::Beta { announcement, .. } => Some(announcement),
            _ => None,
        }
    }
}

/// Unique features toggled via configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    // Stable.
    /// Create a ghost commit at each turn.
    GhostCommit,

    // Experimental
    /// Enable Windows sandbox (restricted token) on Windows.
    WindowsSandbox,
    /// Use the elevated Windows sandbox pipeline (setup + runner).
    WindowsSandboxElevated,
    /// Append additional AGENTS.md guidance to user instructions.
    HierarchicalAgents,
    /// Enforce UTF8 output in Powershell.
    PowershellUtf8,
    /// Enable collab tools.
    Collab,
    WebFetch,
    /// Enable custom web_search tool (DuckDuckGo/Tavily providers).
    WebSearch,
    /// Enable retrieval tool (experimental, requires retrieval.toml configuration).
    Retrieval,
    /// Enable LSP tool for code intelligence (requires pre-installed LSP servers).
    Lsp,
    /// Enable the LS directory listing tool.
    Ls,
    /// Enable MCP resource tools (list_mcp_resources, list_mcp_resource_templates, read_mcp_resource).
    McpResourceTools,
}

impl Feature {
    pub fn key(self) -> &'static str {
        self.info().key
    }

    pub fn stage(self) -> Stage {
        self.info().stage
    }

    pub fn default_enabled(self) -> bool {
        self.info().default_enabled
    }

    fn info(self) -> &'static FeatureSpec {
        all_features()
            .find(|spec| spec.id == self)
            .unwrap_or_else(|| unreachable!("missing FeatureSpec for {:?}", self))
    }
}

/// Holds the effective set of enabled features.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Features {
    enabled: BTreeSet<Feature>,
}

impl Features {
    /// Starts with built-in defaults.
    pub fn with_defaults() -> Self {
        let mut set = BTreeSet::new();
        for spec in all_features() {
            if spec.default_enabled {
                set.insert(spec.id);
            }
        }
        Self { enabled: set }
    }

    pub fn enabled(&self, f: Feature) -> bool {
        self.enabled.contains(&f)
    }

    pub fn enable(&mut self, f: Feature) -> &mut Self {
        self.enabled.insert(f);
        self
    }

    pub fn disable(&mut self, f: Feature) -> &mut Self {
        self.enabled.remove(&f);
        self
    }

    /// Apply a table of key -> bool toggles (e.g. from TOML).
    pub fn apply_map(&mut self, m: &BTreeMap<String, bool>) {
        for (k, v) in m {
            if let Some(feat) = feature_for_key(k) {
                if *v {
                    self.enable(feat);
                } else {
                    self.disable(feat);
                }
            }
            // Unknown keys are silently ignored - callers can use is_known_feature_key() to validate
        }
    }

    pub fn enabled_features(&self) -> Vec<Feature> {
        self.enabled.iter().copied().collect()
    }
}

/// Returns all feature specifications.
pub fn all_features() -> impl Iterator<Item = &'static FeatureSpec> {
    FEATURES.iter()
}

/// Keys accepted in `[features]` tables.
pub fn feature_for_key(key: &str) -> Option<Feature> {
    for spec in all_features() {
        if spec.key == key {
            return Some(spec.id);
        }
    }
    None
}

/// Returns `true` if the provided string matches a known feature toggle key.
pub fn is_known_feature_key(key: &str) -> bool {
    feature_for_key(key).is_some()
}

/// Single, easy-to-read registry of all feature definitions.
#[derive(Debug, Clone, Copy)]
pub struct FeatureSpec {
    pub id: Feature,
    pub key: &'static str,
    pub stage: Stage,
    pub default_enabled: bool,
}

/// Core feature specifications. Use `all_features()` to include ext features.
const FEATURES: &[FeatureSpec] = &[
    // Stable features.
    FeatureSpec {
        id: Feature::GhostCommit,
        key: "undo",
        stage: Stage::Stable,
        default_enabled: false,
    },
    // Beta program. Rendered in the `/experimental` menu for users.
    FeatureSpec {
        id: Feature::HierarchicalAgents,
        key: "hierarchical_agents",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WindowsSandbox,
        key: "experimental_windows_sandbox",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WindowsSandboxElevated,
        key: "elevated_windows_sandbox",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::PowershellUtf8,
        key: "powershell_utf8",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Collab,
        key: "collab",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebFetch,
        key: "web_fetch",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebSearch,
        key: "web_search",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Retrieval,
        key: "code_search",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Lsp,
        key: "lsp",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Ls,
        key: "ls",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::McpResourceTools,
        key: "mcp_resource_tools",
        stage: Stage::Stable,
        default_enabled: true,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_defaults_includes_default_enabled_features() {
        let features = Features::with_defaults();

        // McpResourceTools is default enabled
        assert!(features.enabled(Feature::McpResourceTools));
        // Ls is default enabled
        assert!(features.enabled(Feature::Ls));
    }

    #[test]
    fn test_with_defaults_excludes_non_default_features() {
        let features = Features::with_defaults();

        // WebFetch is not default enabled
        assert!(!features.enabled(Feature::WebFetch));
        // Collab is not default enabled
        assert!(!features.enabled(Feature::Collab));
        // GhostCommit is not default enabled
        assert!(!features.enabled(Feature::GhostCommit));
    }

    #[test]
    fn test_enable_and_disable() {
        let mut features = Features::default();

        // Enable a feature
        features.enable(Feature::WebFetch);
        assert!(features.enabled(Feature::WebFetch));

        // Disable the feature
        features.disable(Feature::WebFetch);
        assert!(!features.enabled(Feature::WebFetch));
    }

    #[test]
    fn test_apply_map_enables_features() {
        let mut features = Features::default();
        let mut map = BTreeMap::new();
        map.insert("web_fetch".to_string(), true);
        map.insert("collab".to_string(), true);

        features.apply_map(&map);

        assert!(features.enabled(Feature::WebFetch));
        assert!(features.enabled(Feature::Collab));
    }

    #[test]
    fn test_apply_map_disables_features() {
        let mut features = Features::with_defaults();
        let mut map = BTreeMap::new();
        // Disable a default-enabled feature
        map.insert("ls".to_string(), false);

        features.apply_map(&map);

        assert!(!features.enabled(Feature::Ls));
    }

    #[test]
    fn test_apply_map_ignores_unknown_keys() {
        let mut features = Features::with_defaults();
        let original = features.clone();
        let mut map = BTreeMap::new();
        map.insert("unknown_feature_xyz".to_string(), true);

        features.apply_map(&map);

        // Features should remain unchanged
        assert_eq!(features, original);
    }

    #[test]
    fn test_feature_for_key_known_keys() {
        assert_eq!(feature_for_key("web_fetch"), Some(Feature::WebFetch));
        assert_eq!(feature_for_key("collab"), Some(Feature::Collab));
        assert_eq!(feature_for_key("undo"), Some(Feature::GhostCommit));
    }

    #[test]
    fn test_feature_for_key_unknown_keys() {
        assert_eq!(feature_for_key("unknown"), None);
        assert_eq!(feature_for_key(""), None);
        assert_eq!(feature_for_key("WEB_FETCH"), None); // Case sensitive
    }

    #[test]
    fn test_is_known_feature_key() {
        assert!(is_known_feature_key("web_fetch"));
        assert!(!is_known_feature_key("unknown"));
        assert!(!is_known_feature_key(""));
    }

    #[test]
    fn test_feature_key_method() {
        assert_eq!(Feature::WebFetch.key(), "web_fetch");
        assert_eq!(Feature::GhostCommit.key(), "undo");
    }

    #[test]
    fn test_feature_stage_method() {
        assert_eq!(Feature::McpResourceTools.stage(), Stage::Stable);
        assert_eq!(Feature::WebFetch.stage(), Stage::Experimental);
    }

    #[test]
    fn test_feature_default_enabled_method() {
        assert!(Feature::Ls.default_enabled());
        assert!(!Feature::WebFetch.default_enabled());
    }

    #[test]
    fn test_enabled_features_returns_all_enabled() {
        let mut features = Features::default();
        features.enable(Feature::WebFetch);
        features.enable(Feature::Collab);

        let enabled = features.enabled_features();
        assert!(enabled.contains(&Feature::WebFetch));
        assert!(enabled.contains(&Feature::Collab));
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn test_all_features_contains_all_variants() {
        let specs: Vec<_> = all_features().collect();
        // Ensure we have a reasonable number of features
        assert!(specs.len() >= 11);

        // Check that some expected features are present
        assert!(specs.iter().any(|s| s.id == Feature::WebFetch));
        assert!(specs.iter().any(|s| s.id == Feature::Ls));
    }

    #[test]
    fn test_stage_beta_methods() {
        let beta_stage = Stage::Beta {
            name: "Test Feature",
            menu_description: "Test description",
            announcement: "Test announcement",
        };

        assert_eq!(beta_stage.beta_menu_name(), Some("Test Feature"));
        assert_eq!(beta_stage.beta_menu_description(), Some("Test description"));
        assert_eq!(beta_stage.beta_announcement(), Some("Test announcement"));

        // Non-beta stages should return None
        assert_eq!(Stage::Stable.beta_menu_name(), None);
        assert_eq!(Stage::Experimental.beta_menu_description(), None);
    }
}
