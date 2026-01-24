//! Query preprocessing.
//!
//! Based on Continue's BaseRetrievalPipeline.ts getCleanedTrigrams implementation.
//!
//! Key insight: Code queries and natural language queries need different processing:
//! - Code identifiers (getUserById, get_user_by_id): NO stemming, NO stopword removal, use trigrams
//! - Natural language (how to authenticate users): stemming OK, stopword removal OK

use std::collections::HashSet;

use once_cell::sync::Lazy;

use crate::config::SearchConfig;

/// Query type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Code identifier query (camelCase, snake_case, no spaces)
    CodeIdentifier,
    /// Symbol search query (type:, name:, file: prefixes)
    SymbolSearch,
    /// Natural language query
    NaturalLanguage,
}

/// Query preprocessor.
///
/// Handles tokenization, stop word removal, and stemming.
/// Automatically detects query type and applies appropriate processing.
pub struct QueryPreprocessor {
    stop_words: HashSet<String>,
    config: SearchConfig,
}

/// Processed query with tokens and n-grams.
#[derive(Debug, Clone)]
pub struct ProcessedQuery {
    /// Original query text
    pub original: String,
    /// Processed tokens
    pub tokens: Vec<String>,
    /// N-grams (if enabled)
    pub ngrams: Vec<String>,
    /// Trigrams for code-aware matching (always generated for code queries)
    pub trigrams: Vec<String>,
    /// Detected query type
    pub query_type: QueryType,
}

impl QueryPreprocessor {
    /// Create a new query preprocessor.
    pub fn new(config: SearchConfig) -> Self {
        Self {
            stop_words: default_stop_words(),
            config,
        }
    }

    /// Detect query type based on content patterns.
    pub fn detect_query_type(query: &str) -> QueryType {
        let trimmed = query.trim();

        // Check for symbol search syntax first
        if has_symbol_syntax(trimmed) {
            return QueryType::SymbolSearch;
        }

        // Check for code identifier patterns
        if is_code_identifier(trimmed) {
            return QueryType::CodeIdentifier;
        }

        QueryType::NaturalLanguage
    }

    /// Process a query with automatic type detection.
    ///
    /// Processing differs based on query type:
    /// - CodeIdentifier: No stemming, no stopword removal, generate trigrams
    /// - SymbolSearch: Extract search terms, no stemming
    /// - NaturalLanguage: Full processing with stemming and stopword removal
    pub fn process(&self, query: &str) -> ProcessedQuery {
        let query_type = Self::detect_query_type(query);

        match query_type {
            QueryType::CodeIdentifier => self.process_code_identifier(query),
            QueryType::SymbolSearch => self.process_symbol_search(query),
            QueryType::NaturalLanguage => self.process_natural_language(query),
        }
    }

    /// Process a code identifier query.
    ///
    /// - NO stemming (preserves exact identifier)
    /// - NO stopword removal (all parts are meaningful)
    /// - Split camelCase/snake_case into parts
    /// - Generate trigrams for substring matching
    fn process_code_identifier(&self, query: &str) -> ProcessedQuery {
        let normalized = normalize_whitespace(query);

        // Split into identifier-aware tokens
        let tokens = tokenize_code_identifier(&normalized);

        // Generate trigrams for code-aware matching
        let trigrams = generate_trigrams(&normalized);

        ProcessedQuery {
            original: query.to_string(),
            tokens,
            ngrams: Vec::new(),
            trigrams,
            query_type: QueryType::CodeIdentifier,
        }
    }

    /// Process a symbol search query (type:, name:, file:).
    ///
    /// - Extract the search value after the prefix
    /// - NO stemming
    /// - Generate trigrams
    fn process_symbol_search(&self, query: &str) -> ProcessedQuery {
        let normalized = normalize_whitespace(query);

        // Extract search terms (values after prefixes)
        let tokens = extract_symbol_search_terms(&normalized);

        // Generate trigrams for the extracted terms
        let trigrams: Vec<String> = tokens.iter().flat_map(|t| generate_trigrams(t)).collect();

        ProcessedQuery {
            original: query.to_string(),
            tokens,
            ngrams: Vec::new(),
            trigrams,
            query_type: QueryType::SymbolSearch,
        }
    }

    /// Process a natural language query.
    ///
    /// Full processing pipeline:
    /// 1. Normalize whitespace
    /// 2. Tokenize
    /// 3. Remove stop words
    /// 4. Stem tokens (if enabled)
    /// 5. Deduplicate
    /// 6. Generate n-grams (if enabled)
    fn process_natural_language(&self, query: &str) -> ProcessedQuery {
        // Step 1: Normalize whitespace
        let normalized = normalize_whitespace(query);

        // Step 2: Tokenize
        let tokens = tokenize(&normalized);

        // Step 3: Remove stop words
        let filtered: Vec<_> = tokens
            .into_iter()
            .filter(|t| !self.stop_words.contains(&t.to_lowercase()))
            .collect();

        // Step 4: Stem tokens (if enabled)
        let stemmed = if self.config.enable_stemming {
            stem_tokens(&filtered)
        } else {
            filtered
        };

        // Step 5: Deduplicate
        let unique: Vec<_> = stemmed
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // Step 6: Generate n-grams (if enabled)
        let ngrams = if self.config.enable_ngrams {
            generate_ngrams(&unique.join(" "), self.config.ngram_size)
        } else {
            Vec::new()
        };

        // Also generate trigrams for better code matching
        let trigrams = generate_trigrams(&normalized);

        ProcessedQuery {
            original: query.to_string(),
            tokens: unique,
            ngrams,
            trigrams,
            query_type: QueryType::NaturalLanguage,
        }
    }
}

/// Normalize whitespace (collapse multiple spaces).
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Tokenize a string.
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}\"'".contains(c))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Stem tokens using rust-stemmers.
fn stem_tokens(tokens: &[String]) -> Vec<String> {
    use rust_stemmers::Algorithm;
    use rust_stemmers::Stemmer;

    let en_stemmer = Stemmer::create(Algorithm::English);

    tokens
        .iter()
        .map(|t| {
            // Only stem ASCII alphabetic tokens
            if t.chars().all(|c| c.is_ascii_alphabetic()) {
                en_stemmer.stem(t).to_string()
            } else {
                t.clone()
            }
        })
        .collect()
}

/// Generate word n-grams from text.
fn generate_ngrams(text: &str, n: i32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < n as usize {
        return vec![text.to_string()];
    }

    words.windows(n as usize).map(|w| w.join(" ")).collect()
}

/// Generate character trigrams from text.
///
/// Trigrams are 3-character sliding windows, ideal for code search:
/// - "getUserById" → ["get", "etU", "tUs", "Use", "ser", ...]
/// - Enables substring matching without breaking identifiers
fn generate_trigrams(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 {
        return vec![text.to_lowercase()];
    }

    chars
        .windows(3)
        .map(|w| w.iter().collect::<String>().to_lowercase())
        .collect()
}

/// Check if query contains symbol search syntax.
///
/// Returns true if the query contains `type:`, `name:`, `file:`, or `path:` prefixes.
fn has_symbol_syntax(query: &str) -> bool {
    query.contains("type:")
        || query.contains("name:")
        || query.contains("file:")
        || query.contains("path:")
}

/// Check if query looks like a code identifier.
///
/// Returns true for:
/// - snake_case: `get_user_by_id`
/// - camelCase: `getUserById`
/// - PascalCase: `UserService`
/// - SCREAMING_SNAKE_CASE: `MAX_SIZE`
///
/// Returns false for:
/// - Natural language words: "error", "help", "find"
/// - Queries with spaces: "how to parse"
fn is_code_identifier(query: &str) -> bool {
    let trimmed = query.trim();

    // Empty or has spaces -> not an identifier
    if trimmed.is_empty() || trimmed.contains(' ') {
        return false;
    }

    // Contains underscore -> likely snake_case identifier
    if trimmed.contains('_') {
        return true;
    }

    // First char should be a letter
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() || !chars[0].is_alphabetic() {
        return false;
    }

    // Check for mixed case (camelCase/PascalCase)
    let has_upper = chars.iter().any(|c| c.is_uppercase());
    let has_lower = chars.iter().any(|c| c.is_lowercase());

    // Mixed case = likely camelCase/PascalCase
    if has_upper && has_lower {
        return true;
    }

    // Pure lowercase single word without special patterns is likely natural language
    // Examples: "error", "help", "find", "search", "config"
    // Only treat as identifier if it looks like a programming term
    if chars.iter().all(|c| c.is_lowercase()) {
        // Short words (< 3 chars) could be identifiers like "id", "db"
        if chars.len() <= 2 {
            return true;
        }
        // Otherwise, treat pure lowercase as natural language
        return false;
    }

    // SCREAMING_CASE or single uppercase word
    chars.iter().all(|c| c.is_alphanumeric())
}

/// Tokenize code identifier, splitting camelCase and snake_case.
///
/// - `getUserById` → ["get", "User", "By", "Id", "getUserById"]
/// - `get_user_by_id` → ["get", "user", "by", "id", "get_user_by_id"]
fn tokenize_code_identifier(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    // Always include the original
    tokens.push(text.to_string());

    // Split by underscore for snake_case
    if text.contains('_') {
        for part in text.split('_') {
            if !part.is_empty() {
                tokens.push(part.to_string());
            }
        }
    }

    // Split camelCase/PascalCase
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            continue;
        }

        // Start new word on uppercase after lowercase
        if c.is_uppercase() && i > 0 && chars[i - 1].is_lowercase() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        }

        current.push(c);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    tokens
        .into_iter()
        .filter(|t| {
            let lower = t.to_lowercase();
            if seen.contains(&lower) {
                false
            } else {
                seen.insert(lower);
                true
            }
        })
        .collect()
}

/// Extract search terms from symbol search syntax.
///
/// - `type:function name:parse` → ["function", "parse"]
/// - `file:src/main.rs` → ["src/main.rs"]
fn extract_symbol_search_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();

    for part in query.split_whitespace() {
        if let Some(value) = part.strip_prefix("type:") {
            if !value.is_empty() {
                terms.push(value.to_string());
            }
        } else if let Some(value) = part.strip_prefix("name:") {
            if !value.is_empty() {
                terms.push(value.to_string());
            }
        } else if let Some(value) = part.strip_prefix("file:") {
            if !value.is_empty() {
                terms.push(value.to_string());
            }
        } else if let Some(value) = part.strip_prefix("path:") {
            if !value.is_empty() {
                terms.push(value.to_string());
            }
        } else {
            // Non-prefixed terms are also included
            terms.push(part.to_string());
        }
    }

    terms
}

/// Default stop words (English and Chinese).
fn default_stop_words() -> HashSet<String> {
    STOP_WORDS.iter().map(|s| s.to_string()).collect()
}

/// Static stop words list.
static STOP_WORDS: Lazy<Vec<&str>> = Lazy::new(|| {
    vec![
        // English stop words
        "the",
        "a",
        "an",
        "is",
        "are",
        "was",
        "were",
        "be",
        "been",
        "being",
        "have",
        "has",
        "had",
        "do",
        "does",
        "did",
        "will",
        "would",
        "could",
        "should",
        "may",
        "might",
        "can",
        "this",
        "that",
        "these",
        "those",
        "i",
        "you",
        "he",
        "she",
        "it",
        "we",
        "they",
        "what",
        "which",
        "who",
        "whom",
        "how",
        "when",
        "where",
        "why",
        "all",
        "each",
        "every",
        "both",
        "few",
        "more",
        "most",
        "other",
        "some",
        "such",
        "no",
        "not",
        "only",
        "same",
        "so",
        "than",
        "too",
        "very",
        "just",
        "but",
        "and",
        "or",
        "if",
        "because",
        "as",
        "until",
        "while",
        "of",
        "at",
        "by",
        "for",
        "with",
        "about",
        "against",
        "between",
        "into",
        "through",
        "during",
        "before",
        "after",
        "above",
        "below",
        "to",
        "from",
        "up",
        "down",
        "in",
        "out",
        "on",
        "off",
        "over",
        "under",
        // Chinese stop words
        "的",
        "了",
        "和",
        "是",
        "就",
        "都",
        "而",
        "及",
        "与",
        "着",
        "或",
        "一个",
        "没有",
        "我们",
        "你们",
        "他们",
        "它们",
        "这个",
        "那个",
        "这些",
        "那些",
        "什么",
        "怎么",
        "如何",
        "为什么",
    ]
});

/// Check if text contains Chinese characters.
pub fn contains_chinese(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9fff}'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("hello   world"), "hello world");
        assert_eq!(normalize_whitespace("  leading"), "leading");
        assert_eq!(normalize_whitespace("trailing  "), "trailing");
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("hello, world! How are you?");
        assert_eq!(tokens, vec!["hello", "world", "How", "are", "you"]);
    }

    #[test]
    fn test_contains_chinese() {
        assert!(contains_chinese("用户认证"));
        assert!(contains_chinese("hello 世界"));
        assert!(!contains_chinese("hello world"));
    }

    #[test]
    fn test_process_natural_language_query() {
        let config = SearchConfig::default();
        let preprocessor = QueryPreprocessor::new(config);
        let result = preprocessor.process("how to authenticate users");

        // Should be detected as natural language
        assert_eq!(result.query_type, QueryType::NaturalLanguage);
        // "how" and "to" should be removed as stop words
        assert!(!result.tokens.contains(&"how".to_string()));
        assert!(!result.tokens.contains(&"to".to_string()));
        // "authenticate" and "users" should remain (possibly stemmed)
        assert!(result.tokens.iter().any(|t| t.contains("authent")));
        assert!(result.tokens.iter().any(|t| t.contains("user")));
    }

    #[test]
    fn test_is_code_identifier() {
        // Snake case
        assert!(is_code_identifier("get_user_by_id"));
        assert!(is_code_identifier("MAX_SIZE"));

        // CamelCase / PascalCase
        assert!(is_code_identifier("getUserById"));
        assert!(is_code_identifier("GetUserById"));
        assert!(is_code_identifier("XMLParser"));

        // Short identifiers (2 chars or less)
        assert!(is_code_identifier("id"));
        assert!(is_code_identifier("db"));
        assert!(is_code_identifier("io"));

        // Pure lowercase words (3+ chars) are natural language, not identifiers
        assert!(!is_code_identifier("main")); // Treated as natural language now
        assert!(!is_code_identifier("foo"));
        assert!(!is_code_identifier("error"));
        assert!(!is_code_identifier("help"));
        assert!(!is_code_identifier("find"));

        // Not identifiers
        assert!(!is_code_identifier("get user name"));
        assert!(!is_code_identifier("how to parse json"));
        assert!(!is_code_identifier(""));
        assert!(!is_code_identifier("123abc"));
    }

    #[test]
    fn test_has_symbol_syntax() {
        assert!(has_symbol_syntax("type:function"));
        assert!(has_symbol_syntax("name:parse"));
        assert!(has_symbol_syntax("file:src/main.rs"));
        assert!(has_symbol_syntax("path:*.rs"));
        assert!(has_symbol_syntax("type:function name:getUserById"));

        assert!(!has_symbol_syntax("parse error"));
        assert!(!has_symbol_syntax("getUserById"));
    }

    #[test]
    fn test_process_code_identifier() {
        let config = SearchConfig::default();
        let preprocessor = QueryPreprocessor::new(config);

        // Test camelCase
        let result = preprocessor.process("getUserById");
        assert_eq!(result.query_type, QueryType::CodeIdentifier);
        // Should NOT be stemmed
        assert!(result.tokens.contains(&"getUserById".to_string()));
        // Should have trigrams
        assert!(!result.trigrams.is_empty());

        // Test snake_case
        let result = preprocessor.process("get_user_by_id");
        assert_eq!(result.query_type, QueryType::CodeIdentifier);
        assert!(result.tokens.contains(&"get_user_by_id".to_string()));
        // "by" should NOT be removed as stopword in code queries
        assert!(result.tokens.iter().any(|t| t == "by"));
    }

    #[test]
    fn test_process_symbol_search() {
        let config = SearchConfig::default();
        let preprocessor = QueryPreprocessor::new(config);

        let result = preprocessor.process("type:function name:parse");
        assert_eq!(result.query_type, QueryType::SymbolSearch);
        assert!(result.tokens.contains(&"function".to_string()));
        assert!(result.tokens.contains(&"parse".to_string()));
    }

    #[test]
    fn test_tokenize_code_identifier() {
        // CamelCase
        let tokens = tokenize_code_identifier("getUserById");
        assert!(tokens.contains(&"getUserById".to_string()));
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"User".to_string()));

        // Snake case
        let tokens = tokenize_code_identifier("get_user_by_id");
        assert!(tokens.contains(&"get_user_by_id".to_string()));
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"by".to_string()));
        assert!(tokens.contains(&"id".to_string()));
    }

    #[test]
    fn test_generate_trigrams() {
        let trigrams = generate_trigrams("getUserById");
        assert!(trigrams.contains(&"get".to_string()));
        assert!(trigrams.contains(&"etu".to_string())); // lowercase
        assert!(trigrams.contains(&"tus".to_string()));
    }

    #[test]
    fn test_extract_symbol_search_terms() {
        let terms = extract_symbol_search_terms("type:function name:parse file:src/main.rs");
        assert_eq!(terms, vec!["function", "parse", "src/main.rs"]);

        let terms = extract_symbol_search_terms("find name:getUserById");
        assert_eq!(terms, vec!["find", "getUserById"]);
    }

    #[test]
    fn test_query_type_detection() {
        // Code identifiers with clear patterns
        assert_eq!(
            QueryPreprocessor::detect_query_type("getUserById"),
            QueryType::CodeIdentifier
        );
        assert_eq!(
            QueryPreprocessor::detect_query_type("get_user_by_id"),
            QueryType::CodeIdentifier
        );
        assert_eq!(
            QueryPreprocessor::detect_query_type("MAX_SIZE"),
            QueryType::CodeIdentifier
        );

        // Symbol search
        assert_eq!(
            QueryPreprocessor::detect_query_type("type:function"),
            QueryType::SymbolSearch
        );

        // Natural language
        assert_eq!(
            QueryPreprocessor::detect_query_type("how to parse json"),
            QueryType::NaturalLanguage
        );
        // Single lowercase words are now natural language
        assert_eq!(
            QueryPreprocessor::detect_query_type("error"),
            QueryType::NaturalLanguage
        );
        assert_eq!(
            QueryPreprocessor::detect_query_type("main"),
            QueryType::NaturalLanguage
        );

        // Short identifiers stay as code identifiers
        assert_eq!(
            QueryPreprocessor::detect_query_type("id"),
            QueryType::CodeIdentifier
        );
    }
}
