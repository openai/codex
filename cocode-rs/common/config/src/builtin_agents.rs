//! Configuration overrides for builtin agents.
//!
//! This module provides JSON-based configuration for the builtin subagent types
//! (bash, general, explore, plan, guide, statusline). Third-party agents continue
//! using the plugin system.
//!
//! # Configuration File
//!
//! Configuration is loaded from `~/.cocode/builtin-agents.json`:
//!
//! ```json
//! {
//!   "explore": {
//!     "max_turns": 30,
//!     "identity": "fast",
//!     "tools": ["Read", "Glob", "Grep", "Bash"],
//!     "disallowed_tools": []
//!   },
//!   "plan": {
//!     "max_turns": 100,
//!     "identity": "main"
//!   }
//! }
//! ```
//!
//! # Merge Behavior
//!
//! - Config values **override** hardcoded defaults
//! - Unspecified fields keep hardcoded values
//! - Unknown agent names are **ignored** (only builtin agents supported)

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Override configuration for a builtin agent.
///
/// All fields are optional. Unspecified fields retain the hardcoded defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuiltinAgentOverride {
    /// Override the maximum number of turns for this agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<i32>,

    /// Model role identity: "main", "fast", "explore", "plan", "vision", "review", "compact", "inherit".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,

    /// Override allowed tools (empty = all tools available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,

    /// Override denied tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disallowed_tools: Option<Vec<String>>,
}

/// Configuration file for builtin agent overrides.
///
/// Maps agent type (e.g., "explore", "bash") to its override configuration.
pub type BuiltinAgentsConfig = HashMap<String, BuiltinAgentOverride>;

/// Load builtin agents config from `~/.cocode/builtin-agents.json`.
///
/// Returns an empty map if the file doesn't exist or cannot be parsed.
/// Errors are logged but not propagated.
pub fn load_builtin_agents_config() -> BuiltinAgentsConfig {
    let path = match dirs::home_dir() {
        Some(h) => h.join(".cocode").join("builtin-agents.json"),
        None => return HashMap::new(),
    };

    if !path.exists() {
        return HashMap::new();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(config) => {
                tracing::debug!(path = %path.display(), "Loaded builtin agents config");
                config
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to parse builtin agents config");
                HashMap::new()
            }
        },
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to read builtin agents config");
            HashMap::new()
        }
    }
}

/// Apply environment variable overrides to config.
///
/// Environment variables follow the pattern:
/// - `COCODE_BUILTIN_AGENT_{AGENT}_{FIELD}`
///
/// Example:
/// - `COCODE_BUILTIN_AGENT_EXPLORE_MAX_TURNS=50`
/// - `COCODE_BUILTIN_AGENT_PLAN_IDENTITY=fast`
pub fn apply_env_overrides(config: &mut BuiltinAgentsConfig) {
    for (key, value) in std::env::vars() {
        if let Some(rest) = key.strip_prefix("COCODE_BUILTIN_AGENT_") {
            // Parse: {AGENT_NAME}_{FIELD}
            // Handle multi-word fields like MAX_TURNS
            if let Some((agent, field)) = rest.rsplit_once('_') {
                let agent = agent.to_lowercase().replace('_', "-");
                let entry = config.entry(agent).or_default();

                match field {
                    "TURNS" if rest.ends_with("MAX_TURNS") => {
                        // Handle MAX_TURNS specially - agent name is everything before _MAX_TURNS
                        if let Some(agent_name) = rest.strip_suffix("_MAX_TURNS") {
                            let agent_name = agent_name.to_lowercase().replace('_', "-");
                            if let Ok(n) = value.parse() {
                                config.entry(agent_name).or_default().max_turns = Some(n);
                            }
                        }
                    }
                    "IDENTITY" => {
                        entry.identity = Some(value);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Known builtin agent types.
pub const BUILTIN_AGENT_TYPES: &[&str] =
    &["bash", "general", "explore", "plan", "guide", "statusline"];

/// Check if an agent type is a builtin agent.
pub fn is_builtin_agent(agent_type: &str) -> bool {
    BUILTIN_AGENT_TYPES.contains(&agent_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_agent_override_defaults() {
        let override_cfg = BuiltinAgentOverride::default();
        assert!(override_cfg.max_turns.is_none());
        assert!(override_cfg.identity.is_none());
        assert!(override_cfg.tools.is_none());
        assert!(override_cfg.disallowed_tools.is_none());
    }

    #[test]
    fn test_parse_config_json() {
        let json = r#"{
            "explore": {
                "max_turns": 30,
                "identity": "fast",
                "tools": ["Read", "Glob", "Grep"]
            },
            "plan": {
                "max_turns": 100
            }
        }"#;

        let config: BuiltinAgentsConfig = serde_json::from_str(json).expect("parse");

        let explore = config.get("explore").expect("explore config");
        assert_eq!(explore.max_turns, Some(30));
        assert_eq!(explore.identity.as_deref(), Some("fast"));
        assert_eq!(
            explore.tools.as_deref(),
            Some(&["Read".to_string(), "Glob".to_string(), "Grep".to_string()][..])
        );
        assert!(explore.disallowed_tools.is_none());

        let plan = config.get("plan").expect("plan config");
        assert_eq!(plan.max_turns, Some(100));
        assert!(plan.identity.is_none());
    }

    #[test]
    fn test_parse_empty_config() {
        let json = "{}";
        let config: BuiltinAgentsConfig = serde_json::from_str(json).expect("parse");
        assert!(config.is_empty());
    }

    #[test]
    fn test_serialize_config() {
        let mut config = BuiltinAgentsConfig::new();
        config.insert(
            "explore".to_string(),
            BuiltinAgentOverride {
                max_turns: Some(50),
                identity: Some("fast".to_string()),
                tools: None,
                disallowed_tools: None,
            },
        );

        let json = serde_json::to_string_pretty(&config).expect("serialize");
        assert!(json.contains("\"max_turns\": 50"));
        assert!(json.contains("\"identity\": \"fast\""));
        // Optional None fields should be skipped
        assert!(!json.contains("tools"));
        assert!(!json.contains("disallowed_tools"));
    }

    #[test]
    fn test_is_builtin_agent() {
        assert!(is_builtin_agent("bash"));
        assert!(is_builtin_agent("explore"));
        assert!(is_builtin_agent("plan"));
        assert!(is_builtin_agent("general"));
        assert!(is_builtin_agent("guide"));
        assert!(is_builtin_agent("statusline"));

        assert!(!is_builtin_agent("custom"));
        assert!(!is_builtin_agent(""));
    }

    #[test]
    fn test_load_nonexistent_file() {
        let config = load_builtin_agents_config();
        // Should return empty map, not error
        assert!(config.is_empty());
    }
}
