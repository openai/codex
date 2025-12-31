//! LLM provider for query rewriting.
//!
//! Provides an abstraction for LLM-based query transformation including
//! translation, intent classification, and query expansion.

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::config::LlmConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::query::ExpansionType;
use crate::query::QueryExpansion;
use crate::query::QueryIntent;
use crate::query::RewriteSource;
use crate::query::RewrittenQuery;

/// Request for LLM completion.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// System prompt
    pub system: String,
    /// User message
    pub user: String,
    /// Maximum tokens to generate
    pub max_tokens: i32,
    /// Temperature (0.0 - 1.0)
    pub temperature: f32,
}

/// Response from LLM completion.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// Generated text
    pub content: String,
    /// Token usage (optional)
    pub usage: Option<TokenUsage>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Input tokens
    pub prompt_tokens: i32,
    /// Output tokens
    pub completion_tokens: i32,
}

/// LLM provider trait for query rewriting.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Execute a completion request.
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;

    /// Check if the provider is available.
    fn is_available(&self) -> bool;
}

/// No-op LLM provider for testing or when LLM is disabled.
pub struct NoopProvider;

impl NoopProvider {
    /// Create a new noop provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for NoopProvider {
    fn name(&self) -> &str {
        "noop"
    }

    async fn complete(&self, _request: &CompletionRequest) -> Result<CompletionResponse> {
        // Return empty response - caller should fall back to rule-based
        Ok(CompletionResponse {
            content: String::new(),
            usage: None,
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}

/// OpenAI-compatible LLM provider.
///
/// Works with OpenAI API and compatible endpoints (Azure, local models).
pub struct OpenAiProvider {
    config: LlmConfig,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider.
    pub fn new(config: LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs as u64))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    /// Get the API endpoint.
    fn endpoint(&self) -> String {
        let base = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1");
        format!("{}/chat/completions", base.trim_end_matches('/'))
    }

    /// Get API key from environment.
    fn api_key(&self) -> Option<String> {
        std::env::var("OPENAI_API_KEY").ok()
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let api_key = self.api_key().ok_or_else(|| RetrievalErr::ConfigError {
            field: "OPENAI_API_KEY".to_string(),
            cause: "API key not set in environment".to_string(),
        })?;

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user}
            ],
            "max_tokens": request.max_tokens,
            "temperature": request.temperature
        });

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            let response = self
                .client
                .post(&self.endpoint())
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let json: OpenAiResponse = resp
                        .json()
                        .await
                        .map_err(|e| RetrievalErr::json_parse("OpenAI response", e))?;

                    let content = json
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_default();

                    return Ok(CompletionResponse {
                        content,
                        usage: json.usage.map(|u| TokenUsage {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                        }),
                    });
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    last_error = Some(format!("HTTP {status}: {body}"));
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            if attempt < self.config.max_retries {
                let delay = 100 * (1 << attempt);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
        }

        Err(RetrievalErr::EmbeddingFailed {
            cause: last_error.unwrap_or_else(|| "Unknown error".to_string()),
        })
    }

    fn is_available(&self) -> bool {
        self.api_key().is_some()
    }
}

/// OpenAI API response structure.
#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
}

/// System prompt for query rewriting.
pub const QUERY_REWRITE_SYSTEM_PROMPT: &str = r#"You are a code search query optimizer. Analyze user queries and transform them for optimal code search.

Output JSON:
{
  "translated": "English translation (same if already English)",
  "intent": "definition|usage|example|implementation|type_signature|general",
  "rewritten": "Optimized search query in English",
  "expansions": [
    {"text": "variant", "type": "synonym|camel_case|snake_case|abbreviation|related", "weight": 0.8}
  ],
  "confidence": 0.95
}

Guidelines:
1. TRANSLATION: Translate non-English to English, preserve technical terms
2. INTENT:
   - definition: where something is defined (fn, class, struct)
   - usage: how something is used (calls, references)
   - example: examples, tests, demos
   - implementation: interface/trait implementations
   - type_signature: type definitions
   - general: general search
3. REWRITING: Remove filler words, use programming terminology
4. EXPANSIONS: Generate variations (synonyms, case variants, abbreviations)

Output ONLY valid JSON, no markdown or explanations."#;

/// LLM response for query rewriting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRewriteResponse {
    /// Translated query
    pub translated: String,
    /// Detected intent
    pub intent: String,
    /// Rewritten query
    pub rewritten: String,
    /// Query expansions
    #[serde(default)]
    pub expansions: Vec<LlmExpansion>,
    /// Confidence score
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

fn default_confidence() -> f32 {
    0.8
}

/// LLM expansion in response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExpansion {
    /// Expansion text
    pub text: String,
    /// Expansion type
    #[serde(rename = "type")]
    pub expansion_type: String,
    /// Weight
    #[serde(default = "default_weight")]
    pub weight: f32,
}

fn default_weight() -> f32 {
    0.8
}

impl LlmRewriteResponse {
    /// Parse LLM response from JSON.
    pub fn parse(content: &str) -> Result<Self> {
        // Try to extract JSON from the response (in case there's surrounding text)
        let json_str = extract_json(content);

        serde_json::from_str(json_str).map_err(|e| RetrievalErr::json_parse("LLM response", e))
    }

    /// Convert to RewrittenQuery.
    pub fn to_rewritten_query(&self, original: &str, latency_ms: i64) -> RewrittenQuery {
        let intent = match self.intent.as_str() {
            "definition" => QueryIntent::Definition,
            "usage" => QueryIntent::Usage,
            "example" => QueryIntent::Example,
            "implementation" => QueryIntent::Implementation,
            "type_signature" => QueryIntent::TypeSignature,
            _ => QueryIntent::General,
        };

        let expansions: Vec<QueryExpansion> = self
            .expansions
            .iter()
            .map(|e| {
                let expansion_type = match e.expansion_type.as_str() {
                    "synonym" => ExpansionType::Synonym,
                    "camel_case" => ExpansionType::CamelCase,
                    "snake_case" => ExpansionType::SnakeCase,
                    "abbreviation" => ExpansionType::Abbreviation,
                    "related" => ExpansionType::Related,
                    _ => ExpansionType::Synonym,
                };
                QueryExpansion::new(&e.text, expansion_type, e.weight)
            })
            .collect();

        let was_translated = self.translated != original;
        let source_lang = if was_translated {
            // Detect source language (simplified)
            if original
                .chars()
                .any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
            {
                Some("zh".to_string())
            } else {
                None
            }
        } else {
            Some("en".to_string())
        };

        RewrittenQuery {
            original: original.to_string(),
            rewritten: self.rewritten.clone(),
            was_translated,
            source_language: source_lang,
            intent,
            expansions,
            confidence: self.confidence,
            source: RewriteSource::Llm,
            latency_ms,
        }
    }
}

/// Extract JSON from a string that might have surrounding text.
///
/// Uses depth tracking to correctly handle nested JSON objects.
/// This fixes the issue where `rfind('}')` would incorrectly match
/// a closing brace from a nested object.
fn extract_json(s: &str) -> &str {
    let s = s.trim();

    // Try to parse as-is first (fast path)
    if s.starts_with('{') && s.ends_with('}') {
        if serde_json::from_str::<serde_json::Value>(s).is_ok() {
            return s;
        }
    }

    // Find balanced JSON object using depth tracking
    let chars: Vec<char> = s.chars().collect();
    let mut depth = 0;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &c) in chars.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match c {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => {
                if start.is_none() {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s_idx) = start {
                        // Found balanced JSON object
                        let end_idx = i + 1;
                        let json_str: String = chars[s_idx..end_idx].iter().collect();
                        // Verify it's valid JSON
                        if serde_json::from_str::<serde_json::Value>(&json_str).is_ok() {
                            // Return slice of original string
                            let byte_start =
                                s.char_indices().nth(s_idx).map(|(idx, _)| idx).unwrap_or(0);
                            let byte_end = s
                                .char_indices()
                                .nth(end_idx)
                                .map(|(idx, _)| idx)
                                .unwrap_or(s.len());
                            return &s[byte_start..byte_end];
                        }
                    }
                    // Reset and continue looking for another object
                    start = None;
                }
            }
            _ => {}
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_provider() {
        let provider = NoopProvider::new();
        assert_eq!(provider.name(), "noop");
        assert!(!provider.is_available());
    }

    #[test]
    fn test_parse_llm_response() {
        let json = r#"{
            "translated": "user authentication function",
            "intent": "definition",
            "rewritten": "user authentication function",
            "expansions": [
                {"text": "getUserAuth", "type": "camel_case", "weight": 0.9},
                {"text": "auth", "type": "abbreviation", "weight": 0.7}
            ],
            "confidence": 0.92
        }"#;

        let response = LlmRewriteResponse::parse(json).unwrap();
        assert_eq!(response.translated, "user authentication function");
        assert_eq!(response.intent, "definition");
        assert_eq!(response.expansions.len(), 2);
        assert_eq!(response.confidence, 0.92);
    }

    #[test]
    fn test_parse_llm_response_with_surrounding_text() {
        let json = r#"Here is the result:
        {"translated": "test", "intent": "general", "rewritten": "test", "expansions": [], "confidence": 0.8}
        Hope this helps!"#;

        let response = LlmRewriteResponse::parse(json).unwrap();
        assert_eq!(response.translated, "test");
    }

    #[test]
    fn test_to_rewritten_query() {
        let response = LlmRewriteResponse {
            translated: "user authentication".to_string(),
            intent: "definition".to_string(),
            rewritten: "user auth".to_string(),
            expansions: vec![LlmExpansion {
                text: "login".to_string(),
                expansion_type: "synonym".to_string(),
                weight: 0.8,
            }],
            confidence: 0.9,
        };

        let original = "用户认证";
        let result = response.to_rewritten_query(original, 100);

        assert_eq!(result.original, original);
        assert_eq!(result.rewritten, "user auth");
        assert!(result.was_translated);
        assert_eq!(result.source_language, Some("zh".to_string()));
        assert_eq!(result.intent, QueryIntent::Definition);
        assert_eq!(result.expansions.len(), 1);
        assert_eq!(result.source, RewriteSource::Llm);
        assert_eq!(result.latency_ms, 100);
    }

    #[test]
    fn test_extract_json() {
        assert_eq!(extract_json(r#"{"a":1}"#), r#"{"a":1}"#);
        assert_eq!(extract_json(r#"Here is JSON: {"a":1} done"#), r#"{"a":1}"#);
        assert_eq!(extract_json(r#"  {"a":1}  "#), r#"{"a":1}"#);
    }

    #[test]
    fn test_extract_json_nested() {
        // Nested object - the old rfind('}') approach would fail here
        let nested = r#"{"outer": {"inner": 1}}"#;
        assert_eq!(extract_json(nested), nested);

        // Deeply nested with surrounding text
        let deep_nested = r#"Result: {"a": {"b": {"c": 1}}} end"#;
        assert_eq!(extract_json(deep_nested), r#"{"a": {"b": {"c": 1}}}"#);

        // Nested with arrays
        let with_array = r#"{"items": [{"id": 1}, {"id": 2}]}"#;
        assert_eq!(extract_json(with_array), with_array);

        // String containing braces (should not confuse depth tracking)
        let string_braces = r#"{"text": "hello { world }"}"#;
        assert_eq!(extract_json(string_braces), string_braces);

        // Escaped quotes in string
        let escaped = r#"{"text": "say \"hello\""}"#;
        assert_eq!(extract_json(escaped), escaped);

        // Multiple objects - should return first valid one
        let multiple = r#"First: {"a":1} Second: {"b":2}"#;
        assert_eq!(extract_json(multiple), r#"{"a":1}"#);
    }

    #[test]
    fn test_extract_json_llm_response_format() {
        // Typical LLM response with nested expansions
        let llm_response = r#"Here is the analysis:
        {
            "translated": "user authentication",
            "intent": "definition",
            "rewritten": "user auth function",
            "expansions": [
                {"text": "auth", "type": "abbreviation", "weight": 0.8},
                {"text": "login", "type": "synonym", "weight": 0.7}
            ],
            "confidence": 0.92
        }
        Hope this helps!"#;

        let extracted = extract_json(llm_response);
        // Should successfully extract the full JSON including nested arrays
        assert!(extracted.contains("\"expansions\""));
        assert!(extracted.contains("\"abbreviation\""));
        assert!(extracted.contains("\"synonym\""));

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(extracted).unwrap();
        assert_eq!(parsed["confidence"], 0.92);
        assert_eq!(parsed["expansions"].as_array().unwrap().len(), 2);
    }
}
