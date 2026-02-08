//! Configuration types for the core loop.
//!
//! These types configure the behavior of the agent's main execution loop.

use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

use crate::PermissionMode;

/// Configuration for the core agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    /// Maximum number of turns before stopping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<i32>,
    /// Maximum tokens to use before stopping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    /// Permission mode for tool execution.
    #[serde(default)]
    pub permission_mode: PermissionMode,
    /// Enable streaming tool execution.
    #[serde(default)]
    pub enable_streaming_tools: bool,
    /// Enable micro-compaction of tool results.
    #[serde(default)]
    pub enable_micro_compaction: bool,
    /// Fallback model to use when primary model fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_model: Option<String>,
    /// Agent identifier (for sub-agents).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Parent agent identifier (for sub-agents).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
    /// Whether to record sidechain events.
    #[serde(default)]
    pub record_sidechain: bool,
    /// Session memory configuration.
    #[serde(default)]
    pub session_memory: SessionMemoryConfig,
    /// Stall detection configuration.
    #[serde(default)]
    pub stall_detection: StallDetectionConfig,
    /// Prompt caching configuration.
    #[serde(default)]
    pub prompt_caching: PromptCachingConfig,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_turns: None,
            max_tokens: None,
            permission_mode: PermissionMode::default(),
            enable_streaming_tools: false,
            enable_micro_compaction: false,
            fallback_model: None,
            agent_id: None,
            parent_agent_id: None,
            record_sidechain: false,
            session_memory: SessionMemoryConfig::default(),
            stall_detection: StallDetectionConfig::default(),
            prompt_caching: PromptCachingConfig::default(),
        }
    }
}

/// Configuration for session memory management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemoryConfig {
    /// Token budget for session memory.
    #[serde(default = "default_budget_tokens")]
    pub budget_tokens: i32,
    /// Priority for file restoration during session recovery.
    #[serde(default)]
    pub restoration_priority: FileRestorationPriority,
    /// Whether session memory is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_budget_tokens() -> i32 {
    4096
}

fn default_true() -> bool {
    true
}

impl Default for SessionMemoryConfig {
    fn default() -> Self {
        Self {
            budget_tokens: default_budget_tokens(),
            restoration_priority: FileRestorationPriority::default(),
            enabled: true,
        }
    }
}

/// Priority for restoring files during session recovery.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileRestorationPriority {
    /// Restore most recently accessed files first.
    #[default]
    MostRecent,
    /// Restore most frequently accessed files first.
    MostAccessed,
}

impl FileRestorationPriority {
    /// Get the priority as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            FileRestorationPriority::MostRecent => "most-recent",
            FileRestorationPriority::MostAccessed => "most-accessed",
        }
    }
}

impl std::fmt::Display for FileRestorationPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration for stream stall detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StallDetectionConfig {
    /// Timeout duration before considering a stream stalled.
    #[serde(with = "humantime_serde", default = "default_stall_timeout")]
    pub stall_timeout: Duration,
    /// Recovery action when a stall is detected.
    #[serde(default)]
    pub recovery: StallRecovery,
    /// Whether stall detection is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_stall_timeout() -> Duration {
    Duration::from_secs(30)
}

impl Default for StallDetectionConfig {
    fn default() -> Self {
        Self {
            stall_timeout: default_stall_timeout(),
            recovery: StallRecovery::default(),
            enabled: true,
        }
    }
}

/// Recovery action when a stream stall is detected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StallRecovery {
    /// Retry the request.
    #[default]
    Retry,
    /// Abort the operation.
    Abort,
    /// Fall back to an alternative model.
    Fallback,
}

impl StallRecovery {
    /// Get the recovery action as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            StallRecovery::Retry => "retry",
            StallRecovery::Abort => "abort",
            StallRecovery::Fallback => "fallback",
        }
    }
}

impl std::fmt::Display for StallRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration for prompt caching.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptCachingConfig {
    /// Whether prompt caching is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Cache breakpoints in the conversation.
    #[serde(default)]
    pub cache_breakpoints: Vec<CacheBreakpoint>,
}

/// A breakpoint for cache insertion in the conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheBreakpoint {
    /// Position in the message list (0-indexed).
    pub position: i32,
    /// Type of cache to use.
    pub cache_type: CacheType,
}

/// Type of cache for prompt caching.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheType {
    /// Ephemeral cache (short-lived).
    #[default]
    Ephemeral,
}

impl CacheType {
    /// Get the cache type as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            CacheType::Ephemeral => "ephemeral",
        }
    }
}

impl std::fmt::Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_config_default() {
        let config = LoopConfig::default();
        assert_eq!(config.max_turns, None);
        assert_eq!(config.max_tokens, None);
        assert_eq!(config.permission_mode, PermissionMode::Default);
        assert!(!config.enable_streaming_tools);
        assert!(!config.enable_micro_compaction);
        assert!(config.session_memory.enabled);
        assert!(config.stall_detection.enabled);
    }

    #[test]
    fn test_session_memory_config_default() {
        let config = SessionMemoryConfig::default();
        assert_eq!(config.budget_tokens, 4096);
        assert_eq!(
            config.restoration_priority,
            FileRestorationPriority::MostRecent
        );
        assert!(config.enabled);
    }

    #[test]
    fn test_stall_detection_config_default() {
        let config = StallDetectionConfig::default();
        assert_eq!(config.stall_timeout, Duration::from_secs(30));
        assert_eq!(config.recovery, StallRecovery::Retry);
        assert!(config.enabled);
    }

    #[test]
    fn test_file_restoration_priority() {
        assert_eq!(FileRestorationPriority::MostRecent.as_str(), "most-recent");
        assert_eq!(
            FileRestorationPriority::MostAccessed.as_str(),
            "most-accessed"
        );
    }

    #[test]
    fn test_stall_recovery() {
        assert_eq!(StallRecovery::Retry.as_str(), "retry");
        assert_eq!(StallRecovery::Abort.as_str(), "abort");
        assert_eq!(StallRecovery::Fallback.as_str(), "fallback");
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = LoopConfig {
            max_turns: Some(10),
            max_tokens: Some(100000),
            permission_mode: PermissionMode::AcceptEdits,
            enable_streaming_tools: true,
            enable_micro_compaction: true,
            fallback_model: Some("gpt-4".to_string()),
            agent_id: Some("agent-1".to_string()),
            parent_agent_id: None,
            record_sidechain: true,
            session_memory: SessionMemoryConfig::default(),
            stall_detection: StallDetectionConfig::default(),
            prompt_caching: PromptCachingConfig::default(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: LoopConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.max_turns, config.max_turns);
        assert_eq!(parsed.max_tokens, config.max_tokens);
        assert_eq!(parsed.permission_mode, config.permission_mode);
        assert_eq!(parsed.enable_streaming_tools, config.enable_streaming_tools);
        assert_eq!(parsed.fallback_model, config.fallback_model);
    }
}
