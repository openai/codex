//! Token counting utilities.
//!
//! Provides token counting for statistics and debugging.
//! Uses tiktoken (cl100k_base) for OpenAI-compatible token counting.
//!
//! Note: Chunk splitting is handled natively by CodeSplitter in token mode.
//! This module is only for statistics/validation purposes.

use tiktoken_rs::cl100k_base;

/// Default maximum tokens per chunk for embedding models.
pub const DEFAULT_MAX_CHUNK_TOKENS: usize = 512;

/// Token counter for statistics and validation.
pub struct TokenCounter {
    bpe: tiktoken_rs::CoreBPE,
    max_tokens: usize,
}

impl TokenCounter {
    /// Create a new token counter with default max tokens (512).
    pub fn new() -> Self {
        Self::with_max_tokens(DEFAULT_MAX_CHUNK_TOKENS)
    }

    /// Create a new token counter with custom max tokens.
    pub fn with_max_tokens(max_tokens: usize) -> Self {
        let bpe = cl100k_base().expect("Failed to load cl100k_base tokenizer");
        Self { bpe, max_tokens }
    }

    /// Count tokens in a text string.
    pub fn count_tokens(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }

    /// Check if a chunk is within the token limit.
    pub fn is_valid(&self, chunk: &str) -> bool {
        self.count_tokens(chunk) <= self.max_tokens
    }

    /// Get the maximum tokens limit.
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        let counter = TokenCounter::new();

        // Simple text
        let tokens = counter.count_tokens("Hello, world!");
        assert!(tokens > 0);
        assert!(tokens < 10);

        // Longer text should have more tokens
        let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let long_tokens = counter.count_tokens(&long_text);
        assert!(long_tokens > tokens);
    }

    #[test]
    fn test_is_valid() {
        let counter = TokenCounter::with_max_tokens(10);

        assert!(counter.is_valid("Hello"));
        assert!(!counter.is_valid(&"word ".repeat(100)));
    }

    #[test]
    fn test_max_tokens() {
        let counter = TokenCounter::with_max_tokens(256);
        assert_eq!(counter.max_tokens(), 256);
    }
}
