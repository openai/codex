//! Compact configuration with all tunable parameters.
//!
//! This module defines `CompactConfig` with 18 configurable fields for
//! controlling compact behavior, thresholds, and context restoration.

use serde::Deserialize;
use serde::Serialize;

/// Complete compact configuration.
///
/// All fields have sensible defaults based on Claude Code best practices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactConfig {
    // ============ Enable/Disable Controls ============
    /// Master switch to enable/disable all compaction
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Enable auto-compact (triggers automatically when threshold exceeded)
    #[serde(default = "default_true")]
    pub auto_compact_enabled: bool,

    // ============ Trigger Thresholds ============
    /// Token threshold to trigger auto-compact (overrides model default)
    /// If not set, uses `model_context_window - free_space_buffer`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_threshold: Option<i64>,

    /// Override auto-compact threshold as percentage of context window (0-100)
    /// Example: 80 = trigger at 80% of context window used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_pct_override: Option<i32>,

    /// Minimum tokens to keep free in context window (default: 13,000)
    #[serde(default = "default_free_space_buffer")]
    pub free_space_buffer: i64,

    /// Warning threshold - tokens from limit before showing warning (default: 20,000)
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: i64,

    // ============ Micro-Compact Settings ============
    /// Minimum token savings required for micro-compact to be worthwhile (default: 20,000)
    #[serde(default = "default_min_tokens_to_save")]
    pub micro_compact_min_tokens_to_save: i64,

    /// Number of recent tool results to keep intact (default: 3)
    #[serde(default = "default_keep_last_n_tools")]
    pub micro_compact_keep_last_n_tools: i32,

    // ============ Compact Model Provider ============
    /// Provider ID for the compact/summarization model.
    /// If set, looks up the corresponding provider from `model_providers` map.
    /// If not set, uses the session's current model provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_model_provider: Option<String>,

    // ============ Context Restoration ============
    /// Maximum files to restore after compaction (default: 5)
    #[serde(default = "default_max_files_restore")]
    pub restore_max_files: i32,

    /// Token limit per restored file (default: 5,000)
    #[serde(default = "default_tokens_per_file")]
    pub restore_tokens_per_file: i64,

    /// Total token budget for all restored files (default: 50,000)
    #[serde(default = "default_total_file_budget")]
    pub restore_total_file_budget: i64,

    /// Restore todo list after compaction
    #[serde(default = "default_true")]
    pub restore_todos: bool,

    /// Restore plan file after compaction (if in plan mode)
    #[serde(default = "default_true")]
    pub restore_plan: bool,

    // ============ Token Counting ============
    /// Safety multiplier for token estimates (default: 1.33)
    #[serde(default = "default_safety_multiplier")]
    pub token_safety_multiplier: f64,

    /// Approximate bytes per token for quick estimates (default: 4)
    #[serde(default = "default_bytes_per_token")]
    pub approx_bytes_per_token: i32,

    /// Fixed token estimate per image in tool results (default: 2,000)
    #[serde(default = "default_tokens_per_image")]
    pub tokens_per_image: i64,

    // ============ User Message Preservation ============
    /// Maximum tokens for preserved user messages in compacted history (default: 20,000)
    #[serde(default = "default_user_message_max_tokens")]
    pub user_message_max_tokens: i64,
}

// Default value functions
fn default_true() -> bool {
    true
}
fn default_free_space_buffer() -> i64 {
    13_000
}
fn default_warning_threshold() -> i64 {
    20_000
}
fn default_min_tokens_to_save() -> i64 {
    20_000
}
fn default_keep_last_n_tools() -> i32 {
    3
}
fn default_max_files_restore() -> i32 {
    5
}
fn default_tokens_per_file() -> i64 {
    5_000
}
fn default_total_file_budget() -> i64 {
    50_000
}
fn default_safety_multiplier() -> f64 {
    1.33
}
fn default_bytes_per_token() -> i32 {
    4
}
fn default_tokens_per_image() -> i64 {
    2_000
}
fn default_user_message_max_tokens() -> i64 {
    20_000
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_compact_enabled: true,
            auto_compact_threshold: None,
            auto_compact_pct_override: None,
            free_space_buffer: default_free_space_buffer(),
            warning_threshold: default_warning_threshold(),
            micro_compact_min_tokens_to_save: default_min_tokens_to_save(),
            micro_compact_keep_last_n_tools: default_keep_last_n_tools(),
            compact_model_provider: None,
            restore_max_files: default_max_files_restore(),
            restore_tokens_per_file: default_tokens_per_file(),
            restore_total_file_budget: default_total_file_budget(),
            restore_todos: true,
            restore_plan: true,
            token_safety_multiplier: default_safety_multiplier(),
            approx_bytes_per_token: default_bytes_per_token(),
            tokens_per_image: default_tokens_per_image(),
            user_message_max_tokens: default_user_message_max_tokens(),
        }
    }
}

impl CompactConfig {
    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(pct) = self.auto_compact_pct_override {
            if pct < 0 || pct > 100 {
                return Err(format!(
                    "auto_compact_pct_override must be 0-100, got {pct}"
                ));
            }
        }
        if self.free_space_buffer < 0 {
            return Err(format!(
                "free_space_buffer must be >= 0, got {}",
                self.free_space_buffer
            ));
        }
        if self.token_safety_multiplier < 1.0 {
            return Err(format!(
                "token_safety_multiplier must be >= 1.0, got {}",
                self.token_safety_multiplier
            ));
        }
        if self.approx_bytes_per_token < 1 {
            return Err(format!(
                "approx_bytes_per_token must be >= 1, got {}",
                self.approx_bytes_per_token
            ));
        }
        // Validate restore_* fields
        if self.restore_max_files < 1 {
            return Err(format!(
                "restore_max_files must be >= 1, got {}",
                self.restore_max_files
            ));
        }
        if self.restore_tokens_per_file < 1 {
            return Err(format!(
                "restore_tokens_per_file must be >= 1, got {}",
                self.restore_tokens_per_file
            ));
        }
        if self.restore_total_file_budget < 1 {
            return Err(format!(
                "restore_total_file_budget must be >= 1, got {}",
                self.restore_total_file_budget
            ));
        }
        // Validate micro_compact_keep_last_n_tools
        if self.micro_compact_keep_last_n_tools < 1 {
            return Err(format!(
                "micro_compact_keep_last_n_tools must be >= 1, got {}",
                self.micro_compact_keep_last_n_tools
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_config_has_expected_values() {
        let config = CompactConfig::default();
        assert!(config.enabled);
        assert!(config.auto_compact_enabled);
        assert_eq!(config.free_space_buffer, 13_000);
        assert_eq!(config.warning_threshold, 20_000);
        assert_eq!(config.micro_compact_min_tokens_to_save, 20_000);
        assert_eq!(config.micro_compact_keep_last_n_tools, 3);
        assert_eq!(config.restore_max_files, 5);
        assert_eq!(config.restore_tokens_per_file, 5_000);
        assert_eq!(config.restore_total_file_budget, 50_000);
        assert!(config.restore_todos);
        assert!(config.restore_plan);
        assert!((config.token_safety_multiplier - 1.33).abs() < f64::EPSILON);
        assert_eq!(config.approx_bytes_per_token, 4);
        assert_eq!(config.tokens_per_image, 2_000);
        assert_eq!(config.user_message_max_tokens, 20_000);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = CompactConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: CompactConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let json = r#"{"enabled": false}"#;
        let config: CompactConfig = serde_json::from_str(json).expect("deserialize");
        assert!(!config.enabled);
        assert!(config.auto_compact_enabled); // default
        assert_eq!(config.free_space_buffer, 13_000); // default
    }

    #[test]
    fn validation_rejects_invalid_pct_override() {
        let mut config = CompactConfig::default();
        config.auto_compact_pct_override = Some(150);
        assert!(config.validate().is_err());

        config.auto_compact_pct_override = Some(-10);
        assert!(config.validate().is_err());

        config.auto_compact_pct_override = Some(80);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validation_rejects_invalid_safety_multiplier() {
        let mut config = CompactConfig::default();
        config.token_safety_multiplier = 0.5;
        assert!(config.validate().is_err());

        config.token_safety_multiplier = 1.5;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validation_rejects_invalid_restore_fields() {
        let mut config = CompactConfig::default();

        config.restore_max_files = 0;
        assert!(config.validate().is_err());
        config.restore_max_files = 5;

        config.restore_tokens_per_file = 0;
        assert!(config.validate().is_err());
        config.restore_tokens_per_file = 5_000;

        config.restore_total_file_budget = 0;
        assert!(config.validate().is_err());
        config.restore_total_file_budget = 50_000;

        assert!(config.validate().is_ok());
    }

    #[test]
    fn validation_rejects_invalid_keep_last_n_tools() {
        let mut config = CompactConfig::default();
        config.micro_compact_keep_last_n_tools = 0;
        assert!(config.validate().is_err());

        config.micro_compact_keep_last_n_tools = 1;
        assert!(config.validate().is_ok());
    }
}
