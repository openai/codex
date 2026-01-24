//! Token usage types.

use serde::Deserialize;
use serde::Serialize;

/// Detailed breakdown of input tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputTokensDetails {
    /// Number of tokens retrieved from cache.
    #[serde(default)]
    pub cached_tokens: i32,

    /// Number of text tokens.
    #[serde(default)]
    pub text_tokens: i32,

    /// Number of image tokens.
    #[serde(default)]
    pub image_tokens: i32,

    /// Number of audio tokens.
    #[serde(default)]
    pub audio_tokens: i32,
}

/// Detailed breakdown of output tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputTokensDetails {
    /// Number of reasoning tokens.
    #[serde(default)]
    pub reasoning_tokens: i32,

    /// Number of text tokens.
    #[serde(default)]
    pub text_tokens: i32,

    /// Number of audio tokens.
    #[serde(default)]
    pub audio_tokens: i32,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of input tokens.
    pub input_tokens: i32,

    /// Number of output tokens.
    pub output_tokens: i32,

    /// Total number of tokens.
    #[serde(default)]
    pub total_tokens: i32,

    /// Detailed breakdown of input tokens.
    #[serde(default)]
    pub input_tokens_details: InputTokensDetails,

    /// Detailed breakdown of output tokens.
    #[serde(default)]
    pub output_tokens_details: OutputTokensDetails,
}

impl Usage {
    /// Get reasoning tokens from output details.
    pub fn reasoning_tokens(&self) -> i32 {
        self.output_tokens_details.reasoning_tokens
    }

    /// Get cached tokens from input details (prompt caching).
    pub fn cached_tokens(&self) -> i32 {
        self.input_tokens_details.cached_tokens
    }

    /// Get text tokens from input details.
    pub fn input_text_tokens(&self) -> i32 {
        self.input_tokens_details.text_tokens
    }

    /// Get image tokens from input details.
    pub fn image_tokens(&self) -> i32 {
        self.input_tokens_details.image_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(usage.reasoning_tokens(), 0);
        assert_eq!(usage.cached_tokens(), 0);
    }

    #[test]
    fn test_usage_with_details() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            input_tokens_details: InputTokensDetails {
                cached_tokens: 20,
                text_tokens: 60,
                image_tokens: 20,
                audio_tokens: 0,
            },
            output_tokens_details: OutputTokensDetails {
                reasoning_tokens: 30,
                text_tokens: 20,
                audio_tokens: 0,
            },
        };
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.reasoning_tokens(), 30);
        assert_eq!(usage.cached_tokens(), 20);
        assert_eq!(usage.input_text_tokens(), 60);
        assert_eq!(usage.image_tokens(), 20);
    }

    #[test]
    fn test_usage_serde() {
        let json = r#"{
            "input_tokens": 100,
            "output_tokens": 50,
            "total_tokens": 150,
            "input_tokens_details": {"cached_tokens": 20, "text_tokens": 60},
            "output_tokens_details": {"reasoning_tokens": 30}
        }"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.reasoning_tokens(), 30);
        assert_eq!(usage.cached_tokens(), 20);
    }

    #[test]
    fn test_usage_serde_missing_details() {
        // Test that missing details default to 0
        let json = r#"{"input_tokens": 100, "output_tokens": 50, "total_tokens": 150}"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.reasoning_tokens(), 0);
        assert_eq!(usage.cached_tokens(), 0);
    }
}
