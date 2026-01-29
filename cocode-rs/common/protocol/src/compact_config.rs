//! Compaction and session memory configuration.
//!
//! Defines settings for automatic context compaction and session memory management.

use serde::{Deserialize, Serialize};

/// Default minimum tokens for session memory extraction.
pub const DEFAULT_SESSION_MEMORY_MIN_TOKENS: i32 = 10000;

/// Default maximum tokens for session memory extraction.
pub const DEFAULT_SESSION_MEMORY_MAX_TOKENS: i32 = 40000;

/// Default cooldown in seconds between memory extractions.
pub const DEFAULT_EXTRACTION_COOLDOWN_SECS: i32 = 60;

/// Default maximum files for context restoration.
pub const DEFAULT_CONTEXT_RESTORE_MAX_FILES: i32 = 5;

/// Default token budget for context restoration.
pub const DEFAULT_CONTEXT_RESTORE_BUDGET: i32 = 50000;

/// Compaction and session memory configuration.
///
/// Controls automatic context compaction behavior and session memory management.
///
/// # Environment Variables
///
/// - `DISABLE_COMPACT`: Completely disable compaction feature
/// - `DISABLE_AUTO_COMPACT`: Disable automatic compaction (manual still works)
/// - `DISABLE_MICRO_COMPACT`: Disable micro-compaction (frequent small compactions)
/// - `COCODE_AUTOCOMPACT_PCT_OVERRIDE`: Override auto-compact percentage threshold (0-100)
/// - `COCODE_BLOCKING_LIMIT_OVERRIDE`: Override blocking limit for compaction
/// - `COCODE_SESSION_MEMORY_MIN_TOKENS`: Minimum tokens for session memory
/// - `COCODE_SESSION_MEMORY_MAX_TOKENS`: Maximum tokens for session memory
/// - `COCODE_EXTRACTION_COOLDOWN_SECS`: Cooldown between memory extractions
/// - `COCODE_CONTEXT_RESTORE_MAX_FILES`: Maximum files for context restoration
/// - `COCODE_CONTEXT_RESTORE_BUDGET`: Token budget for context restoration
///
/// # Example
///
/// ```json
/// {
///   "compact": {
///     "disable_compact": false,
///     "disable_auto_compact": false,
///     "session_memory_min_tokens": 15000,
///     "session_memory_max_tokens": 50000
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CompactConfig {
    /// Completely disable compaction feature.
    #[serde(default)]
    pub disable_compact: bool,

    /// Disable automatic compaction (manual still works).
    #[serde(default)]
    pub disable_auto_compact: bool,

    /// Disable micro-compaction (frequent small compactions).
    #[serde(default)]
    pub disable_micro_compact: bool,

    /// Override auto-compact percentage threshold (0-100).
    #[serde(default)]
    pub autocompact_pct_override: Option<i32>,

    /// Override blocking limit for compaction.
    #[serde(default)]
    pub blocking_limit_override: Option<i32>,

    /// Minimum tokens for session memory extraction.
    #[serde(default = "default_session_memory_min_tokens")]
    pub session_memory_min_tokens: i32,

    /// Maximum tokens for session memory extraction.
    #[serde(default = "default_session_memory_max_tokens")]
    pub session_memory_max_tokens: i32,

    /// Cooldown in seconds between memory extractions.
    #[serde(default = "default_extraction_cooldown_secs")]
    pub extraction_cooldown_secs: i32,

    /// Maximum files for context restoration.
    #[serde(default = "default_context_restore_max_files")]
    pub context_restore_max_files: i32,

    /// Token budget for context restoration.
    #[serde(default = "default_context_restore_budget")]
    pub context_restore_budget: i32,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            disable_compact: false,
            disable_auto_compact: false,
            disable_micro_compact: false,
            autocompact_pct_override: None,
            blocking_limit_override: None,
            session_memory_min_tokens: DEFAULT_SESSION_MEMORY_MIN_TOKENS,
            session_memory_max_tokens: DEFAULT_SESSION_MEMORY_MAX_TOKENS,
            extraction_cooldown_secs: DEFAULT_EXTRACTION_COOLDOWN_SECS,
            context_restore_max_files: DEFAULT_CONTEXT_RESTORE_MAX_FILES,
            context_restore_budget: DEFAULT_CONTEXT_RESTORE_BUDGET,
        }
    }
}

impl CompactConfig {
    /// Check if compaction is enabled (not disabled).
    pub fn is_compaction_enabled(&self) -> bool {
        !self.disable_compact
    }

    /// Check if auto-compaction is enabled.
    pub fn is_auto_compact_enabled(&self) -> bool {
        !self.disable_compact && !self.disable_auto_compact
    }

    /// Check if micro-compaction is enabled.
    pub fn is_micro_compact_enabled(&self) -> bool {
        !self.disable_compact && !self.disable_micro_compact
    }

    /// Validate configuration values.
    ///
    /// Returns an error message if any values are invalid.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(pct) = self.autocompact_pct_override {
            if !(0..=100).contains(&pct) {
                return Err(format!("autocompact_pct_override must be 0-100, got {pct}"));
            }
        }

        if self.session_memory_min_tokens > self.session_memory_max_tokens {
            return Err(format!(
                "session_memory_min_tokens ({}) > session_memory_max_tokens ({})",
                self.session_memory_min_tokens, self.session_memory_max_tokens
            ));
        }

        if self.extraction_cooldown_secs < 0 {
            return Err(format!(
                "extraction_cooldown_secs must be >= 0, got {}",
                self.extraction_cooldown_secs
            ));
        }

        if self.context_restore_max_files < 0 {
            return Err(format!(
                "context_restore_max_files must be >= 0, got {}",
                self.context_restore_max_files
            ));
        }

        if self.context_restore_budget < 0 {
            return Err(format!(
                "context_restore_budget must be >= 0, got {}",
                self.context_restore_budget
            ));
        }

        Ok(())
    }
}

fn default_session_memory_min_tokens() -> i32 {
    DEFAULT_SESSION_MEMORY_MIN_TOKENS
}

fn default_session_memory_max_tokens() -> i32 {
    DEFAULT_SESSION_MEMORY_MAX_TOKENS
}

fn default_extraction_cooldown_secs() -> i32 {
    DEFAULT_EXTRACTION_COOLDOWN_SECS
}

fn default_context_restore_max_files() -> i32 {
    DEFAULT_CONTEXT_RESTORE_MAX_FILES
}

fn default_context_restore_budget() -> i32 {
    DEFAULT_CONTEXT_RESTORE_BUDGET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_config_default() {
        let config = CompactConfig::default();
        assert!(!config.disable_compact);
        assert!(!config.disable_auto_compact);
        assert!(!config.disable_micro_compact);
        assert!(config.autocompact_pct_override.is_none());
        assert!(config.blocking_limit_override.is_none());
        assert_eq!(
            config.session_memory_min_tokens,
            DEFAULT_SESSION_MEMORY_MIN_TOKENS
        );
        assert_eq!(
            config.session_memory_max_tokens,
            DEFAULT_SESSION_MEMORY_MAX_TOKENS
        );
        assert_eq!(
            config.extraction_cooldown_secs,
            DEFAULT_EXTRACTION_COOLDOWN_SECS
        );
        assert_eq!(
            config.context_restore_max_files,
            DEFAULT_CONTEXT_RESTORE_MAX_FILES
        );
        assert_eq!(
            config.context_restore_budget,
            DEFAULT_CONTEXT_RESTORE_BUDGET
        );
    }

    #[test]
    fn test_compact_config_serde() {
        let json = r#"{
            "disable_compact": true,
            "disable_auto_compact": true,
            "autocompact_pct_override": 80,
            "session_memory_min_tokens": 15000,
            "session_memory_max_tokens": 50000
        }"#;
        let config: CompactConfig = serde_json::from_str(json).unwrap();
        assert!(config.disable_compact);
        assert!(config.disable_auto_compact);
        assert_eq!(config.autocompact_pct_override, Some(80));
        assert_eq!(config.session_memory_min_tokens, 15000);
        assert_eq!(config.session_memory_max_tokens, 50000);
    }

    #[test]
    fn test_is_compaction_enabled() {
        let mut config = CompactConfig::default();
        assert!(config.is_compaction_enabled());

        config.disable_compact = true;
        assert!(!config.is_compaction_enabled());
    }

    #[test]
    fn test_is_auto_compact_enabled() {
        let mut config = CompactConfig::default();
        assert!(config.is_auto_compact_enabled());

        config.disable_auto_compact = true;
        assert!(!config.is_auto_compact_enabled());

        config.disable_auto_compact = false;
        config.disable_compact = true;
        assert!(!config.is_auto_compact_enabled());
    }

    #[test]
    fn test_validate_valid_config() {
        let config = CompactConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_pct() {
        let config = CompactConfig {
            autocompact_pct_override: Some(150),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_min_greater_than_max() {
        let config = CompactConfig {
            session_memory_min_tokens: 50000,
            session_memory_max_tokens: 10000,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
