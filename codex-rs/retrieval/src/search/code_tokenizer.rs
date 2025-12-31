//! Code-specific tokenizer for BM25 search.
//!
//! Handles code-specific patterns like:
//! - snake_case → [snake, case]
//! - camelCase → [camel, case]
//! - foo.bar → [foo, bar]
//! - foo::bar → [foo, bar]
//! - foo->bar → [foo, bar]

use bm25::Tokenizer;
use once_cell::sync::Lazy;
use regex::Regex;

/// Code-specific tokenizer that understands programming naming conventions.
///
/// Splits identifiers by:
/// - Underscores (snake_case)
/// - camelCase boundaries
/// - Dots, colons, arrows (member access)
/// - Standard whitespace and punctuation
#[derive(Debug, Clone, Default)]
pub struct CodeTokenizer {
    /// Minimum token length to include (filters noise)
    min_token_len: usize,
    /// Whether to lowercase tokens
    lowercase: bool,
}

impl CodeTokenizer {
    /// Create a new code tokenizer with default settings.
    pub fn new() -> Self {
        Self {
            min_token_len: 2,
            lowercase: true,
        }
    }

    /// Create a code tokenizer with custom settings.
    pub fn with_config(min_token_len: usize, lowercase: bool) -> Self {
        Self {
            min_token_len,
            lowercase,
        }
    }

    /// Preprocess code text by splitting naming conventions.
    fn preprocess_code(&self, text: &str) -> String {
        // 1. Replace common code separators with spaces
        static SEPARATOR_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(::|\->|\.|\-|/|\\)").expect("invalid regex"));
        let text = SEPARATOR_RE.replace_all(text, " ");

        // 2. Split snake_case: foo_bar → foo bar
        static SNAKE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"_+").expect("invalid regex"));
        let text = SNAKE_RE.replace_all(&text, " ");

        // 3. Split camelCase and PascalCase: fooBar → foo Bar, FooBar → Foo Bar
        // Insert space before uppercase letters that follow lowercase letters
        static CAMEL_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"([a-z])([A-Z])").expect("invalid regex"));
        let text = CAMEL_RE.replace_all(&text, "$1 $2");

        // 4. Split sequences like HTTPServer → HTTP Server (uppercase followed by uppercase+lowercase)
        static ACRONYM_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"([A-Z]+)([A-Z][a-z])").expect("invalid regex"));
        let text = ACRONYM_RE.replace_all(&text, "$1 $2");

        // 5. Remove common code symbols and punctuation
        static SYMBOL_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"[(){}\[\]<>;,=+*&|!?@#$%^~`"']"#).expect("invalid regex"));
        let text = SYMBOL_RE.replace_all(&text, " ");

        text.into_owned()
    }
}

impl Tokenizer for CodeTokenizer {
    fn tokenize(&self, input_text: &str) -> Vec<String> {
        let preprocessed = self.preprocess_code(input_text);

        preprocessed
            .split_whitespace()
            .filter_map(|token| {
                // Apply lowercase if configured
                let token = if self.lowercase {
                    token.to_lowercase()
                } else {
                    token.to_string()
                };

                // Filter by minimum length
                if token.len() >= self.min_token_len {
                    Some(token)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case_splitting() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("user_service_handler");
        assert_eq!(tokens, vec!["user", "service", "handler"]);
    }

    #[test]
    fn test_camel_case_splitting() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("getUserById");
        assert_eq!(tokens, vec!["get", "user", "by", "id"]);
    }

    #[test]
    fn test_pascal_case_splitting() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("UserServiceHandler");
        assert_eq!(tokens, vec!["user", "service", "handler"]);
    }

    #[test]
    fn test_acronym_splitting() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("HTTPServerConfig");
        assert_eq!(tokens, vec!["http", "server", "config"]);
    }

    #[test]
    fn test_dot_notation() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("self.user.get_name()");
        assert_eq!(tokens, vec!["self", "user", "get", "name"]);
    }

    #[test]
    fn test_path_separator() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("std::collections::HashMap");
        assert_eq!(tokens, vec!["std", "collections", "hash", "map"]);
    }

    #[test]
    fn test_arrow_operator() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("ptr->next->data");
        assert_eq!(tokens, vec!["ptr", "next", "data"]);
    }

    #[test]
    fn test_mixed_code() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("fn get_user_by_id(userId: i32) -> Option<User>");
        // fn is filtered (< 2 chars), i32 becomes [i32]
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"by".to_string()));
        assert!(tokens.contains(&"id".to_string()));
        assert!(tokens.contains(&"option".to_string()));
    }

    #[test]
    fn test_real_rust_code() {
        let tokenizer = CodeTokenizer::new();
        let code = r#"
            pub async fn search_bm25(&self, query: &str) -> Result<Vec<SearchResult>> {
                let embedder = self.embedder.read().await;
                let scorer = self.scorer.read().await;
                scorer.matches(&embedder.embed(query))
            }
        "#;
        let tokens = tokenizer.tokenize(code);

        // Should contain key terms
        assert!(tokens.contains(&"pub".to_string()));
        assert!(tokens.contains(&"async".to_string()));
        assert!(tokens.contains(&"search".to_string()));
        assert!(tokens.contains(&"bm25".to_string()));
        assert!(tokens.contains(&"query".to_string()));
        assert!(tokens.contains(&"result".to_string()));
        assert!(tokens.contains(&"embedder".to_string()));
        assert!(tokens.contains(&"scorer".to_string()));
    }

    #[test]
    fn test_filter_short_tokens() {
        let tokenizer = CodeTokenizer::new();
        // Single-char tokens should be filtered
        let tokens = tokenizer.tokenize("a b c foo bar");
        assert!(!tokens.contains(&"a".to_string()));
        assert!(!tokens.contains(&"b".to_string()));
        assert!(!tokens.contains(&"c".to_string()));
        assert!(tokens.contains(&"foo".to_string()));
        assert!(tokens.contains(&"bar".to_string()));
    }

    #[test]
    fn test_lowercase() {
        let tokenizer = CodeTokenizer::new();
        let tokens = tokenizer.tokenize("FOO Bar BAZ");
        assert_eq!(tokens, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_no_lowercase() {
        let tokenizer = CodeTokenizer::with_config(2, false);
        let tokens = tokenizer.tokenize("FOO Bar");
        assert_eq!(tokens, vec!["FOO", "Bar"]);
    }
}
