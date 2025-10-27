//! Centralized feature flags and metadata.
//!
//! This module defines a small set of toggles that gate experimental and
//! optional behavior across the codebase. Instead of wiring individual
//! booleans through multiple types, call sites consult a single `Features`
//! container attached to `Config`.

use crate::config::ConfigToml;
use crate::config_profile::ConfigProfile;
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
    /// Enable experimental RMCP features such as OAuth login.
    RmcpClient,
    /// Include the freeform apply_patch tool.
    ApplyPatchFreeform,
    /// Include the view_image tool.
    ViewImageTool,
    /// Allow the model to request web searches.
    WebSearchRequest,
    /// Enable the model-based risk assessments for sandboxed commands.
    SandboxCommandAssessment,
    /// Create a ghost commit at each turn.
    GhostCommit,
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

#[derive(Debug, Clone, Default)]
pub struct FeatureOverrides {
    pub include_apply_patch_tool: Option<bool>,
    pub include_view_image_tool: Option<bool>,
    pub web_search_request: Option<bool>,
    pub experimental_sandbox_command_assessment: Option<bool>,
}

impl FeatureOverrides {
    fn apply(self, features: &mut Features) {
        LegacyFeatureToggles {
            include_apply_patch_tool: self.include_apply_patch_tool,
            include_view_image_tool: self.include_view_image_tool,
            tools_web_search: self.web_search_request,
            ..Default::default()
        }
        .apply(features);
    }
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

    pub fn from_config(
        cfg: &ConfigToml,
        config_profile: &ConfigProfile,
        overrides: FeatureOverrides,
    ) -> Self {
        let mut features = Features::with_defaults();

        let base_legacy = LegacyFeatureToggles {
            experimental_sandbox_command_assessment: cfg.experimental_sandbox_command_assessment,
            experimental_use_freeform_apply_patch: cfg.experimental_use_freeform_apply_patch,
            experimental_use_exec_command_tool: cfg.experimental_use_exec_command_tool,
            experimental_use_unified_exec_tool: cfg.experimental_use_unified_exec_tool,
            experimental_use_rmcp_client: cfg.experimental_use_rmcp_client,
            tools_web_search: cfg.tools.as_ref().and_then(|t| t.web_search),
            tools_view_image: cfg.tools.as_ref().and_then(|t| t.view_image),
            ..Default::default()
        };
        base_legacy.apply(&mut features);

        if let Some(base_features) = cfg.features.as_ref() {
            features.apply_map(&base_features.entries);
        }

        let profile_legacy = LegacyFeatureToggles {
            include_apply_patch_tool: config_profile.include_apply_patch_tool,
            include_view_image_tool: config_profile.include_view_image_tool,
            experimental_sandbox_command_assessment: config_profile
                .experimental_sandbox_command_assessment,
            experimental_use_freeform_apply_patch: config_profile
                .experimental_use_freeform_apply_patch,
            experimental_use_exec_command_tool: config_profile.experimental_use_exec_command_tool,
            experimental_use_unified_exec_tool: config_profile.experimental_use_unified_exec_tool,
            experimental_use_rmcp_client: config_profile.experimental_use_rmcp_client,
            tools_web_search: config_profile.tools_web_search,
            tools_view_image: config_profile.tools_view_image,
        };
        profile_legacy.apply(&mut features);
        if let Some(profile_features) = config_profile.features.as_ref() {
            features.apply_map(&profile_features.entries);
        }

        overrides.apply(&mut features);

        features
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
    FeatureSpec {
        id: Feature::SandboxCommandAssessment,
        key: "experimental_sandbox_command_assessment",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::GhostCommit,
        key: "ghost_commit",
        stage: Stage::Experimental,
        default_enabled: false,
    },
];
