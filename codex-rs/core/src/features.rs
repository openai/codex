//! Centralized feature flags and metadata.
//!
//! This module defines a small set of toggles that gate experimental and
//! optional behavior across the codebase. Instead of wiring individual
//! booleans through multiple types, call sites consult a single `Features`
//! container attached to `Config`.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

mod legacy;
pub(crate) use legacy::LegacyFeatureToggles;

/// High-level lifecycle stage for a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Experimental,
    Beta,
    Stable,
    Deprecated,
    Removed,
}

/// Unique features toggled via configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// Use the single unified PTY-backed exec tool.
    UnifiedExec,
    /// Use the streamable exec-command/write-stdin tool pair.
    StreamableShell,
    /// Use the official Rust MCP client (rmcp).
    RmcpClient,
    /// Include the plan tool.
    PlanTool,
    /// Include the freeform apply_patch tool.
    ApplyPatchFreeform,
    /// Include the view_image tool.
    ViewImageTool,
    /// Allow the model to request web searches.
    WebSearchRequest,
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
        FEATURES
            .iter()
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
        for spec in FEATURES {
            if spec.default_enabled {
                set.insert(spec.id);
            }
        }
        Self { enabled: set }
    }

    pub fn enabled(&self, f: Feature) -> bool {
        self.enabled.contains(&f)
    }

    pub fn enable(&mut self, f: Feature) {
        self.enabled.insert(f);
    }

    pub fn disable(&mut self, f: Feature) {
        self.enabled.remove(&f);
    }

    /// Apply a table of key -> bool toggles (e.g. from TOML).
    pub fn apply_map(&mut self, m: &BTreeMap<String, bool>) {
        for (k, v) in m {
            match feature_for_key(k) {
                Some(feat) => {
                    if *v {
                        self.enable(feat);
                    } else {
                        self.disable(feat);
                    }
                }
                None => {
                    tracing::warn!("unknown feature key in config: {k}");
                }
            }
        }
    }
}

/// Keys accepted in `[features]` tables.
fn feature_for_key(key: &str) -> Option<Feature> {
    for spec in FEATURES {
        if spec.key == key {
            return Some(spec.id);
        }
    }
    legacy::feature_for_key(key)
}

/// Deserializable features table for TOML.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct FeaturesToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

/// Single, easy-to-read registry of all feature definitions.
#[derive(Debug, Clone, Copy)]
pub struct FeatureSpec {
    pub id: Feature,
    pub key: &'static str,
    pub stage: Stage,
    pub default_enabled: bool,
}

pub const FEATURES: &[FeatureSpec] = &[
    FeatureSpec {
        id: Feature::UnifiedExec,
        key: "unified_exec",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::StreamableShell,
        key: "streamable_shell",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::RmcpClient,
        key: "rmcp_client",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::PlanTool,
        key: "plan_tool",
        stage: Stage::Stable,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ApplyPatchFreeform,
        key: "apply_patch_freeform",
        stage: Stage::Beta,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ViewImageTool,
        key: "view_image_tool",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::WebSearchRequest,
        key: "web_search_request",
        stage: Stage::Stable,
        default_enabled: false,
    },
];
