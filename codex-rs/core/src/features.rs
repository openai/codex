//! Centralized feature flags and metadata.
//!
//! This module defines a small set of toggles that gate experimental and
//! optional behavior across the codebase. Instead of wiring individual
//! booleans through multiple types, call sites consult a single `Features`
//! container attached to `Config`.

use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

mod legacy;
pub(crate) use legacy::LegacyFeatureToggles;

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
    /// Include the view_image tool.
    ViewImageTool,
    /// Send warnings to the model to correct it on the tool usage.
    ModelWarnings,
    /// Enable the default shell tool.
    ShellTool,

    // Experimental
    /// Use the single unified PTY-backed exec tool.
    UnifiedExec,
    /// Include the freeform apply_patch tool.
    ApplyPatchFreeform,
    /// Allow the model to request web searches.
    WebSearchRequest,
    /// Allow request body compression when using ChatGPT auth.
    RequestCompression,
    /// Gate the execpolicy enforcement for shell/unified exec.
    ExecPolicy,
    /// Enable Windows sandbox (restricted token) on Windows.
    WindowsSandbox,
    /// Use the elevated Windows sandbox pipeline (setup + runner).
    WindowsSandboxElevated,
    /// Remote compaction enabled (only for ChatGPT auth)
    RemoteCompaction,
    /// Refresh remote models and emit AppReady once the list is available.
    RemoteModels,
    /// Allow model to call multiple tools in parallel (only for models supporting it).
    ParallelToolCalls,
    /// Experimental shell snapshotting.
    ShellSnapshot,
    /// Experimental TUI v2 (viewport) implementation.
    Tui2,
    /// Enable discovery and injection of skills.
    Skills,
    /// Enforce UTF8 output in Powershell.
    PowershellUtf8,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyFeatureUsage {
    pub alias: String,
    pub feature: Feature,
}

/// Holds the effective set of enabled features.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Features {
    enabled: BTreeSet<Feature>,
    legacy_usages: BTreeSet<LegacyFeatureUsage>,
    request_compression: RequestCompressionFeature,
}

#[derive(Debug, Clone, Default)]
pub struct FeatureOverrides {
    pub include_apply_patch_tool: Option<bool>,
    pub web_search_request: Option<bool>,
}

impl FeatureOverrides {
    fn apply(self, features: &mut Features) {
        LegacyFeatureToggles {
            include_apply_patch_tool: self.include_apply_patch_tool,
            tools_web_search: self.web_search_request,
            ..Default::default()
        }
        .apply(features);
    }
}

impl Features {
    /// Starts with built-in defaults.
    pub fn with_defaults() -> Self {
        let mut features = Self {
            enabled: BTreeSet::new(),
            legacy_usages: BTreeSet::new(),
            request_compression: RequestCompressionFeature::Disabled,
        };
        for spec in FEATURES {
            if spec.default_enabled {
                features.enable(spec.id);
            }
        }
        features
    }

    pub fn enabled(&self, f: Feature) -> bool {
        self.enabled.contains(&f)
    }

    pub fn enable(&mut self, f: Feature) -> &mut Self {
        self.enabled.insert(f);
        if matches!(f, Feature::RequestCompression) {
            self.request_compression = RequestCompressionFeature::Zstd;
        }
        self
    }

    pub fn disable(&mut self, f: Feature) -> &mut Self {
        self.enabled.remove(&f);
        if matches!(f, Feature::RequestCompression) {
            self.request_compression = RequestCompressionFeature::Disabled;
        }
        self
    }

    pub fn record_legacy_usage_force(&mut self, alias: &str, feature: Feature) {
        self.legacy_usages.insert(LegacyFeatureUsage {
            alias: alias.to_string(),
            feature,
        });
    }

    pub fn record_legacy_usage(&mut self, alias: &str, feature: Feature) {
        if alias == feature.key() {
            return;
        }
        self.record_legacy_usage_force(alias, feature);
    }

    pub fn legacy_feature_usages(&self) -> impl Iterator<Item = (&str, Feature)> + '_ {
        self.legacy_usages
            .iter()
            .map(|usage| (usage.alias.as_str(), usage.feature))
    }

    pub fn request_compression(&self) -> RequestCompressionFeature {
        self.request_compression
    }

    pub fn set_request_compression(
        &mut self,
        request_compression: RequestCompressionFeature,
    ) -> &mut Self {
        self.request_compression = request_compression;
        if self.request_compression == RequestCompressionFeature::Disabled {
            self.enabled.remove(&Feature::RequestCompression);
        } else {
            self.enabled.insert(Feature::RequestCompression);
        }
        self
    }

    /// Apply a table of key -> value toggles (e.g. from TOML).
    pub fn apply_map(&mut self, m: &BTreeMap<String, FeatureValue>) {
        for (k, v) in m {
            match feature_for_key(k) {
                Some(feat) => {
                    if k != feat.key() {
                        self.record_legacy_usage(k.as_str(), feat);
                    }
                    if feat == Feature::RequestCompression {
                        match v {
                            FeatureValue::Bool(enabled) => {
                                let request_compression = if *enabled {
                                    RequestCompressionFeature::Zstd
                                } else {
                                    RequestCompressionFeature::Disabled
                                };
                                self.set_request_compression(request_compression);
                            }
                            FeatureValue::String(value) => {
                                match RequestCompressionFeature::parse(value) {
                                    Some(request_compression) => {
                                        self.set_request_compression(request_compression);
                                    }
                                    None => {
                                        tracing::warn!(
                                            "unknown request_compression feature value in config: {value}"
                                        );
                                    }
                                }
                            }
                        }
                    } else if let FeatureValue::Bool(enabled) = v {
                        if *enabled {
                            self.enable(feat);
                        } else {
                            self.disable(feat);
                        }
                    } else {
                        tracing::warn!("feature key expects boolean value: {k}");
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
            experimental_use_freeform_apply_patch: cfg.experimental_use_freeform_apply_patch,
            experimental_use_unified_exec_tool: cfg.experimental_use_unified_exec_tool,
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
            experimental_use_freeform_apply_patch: config_profile
                .experimental_use_freeform_apply_patch,

            experimental_use_unified_exec_tool: config_profile.experimental_use_unified_exec_tool,
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

    pub fn enabled_features(&self) -> Vec<Feature> {
        self.enabled.iter().copied().collect()
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

/// Returns `true` if the provided string matches a known feature toggle key.
pub fn is_known_feature_key(key: &str) -> bool {
    feature_for_key(key).is_some()
}

/// Deserializable features table for TOML.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct FeaturesToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FeatureValue {
    Bool(bool),
    String(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestCompressionFeature {
    #[default]
    Disabled,
    Zstd,
}

impl RequestCompressionFeature {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "none" | "disabled" => Some(Self::Disabled),
            "zstd" => Some(Self::Zstd),
            _ => None,
        }
    }
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
    // Stable features.
    FeatureSpec {
        id: Feature::GhostCommit,
        key: "undo",
        stage: Stage::Stable,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ParallelToolCalls,
        key: "parallel",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ViewImageTool,
        key: "view_image_tool",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ShellTool,
        key: "shell_tool",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ModelWarnings,
        key: "warnings",
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
        id: Feature::RequestCompression,
        key: "request_compression",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    // Beta program. Rendered in the `/experimental` menu for users.
    FeatureSpec {
        id: Feature::UnifiedExec,
        key: "unified_exec",
        stage: Stage::Beta {
            name: "Background terminal",
            menu_description: "Run long-running terminal commands in the background.",
            announcement: "NEW! Try Background terminals for long running processes. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ShellSnapshot,
        key: "shell_snapshot",
        stage: Stage::Beta {
            name: "Shell snapshot",
            menu_description: "Snapshot your shell environment to avoid re-running login scripts for every command.",
            announcement: "NEW! Try shell snapshotting to make your Codex faster. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ApplyPatchFreeform,
        key: "apply_patch_freeform",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ExecPolicy,
        key: "exec_policy",
        stage: Stage::Experimental,
        default_enabled: true,
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
        id: Feature::RemoteCompaction,
        key: "remote_compaction",
        stage: Stage::Experimental,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::RemoteModels,
        key: "remote_models",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Skills,
        key: "skills",
        stage: Stage::Experimental,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::PowershellUtf8,
        key: "powershell_utf8",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Tui2,
        key: "tui2",
        stage: Stage::Experimental,
        default_enabled: false,
    },
];
