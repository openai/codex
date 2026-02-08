//! JSON configuration types for config.json.
//!
//! This module defines the file format types for `~/.cocode/config.json`.
//! These types represent the JSON structure and are separate from the runtime
//! feature types in `cocode_protocol::features`.
//!
//! # Profile System
//!
//! Profiles allow quick switching between different model/provider configurations.
//! Profiles are defined inline in `config.json` and can override top-level settings.
//!
//! ## Resolution Order
//!
//! 1. Profile field (if profile is selected)
//! 2. Top-level field
//! 3. Built-in default
//!
//! ## Example
//!
//! ```json
//! {
//!   "models": {
//!     "main": "anthropic/claude-opus-4",
//!     "fast": "anthropic/claude-haiku",
//!     "vision": "openai/gpt-4o"
//!   },
//!   "logging": {
//!     "level": "info"
//!   },
//!   "features": {
//!     "subagent": true
//!   },
//!   "profile": "fast",
//!   "profiles": {
//!     "openai": {
//!       "models": {
//!         "main": "openai/gpt-5",
//!         "fast": "openai/gpt-5-mini"
//!       }
//!     },
//!     "debug": {
//!       "logging": {
//!         "level": "debug",
//!         "location": true
//!       }
//!     }
//!   }
//! }
//! ```

use cocode_protocol::AttachmentConfig;
use cocode_protocol::CompactConfig;
use cocode_protocol::Features;
use cocode_protocol::PathConfig;
use cocode_protocol::PlanModeConfig;
use cocode_protocol::ToolConfig;
use cocode_protocol::model::ModelRoles;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Permission rules configuration section.
///
/// Defines allow/deny/ask rules for tool execution.
/// Rules follow the pattern: tool name optionally followed by a command
/// pattern in parentheses, e.g. `"Bash(git *)"`, `"Read"`, `"Edit"`.
///
/// # Example
///
/// ```json
/// {
///   "permissions": {
///     "allow": ["Read", "Glob", "Bash(git *)", "Bash(npm *)"],
///     "deny": ["Bash(rm -rf *)"],
///     "ask": ["Bash(sudo *)"]
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct PermissionsConfig {
    /// Tool patterns that are always allowed without prompting.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Tool patterns that are always denied.
    #[serde(default)]
    pub deny: Vec<String>,
    /// Tool patterns that require user approval each time.
    #[serde(default)]
    pub ask: Vec<String>,
}

/// Profile configuration that can override top-level settings.
///
/// All fields are optional - only set fields will override top-level config.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigProfile {
    /// Role-based model configuration.
    #[serde(default)]
    pub models: Option<ModelRoles>,

    /// Override features.
    #[serde(default)]
    pub features: Option<FeaturesConfig>,

    /// Override logging.
    #[serde(default)]
    pub logging: Option<LoggingConfig>,
}

/// Application configuration file (~/.cocode/config.json).
///
/// # Example
///
/// ```json
/// {
///   "models": {
///     "main": "anthropic/claude-opus-4",
///     "fast": "anthropic/claude-haiku",
///     "vision": "openai/gpt-4o"
///   },
///   "logging": {
///     "level": "debug",
///     "location": true,
///     "target": false
///   },
///   "features": {
///     "subagent": true,
///     "web_fetch": true
///   },
///   "profile": "fast",
///   "profiles": {
///     "fast": {
///       "models": {
///         "fast": "openai/gpt-5-mini"
///       }
///     }
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct AppConfig {
    /// Role-based model configuration.
    #[serde(default)]
    pub models: Option<ModelRoles>,

    /// Profile name to use (selects from `profiles` table).
    #[serde(default)]
    pub profile: Option<String>,

    /// Logging configuration.
    #[serde(default)]
    pub logging: Option<LoggingConfig>,

    /// Feature toggles.
    #[serde(default)]
    pub features: Option<FeaturesConfig>,

    /// Profile definitions for quick switching.
    #[serde(default)]
    pub profiles: HashMap<String, ConfigProfile>,

    /// Tool execution configuration.
    #[serde(default)]
    pub tool: Option<ToolConfig>,

    /// Compaction configuration.
    #[serde(default)]
    pub compact: Option<CompactConfig>,

    /// Plan mode configuration.
    #[serde(default)]
    pub plan: Option<PlanModeConfig>,

    /// Attachment configuration.
    #[serde(default)]
    pub attachment: Option<AttachmentConfig>,

    /// Extended path configuration.
    #[serde(default)]
    pub paths: Option<PathConfig>,

    /// Preferred language for responses (e.g., "en", "zh", "ja").
    /// When set, the agent will respond in this language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_preference: Option<String>,

    /// Permission rules for tool execution.
    #[serde(default)]
    pub permissions: Option<PermissionsConfig>,
}

/// Resolved configuration with profile applied.
///
/// This is the effective configuration after merging profile overrides
/// with top-level settings.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedAppConfig {
    /// Effective role-based models.
    pub models: ModelRoles,
    /// Effective logging configuration.
    pub logging: Option<LoggingConfig>,
    /// Effective features.
    pub features: Features,
    /// Effective tool configuration.
    pub tool: Option<ToolConfig>,
    /// Effective compaction configuration.
    pub compact: Option<CompactConfig>,
    /// Effective plan mode configuration.
    pub plan: Option<PlanModeConfig>,
    /// Effective attachment configuration.
    pub attachment: Option<AttachmentConfig>,
    /// Effective path configuration.
    pub paths: Option<PathConfig>,
    /// Effective language preference.
    pub language_preference: Option<String>,
}

impl AppConfig {
    /// Resolve effective config with profile applied.
    ///
    /// Priority: Profile field > Top-level field > Built-in default
    pub fn resolve(&self) -> ResolvedAppConfig {
        let profile = self
            .profile
            .as_ref()
            .and_then(|name| self.profiles.get(name));

        ResolvedAppConfig {
            models: self.resolve_models(profile),
            logging: self.resolve_logging(profile),
            features: self.resolve_features_with_profile(profile),
            tool: self.tool.clone(),
            compact: self.compact.clone(),
            plan: self.plan.clone(),
            attachment: self.attachment.clone(),
            paths: self.paths.clone(),
            language_preference: self.language_preference.clone(),
        }
    }

    /// Resolve models with profile override.
    fn resolve_models(&self, profile: Option<&ConfigProfile>) -> ModelRoles {
        let mut models = self.models.clone().unwrap_or_default();

        if let Some(profile_models) = profile.and_then(|p| p.models.as_ref()) {
            models.merge(profile_models);
        }

        models
    }

    /// Get the currently selected profile (if any).
    pub fn selected_profile(&self) -> Option<&ConfigProfile> {
        self.profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }

    /// Resolve logging config with profile override.
    fn resolve_logging(&self, profile: Option<&ConfigProfile>) -> Option<LoggingConfig> {
        match (profile.and_then(|p| p.logging.clone()), &self.logging) {
            (Some(profile_logging), Some(base)) => Some(merge_logging(base, &profile_logging)),
            (Some(profile_logging), None) => Some(profile_logging),
            (None, base) => base.clone(),
        }
    }

    /// Resolve features with profile override.
    fn resolve_features_with_profile(&self, profile: Option<&ConfigProfile>) -> Features {
        let base = self.resolve_features();
        if let Some(profile_features) = profile.and_then(|p| p.features.as_ref()) {
            let mut merged = base;
            merged.apply_map(&profile_features.entries);
            merged
        } else {
            base
        }
    }

    /// Resolve features to runtime type (without profile).
    ///
    /// Returns the configured features merged with defaults, or just defaults
    /// if no features section is present.
    pub fn resolve_features(&self) -> Features {
        self.features
            .clone()
            .map(|f| f.into_features())
            .unwrap_or_else(Features::with_defaults)
    }

    /// List all available profile names.
    pub fn list_profiles(&self) -> Vec<&str> {
        self.profiles.keys().map(String::as_str).collect()
    }

    /// Check if a profile exists.
    pub fn has_profile(&self, name: &str) -> bool {
        self.profiles.contains_key(name)
    }
}

/// Merge two LoggingConfig instances (profile overrides base).
fn merge_logging(base: &LoggingConfig, profile: &LoggingConfig) -> LoggingConfig {
    LoggingConfig {
        level: profile.level.clone().or_else(|| base.level.clone()),
        location: profile.location.or(base.location),
        target: profile.target.or(base.target),
        timezone: profile.timezone.clone().or_else(|| base.timezone.clone()),
        modules: profile.modules.clone().or_else(|| base.modules.clone()),
    }
}

/// Logging configuration section.
///
/// # Example
///
/// ```json
/// {
///   "logging": {
///     "level": "debug",
///     "timezone": "local",
///     "modules": ["cocode_core=debug", "cocode_api=trace"],
///     "location": true,
///     "target": false
///   }
/// }
/// ```
///
/// # Note
///
/// Logging destination is determined by the runtime mode:
/// - TUI mode: Logs to `~/.cocode/log/cocode-tui.log`
/// - REPL mode (`--no-tui`): Logs to stderr
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct LoggingConfig {
    /// Log level (e.g., "trace", "debug", "info", "warn", "error").
    #[serde(default)]
    pub level: Option<String>,

    /// Include source location in logs.
    #[serde(default)]
    pub location: Option<bool>,

    /// Include target module path in logs.
    #[serde(default)]
    pub target: Option<bool>,

    /// Timezone for log timestamps ("local" or "utc", default: "local").
    #[serde(default)]
    pub timezone: Option<String>,

    /// Per-module log levels (e.g., ["cocode_core=debug", "cocode_api=trace"]).
    #[serde(default)]
    pub modules: Option<Vec<String>>,
}

impl LoggingConfig {
    /// Convert to `cocode_utils_common::LoggingConfig` for use with the
    /// `configure_fmt_layer!` macro.
    pub fn to_common_logging(&self) -> cocode_utils_common::LoggingConfig {
        cocode_utils_common::LoggingConfig {
            level: self.level.clone().unwrap_or_else(|| "info".to_string()),
            location: self.location.unwrap_or(false),
            target: self.target.unwrap_or(false),
            timezone: match self.timezone.as_deref() {
                Some("utc") => cocode_utils_common::TimezoneConfig::Utc,
                _ => cocode_utils_common::TimezoneConfig::Local,
            },
            modules: self.modules.clone().unwrap_or_default(),
        }
    }
}

/// Feature toggles section in JSON format.
///
/// This type represents the `features` object in config.json.
/// Use `into_features()` to convert to the runtime `Features` type.
///
/// # Example
///
/// ```json
/// {
///   "features": {
///     "subagent": true,
///     "web_fetch": true
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct FeaturesConfig {
    /// Feature key to enabled/disabled mapping.
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

impl FeaturesConfig {
    /// Convert to runtime `Features` type.
    ///
    /// Applies the JSON entries on top of the default feature set.
    pub fn into_features(self) -> cocode_protocol::Features {
        let mut features = cocode_protocol::Features::with_defaults();
        features.apply_map(&self.entries);
        features
    }

    /// Check if a specific feature is set in this JSON config.
    pub fn get(&self, key: &str) -> Option<bool> {
        self.entries.get(key).copied()
    }

    /// Check if any features are configured.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Validate feature keys and return any unknown keys.
    ///
    /// Returns a list of keys that don't match any known feature.
    /// Can be used to warn users about typos in their config.
    pub fn unknown_keys(&self) -> Vec<String> {
        self.entries
            .keys()
            .filter(|k| !cocode_protocol::is_known_feature_key(k))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::Feature;

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(config.models.is_none());
        assert!(config.profile.is_none());
        assert!(config.logging.is_none());
        assert!(config.features.is_none());
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_app_config_parse_minimal() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let models = config.models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().provider, "openai");
        assert_eq!(models.main.as_ref().unwrap().model, "gpt-5");
    }

    #[test]
    fn test_app_config_parse_full() {
        let json_str = r#"{
            "models": {
                "main": "genai/gemini-3-pro",
                "fast": "genai/gemini-3-flash"
            },
            "profile": "coding",
            "logging": {
                "level": "debug",
                "location": true,
                "target": false
            },
            "features": {
                "subagent": true,
                "web_fetch": true
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let models = config.models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().provider, "genai");
        assert_eq!(models.main.as_ref().unwrap().model, "gemini-3-pro");
        assert_eq!(config.profile, Some("coding".to_string()));

        let logging = config.logging.unwrap();
        assert_eq!(logging.level, Some("debug".to_string()));
        assert_eq!(logging.location, Some(true));
        assert_eq!(logging.target, Some(false));

        let features = config.features.unwrap();
        assert_eq!(features.get("subagent"), Some(true));
        assert_eq!(features.get("web_fetch"), Some(true));
    }

    #[test]
    fn test_app_config_parse_with_profiles() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "profile": "fast",
            "logging": {
                "level": "info"
            },
            "features": {
                "subagent": true
            },
            "profiles": {
                "anthropic": {
                    "models": {
                        "main": "anthropic/claude-opus-4"
                    }
                },
                "fast": {
                    "models": {
                        "main": "openai/gpt-5-mini"
                    },
                    "features": {
                        "subagent": false
                    }
                },
                "debug": {
                    "logging": {
                        "level": "debug",
                        "location": true
                    }
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();

        // Verify top-level
        let models = config.models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().model, "gpt-5");
        assert_eq!(config.profile, Some("fast".to_string()));

        // Verify profiles parsed
        assert_eq!(config.profiles.len(), 3);
        assert!(config.has_profile("anthropic"));
        assert!(config.has_profile("fast"));
        assert!(config.has_profile("debug"));

        // Check profile contents
        let anthropic = &config.profiles["anthropic"];
        let anthropic_models = anthropic.models.as_ref().unwrap();
        assert_eq!(
            anthropic_models.main.as_ref().unwrap().provider,
            "anthropic"
        );
        assert_eq!(
            anthropic_models.main.as_ref().unwrap().model,
            "claude-opus-4"
        );

        let fast = &config.profiles["fast"];
        assert!(fast.features.is_some());
        let fast_models = fast.models.as_ref().unwrap();
        assert_eq!(fast_models.main.as_ref().unwrap().model, "gpt-5-mini");

        let debug = &config.profiles["debug"];
        assert!(debug.logging.is_some());
        assert_eq!(
            debug.logging.as_ref().unwrap().level,
            Some("debug".to_string())
        );
    }

    #[test]
    fn test_resolve_with_no_profile() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "features": {
                "subagent": true
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        let main = resolved.models.main().unwrap();
        assert_eq!(main.provider, "openai");
        assert_eq!(main.model, "gpt-5");
        assert!(resolved.features.enabled(Feature::Subagent));
    }

    #[test]
    fn test_resolve_with_profile_override() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "profile": "fast",
            "features": {
                "subagent": true
            },
            "profiles": {
                "fast": {
                    "models": {
                        "main": "openai/gpt-5-mini"
                    },
                    "features": {
                        "subagent": false
                    }
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        // main model from profile
        let main = resolved.models.main().unwrap();
        assert_eq!(main.model, "gpt-5-mini");
        // features: profile overrides subagent to false
        assert!(!resolved.features.enabled(Feature::Subagent));
    }

    #[test]
    fn test_resolve_provider_override() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "profile": "anthropic",
            "profiles": {
                "anthropic": {
                    "models": {
                        "main": "anthropic/claude-opus-4"
                    }
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        let main = resolved.models.main().unwrap();
        assert_eq!(main.provider, "anthropic");
        assert_eq!(main.model, "claude-opus-4");
    }

    #[test]
    fn test_resolve_logging_merge() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "profile": "debug",
            "logging": {
                "level": "info",
                "target": true
            },
            "profiles": {
                "debug": {
                    "logging": {
                        "level": "debug",
                        "location": true
                    }
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        let logging = resolved.logging.unwrap();
        // level from profile
        assert_eq!(logging.level, Some("debug".to_string()));
        // location from profile
        assert_eq!(logging.location, Some(true));
        // target from base (not overridden)
        assert_eq!(logging.target, Some(true));
    }

    #[test]
    fn test_resolve_nonexistent_profile() {
        let json_str = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "profile": "nonexistent"
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        // Falls back to top-level
        let main = resolved.models.main().unwrap();
        assert_eq!(main.provider, "openai");
        assert_eq!(main.model, "gpt-5");
    }

    #[test]
    fn test_features_config_into_features() {
        let mut entries = BTreeMap::new();
        entries.insert("subagent".to_string(), true);
        entries.insert("ls".to_string(), false);

        let features_config = FeaturesConfig { entries };
        let features = features_config.into_features();

        // subagent should be enabled (it was set to true)
        assert!(features.enabled(Feature::Subagent));
        // ls should be disabled (it was set to false, overriding default true)
        assert!(!features.enabled(Feature::Ls));
    }

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert!(config.level.is_none());
        assert!(config.location.is_none());
        assert!(config.target.is_none());
    }

    #[test]
    fn test_app_config_resolve_features_with_features() {
        let mut entries = BTreeMap::new();
        entries.insert("subagent".to_string(), true);

        let config = AppConfig {
            features: Some(FeaturesConfig { entries }),
            ..Default::default()
        };

        let features = config.resolve_features();
        assert!(features.enabled(Feature::Subagent));
    }

    #[test]
    fn test_app_config_resolve_features_without_features() {
        let config = AppConfig::default();
        let features = config.resolve_features();

        // Should return defaults
        assert!(features.enabled(Feature::Ls));
        assert!(!features.enabled(Feature::Subagent));
    }

    #[test]
    fn test_features_config_unknown_keys_empty() {
        let mut entries = BTreeMap::new();
        entries.insert("subagent".to_string(), true);
        entries.insert("ls".to_string(), false);

        let features = FeaturesConfig { entries };
        assert!(features.unknown_keys().is_empty());
    }

    #[test]
    fn test_features_config_unknown_keys_with_unknown() {
        let mut entries = BTreeMap::new();
        entries.insert("subagent".to_string(), true);
        entries.insert("unknown_feature".to_string(), true);
        entries.insert("another_unknown".to_string(), false);

        let features = FeaturesConfig { entries };
        let unknown = features.unknown_keys();

        assert_eq!(unknown.len(), 2);
        assert!(unknown.contains(&"unknown_feature".to_string()));
        assert!(unknown.contains(&"another_unknown".to_string()));
    }

    #[test]
    fn test_list_profiles() {
        let json_str = r#"{
            "profiles": {
                "main": {
                    "models": {"main": "openai/gpt-5"}
                },
                "fast": {
                    "models": {"main": "openai/gpt-5-mini"}
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let profiles = config.list_profiles();

        assert_eq!(profiles.len(), 2);
        assert!(profiles.contains(&"main"));
        assert!(profiles.contains(&"fast"));
    }

    #[test]
    fn test_selected_profile() {
        let json_str = r#"{
            "profile": "fast",
            "profiles": {
                "fast": {
                    "models": {"main": "openai/gpt-5-mini"}
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let profile = config.selected_profile();

        assert!(profile.is_some());
        let models = profile.unwrap().models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().model, "gpt-5-mini");
    }

    #[test]
    fn test_merge_logging() {
        let base = LoggingConfig {
            level: Some("info".to_string()),
            location: Some(false),
            target: Some(true),
            timezone: Some("local".to_string()),
            modules: Some(vec!["cocode_core=info".to_string()]),
        };
        let override_config = LoggingConfig {
            level: Some("debug".to_string()),
            location: Some(true),
            target: None,
            timezone: None,
            modules: Some(vec!["cocode_core=debug".to_string()]),
        };

        let merged = merge_logging(&base, &override_config);

        assert_eq!(merged.level, Some("debug".to_string()));
        assert_eq!(merged.location, Some(true));
        assert_eq!(merged.target, Some(true)); // Kept from base
        assert_eq!(merged.timezone, Some("local".to_string())); // Kept from base
        assert_eq!(merged.modules, Some(vec!["cocode_core=debug".to_string()])); // From override
    }

    // ==========================================================
    // Tests for ModelRoles integration
    // ==========================================================

    #[test]
    fn test_resolve_with_models_field() {
        let json_str = r#"{
            "models": {
                "main": "anthropic/claude-opus-4",
                "fast": "anthropic/claude-haiku",
                "vision": "openai/gpt-4o"
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        // models field should be populated
        let main = resolved.models.main().unwrap();
        assert_eq!(main.provider, "anthropic");
        assert_eq!(main.model, "claude-opus-4");

        let fast = resolved.models.fast.as_ref().unwrap();
        assert_eq!(fast.provider, "anthropic");
        assert_eq!(fast.model, "claude-haiku");

        let vision = resolved.models.vision.as_ref().unwrap();
        assert_eq!(vision.provider, "openai");
        assert_eq!(vision.model, "gpt-4o");
    }

    #[test]
    fn test_resolve_profile_models_override() {
        let json_str = r#"{
            "models": {
                "main": "anthropic/claude-opus-4",
                "fast": "anthropic/claude-haiku"
            },
            "profile": "openai",
            "profiles": {
                "openai": {
                    "models": {
                        "main": "openai/gpt-5",
                        "vision": "openai/gpt-4o"
                    }
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();
        let resolved = config.resolve();

        // main overridden by profile
        let main = resolved.models.main().unwrap();
        assert_eq!(main.provider, "openai");
        assert_eq!(main.model, "gpt-5");

        // fast NOT overridden (kept from base)
        let fast = resolved.models.fast.as_ref().unwrap();
        assert_eq!(fast.model, "claude-haiku");

        // vision added by profile
        let vision = resolved.models.vision.as_ref().unwrap();
        assert_eq!(vision.model, "gpt-4o");
    }

    #[test]
    fn test_app_config_parse_with_models() {
        let json_str = r#"{
            "models": {
                "main": "anthropic/claude-opus-4",
                "fast": "anthropic/claude-haiku"
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();

        assert!(config.models.is_some());
        let models = config.models.as_ref().unwrap();
        assert!(models.main.is_some());
        assert!(models.fast.is_some());
    }

    #[test]
    fn test_config_profile_with_models() {
        let json_str = r#"{
            "profiles": {
                "test": {
                    "models": {
                        "main": "openai/gpt-5"
                    }
                }
            }
        }"#;
        let config: AppConfig = serde_json::from_str(json_str).unwrap();

        let profile = &config.profiles["test"];
        assert!(profile.models.is_some());
        let models = profile.models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().model, "gpt-5");
    }

    #[test]
    fn test_resolved_models_empty_when_no_config() {
        let config = AppConfig::default();
        let resolved = config.resolve();

        // models should be empty default
        assert!(resolved.models.is_empty());
        assert!(resolved.models.main().is_none());
    }
}
