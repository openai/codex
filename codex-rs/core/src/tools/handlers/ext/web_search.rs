//! Web Search Handler
//!
//! Executes web searches via DuckDuckGo or Tavily backends.
//! Returns formatted markdown results with citation markers.
//! Includes LRU cache with TTL to reduce API calls.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::config_types_ext::WebSearchConfig;
use codex_protocol::config_types_ext::WebSearchProvider;
use lru::LruCache;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

// Constants
const SEARCH_TIMEOUT_SECS: u64 = 15;
const CACHE_SIZE: usize = 100;
const CACHE_TTL_SECS: u64 = 15 * 60; // 15 minutes

/// Static HTTP client for connection pooling
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS))
        .user_agent("codex-web-search/1.0")
        .build()
        .expect("Failed to create HTTP client")
});

/// Cached search result with timestamp
struct CachedResult {
    response: SearchResponse,
    cached_at: Instant,
}

/// LRU cache for search results with TTL
static SEARCH_CACHE: LazyLock<Mutex<LruCache<String, CachedResult>>> = LazyLock::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(CACHE_SIZE).expect("CACHE_SIZE must be > 0"),
    ))
});

/// Get cached search result if not expired
fn get_cached(
    query: &str,
    provider: WebSearchProvider,
    max_results: usize,
) -> Option<SearchResponse> {
    let key = format!("{provider:?}:{max_results}:{query}");
    let mut cache = SEARCH_CACHE.lock().ok()?;
    if let Some(cached) = cache.get(&key) {
        if cached.cached_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
            tracing::debug!("web_search cache hit for: {}", query);
            return Some(cached.response.clone());
        }
        // Expired - remove from cache
        cache.pop(&key);
    }
    None
}

/// Store search result in cache
fn set_cached(
    query: &str,
    provider: WebSearchProvider,
    max_results: usize,
    response: SearchResponse,
) {
    let key = format!("{provider:?}:{max_results}:{query}");
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
        cache.put(
            key,
            CachedResult {
                response,
                cached_at: Instant::now(),
            },
        );
    }
}

/// Error types for web_search operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebSearchErrorType {
    InvalidQuery,
    ProviderError,
    NetworkError,
    Timeout,
    RateLimited,
    ApiKeyMissing,
    ParseError,
}

impl WebSearchErrorType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidQuery => "INVALID_QUERY",
            Self::ProviderError => "PROVIDER_ERROR",
            Self::NetworkError => "NETWORK_ERROR",
            Self::Timeout => "TIMEOUT",
            Self::RateLimited => "RATE_LIMITED",
            Self::ApiKeyMissing => "API_KEY_MISSING",
            Self::ParseError => "PARSE_ERROR",
        }
    }
}

/// Web search tool arguments
#[derive(Debug, Clone, Deserialize)]
struct WebSearchArgs {
    query: String,
    #[serde(default)]
    max_results: Option<i64>,
}

/// A single search result
#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Search response with results and metadata
#[derive(Debug, Clone)]
struct SearchResponse {
    results: Vec<SearchResult>,
    query: String,
}

/// Web Search Handler
///
/// Searches the web using configured provider (DuckDuckGo or Tavily).
/// This is a mutating handler - requires approval before execution.
pub struct WebSearchHandler {
    config: WebSearchConfig,
}

impl WebSearchHandler {
    pub fn new(config: WebSearchConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for WebSearchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    /// Mark as mutating - requires approval before execution
    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for web_search".to_string(),
                ));
            }
        };

        let args: WebSearchArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // 2. Validate query
        let query = args.query.trim();
        if query.is_empty() {
            return make_error_response(
                WebSearchErrorType::InvalidQuery,
                "Query must not be empty",
            );
        }

        // 3. Determine max_results (clamp to valid range)
        let max_results = args
            .max_results
            .map(|n| n as usize)
            .unwrap_or(self.config.max_results)
            .clamp(1, 20);

        // 4. Check cache first
        if let Some(cached_response) = get_cached(query, self.config.provider, max_results) {
            let content = format_search_results(&cached_response);
            return Ok(ToolOutput::Function {
                content,
                content_items: None,
                success: Some(true),
            });
        }

        // 5. Execute search based on provider
        let response = match self.config.provider {
            WebSearchProvider::DuckDuckGo => search_duckduckgo(query, max_results).await,
            WebSearchProvider::Tavily => search_tavily(query, max_results, &self.config).await,
            WebSearchProvider::OpenAI => {
                // OpenAI native search not implemented - use DuckDuckGo fallback
                search_duckduckgo(query, max_results).await
            }
        };

        // 6. Format and return results, cache successful responses
        match response {
            Ok(search_response) => {
                // Cache successful result
                set_cached(
                    query,
                    self.config.provider,
                    max_results,
                    search_response.clone(),
                );
                let content = format_search_results(&search_response);
                Ok(ToolOutput::Function {
                    content,
                    content_items: None,
                    success: Some(true),
                })
            }
            Err((error_type, message)) => make_error_response(error_type, &message),
        }
    }
}

/// URL-encode a query string for use in URLs
fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// DuckDuckGo search implementation using HTML scraping
async fn search_duckduckgo(
    query: &str,
    max_results: usize,
) -> Result<SearchResponse, (WebSearchErrorType, String)> {
    // DuckDuckGo HTML search URL
    let url = format!("https://html.duckduckgo.com/html/?q={}", url_encode(query));

    let response = HTTP_CLIENT
        .get(&url)
        .header("Accept", "text/html")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                (WebSearchErrorType::Timeout, "Search timed out".to_string())
            } else {
                (
                    WebSearchErrorType::NetworkError,
                    format!("Network error: {e}"),
                )
            }
        })?;

    if !response.status().is_success() {
        return Err((
            WebSearchErrorType::ProviderError,
            format!("DuckDuckGo returned status {}", response.status()),
        ));
    }

    let html = response.text().await.map_err(|e| {
        (
            WebSearchErrorType::ParseError,
            format!("Failed to read response: {e}"),
        )
    })?;

    // Parse HTML to extract results
    parse_duckduckgo_html(&html, query, max_results)
}

/// Parse DuckDuckGo HTML results page
fn parse_duckduckgo_html(
    html: &str,
    query: &str,
    max_results: usize,
) -> Result<SearchResponse, (WebSearchErrorType, String)> {
    let mut results = Vec::new();

    // DuckDuckGo HTML structure uses <a class="result__a"> for links
    // and <a class="result__snippet"> for snippets
    // We'll use a simple regex-based approach

    // Pattern for result links: class="result__a" href="..." followed by link text
    let link_re =
        regex_lite::Regex::new(r#"class="result__a"[^>]*href="([^"]+)"[^>]*>([^<]+)</a>"#).unwrap();

    // Pattern for snippets
    let snippet_re = regex_lite::Regex::new(r#"class="result__snippet"[^>]*>([^<]+)"#).unwrap();

    let mut links: Vec<(String, String)> = Vec::new();
    for cap in link_re.captures_iter(html) {
        let url = decode_duckduckgo_url(&cap[1]);
        let title = html_entities_decode(&cap[2]);
        if !url.is_empty() && !title.is_empty() {
            links.push((url, title));
        }
    }

    let snippets: Vec<String> = snippet_re
        .captures_iter(html)
        .map(|cap| html_entities_decode(&cap[1]))
        .collect();

    // Combine links and snippets
    for (idx, (url, title)) in links.into_iter().take(max_results).enumerate() {
        let snippet = snippets.get(idx).cloned().unwrap_or_default();
        results.push(SearchResult {
            title,
            url,
            snippet,
        });
    }

    Ok(SearchResponse {
        results,
        query: query.to_string(),
    })
}

/// Simple percent-decode for URL paths
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

/// Decode DuckDuckGo redirect URLs
fn decode_duckduckgo_url(encoded: &str) -> String {
    // DuckDuckGo uses //duckduckgo.com/l/?uddg=URL format
    if let Some(uddg_start) = encoded.find("uddg=") {
        let url_part = &encoded[uddg_start + 5..];
        if let Some(end) = url_part.find('&') {
            return percent_decode(&url_part[..end]);
        }
        return percent_decode(url_part);
    }
    // If not a redirect URL, return as-is
    encoded.to_string()
}

/// Tavily API response types
#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
    #[allow(dead_code)]
    score: f64,
}

/// Tavily search implementation using REST API
async fn search_tavily(
    query: &str,
    max_results: usize,
    config: &WebSearchConfig,
) -> Result<SearchResponse, (WebSearchErrorType, String)> {
    // Get API key: config takes precedence, then env var
    let api_key = config
        .api_key
        .clone()
        .or_else(|| std::env::var("TAVILY_API_KEY").ok())
        .ok_or_else(|| {
            (
                WebSearchErrorType::ApiKeyMissing,
                "TAVILY_API_KEY not set. Configure in [tools.web_search_config] api_key = \"...\" \
                 or set TAVILY_API_KEY env var. Get key at https://tavily.com"
                    .to_string(),
            )
        })?;

    let request_body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": max_results,
        "search_depth": "basic",
        "include_answer": false,
        "include_raw_content": false,
    });

    let response = HTTP_CLIENT
        .post("https://api.tavily.com/search")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                (WebSearchErrorType::Timeout, "Search timed out".to_string())
            } else {
                (
                    WebSearchErrorType::NetworkError,
                    format!("Network error: {e}"),
                )
            }
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err((
            WebSearchErrorType::RateLimited,
            "Tavily API rate limit exceeded".to_string(),
        ));
    }
    if !status.is_success() {
        return Err((
            WebSearchErrorType::ProviderError,
            format!("Tavily API returned status {}", status),
        ));
    }

    let tavily_response: TavilyResponse = response.json().await.map_err(|e| {
        (
            WebSearchErrorType::ParseError,
            format!("Failed to parse response: {e}"),
        )
    })?;

    let results = tavily_response
        .results
        .into_iter()
        .map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.content,
        })
        .collect();

    Ok(SearchResponse {
        results,
        query: query.to_string(),
    })
}

/// Format search results as markdown with citation markers
fn format_search_results(response: &SearchResponse) -> String {
    if response.results.is_empty() {
        return format!(
            "No search results found for \"{}\"\n\n\
             Try rephrasing your query or using different keywords.",
            response.query
        );
    }

    let mut output = format!("Web search results for \"{}\":\n\n", response.query);

    // Format each result with citation marker
    for (idx, result) in response.results.iter().enumerate() {
        let citation_num = idx + 1;
        output.push_str(&format!(
            "[{citation_num}] **{}**\n{}\nSource: {}\n\n",
            result.title,
            result.snippet.trim(),
            result.url
        ));
    }

    // Add sources footer
    output.push_str("Sources:\n");
    for (idx, result) in response.results.iter().enumerate() {
        output.push_str(&format!(
            "[{}] {} ({})\n",
            idx + 1,
            result.title,
            result.url
        ));
    }

    output
}

/// Create standardized error response
fn make_error_response(
    error_type: WebSearchErrorType,
    message: &str,
) -> Result<ToolOutput, FunctionCallError> {
    Ok(ToolOutput::Function {
        content: format!("[{}] {}", error_type.as_str(), message),
        content_items: None,
        success: Some(false),
    })
}

/// Decode HTML entities
fn html_entities_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_kind() {
        let config = WebSearchConfig::default();
        let handler = WebSearchHandler::new(config);
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let config = WebSearchConfig::default();
        let handler = WebSearchHandler::new(config);

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_parse_valid_args() {
        let args: WebSearchArgs =
            serde_json::from_str(r#"{"query": "rust programming"}"#).expect("should parse");
        assert_eq!(args.query, "rust programming");
        assert!(args.max_results.is_none());
    }

    #[test]
    fn test_parse_args_with_max_results() {
        let args: WebSearchArgs =
            serde_json::from_str(r#"{"query": "test", "max_results": 10}"#).expect("should parse");
        assert_eq!(args.query, "test");
        assert_eq!(args.max_results, Some(10));
    }

    #[test]
    fn test_decode_duckduckgo_url() {
        let encoded = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=abc";
        let decoded = decode_duckduckgo_url(encoded);
        assert_eq!(decoded, "https://example.com");
    }

    #[test]
    fn test_decode_duckduckgo_url_direct() {
        let url = "https://example.com/page";
        let decoded = decode_duckduckgo_url(url);
        assert_eq!(decoded, url);
    }

    #[test]
    fn test_format_search_results_with_results() {
        let response = SearchResponse {
            query: "rust programming".to_string(),
            results: vec![
                SearchResult {
                    title: "Rust Programming".to_string(),
                    url: "https://rust-lang.org".to_string(),
                    snippet: "A language empowering everyone".to_string(),
                },
                SearchResult {
                    title: "Learn Rust".to_string(),
                    url: "https://doc.rust-lang.org".to_string(),
                    snippet: "Official documentation".to_string(),
                },
            ],
        };

        let formatted = format_search_results(&response);
        assert!(formatted.contains("[1]"));
        assert!(formatted.contains("[2]"));
        assert!(formatted.contains("Sources:"));
        assert!(formatted.contains("rust-lang.org"));
        assert!(formatted.contains("Rust Programming"));
    }

    #[test]
    fn test_format_empty_results() {
        let response = SearchResponse {
            query: "xyznonexistent".to_string(),
            results: vec![],
        };

        let formatted = format_search_results(&response);
        assert!(formatted.contains("No search results found"));
        assert!(formatted.contains("xyznonexistent"));
    }

    #[test]
    fn test_error_type_as_str() {
        assert_eq!(WebSearchErrorType::InvalidQuery.as_str(), "INVALID_QUERY");
        assert_eq!(
            WebSearchErrorType::ApiKeyMissing.as_str(),
            "API_KEY_MISSING"
        );
        assert_eq!(WebSearchErrorType::NetworkError.as_str(), "NETWORK_ERROR");
        assert_eq!(WebSearchErrorType::Timeout.as_str(), "TIMEOUT");
        assert_eq!(WebSearchErrorType::RateLimited.as_str(), "RATE_LIMITED");
        assert_eq!(WebSearchErrorType::ProviderError.as_str(), "PROVIDER_ERROR");
        assert_eq!(WebSearchErrorType::ParseError.as_str(), "PARSE_ERROR");
    }

    #[test]
    fn test_make_error_response() {
        let result = make_error_response(WebSearchErrorType::InvalidQuery, "test error").unwrap();
        if let ToolOutput::Function {
            content, success, ..
        } = result
        {
            assert!(content.contains("[INVALID_QUERY]"));
            assert!(content.contains("test error"));
            assert_eq!(success, Some(false));
        } else {
            panic!("Expected ToolOutput::Function");
        }
    }

    #[test]
    fn test_html_entities_decode() {
        assert_eq!(html_entities_decode("&amp;"), "&");
        assert_eq!(html_entities_decode("&lt;tag&gt;"), "<tag>");
        assert_eq!(html_entities_decode("&quot;quoted&quot;"), "\"quoted\"");
        assert_eq!(html_entities_decode("it&#39;s"), "it's");
        assert_eq!(html_entities_decode("a&nbsp;b"), "a b");
    }

    #[test]
    fn test_static_http_client_is_accessible() {
        // Verify the static client can be accessed without panic
        let _ = &*HTTP_CLIENT;
    }
}
