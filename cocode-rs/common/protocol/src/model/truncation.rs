//! Truncation policy configuration.

use serde::{Deserialize, Serialize};

/// Truncation mode for tool output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncationMode {
    /// Truncate by bytes.
    Bytes,
    /// Truncate by tokens.
    Tokens,
}

/// Truncation policy for tool output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruncationPolicyConfig {
    /// Truncation mode.
    pub mode: TruncationMode,
    /// Truncation limit.
    pub limit: i64,
}

impl TruncationPolicyConfig {
    /// Create a bytes-based truncation policy.
    pub const fn bytes(limit: i64) -> Self {
        Self {
            mode: TruncationMode::Bytes,
            limit,
        }
    }

    /// Create a tokens-based truncation policy.
    pub const fn tokens(limit: i64) -> Self {
        Self {
            mode: TruncationMode::Tokens,
            limit,
        }
    }
}

impl Default for TruncationPolicyConfig {
    fn default() -> Self {
        Self::tokens(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let policy = TruncationPolicyConfig::default();
        assert_eq!(policy.mode, TruncationMode::Tokens);
        assert_eq!(policy.limit, 10_000);
    }

    #[test]
    fn test_bytes_constructor() {
        let policy = TruncationPolicyConfig::bytes(5000);
        assert_eq!(policy.mode, TruncationMode::Bytes);
        assert_eq!(policy.limit, 5000);
    }

    #[test]
    fn test_tokens_constructor() {
        let policy = TruncationPolicyConfig::tokens(8000);
        assert_eq!(policy.mode, TruncationMode::Tokens);
        assert_eq!(policy.limit, 8000);
    }

    #[test]
    fn test_serde() {
        let policy = TruncationPolicyConfig::bytes(1000);
        let json = serde_json::to_string(&policy).expect("serialize");
        assert!(json.contains("\"bytes\""));
        assert!(json.contains("1000"));

        let parsed: TruncationPolicyConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, policy);
    }
}
