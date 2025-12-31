//! Query rewriting and translation.
//!
//! Provides query transformation for improved search results,
//! including translation of non-English queries, intent classification,
//! and query expansion for better code search.

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::error::Result;
use crate::query::preprocessor::contains_chinese;

/// Query intent classification.
///
/// Helps optimize search strategy based on what the user is looking for.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueryIntent {
    /// Looking for where something is defined
    Definition,
    /// Looking for how something is used
    Usage,
    /// Looking for examples or demos
    Example,
    /// Looking for trait/interface implementations
    Implementation,
    /// Looking for type signatures or definitions
    TypeSignature,
    /// General search (default)
    #[default]
    General,
}

impl QueryIntent {
    /// Get search boost factors for this intent.
    pub fn search_boosts(&self) -> IntentBoosts {
        match self {
            Self::Definition => IntentBoosts {
                symbol_weight: 2.0,
                content_weight: 1.0,
                prefer_declarations: true,
            },
            Self::Usage => IntentBoosts {
                symbol_weight: 1.0,
                content_weight: 2.0,
                prefer_declarations: false,
            },
            Self::Example => IntentBoosts {
                symbol_weight: 0.5,
                content_weight: 2.0,
                prefer_declarations: false,
            },
            Self::Implementation => IntentBoosts {
                symbol_weight: 1.5,
                content_weight: 1.5,
                prefer_declarations: true,
            },
            Self::TypeSignature => IntentBoosts {
                symbol_weight: 2.5,
                content_weight: 0.5,
                prefer_declarations: true,
            },
            Self::General => IntentBoosts::default(),
        }
    }
}

/// Search weight adjustments based on query intent.
#[derive(Debug, Clone)]
pub struct IntentBoosts {
    /// Weight multiplier for symbol/name matches
    pub symbol_weight: f32,
    /// Weight multiplier for content matches
    pub content_weight: f32,
    /// Whether to prefer declaration sites over usage sites
    pub prefer_declarations: bool,
}

impl Default for IntentBoosts {
    fn default() -> Self {
        Self {
            symbol_weight: 1.0,
            content_weight: 1.0,
            prefer_declarations: false,
        }
    }
}

/// Type of query expansion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionType {
    /// Synonym (function -> method)
    Synonym,
    /// CamelCase variant (get user -> getUser)
    CamelCase,
    /// snake_case variant (getUserInfo -> get_user_info)
    SnakeCase,
    /// Abbreviation (authentication -> auth)
    Abbreviation,
    /// Related term (database -> storage)
    Related,
}

/// A single query expansion with metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryExpansion {
    /// The expanded text
    pub text: String,
    /// Type of expansion
    pub expansion_type: ExpansionType,
    /// Weight for search (0.0 - 1.0)
    pub weight: f32,
}

impl QueryExpansion {
    /// Create a new query expansion.
    pub fn new(text: impl Into<String>, expansion_type: ExpansionType, weight: f32) -> Self {
        Self {
            text: text.into(),
            expansion_type,
            weight: weight.clamp(0.0, 1.0),
        }
    }

    /// Create a synonym expansion.
    pub fn synonym(text: impl Into<String>, weight: f32) -> Self {
        Self::new(text, ExpansionType::Synonym, weight)
    }

    /// Create a camelCase expansion.
    pub fn camel_case(text: impl Into<String>, weight: f32) -> Self {
        Self::new(text, ExpansionType::CamelCase, weight)
    }

    /// Create a snake_case expansion.
    pub fn snake_case(text: impl Into<String>, weight: f32) -> Self {
        Self::new(text, ExpansionType::SnakeCase, weight)
    }

    /// Create an abbreviation expansion.
    pub fn abbreviation(text: impl Into<String>, weight: f32) -> Self {
        Self::new(text, ExpansionType::Abbreviation, weight)
    }
}

/// Source of the rewriting result.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RewriteSource {
    /// Result from LLM generation
    Llm,
    /// Result from rule-based rewriting
    Rule,
    /// Result from cache hit
    Cache,
    /// Combination of sources
    #[default]
    Hybrid,
    /// Result from fallback (LLM failed, used rule-based)
    /// These results should NOT be cached to allow LLM retry on next request
    Fallback,
}

/// Result of query rewriting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewrittenQuery {
    /// Original query
    pub original: String,
    /// Rewritten/translated query
    pub rewritten: String,
    /// Whether translation was applied
    pub was_translated: bool,
    /// Source language (ISO 639-1: "zh", "en", "ja")
    #[serde(default)]
    pub source_language: Option<String>,
    /// Query intent
    #[serde(default)]
    pub intent: QueryIntent,
    /// Structured expansions with weights
    #[serde(default)]
    pub expansions: Vec<QueryExpansion>,
    /// Confidence score (0.0 - 1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Source of this result
    #[serde(default)]
    pub source: RewriteSource,
    /// Processing latency in milliseconds
    #[serde(default)]
    pub latency_ms: i64,
}

fn default_confidence() -> f32 {
    1.0
}

impl RewrittenQuery {
    /// Create a new rewritten query with no changes.
    pub fn unchanged(query: &str) -> Self {
        Self {
            original: query.to_string(),
            rewritten: query.to_string(),
            was_translated: false,
            source_language: Some("en".to_string()),
            intent: QueryIntent::General,
            expansions: Vec::new(),
            confidence: 1.0,
            source: RewriteSource::Rule,
            latency_ms: 0,
        }
    }

    /// Create a new translated query.
    pub fn translated(original: &str, translated: &str) -> Self {
        Self {
            original: original.to_string(),
            rewritten: translated.to_string(),
            was_translated: true,
            source_language: None,
            intent: QueryIntent::General,
            expansions: Vec::new(),
            confidence: 1.0,
            source: RewriteSource::Llm,
            latency_ms: 0,
        }
    }

    /// Add expansion terms (legacy compatibility).
    pub fn with_expansions(mut self, expansions: Vec<String>) -> Self {
        self.expansions = expansions
            .into_iter()
            .map(|text| QueryExpansion::synonym(text, 0.8))
            .collect();
        self
    }

    /// Add structured expansions.
    pub fn with_structured_expansions(mut self, expansions: Vec<QueryExpansion>) -> Self {
        self.expansions = expansions;
        self
    }

    /// Set the query intent.
    pub fn with_intent(mut self, intent: QueryIntent) -> Self {
        self.intent = intent;
        self
    }

    /// Set the source language.
    pub fn with_source_language(mut self, lang: impl Into<String>) -> Self {
        self.source_language = Some(lang.into());
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the rewrite source.
    pub fn with_source(mut self, source: RewriteSource) -> Self {
        self.source = source;
        self
    }

    /// Set the latency.
    pub fn with_latency_ms(mut self, latency_ms: i64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Get the effective query for search (combines rewritten + expansions).
    pub fn effective_query(&self) -> String {
        if self.expansions.is_empty() {
            self.rewritten.clone()
        } else {
            let expansion_text: Vec<&str> =
                self.expansions.iter().map(|e| e.text.as_str()).collect();
            format!("{} {}", self.rewritten, expansion_text.join(" "))
        }
    }

    /// Get weighted expansion terms for search.
    pub fn weighted_expansions(&self) -> Vec<(&str, f32)> {
        self.expansions
            .iter()
            .map(|e| (e.text.as_str(), e.weight))
            .collect()
    }

    /// Get intent-based search boosts.
    pub fn search_boosts(&self) -> IntentBoosts {
        self.intent.search_boosts()
    }

    /// Check if expansions contain a specific text.
    pub fn has_expansion(&self, text: &str) -> bool {
        self.expansions.iter().any(|e| e.text == text)
    }

    /// Get all expansion texts as strings.
    pub fn expansion_texts(&self) -> Vec<&str> {
        self.expansions.iter().map(|e| e.text.as_str()).collect()
    }
}

/// Trait for query rewriters.
#[async_trait]
pub trait QueryRewriter: Send + Sync {
    /// Rewrite a query for improved search.
    ///
    /// This may include:
    /// - Translation (e.g., Chinese to English)
    /// - Query expansion (synonyms, related terms)
    /// - Normalization
    async fn rewrite(&self, query: &str) -> Result<RewrittenQuery>;

    /// Check if this rewriter can handle the query.
    fn can_handle(&self, query: &str) -> bool;
}

/// Simple query rewriter that detects Chinese and prepares for translation.
///
/// This is a placeholder implementation. In production, you would integrate
/// with an LLM for actual translation.
pub struct SimpleRewriter {
    /// Whether to enable translation
    enable_translation: bool,
    /// Whether to enable query expansion
    enable_expansion: bool,
    /// Whether to enable case variant generation
    enable_case_variants: bool,
    /// Custom synonyms (term -> expansions)
    custom_synonyms: std::collections::HashMap<String, Vec<String>>,
}

impl SimpleRewriter {
    /// Create a new simple rewriter.
    pub fn new() -> Self {
        Self {
            enable_translation: true,
            enable_expansion: false,
            enable_case_variants: false,
            custom_synonyms: std::collections::HashMap::new(),
        }
    }

    /// Enable or disable translation.
    pub fn with_translation(mut self, enable: bool) -> Self {
        self.enable_translation = enable;
        self
    }

    /// Enable or disable query expansion.
    pub fn with_expansion(mut self, enable: bool) -> Self {
        self.enable_expansion = enable;
        self
    }

    /// Enable or disable case variant generation.
    pub fn with_case_variants(mut self, enable: bool) -> Self {
        self.enable_case_variants = enable;
        self
    }

    /// Add custom synonyms.
    pub fn with_custom_synonyms(
        mut self,
        synonyms: std::collections::HashMap<String, Vec<String>>,
    ) -> Self {
        self.custom_synonyms = synonyms;
        self
    }

    /// Expand a query with related programming terms.
    pub fn expand_query(&self, query: &str) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let mut expansions = Vec::new();

        // Programming-specific synonyms
        let default_synonyms = [
            ("function", vec!["fn", "func", "method", "def"]),
            ("class", vec!["struct", "type", "interface"]),
            ("error", vec!["err", "exception", "panic", "fail"]),
            ("test", vec!["spec", "unittest", "testcase"]),
            ("config", vec!["configuration", "settings", "options"]),
            ("auth", vec!["authentication", "authorize", "login"]),
            ("user", vec!["usr", "account", "profile"]),
            ("database", vec!["db", "storage", "datastore"]),
            ("request", vec!["req", "http", "api"]),
            ("response", vec!["resp", "res", "reply"]),
        ];

        for (term, synonyms) in default_synonyms {
            if query_lower.contains(term) {
                expansions.extend(synonyms.into_iter().map(String::from));
            }
        }

        // Apply custom synonyms
        for (term, synonyms) in &self.custom_synonyms {
            if query_lower.contains(&term.to_lowercase()) {
                expansions.extend(synonyms.clone());
            }
        }

        expansions
    }

    /// Generate structured expansions with types and weights.
    pub fn expand_query_structured(&self, query: &str) -> Vec<QueryExpansion> {
        let mut expansions = Vec::new();
        let query_lower = query.to_lowercase();

        // Synonym expansions (weight 0.8)
        let synonyms = self.expand_query(query);
        for syn in synonyms {
            expansions.push(QueryExpansion::synonym(syn, 0.8));
        }

        // Case variant expansions (weight 0.9)
        if self.enable_case_variants {
            let variants = generate_case_variants(query);
            for (text, expansion_type) in variants {
                expansions.push(QueryExpansion::new(text, expansion_type, 0.9));
            }
        }

        // Abbreviation expansions (weight 0.7)
        let abbreviations = generate_abbreviations(&query_lower);
        for abbrev in abbreviations {
            expansions.push(QueryExpansion::abbreviation(abbrev, 0.7));
        }

        expansions
    }
}

impl Default for SimpleRewriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate case variants (camelCase, snake_case, PascalCase) from a query.
fn generate_case_variants(query: &str) -> Vec<(String, ExpansionType)> {
    let mut variants = Vec::new();
    let words: Vec<&str> = query.split_whitespace().collect();

    if words.len() >= 2 && words.len() <= 5 {
        // camelCase: first word lowercase, rest capitalized
        let camel = words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if i == 0 {
                    w.to_lowercase()
                } else {
                    capitalize_first(w)
                }
            })
            .collect::<String>();
        variants.push((camel, ExpansionType::CamelCase));

        // PascalCase: all words capitalized
        let pascal = words
            .iter()
            .map(|w| capitalize_first(w))
            .collect::<String>();
        variants.push((pascal, ExpansionType::CamelCase));

        // snake_case: all lowercase, joined with underscore
        let snake = words
            .iter()
            .map(|w| w.to_lowercase())
            .collect::<Vec<_>>()
            .join("_");
        variants.push((snake, ExpansionType::SnakeCase));

        // kebab-case: all lowercase, joined with hyphen
        let kebab = words
            .iter()
            .map(|w| w.to_lowercase())
            .collect::<Vec<_>>()
            .join("-");
        variants.push((kebab, ExpansionType::SnakeCase));
    }

    variants
}

/// Capitalize the first letter of a word.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(|c| c.to_lowercase()))
            .collect(),
    }
}

/// Generate common abbreviations from a query.
fn generate_abbreviations(query: &str) -> Vec<String> {
    let mut abbreviations = Vec::new();

    // Common abbreviation patterns
    let abbrev_map = [
        ("authentication", "auth"),
        ("configuration", "config"),
        ("database", "db"),
        ("repository", "repo"),
        ("application", "app"),
        ("environment", "env"),
        ("development", "dev"),
        ("production", "prod"),
        ("message", "msg"),
        ("information", "info"),
        ("maximum", "max"),
        ("minimum", "min"),
        ("temporary", "temp"),
        ("parameter", "param"),
        ("arguments", "args"),
        ("initialize", "init"),
        ("callback", "cb"),
        ("response", "resp"),
        ("request", "req"),
    ];

    for (full, abbrev) in abbrev_map {
        if query.contains(full) {
            abbreviations.push(abbrev.to_string());
        }
        // Also check reverse (expand abbreviations)
        if query.contains(abbrev) && !query.contains(full) {
            abbreviations.push(full.to_string());
        }
    }

    abbreviations
}

#[async_trait]
impl QueryRewriter for SimpleRewriter {
    async fn rewrite(&self, query: &str) -> Result<RewrittenQuery> {
        let needs_translation = self.enable_translation && contains_chinese(query);

        let mut result = if needs_translation {
            // In a real implementation, this would call an LLM for translation.
            // For now, we just mark it as needing translation.
            // The actual translation would be done by the LlmTranslator.
            RewrittenQuery::unchanged(query)
                .with_source_language("zh")
                .with_source(RewriteSource::Rule)
        } else {
            RewrittenQuery::unchanged(query)
        };

        // Add expansions if enabled
        if self.enable_expansion {
            let expansions = self.expand_query(&result.rewritten);
            result = result.with_expansions(expansions);
        }

        Ok(result)
    }

    fn can_handle(&self, _query: &str) -> bool {
        true // SimpleRewriter handles all queries
    }
}

/// Trait for LLM-based translation.
///
/// This would be implemented by the core module to provide
/// translation services using the configured LLM.
#[async_trait]
pub trait Translator: Send + Sync {
    /// Translate text from source language to English.
    async fn translate_to_english(&self, text: &str) -> Result<String>;

    /// Detect the language of the text.
    fn detect_language(&self, text: &str) -> Option<String>;
}

/// Query rewriter with LLM translation support.
pub struct LlmRewriter<T: Translator> {
    translator: T,
    simple_rewriter: SimpleRewriter,
}

impl<T: Translator> LlmRewriter<T> {
    /// Create a new LLM rewriter.
    pub fn new(translator: T) -> Self {
        Self {
            translator,
            simple_rewriter: SimpleRewriter::new().with_expansion(true),
        }
    }
}

#[async_trait]
impl<T: Translator + 'static> QueryRewriter for LlmRewriter<T> {
    async fn rewrite(&self, query: &str) -> Result<RewrittenQuery> {
        let needs_translation = contains_chinese(query);

        if needs_translation {
            let translated = self.translator.translate_to_english(query).await?;
            let expansions = self.simple_rewriter.expand_query(&translated);

            Ok(RewrittenQuery::translated(query, &translated)
                .with_source_language("zh")
                .with_source(RewriteSource::Llm)
                .with_expansions(expansions))
        } else {
            self.simple_rewriter.rewrite(query).await
        }
    }

    fn can_handle(&self, _query: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewritten_query_unchanged() {
        let query = RewrittenQuery::unchanged("test query");
        assert_eq!(query.original, "test query");
        assert_eq!(query.rewritten, "test query");
        assert!(!query.was_translated);
    }

    #[test]
    fn test_rewritten_query_translated() {
        let query = RewrittenQuery::translated("用户认证", "user authentication");
        assert_eq!(query.original, "用户认证");
        assert_eq!(query.rewritten, "user authentication");
        assert!(query.was_translated);
    }

    #[test]
    fn test_effective_query_with_expansions() {
        let query = RewrittenQuery::unchanged("test function")
            .with_expansions(vec!["fn".to_string(), "method".to_string()]);
        assert_eq!(query.effective_query(), "test function fn method");
    }

    #[tokio::test]
    async fn test_simple_rewriter() {
        let rewriter = SimpleRewriter::new();

        // English query - no translation
        let result = rewriter.rewrite("find user authentication").await.unwrap();
        assert!(!result.was_translated);

        // Query with expansion
        let rewriter = SimpleRewriter::new().with_expansion(true);
        let result = rewriter.rewrite("test function").await.unwrap();
        assert!(!result.expansions.is_empty());
    }

    #[test]
    fn test_query_expansion() {
        let rewriter = SimpleRewriter::new();
        let expansions = rewriter.expand_query("find authentication function");

        assert!(expansions.contains(&"fn".to_string()));
        assert!(expansions.contains(&"login".to_string()));
    }

    #[test]
    fn test_case_variants() {
        let variants = generate_case_variants("get user info");

        // Should have camelCase, PascalCase, snake_case, kebab-case
        let texts: Vec<&str> = variants.iter().map(|(t, _)| t.as_str()).collect();
        assert!(texts.contains(&"getUserInfo"));
        assert!(texts.contains(&"GetUserInfo"));
        assert!(texts.contains(&"get_user_info"));
        assert!(texts.contains(&"get-user-info"));
    }

    #[test]
    fn test_case_variants_single_word() {
        // Single word shouldn't generate variants
        let variants = generate_case_variants("function");
        assert!(variants.is_empty());
    }

    #[test]
    fn test_abbreviations() {
        let abbrevs = generate_abbreviations("user authentication configuration");
        assert!(abbrevs.contains(&"auth".to_string()));
        assert!(abbrevs.contains(&"config".to_string()));
    }

    #[test]
    fn test_abbreviation_expansion() {
        // When query has abbreviation, should expand to full word
        let abbrevs = generate_abbreviations("db connection");
        assert!(abbrevs.contains(&"database".to_string()));
    }

    #[test]
    fn test_structured_expansion() {
        let rewriter = SimpleRewriter::new()
            .with_expansion(true)
            .with_case_variants(true);

        let expansions = rewriter.expand_query_structured("get user info");

        // Should have case variants
        assert!(expansions.iter().any(|e| e.text == "getUserInfo"));
        assert!(
            expansions
                .iter()
                .any(|e| e.expansion_type == ExpansionType::CamelCase)
        );
        assert!(
            expansions
                .iter()
                .any(|e| e.expansion_type == ExpansionType::SnakeCase)
        );

        // Should have synonym expansions for "user"
        assert!(expansions.iter().any(|e| e.text == "account"));
    }

    #[test]
    fn test_capitalize_first() {
        assert_eq!(capitalize_first("hello"), "Hello");
        assert_eq!(capitalize_first("HELLO"), "Hello");
        assert_eq!(capitalize_first(""), "");
        assert_eq!(capitalize_first("a"), "A");
    }

    #[test]
    fn test_custom_synonyms() {
        let mut custom = std::collections::HashMap::new();
        custom.insert(
            "handler".to_string(),
            vec!["processor".to_string(), "worker".to_string()],
        );

        let rewriter = SimpleRewriter::new().with_custom_synonyms(custom);
        let expansions = rewriter.expand_query("event handler");

        assert!(expansions.contains(&"processor".to_string()));
        assert!(expansions.contains(&"worker".to_string()));
    }
}
