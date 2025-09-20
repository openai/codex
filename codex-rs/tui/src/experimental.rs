use codex_core::config::Config;

/// Definition of a single experimental feature toggle.
#[derive(Debug, Clone, Copy)]
pub struct Feature {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default_on: bool,
}

/// Central registry of experimental features.
/// Add new toggles here and gate UI code using `is_enabled(cfg, feature.key)`.
pub const ALL_FEATURES: &[Feature] = &[Feature {
    key: "comment",
    name: "/comment command",
    description: "Enable a /comment command that opens a browser",
    default_on: false,
}];

fn default_for(key: &str) -> bool {
    ALL_FEATURES
        .iter()
        .find(|f| f.key == key)
        .map(|f| f.default_on)
        .unwrap_or(false)
}

/// Returns whether a feature is enabled using the current config and built-in default.
pub fn is_enabled(config: &Config, key: &str) -> bool {
    config
        .experimental_flags
        .get(key)
        .copied()
        .unwrap_or_else(|| default_for(key))
}
