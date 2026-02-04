//! MCP (Model Context Protocol) configuration types.
//!
//! This module provides configuration for MCP-related features:
//! - Auto-search mode for large tool lists
//! - Tool discovery caching
//! - Server health monitoring

use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// Auto-Search Constants
// ============================================================================

/// Default context threshold for enabling auto-search (10% of context window).
pub const DEFAULT_AUTOSEARCH_CONTEXT_THRESHOLD: f32 = 0.10;

/// Default minimum context window for auto-search (32k tokens).
pub const DEFAULT_AUTOSEARCH_MIN_CONTEXT_WINDOW: i32 = 32000;

/// Default chars per token estimate for threshold calculation.
pub const DEFAULT_CHARS_PER_TOKEN: f32 = 2.5;

// ============================================================================
// Tool Cache Constants
// ============================================================================

/// Default tool cache TTL in seconds (5 minutes).
pub const DEFAULT_TOOL_CACHE_TTL_SECS: i32 = 300;

// ============================================================================
// McpAutoSearchConfig
// ============================================================================

/// MCP auto-search configuration.
///
/// When the total description characters of MCP tools exceeds a threshold,
/// auto-search mode is enabled. Instead of including all tool descriptions
/// in the system prompt, tools are discovered on-demand via the MCPSearch tool.
///
/// # Threshold Calculation
///
/// ```text
/// threshold = context_threshold × context_window × chars_per_token
///           = 0.10 × 200000 × 2.5
///           = 50000 chars
/// ```
///
/// If total MCP tool description chars >= threshold, auto-search is enabled.
///
/// # Requirements
///
/// Auto-search requires:
/// - Model supports tool calling
/// - Context window >= min_context_window (32k default)
/// - MCPSearch tool available
///
/// # Example
///
/// ```json
/// {
///   "mcp_auto_search": {
///     "enabled": true,
///     "context_threshold": 0.10,
///     "min_context_window": 32000,
///     "search_on_list_changed": true
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpAutoSearchConfig {
    /// Enable auto-search mode (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Context threshold for enabling search (default: 0.10 = 10%).
    ///
    /// When total MCP tool description chars exceed this percentage of
    /// the context window, auto-search is enabled.
    #[serde(default = "default_context_threshold")]
    pub context_threshold: f32,

    /// Minimum context window required for auto-search (default: 32000).
    #[serde(default = "default_min_context_window")]
    pub min_context_window: i32,

    /// Trigger tool refresh on tools/list_changed notification (default: true).
    #[serde(default = "default_true")]
    pub search_on_list_changed: bool,

    /// Chars per token estimate for threshold calculation (default: 2.5).
    #[serde(default = "default_chars_per_token")]
    pub chars_per_token: f32,
}

impl Default for McpAutoSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            context_threshold: DEFAULT_AUTOSEARCH_CONTEXT_THRESHOLD,
            min_context_window: DEFAULT_AUTOSEARCH_MIN_CONTEXT_WINDOW,
            search_on_list_changed: true,
            chars_per_token: DEFAULT_CHARS_PER_TOKEN,
        }
    }
}

impl McpAutoSearchConfig {
    /// Calculate the character threshold for a given context window.
    ///
    /// Formula: threshold = context_threshold × context_window × chars_per_token
    pub fn char_threshold(&self, context_window: i32) -> i32 {
        (self.context_threshold * context_window as f32 * self.chars_per_token) as i32
    }

    /// Check if auto-search should be used.
    ///
    /// # Arguments
    /// * `context_window` - Model's context window size in tokens
    /// * `total_description_chars` - Total characters of all MCP tool descriptions
    /// * `has_tool_calling` - Whether the model supports tool calling
    ///
    /// # Returns
    /// `true` if auto-search should be enabled
    pub fn should_use_auto_search(
        &self,
        context_window: i32,
        total_description_chars: i32,
        has_tool_calling: bool,
    ) -> bool {
        // Check if enabled
        if !self.enabled {
            return false;
        }

        // Requires tool calling capability
        if !has_tool_calling {
            return false;
        }

        // Check minimum context window
        if context_window < self.min_context_window {
            return false;
        }

        // Check threshold
        let threshold = self.char_threshold(context_window);
        total_description_chars >= threshold
    }

    /// Load configuration with environment variable overrides.
    ///
    /// Supported environment variables:
    /// - `COCODE_MCP_AUTOSEARCH`: Enable/disable auto-search (true/false)
    /// - `COCODE_MCP_AUTOSEARCH_THRESHOLD`: Override context_threshold (0.0-1.0)
    /// - `COCODE_MCP_AUTOSEARCH_MIN_CONTEXT`: Override min_context_window
    pub fn with_env_overrides(mut self) -> Self {
        // COCODE_MCP_AUTOSEARCH - master toggle
        if let Ok(val) = std::env::var("COCODE_MCP_AUTOSEARCH") {
            if let Ok(enabled) = val.parse::<bool>() {
                self.enabled = enabled;
            }
        }

        // COCODE_MCP_AUTOSEARCH_THRESHOLD - context threshold
        if let Ok(val) = std::env::var("COCODE_MCP_AUTOSEARCH_THRESHOLD") {
            if let Ok(threshold) = val.parse::<f32>() {
                if (0.0..=1.0).contains(&threshold) {
                    self.context_threshold = threshold;
                }
            }
        }

        // COCODE_MCP_AUTOSEARCH_MIN_CONTEXT - minimum context window
        if let Ok(val) = std::env::var("COCODE_MCP_AUTOSEARCH_MIN_CONTEXT") {
            if let Ok(min_context) = val.parse::<i32>() {
                if min_context > 0 {
                    self.min_context_window = min_context;
                }
            }
        }

        self
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.context_threshold) {
            return Err(format!(
                "mcp_auto_search.context_threshold must be 0.0-1.0, got {}",
                self.context_threshold
            ));
        }

        if self.min_context_window < 0 {
            return Err(format!(
                "mcp_auto_search.min_context_window must be >= 0, got {}",
                self.min_context_window
            ));
        }

        if self.chars_per_token <= 0.0 {
            return Err(format!(
                "mcp_auto_search.chars_per_token must be > 0, got {}",
                self.chars_per_token
            ));
        }

        Ok(())
    }
}

// ============================================================================
// McpToolCacheConfig
// ============================================================================

/// Configuration for MCP tool discovery caching.
///
/// Caches tool lists per server to avoid repeated discovery calls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolCacheConfig {
    /// Enable tool caching (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cache TTL in seconds (default: 300 = 5 minutes).
    #[serde(default = "default_cache_ttl")]
    pub ttl_secs: i32,

    /// Invalidate cache on tools/list_changed notification (default: true).
    #[serde(default = "default_true")]
    pub invalidate_on_list_changed: bool,
}

impl Default for McpToolCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: DEFAULT_TOOL_CACHE_TTL_SECS,
            invalidate_on_list_changed: true,
        }
    }
}

impl McpToolCacheConfig {
    /// Get the TTL as a Duration.
    pub fn ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.ttl_secs as u64)
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), String> {
        if self.ttl_secs < 0 {
            return Err(format!(
                "mcp_tool_cache.ttl_secs must be >= 0, got {}",
                self.ttl_secs
            ));
        }
        Ok(())
    }
}

// ============================================================================
// McpConfig
// ============================================================================

/// Top-level MCP configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct McpConfig {
    /// Auto-search configuration.
    #[serde(default)]
    pub auto_search: McpAutoSearchConfig,

    /// Tool cache configuration.
    #[serde(default)]
    pub tool_cache: McpToolCacheConfig,
}

impl McpConfig {
    /// Load configuration with environment variable overrides.
    pub fn with_env_overrides(mut self) -> Self {
        self.auto_search = self.auto_search.with_env_overrides();
        self
    }

    /// Validate all configuration values.
    pub fn validate(&self) -> Result<(), String> {
        self.auto_search.validate()?;
        self.tool_cache.validate()?;
        Ok(())
    }
}

// ============================================================================
// Default value functions for serde
// ============================================================================

fn default_true() -> bool {
    true
}

fn default_context_threshold() -> f32 {
    DEFAULT_AUTOSEARCH_CONTEXT_THRESHOLD
}

fn default_min_context_window() -> i32 {
    DEFAULT_AUTOSEARCH_MIN_CONTEXT_WINDOW
}

fn default_chars_per_token() -> f32 {
    DEFAULT_CHARS_PER_TOKEN
}

fn default_cache_ttl() -> i32 {
    DEFAULT_TOOL_CACHE_TTL_SECS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_search_config_default() {
        let config = McpAutoSearchConfig::default();
        assert!(config.enabled);
        assert!((config.context_threshold - 0.10).abs() < f32::EPSILON);
        assert_eq!(config.min_context_window, 32000);
        assert!(config.search_on_list_changed);
        assert!((config.chars_per_token - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_char_threshold() {
        let config = McpAutoSearchConfig::default();

        // 200k context: threshold = 0.1 * 200000 * 2.5 = 50000
        assert_eq!(config.char_threshold(200000), 50000);

        // 128k context: threshold = 0.1 * 128000 * 2.5 = 32000
        assert_eq!(config.char_threshold(128000), 32000);
    }

    #[test]
    fn test_should_use_auto_search() {
        let config = McpAutoSearchConfig::default();

        // Disabled
        let disabled = McpAutoSearchConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!disabled.should_use_auto_search(200000, 100000, true));

        // No tool calling
        assert!(!config.should_use_auto_search(200000, 100000, false));

        // Context too small
        assert!(!config.should_use_auto_search(16000, 100000, true));

        // Below threshold (200k context, threshold = 50k chars)
        assert!(!config.should_use_auto_search(200000, 40000, true));

        // Above threshold
        assert!(config.should_use_auto_search(200000, 60000, true));

        // Exactly at threshold
        assert!(config.should_use_auto_search(200000, 50000, true));
    }

    #[test]
    fn test_tool_cache_config_default() {
        let config = McpToolCacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.ttl_secs, 300);
        assert!(config.invalidate_on_list_changed);
    }

    #[test]
    fn test_tool_cache_ttl() {
        let config = McpToolCacheConfig::default();
        assert_eq!(config.ttl(), std::time::Duration::from_secs(300));
    }

    #[test]
    fn test_mcp_config_default() {
        let config = McpConfig::default();
        assert!(config.auto_search.enabled);
        assert!(config.tool_cache.enabled);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = McpConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_threshold() {
        let config = McpAutoSearchConfig {
            context_threshold: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        let config = McpAutoSearchConfig {
            context_threshold: -0.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_min_context() {
        let config = McpAutoSearchConfig {
            min_context_window: -1000,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_ttl() {
        let config = McpToolCacheConfig {
            ttl_secs: -10,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = McpConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_serde_partial() {
        // Test that we can parse partial config with defaults
        let json = r#"{
            "auto_search": {
                "enabled": false
            }
        }"#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        assert!(!config.auto_search.enabled);
        assert!(config.tool_cache.enabled); // Default
    }
}
