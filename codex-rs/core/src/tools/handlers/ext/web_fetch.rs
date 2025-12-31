//! Web Fetch Handler
//!
//! Fetches URL content, converts HTML to text, and returns to the model.
//! Based on gemini-cli web-fetch.ts design.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

/// Constants from gemini-cli
const URL_FETCH_TIMEOUT_SECS: u64 = 10;
const MAX_CONTENT_LENGTH: usize = 100_000;
const MAX_LINE_WIDTH: usize = 120;

/// Web fetch tool arguments
#[derive(Debug, Clone, Deserialize)]
struct WebFetchArgs {
    url: String,
}

/// Error types for web_fetch operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebFetchErrorType {
    InvalidUrl,
    UnsupportedProtocol,
    NetworkError,
    Timeout,
    HttpError,
    ContentTooLarge,
}

impl WebFetchErrorType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidUrl => "INVALID_URL",
            Self::UnsupportedProtocol => "UNSUPPORTED_PROTOCOL",
            Self::NetworkError => "NETWORK_ERROR",
            Self::Timeout => "TIMEOUT",
            Self::HttpError => "HTTP_ERROR",
            Self::ContentTooLarge => "CONTENT_TOO_LARGE",
        }
    }
}

/// Static HTTP client for connection pooling and efficiency
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(URL_FETCH_TIMEOUT_SECS))
        .user_agent("codex-web-fetch/1.0")
        .build()
        .expect("Failed to create HTTP client")
});

/// Web Fetch Handler
///
/// Fetches content from URLs and converts HTML to plain text.
/// This is a mutating handler - requires approval flow.
pub struct WebFetchHandler;

#[async_trait]
impl ToolHandler for WebFetchHandler {
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
                    "Invalid payload type for web_fetch".to_string(),
                ));
            }
        };

        let args: WebFetchArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // 2. Validate URL is not empty
        if args.url.trim().is_empty() {
            return make_error_response(WebFetchErrorType::InvalidUrl, "URL must not be empty");
        }

        // 3. Parse and validate URL
        let parsed_url = match Url::parse(&args.url) {
            Ok(url) => url,
            Err(e) => {
                return make_error_response(
                    WebFetchErrorType::InvalidUrl,
                    &format!("Invalid URL '{}': {}", args.url, e),
                );
            }
        };

        // 4. Check protocol (http/https only)
        let scheme = parsed_url.scheme();
        if scheme != "http" && scheme != "https" {
            return make_error_response(
                WebFetchErrorType::UnsupportedProtocol,
                &format!(
                    "Unsupported protocol '{}'. Only http and https are supported.",
                    scheme
                ),
            );
        }

        // 5. Transform GitHub blob URLs to raw URLs
        let fetch_url = transform_github_url(parsed_url.as_str());

        // 6. Fetch with static HTTP client (connection pooling)
        let response = match HTTP_CLIENT.get(&fetch_url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                if e.is_timeout() {
                    return make_error_response(
                        WebFetchErrorType::Timeout,
                        &format!("Request timed out after {} seconds", URL_FETCH_TIMEOUT_SECS),
                    );
                }
                return make_error_response(
                    WebFetchErrorType::NetworkError,
                    &format!("Network error: {e}"),
                );
            }
        };

        // 7. Check HTTP status
        let status = response.status();
        if !status.is_success() {
            return make_error_response(
                WebFetchErrorType::HttpError,
                &format!(
                    "HTTP error: {} {}",
                    status.as_u16(),
                    status.canonical_reason().unwrap_or("Unknown")
                ),
            );
        }

        // 8. Check Content-Length to prevent OOM on huge responses
        if let Some(content_length) = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
        {
            // Reject if > 2x max content length (allow some overhead for HTML conversion)
            if content_length > MAX_CONTENT_LENGTH * 2 {
                return make_error_response(
                    WebFetchErrorType::ContentTooLarge,
                    &format!(
                        "Content too large: {} bytes (max: {} bytes)",
                        content_length,
                        MAX_CONTENT_LENGTH * 2
                    ),
                );
            }
        }

        // 9. Get content type
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        // 10. Get response body
        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return make_error_response(
                    WebFetchErrorType::NetworkError,
                    &format!("Failed to read response body: {e}"),
                );
            }
        };

        // 11. Convert HTML to text if needed
        let text_content = if content_type.contains("text/html") || content_type.is_empty() {
            html_to_text(&body)
        } else {
            body
        };

        // 12. Truncate if needed (UTF-8 safe)
        let truncated = if text_content.len() > MAX_CONTENT_LENGTH {
            let truncated_content = truncate_utf8_safe(&text_content, MAX_CONTENT_LENGTH);
            format!(
                "{}\n\n[Content truncated. Showing first {} of {} bytes]",
                truncated_content,
                truncated_content.len(),
                text_content.len()
            )
        } else {
            text_content
        };

        // 13. Return success
        let content = format!("Content from {}:\n\n{}", fetch_url, truncated);
        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}

/// Transform GitHub blob URLs to raw.githubusercontent.com URLs
fn transform_github_url(url: &str) -> String {
    if url.contains("github.com") && url.contains("/blob/") {
        url.replace("github.com", "raw.githubusercontent.com")
            .replace("/blob/", "/")
    } else {
        url.to_string()
    }
}

/// Truncate string at a valid UTF-8 character boundary.
///
/// This prevents panics when slicing multi-byte UTF-8 characters (Chinese, emoji, etc.).
fn truncate_utf8_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the largest valid char boundary <= max_bytes
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

/// Convert HTML to plain text using html2text
fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), MAX_LINE_WIDTH).unwrap_or_else(|_| html.to_string())
}

/// Create standardized error response
fn make_error_response(
    error_type: WebFetchErrorType,
    message: &str,
) -> Result<ToolOutput, FunctionCallError> {
    Ok(ToolOutput::Function {
        content: format!("[{}] {}", error_type.as_str(), message),
        content_items: None,
        success: Some(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_kind() {
        let handler = WebFetchHandler;
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let handler = WebFetchHandler;

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_parse_valid_args() {
        let args: WebFetchArgs =
            serde_json::from_str(r#"{"url": "https://example.com"}"#).expect("should parse");
        assert_eq!(args.url, "https://example.com");
    }

    #[test]
    fn test_parse_invalid_args_missing_url() {
        let result: Result<WebFetchArgs, _> = serde_json::from_str(r#"{"invalid": "json"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_github_url_blob() {
        let url = "https://github.com/user/repo/blob/main/file.txt";
        let transformed = transform_github_url(url);
        assert_eq!(
            transformed,
            "https://raw.githubusercontent.com/user/repo/main/file.txt"
        );
    }

    #[test]
    fn test_transform_github_url_non_blob() {
        let url = "https://github.com/user/repo";
        let transformed = transform_github_url(url);
        assert_eq!(transformed, url);
    }

    #[test]
    fn test_transform_non_github_url() {
        let url = "https://example.com/page";
        let transformed = transform_github_url(url);
        assert_eq!(transformed, url);
    }

    #[test]
    fn test_html_to_text_simple() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_html_to_text_strips_tags() {
        let html = "<p><strong>Bold</strong> and <em>italic</em></p>";
        let text = html_to_text(html);
        assert!(text.contains("Bold"));
        assert!(text.contains("italic"));
        assert!(!text.contains("<strong>"));
        assert!(!text.contains("<em>"));
    }

    #[test]
    fn test_error_type_as_str() {
        assert_eq!(WebFetchErrorType::InvalidUrl.as_str(), "INVALID_URL");
        assert_eq!(
            WebFetchErrorType::UnsupportedProtocol.as_str(),
            "UNSUPPORTED_PROTOCOL"
        );
        assert_eq!(WebFetchErrorType::NetworkError.as_str(), "NETWORK_ERROR");
        assert_eq!(WebFetchErrorType::Timeout.as_str(), "TIMEOUT");
        assert_eq!(WebFetchErrorType::HttpError.as_str(), "HTTP_ERROR");
        assert_eq!(
            WebFetchErrorType::ContentTooLarge.as_str(),
            "CONTENT_TOO_LARGE"
        );
    }

    #[test]
    fn test_make_error_response() {
        let result = make_error_response(WebFetchErrorType::InvalidUrl, "test error").unwrap();
        if let ToolOutput::Function {
            content, success, ..
        } = result
        {
            assert!(content.contains("[INVALID_URL]"));
            assert!(content.contains("test error"));
            assert_eq!(success, Some(false));
        } else {
            panic!("Expected ToolOutput::Function");
        }
    }

    // ========== UTF-8 Truncation Safety Tests ==========

    #[test]
    fn test_truncate_utf8_safe_ascii() {
        let s = "hello world";
        assert_eq!(truncate_utf8_safe(s, 5), "hello");
    }

    #[test]
    fn test_truncate_utf8_safe_multibyte() {
        // Chinese chars: 中 = 3 bytes each (E4 B8 AD)
        let s = "中文测试"; // 12 bytes total (4 chars × 3 bytes)
        let truncated = truncate_utf8_safe(s, 7); // Should cut at char boundary (6 bytes)
        assert_eq!(truncated, "中文"); // 6 bytes, not 7 (avoids split)
        assert_eq!(truncated.len(), 6);
    }

    #[test]
    fn test_truncate_utf8_safe_emoji() {
        // Emoji: 👋 = 4 bytes (F0 9F 91 8B), 🌍 = 4 bytes
        let s = "Hello 👋🌍"; // "Hello " = 6 bytes, 👋 = 4 bytes, 🌍 = 4 bytes = 14 total
        let truncated = truncate_utf8_safe(s, 10); // "Hello " + 👋 = 10 bytes exactly
        assert_eq!(truncated, "Hello 👋");
        assert_eq!(truncated.len(), 10);
    }

    #[test]
    fn test_truncate_utf8_safe_no_truncation() {
        let s = "short";
        assert_eq!(truncate_utf8_safe(s, 100), "short");
    }

    #[test]
    fn test_truncate_utf8_safe_boundary_in_middle_of_char() {
        // Cut at position 7 which is in the middle of 测 (bytes 6-8)
        let s = "中文测试"; // 中=0-2, 文=3-5, 测=6-8, 试=9-11
        let truncated = truncate_utf8_safe(s, 8); // 8 is in middle of 测
        assert_eq!(truncated, "中文"); // Should back up to byte 6
        assert_eq!(truncated.len(), 6);
    }

    #[test]
    fn test_truncation_preserves_utf8_boundary_large() {
        // Simulate truncation at MAX_CONTENT_LENGTH with multibyte content
        let chinese = "测".repeat(50000); // 150,000 bytes
        let truncated = truncate_utf8_safe(&chinese, MAX_CONTENT_LENGTH);
        assert!(truncated.len() <= MAX_CONTENT_LENGTH);
        // Verify it's a valid char boundary
        assert!(chinese.is_char_boundary(truncated.len()));
        // Should not panic when iterating chars
        assert!(truncated.chars().count() > 0);
    }

    #[test]
    fn test_truncate_utf8_safe_empty_string() {
        assert_eq!(truncate_utf8_safe("", 10), "");
    }

    #[test]
    fn test_truncate_utf8_safe_zero_max() {
        assert_eq!(truncate_utf8_safe("hello", 0), "");
    }

    // ========== Static HTTP Client Test ==========

    #[test]
    fn test_static_http_client_is_accessible() {
        // Verify the static client can be accessed without panic
        let _ = &*HTTP_CLIENT;
    }
}
