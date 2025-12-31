//! Token counting utilities.
//!
//! Provides approximate token counting with configurable safety margins.
//! Uses bytes-per-token heuristic for fast local estimation.

use super::config::CompactConfig;

/// Token counter with configurable parameters.
#[derive(Debug, Clone)]
pub struct TokenCounter {
    /// Safety multiplier (default: 1.33)
    pub safety_margin: f64,
    /// Approximate bytes per token (default: 4)
    pub bytes_per_token: i32,
    /// Fixed token estimate per image (default: 2,000)
    pub tokens_per_image: i64,
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self {
            safety_margin: 1.33,
            bytes_per_token: 4,
            tokens_per_image: 2_000,
        }
    }
}

impl From<&CompactConfig> for TokenCounter {
    fn from(config: &CompactConfig) -> Self {
        Self {
            safety_margin: config.token_safety_multiplier,
            bytes_per_token: config.approx_bytes_per_token,
            tokens_per_image: config.tokens_per_image,
        }
    }
}

impl TokenCounter {
    /// Create a new TokenCounter with custom parameters.
    pub fn new(safety_margin: f64, bytes_per_token: i32, tokens_per_image: i64) -> Self {
        Self {
            safety_margin,
            bytes_per_token,
            tokens_per_image,
        }
    }

    /// Quick approximate count (for local decisions).
    ///
    /// Matches Claude Code's gG / approximateTokenCount function.
    pub fn approximate(&self, text: &str) -> i64 {
        if self.bytes_per_token <= 0 {
            return 0;
        }
        (text.len() as i64 + self.bytes_per_token as i64 - 1) / self.bytes_per_token as i64
    }

    /// Approximate with safety margin (for threshold decisions).
    ///
    /// Matches Claude Code's EQ0 / countMessageTokensWithSafetyMargin function.
    pub fn with_safety_margin(&self, text: &str) -> i64 {
        ((self.approximate(text) as f64) * self.safety_margin).ceil() as i64
    }

    /// Count tokens for multiple text strings.
    pub fn approximate_total(&self, texts: &[&str]) -> i64 {
        texts.iter().map(|t| self.approximate(t)).sum()
    }

    /// Count tokens with safety margin for multiple texts.
    pub fn total_with_margin(&self, texts: &[&str]) -> i64 {
        let total: i64 = texts.iter().map(|t| self.approximate(t)).sum();
        ((total as f64) * self.safety_margin).ceil() as i64
    }

    /// Count tokens for an image (fixed estimate).
    pub fn image_tokens(&self) -> i64 {
        self.tokens_per_image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn approximate_token_count() {
        let counter = TokenCounter::default();

        // Empty string
        assert_eq!(counter.approximate(""), 0);

        // 4 bytes = 1 token
        assert_eq!(counter.approximate("test"), 1);

        // 8 bytes = 2 tokens
        assert_eq!(counter.approximate("test1234"), 2);

        // Rounding up
        assert_eq!(counter.approximate("ab"), 1); // 2 bytes -> 1 token
        assert_eq!(counter.approximate("abcde"), 2); // 5 bytes -> 2 tokens
    }

    #[test]
    fn with_safety_margin() {
        let counter = TokenCounter::default();

        // 100 bytes = 25 tokens * 1.33 = 33.25 -> 34
        let text = "a".repeat(100);
        let with_margin = counter.with_safety_margin(&text);
        assert_eq!(with_margin, 34);
    }

    #[test]
    fn custom_bytes_per_token() {
        let counter = TokenCounter::new(1.0, 3, 1000);

        // 9 bytes / 3 = 3 tokens
        assert_eq!(counter.approximate("123456789"), 3);
    }

    #[test]
    fn custom_safety_margin() {
        let counter = TokenCounter::new(2.0, 4, 1000);

        // 8 bytes = 2 tokens * 2.0 = 4
        assert_eq!(counter.with_safety_margin("12345678"), 4);
    }

    #[test]
    fn approximate_total() {
        let counter = TokenCounter::default();

        let texts = &["test", "hello", "world"];
        // Ceiling division: test=1, hello=2, world=2 = 5 tokens
        let total = counter.approximate_total(texts);
        assert_eq!(total, 5);
    }

    #[test]
    fn total_with_margin() {
        let counter = TokenCounter::default();

        let texts = &["test", "hello", "world"];
        // 5 tokens * 1.33 = 6.65 -> ceil = 7
        let total = counter.total_with_margin(texts);
        assert_eq!(total, 7);
    }

    #[test]
    fn from_config() {
        let mut config = CompactConfig::default();
        config.token_safety_multiplier = 1.5;
        config.approx_bytes_per_token = 3;
        config.tokens_per_image = 3000;

        let counter = TokenCounter::from(&config);
        assert!((counter.safety_margin - 1.5).abs() < f64::EPSILON);
        assert_eq!(counter.bytes_per_token, 3);
        assert_eq!(counter.tokens_per_image, 3000);
    }

    #[test]
    fn image_tokens() {
        let counter = TokenCounter::default();
        assert_eq!(counter.image_tokens(), 2_000);

        let custom = TokenCounter::new(1.33, 4, 5000);
        assert_eq!(custom.image_tokens(), 5000);
    }
}
