//! LLM correction with instruction context and ModelClient integration
//!
//! This module provides LLM-powered correction for failed edit attempts.
//! Uses the instruction parameter for semantic context, improving correction accuracy.

use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::error::CodexErr;
use crate::tools::handlers::ext::smart_edit::common::hash_content;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use lru::LruCache;
use serde_json::json;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tokio::time::timeout;

/// Correction timeout (40 seconds for LLM calls, matches gemini-cli)
const CORRECTION_TIMEOUT: Duration = Duration::from_secs(40);

/// Maximum cache size for LLM correction results (matches gemini-cli)
const MAX_CACHE_SIZE: usize = 50;

/// LRU cache for LLM correction results to avoid redundant API calls
static CORRECTION_CACHE: LazyLock<Mutex<LruCache<String, CorrectedEdit>>> = LazyLock::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(MAX_CACHE_SIZE).expect("MAX_CACHE_SIZE must be > 0"),
    ))
});

/// Result of LLM correction attempt
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorrectedEdit {
    pub search: String,
    pub replace: String,
    #[serde(default)]
    pub no_changes_required: bool,
    pub explanation: String,
}

/// Attempt LLM correction using ModelClient directly
///
/// This function:
/// 1. Builds a prompt with instruction context and JSON schema
/// 2. Calls ModelClient.stream() with timeout wrapper
/// 3. Parses JSON response to extract corrected search/replace
///
/// # Arguments
/// * `client` - ModelClient from invocation.turn.client
/// * `instruction` - Semantic instruction (WHY/WHERE/WHAT/OUTCOME)
/// * `old_string` - Original search string that failed
/// * `new_string` - Original replace string
/// * `file_content` - Current file content
/// * `error_msg` - Error message describing why it failed
pub async fn attempt_llm_correction(
    client: &ModelClient,
    instruction: &str,
    old_string: &str,
    new_string: &str,
    file_content: &str,
    error_msg: &str,
) -> Result<CorrectedEdit, CodexErr> {
    let start = Instant::now();

    // Generate cache key from all inputs
    let cache_key = hash_content(&format!(
        "{instruction}\n{old_string}\n{new_string}\n{file_content}\n{error_msg}"
    ));

    // Check cache first
    if let Some(cached) = CORRECTION_CACHE.lock().unwrap().get(&cache_key) {
        tracing::info!("Smart edit LLM correction: cache hit");
        return Ok(cached.clone());
    }

    // Define JSON schema for structured output
    let correction_schema = json!({
        "type": "object",
        "properties": {
            "search": {
                "type": "string",
                "description": "Corrected search string - exact literal text from file"
            },
            "replace": {
                "type": "string",
                "description": "Corrected replace string - usually unchanged from original"
            },
            "explanation": {
                "type": "string",
                "description": "Brief explanation of what was wrong and how you fixed it"
            },
            "no_changes_required": {
                "type": "boolean",
                "description": "True if the desired change already exists in the file"
            }
        },
        "required": ["search", "replace", "explanation", "no_changes_required"]
    });

    // Build user prompt with instruction context
    let user_prompt = format!(
        r#"# Original Edit Goal
{instruction}

# Failed Search String
```
{old_string}
```

# Replacement String
```
{new_string}
```

# Error
{error_msg}

# Full File Content
```
{file_content}
```

Analyze why the search string didn't match and provide corrected values in JSON format."#
    );

    // Build prompt for ModelClient
    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: user_prompt }],
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions_override: Some(CORRECTION_SYSTEM_PROMPT.to_string()),
        output_schema: Some(correction_schema),
        previous_response_id: None,
    };

    // Stream with timeout wrapper (key pattern from research)
    let result = timeout(CORRECTION_TIMEOUT, async move {
        let mut stream = client
            .stream(&prompt)
            .await
            .map_err(|e| CodexErr::Fatal(format!("LLM stream failed: {e}")))?;

        let mut output = String::new();

        while let Some(event) = stream.next().await {
            match event {
                Ok(ResponseEvent::OutputTextDelta(text)) => {
                    output.push_str(&text);
                }
                Ok(ResponseEvent::Completed { .. }) => break,
                Ok(_) => continue, // Ignore other events
                Err(e) => return Err(CodexErr::Fatal(format!("Stream error: {e}"))),
            }
        }

        Ok(output)
    })
    .await;

    // Handle timeout
    let response = match result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            tracing::error!(
                duration_ms = start.elapsed().as_millis(),
                "Smart edit LLM correction failed"
            );
            return Err(e);
        }
        Err(_) => {
            tracing::error!(
                duration_ms = CORRECTION_TIMEOUT.as_millis(),
                "Smart edit LLM correction timed out"
            );
            return Err(CodexErr::Fatal(
                "LLM correction timed out after 40 seconds".to_string(),
            ));
        }
    };

    // Parse JSON response
    let correction: CorrectedEdit = serde_json::from_str(&response).map_err(|e| {
        tracing::error!(
            duration_ms = start.elapsed().as_millis(),
            error = %e,
            "Smart edit LLM correction: JSON parsing failed"
        );
        CodexErr::Fatal(format!("JSON parsing failed: {e}"))
    })?;

    // Store in cache for future use
    CORRECTION_CACHE
        .lock()
        .unwrap()
        .put(cache_key.clone(), correction.clone());

    tracing::info!(
        duration_ms = start.elapsed().as_millis(),
        no_changes_required = correction.no_changes_required,
        "Smart edit LLM correction succeeded"
    );

    Ok(correction)
}

/// Correct over-escaped new_string when old_string was found
///
/// This function handles cases where the search string matched but the
/// replacement string appears to have LLM over-escaping issues.
///
/// Uses simple heuristic first (unescape_string_for_llm_bug), which handles
/// most cases without needing an LLM call.
///
/// Ported from gemini-cli's `correctNewStringEscaping()`.
///
/// # Arguments
/// * `new_string` - The replacement string that might be over-escaped
///
/// # Returns
/// The corrected replacement string (may be unchanged if no escaping issues found)
pub fn correct_new_string_escaping(new_string: &str) -> String {
    use super::common::unescape_string_for_llm_bug;

    let unescaped = unescape_string_for_llm_bug(new_string);

    // If unescaping changed something, use the unescaped version
    if unescaped != new_string {
        tracing::info!(
            original_len = new_string.len(),
            unescaped_len = unescaped.len(),
            "Smart edit: corrected over-escaped new_string"
        );
        return unescaped;
    }

    // No escaping issues found, return original
    new_string.to_string()
}

/// Check if a string appears to be potentially over-escaped
///
/// Returns true if the string contains patterns that suggest LLM over-escaping:
/// - `\\n`, `\\t`, `\\r` (escaped control characters)
/// - `\\"`, `\\'`, `\\`` (escaped quotes)
/// - `\\\\` (escaped backslash)
pub fn is_potentially_over_escaped(s: &str) -> bool {
    s.contains("\\n")
        || s.contains("\\t")
        || s.contains("\\r")
        || s.contains("\\\"")
        || s.contains("\\'")
        || s.contains("\\`")
        || s.contains("\\\\")
}

/// System prompt for LLM correction
const CORRECTION_SYSTEM_PROMPT: &str = r#"You are an expert code-editing assistant specializing in debugging failed search-and-replace operations.

Your task: Analyze the failed edit using the provided instruction context and provide corrected search/replace strings that will match the file precisely.

**Key Principles:**
1. **Understand Intent**: Use the instruction to understand WHY, WHERE, and WHAT the change should be
2. **Minimal Correction**: Stay close to the original search string, only fix issues like whitespace or escaping
3. **Exact Match**: The new search string must be EXACT literal text from the file
4. **Preserve Replace**: Usually keep the original replace string unchanged unless it has escaping issues
5. **No Changes Case**: If the desired change already exists in the file, set no_changes_required to true

**Common Issues:**
- Over-escaped characters (\\n, \\t, \\" etc) - LLMs often do this
- Whitespace/indentation mismatches
- Missing context lines or wrong context
- Using approximate text instead of exact text from file

**Output Format (JSON):**
{
  "search": "corrected search string - must be exact literal text from file",
  "replace": "corrected replace string - usually unchanged from original",
  "explanation": "brief explanation of what was wrong and how you fixed it",
  "no_changes_required": false
}

**Important**: Your corrected search string must appear EXACTLY in the file content provided. Copy it character-for-character, including all whitespace and indentation."#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_correction_json_valid() {
        let json = r#"{
            "search": "const x = 1;",
            "replace": "const x = 2;",
            "explanation": "Value changed from 1 to 2",
            "no_changes_required": false
        }"#;

        let result: CorrectedEdit = serde_json::from_str(json).expect("Should parse");
        assert_eq!(result.search, "const x = 1;");
        assert_eq!(result.replace, "const x = 2;");
        assert_eq!(result.explanation, "Value changed from 1 to 2");
        assert!(!result.no_changes_required);
    }

    #[test]
    fn test_parse_correction_json_no_changes() {
        let json = r#"{
            "search": "unchanged",
            "replace": "unchanged",
            "explanation": "Already correct",
            "no_changes_required": true
        }"#;

        let result: CorrectedEdit = serde_json::from_str(json).expect("Should parse");
        assert!(result.no_changes_required);
        assert_eq!(result.explanation, "Already correct");
    }

    #[test]
    fn test_parse_correction_json_multiline() {
        let json = r#"{
            "search": "fn test() {\n    old_code();\n}",
            "replace": "fn test() {\n    new_code();\n}",
            "explanation": "Updated function body",
            "no_changes_required": false
        }"#;

        let result: CorrectedEdit = serde_json::from_str(json).expect("Should parse");
        assert!(result.search.contains("fn test()"));
        assert!(result.search.contains("old_code()"));
        assert!(result.replace.contains("new_code()"));
    }

    #[test]
    fn test_parse_correction_json_missing_field() {
        let json = r#"{
            "search": "test",
            "explanation": "Missing replace field"
        }"#;

        let result: Result<CorrectedEdit, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing field"));
    }

    #[test]
    fn test_parse_correction_json_default_no_changes() {
        // Test that no_changes_required defaults to false when missing
        let json = r#"{
            "search": "test",
            "replace": "test2",
            "explanation": "Test"
        }"#;

        let result: CorrectedEdit = serde_json::from_str(json).expect("Should parse");
        assert!(!result.no_changes_required); // Should default to false
    }

    #[test]
    fn test_parse_correction_json_invalid() {
        let json = r#"not valid json{}"#;

        let result: Result<CorrectedEdit, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_corrected_edit_serialization() {
        let edit = CorrectedEdit {
            search: "old text".to_string(),
            replace: "new text".to_string(),
            no_changes_required: false,
            explanation: "Fixed typo".to_string(),
        };

        let json = serde_json::to_string(&edit).expect("Should serialize");
        let parsed: CorrectedEdit = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(parsed.search, edit.search);
        assert_eq!(parsed.replace, edit.replace);
        assert_eq!(parsed.no_changes_required, edit.no_changes_required);
        assert_eq!(parsed.explanation, edit.explanation);
    }

    // Tests for correct_new_string_escaping

    #[test]
    fn test_correct_new_string_escaping_no_change() {
        // No escaping issues - should return unchanged
        let result = correct_new_string_escaping("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_correct_new_string_escaping_newline() {
        // Over-escaped newline should be fixed
        let result = correct_new_string_escaping("line1\\nline2");
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_correct_new_string_escaping_mixed() {
        // Multiple escape sequences
        let result = correct_new_string_escaping("hello\\t\\\"world\\\"\\n");
        assert_eq!(result, "hello\t\"world\"\n");
    }

    // Tests for is_potentially_over_escaped

    #[test]
    fn test_is_potentially_over_escaped_true() {
        assert!(is_potentially_over_escaped("hello\\nworld"));
        assert!(is_potentially_over_escaped("tab\\there"));
        assert!(is_potentially_over_escaped("quote\\\"here"));
        assert!(is_potentially_over_escaped("back\\\\slash"));
    }

    #[test]
    fn test_is_potentially_over_escaped_false() {
        assert!(!is_potentially_over_escaped("hello world"));
        assert!(!is_potentially_over_escaped("normal text"));
        // Actual escaped chars in Rust string literals are not detected
        assert!(!is_potentially_over_escaped("line1\nline2"));
    }
}
