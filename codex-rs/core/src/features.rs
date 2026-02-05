//! Centralized feature flags and metadata.
//!
//! This module defines a small set of toggles that gate experimental and
//! optional behavior across the codebase. Instead of wiring individual
//! booleans through multiple types, call sites consult a single `Features`
//! container attached to `Config`.

use crate::config::CONFIG_TOML_FILE;
use crate::config::Config;
use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::WarningEvent;
use codex_otel::OtelManager;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use toml::Value as TomlValue;

mod legacy;
pub(crate) use legacy::LegacyFeatureToggles;
pub(crate) use legacy::legacy_feature_keys;

/// High-level lifecycle stage for a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Features that are still under development, not ready for external use
    UnderDevelopment,
    /// Experimental features made available to users through the `/experimental` menu
    Experimental {
        name: &'static str,
        menu_description: &'static str,
        announcement: &'static str,
    },
    /// Stable features. The feature flag is kept for ad-hoc enabling/disabling
    Stable,
    /// Deprecated feature that should not be used anymore.
    Deprecated,
    /// The feature flag is useless but kept for backward compatibility reason.
    Removed,
}

impl Stage {
    pub fn experimental_menu_name(self) -> Option<&'static str> {
        match self {
            Stage::Experimental { name, .. } => Some(name),
            _ => None,
        }
    }

    pub fn experimental_menu_description(self) -> Option<&'static str> {
        match self {
            Stage::Experimental {
                menu_description, ..
            } => Some(menu_description),
            _ => None,
        }
    }

    pub fn experimental_announcement(self) -> Option<&'static str> {
        match self {
            Stage::Experimental { announcement, .. } => Some(announcement),
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
    /// Enable the default shell tool.
    ShellTool,

    // Experimental
    /// Use the single unified PTY-backed exec tool.
    UnifiedExec,
    /// Include the freeform apply_patch tool.
    ApplyPatchFreeform,
    /// Allow the model to request web searches that fetch live content.
    WebSearchRequest,
    /// Allow the model to request web searches that fetch cached content.
    /// Takes precedence over `WebSearchRequest`.
    WebSearchCached,
    /// Gate the execpolicy enforcement for shell/unified exec.
    ExecPolicy,
    /// Use the bubblewrap-based Linux sandbox pipeline.
    UseLinuxSandboxBwrap,
    /// Allow the model to request approval and propose exec rules.
    RequestRule,
    /// Enable Windows sandbox (restricted token) on Windows.
    WindowsSandbox,
    /// Use the elevated Windows sandbox pipeline (setup + runner).
    WindowsSandboxElevated,
    /// Remote compaction enabled (only for ChatGPT auth)
    RemoteCompaction,
    /// Refresh remote models and emit AppReady once the list is available.
    RemoteModels,
    /// Experimental shell snapshotting.
    ShellSnapshot,
    /// Enable runtime metrics snapshots via a manual reader.
    RuntimeMetrics,
    /// Persist rollout metadata to a local SQLite database.
    Sqlite,
    /// Append additional AGENTS.md guidance to user instructions.
    ChildAgentsMd,
    /// Enforce UTF8 output in Powershell.
    PowershellUtf8,
    /// Compress request bodies (zstd) when sending streaming requests to codex-backend.
    EnableRequestCompression,
    /// Enable collab tools.
    Collab,
    /// Enable apps.
    Apps,
    /// Allow prompting and installing missing MCP dependencies.
    SkillMcpDependencyInstall,
    /// Prompt for missing skill env var dependencies.
    SkillEnvVarDependencyPrompt,
    /// Steer feature flag - when enabled, Enter submits immediately instead of queuing.
    Steer,
    /// Enable collaboration modes (Plan, Default).
    CollaborationModes,
    /// Enable personality selection in the TUI.
    Personality,
    /// Use the Responses API WebSocket transport for OpenAI by default.
    ResponsesWebsockets,
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
    pub summary: String,
    pub details: Option<String>,
}

/// Holds the effective set of enabled features.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Features {
    enabled: BTreeSet<Feature>,
    legacy_usages: BTreeSet<LegacyFeatureUsage>,
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
        let mut set = BTreeSet::new();
        for spec in FEATURES {
            if spec.default_enabled {
                set.insert(spec.id);
            }
        }
        Self {
            enabled: set,
            legacy_usages: BTreeSet::new(),
        }
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

    pub fn record_legacy_usage_force(&mut self, alias: &str, feature: Feature) {
        let (summary, details) = legacy_usage_notice(alias, feature);
        self.legacy_usages.insert(LegacyFeatureUsage {
            alias: alias.to_string(),
            feature,
            summary,
            details,
        });
    }

    pub fn record_legacy_usage(&mut self, alias: &str, feature: Feature) {
        if alias == feature.key() {
            return;
        }
        self.record_legacy_usage_force(alias, feature);
    }

    pub fn legacy_feature_usages(&self) -> impl Iterator<Item = &LegacyFeatureUsage> + '_ {
        self.legacy_usages.iter()
    }

    pub fn emit_metrics(&self, otel: &OtelManager) {
        for feature in FEATURES {
            if self.enabled(feature.id) != feature.default_enabled {
                otel.counter(
                    "codex.feature.state",
                    1,
                    &[
                        ("feature", feature.key),
                        ("value", &self.enabled(feature.id).to_string()),
                    ],
                );
            }
        }
    }

    /// Apply a table of key -> bool toggles (e.g. from TOML).
    pub fn apply_map(&mut self, m: &BTreeMap<String, bool>) {
        for (k, v) in m {
            match k.as_str() {
                "web_search_request" => {
                    self.record_legacy_usage_force(
                        "features.web_search_request",
                        Feature::WebSearchRequest,
                    );
                }
                "web_search_cached" => {
                    self.record_legacy_usage_force(
                        "features.web_search_cached",
                        Feature::WebSearchCached,
                    );
                }
                _ => {}
            }
            match feature_for_key(k) {
                Some(feat) => {
                    if k != feat.key() {
                        self.record_legacy_usage(k.as_str(), feat);
                    }
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
            experimental_use_freeform_apply_patch: cfg.experimental_use_freeform_apply_patch,
            experimental_use_unified_exec_tool: cfg.experimental_use_unified_exec_tool,
            tools_web_search: cfg.tools.as_ref().and_then(|t| t.web_search),
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

fn legacy_usage_notice(alias: &str, feature: Feature) -> (String, Option<String>) {
    let canonical = feature.key();
    match feature {
        Feature::WebSearchRequest | Feature::WebSearchCached => {
            let label = match alias {
                "web_search" => "[features].web_search",
                "tools.web_search" => "[tools].web_search",
                "features.web_search_request" | "web_search_request" => {
                    "[features].web_search_request"
                }
                "features.web_search_cached" | "web_search_cached" => {
                    "[features].web_search_cached"
                }
                _ => alias,
            };
            let summary = format!("`{label}` is deprecated. Use `web_search` instead.");
            (summary, Some(web_search_details().to_string()))
        }
        _ => {
            let summary = format!("`{alias}` is deprecated. Use `[features].{canonical}` instead.");
            let details = if alias == canonical {
                None
            } else {
                Some(format!(
                    "Enable it with `--enable {canonical}` or `[features].{canonical}` in config.toml. See https://github.com/openai/codex/blob/main/docs/config.md#feature-flags for details."
                ))
            };
            (summary, details)
        }
    }
}

fn web_search_details() -> &'static str {
    "Set `web_search` to `\"live\"`, `\"cached\"`, or `\"disabled\"` at the top level (or under a profile) in config.toml."
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
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema)]
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
    // Stable features.
    FeatureSpec {
        id: Feature::GhostCommit,
        key: "undo",
        stage: Stage::Stable,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ShellTool,
        key: "shell_tool",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::UnifiedExec,
        key: "unified_exec",
        stage: Stage::Stable,
        default_enabled: !cfg!(windows),
    },
    FeatureSpec {
        id: Feature::WebSearchRequest,
        key: "web_search_request",
        stage: Stage::Deprecated,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebSearchCached,
        key: "web_search_cached",
        stage: Stage::Deprecated,
        default_enabled: false,
    },
    // Experimental program. Rendered in the `/experimental` menu for users.
    FeatureSpec {
        id: Feature::ShellSnapshot,
        key: "shell_snapshot",
        stage: Stage::Experimental {
            name: "Shell snapshot",
            menu_description: "Snapshot your shell environment to avoid re-running login scripts for every command.",
            announcement: "NEW! Try shell snapshotting to make your Codex faster. Enable in /experimental!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::RuntimeMetrics,
        key: "runtime_metrics",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Sqlite,
        key: "sqlite",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ChildAgentsMd,
        key: "child_agents_md",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ApplyPatchFreeform,
        key: "apply_patch_freeform",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ExecPolicy,
        key: "exec_policy",
        stage: Stage::UnderDevelopment,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::UseLinuxSandboxBwrap,
        key: "use_linux_sandbox_bwrap",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::RequestRule,
        key: "request_rule",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::WindowsSandbox,
        key: "experimental_windows_sandbox",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WindowsSandboxElevated,
        key: "elevated_windows_sandbox",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::RemoteCompaction,
        key: "remote_compaction",
        stage: Stage::UnderDevelopment,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::RemoteModels,
        key: "remote_models",
        stage: Stage::UnderDevelopment,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::PowershellUtf8,
        key: "powershell_utf8",
        #[cfg(windows)]
        stage: Stage::Stable,
        #[cfg(windows)]
        default_enabled: true,
        #[cfg(not(windows))]
        stage: Stage::UnderDevelopment,
        #[cfg(not(windows))]
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::EnableRequestCompression,
        key: "enable_request_compression",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::Collab,
        key: "collab",
        stage: Stage::Experimental {
            name: "Sub-agents",
            menu_description: "Ask Codex to spawn multiple agents to parallelize the work and win in efficiency.",
            announcement: "NEW: Sub-agents can now be spawned by Codex. Enable in /experimental and restart Codex!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Apps,
        key: "apps",
        stage: Stage::Experimental {
            name: "Apps",
            menu_description: "Use a connected ChatGPT App using \"$\". Install Apps via /apps command. Restart Codex after enabling.",
            announcement: "NEW: Use ChatGPT Apps (Connectors) in Codex via $ mentions. Enable in /experimental and restart Codex!",
        },
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::SkillMcpDependencyInstall,
        key: "skill_mcp_dependency_install",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::SkillEnvVarDependencyPrompt,
        key: "skill_env_var_dependency_prompt",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Steer,
        key: "steer",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::CollaborationModes,
        key: "collaboration_modes",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::Personality,
        key: "personality",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ResponsesWebsockets,
        key: "responses_websockets",
        stage: Stage::UnderDevelopment,
        default_enabled: false,
    },
];

/// Push a warning event if any under-development features are enabled.
pub fn maybe_push_unstable_features_warning(
    config: &Config,
    post_session_configured_events: &mut Vec<Event>,
) {
    if config.suppress_unstable_features_warning {
        return;
    }

    let mut under_development_feature_keys = Vec::new();
    if let Some(table) = config
        .config_layer_stack
        .effective_config()
        .get("features")
        .and_then(TomlValue::as_table)
    {
        for (key, value) in table {
            if value.as_bool() != Some(true) {
                continue;
            }
            let Some(spec) = FEATURES.iter().find(|spec| spec.key == key.as_str()) else {
                continue;
            };
            if !config.features.enabled(spec.id) {
                continue;
            }
            if matches!(spec.stage, Stage::UnderDevelopment) {
                under_development_feature_keys.push(spec.key.to_string());
            }
        }
    }

    if under_development_feature_keys.is_empty() {
        return;
    }

    let under_development_feature_keys = under_development_feature_keys.join(", ");
    let config_path = config
        .codex_home
        .join(CONFIG_TOML_FILE)
        .display()
        .to_string();
    let message = format!(
        "Under-development features enabled: {under_development_feature_keys}. Under-development features are incomplete and may behave unpredictably. To suppress this warning, set `suppress_unstable_features_warning = true` in {config_path}."
    );
    post_session_configured_events.push(Event {
        id: "".to_owned(),
        msg: EventMsg::Warning(WarningEvent { message }),
    });
}
