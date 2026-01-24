//! Micro-compact: Fast tool result compression without API calls.
//!
//! Tier 1 compaction that compresses old tool results to save tokens
//! without requiring an LLM API call. Only eligible tools are compressed.

use super::CompactState;
use super::config::CompactConfig;
use super::token_counter::TokenCounter;
use codex_protocol::models::ResponseItem;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::collections::HashSet;

/// Tools eligible for micro-compact compression.
///
/// Based on Claude Code's pD5 set. Only these tool types will have
/// their results compressed during micro-compact.
pub static ELIGIBLE_TOOLS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "shell",       // Bash command output
        "read",        // File content
        "grep",        // Search results
        "glob",        // File list
        "list_dir",    // Directory listing
        "web_fetch",   // Web content
        "web_search",  // Search results
        "task_output", // Subagent output
    ])
});

/// Configuration for micro-compact.
#[derive(Debug, Clone)]
pub struct MicroCompactConfig {
    /// Minimum token savings required (default: 20,000)
    pub min_tokens_to_save: i64,
    /// Number of recent tool results to keep intact (default: 3)
    pub keep_last_n_tools: i32,
    /// Fixed token estimate per image (default: 2,000)
    pub tokens_per_image: i64,
}

impl Default for MicroCompactConfig {
    fn default() -> Self {
        Self {
            min_tokens_to_save: 20_000,
            keep_last_n_tools: 3,
            tokens_per_image: 2_000,
        }
    }
}

impl From<&CompactConfig> for MicroCompactConfig {
    fn from(config: &CompactConfig) -> Self {
        Self {
            min_tokens_to_save: config.micro_compact_min_tokens_to_save,
            keep_last_n_tools: config.micro_compact_keep_last_n_tools,
            tokens_per_image: config.tokens_per_image,
        }
    }
}

/// Result of a micro-compact operation.
#[derive(Debug, Clone)]
pub struct MicroCompactResult {
    /// Whether micro-compact was effective (saved enough tokens)
    pub was_effective: bool,
    /// Number of tool results compacted
    pub tools_compacted: i32,
    /// Total tokens saved
    pub tokens_saved: i64,
    /// The compacted message list
    pub compacted_items: Vec<ResponseItem>,
}

/// Placeholder text for cleared tool results.
const CLEARED_PLACEHOLDER: &str = "[Old tool result content cleared]";

/// Scanned tool information.
#[allow(dead_code)] // Tool tracking for micro-compaction
#[derive(Debug, Clone)]
struct ToolInfo {
    /// Index of the message containing the tool_use
    message_idx: usize,
    /// Index of the content item within the message
    content_idx: usize,
    /// Tool name (e.g., "shell", "read")
    name: String,
    /// Token count of the result (if result found)
    result_tokens: Option<i64>,
    /// Index of the message containing the tool_result
    result_message_idx: Option<usize>,
    /// Index of the result content item
    result_content_idx: Option<usize>,
}

/// Attempt micro-compact on the message history.
///
/// Matches Claude Code's Si function.
///
/// Algorithm:
/// 1. Scan for tool_use/tool_result pairs from eligible tools
/// 2. Skip already-compacted IDs
/// 3. Calculate tokens per tool_result
/// 4. Keep last N results (LRU-like)
/// 5. Sum potential savings from older results
/// 6. If savings >= min_threshold: replace content with placeholder
/// 7. Track compacted IDs for idempotency
pub fn try_micro_compact(
    messages: &[ResponseItem],
    compact_state: &mut CompactState,
    config: &MicroCompactConfig,
    token_counter: &TokenCounter,
) -> Option<MicroCompactResult> {
    // Step 1: Scan for eligible tool_use/tool_result pairs
    let mut tool_infos: Vec<(String, ToolInfo)> = Vec::new();
    let mut call_id_to_tool: HashMap<String, usize> = HashMap::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        if let ResponseItem::FunctionCall {
            call_id, name, id, ..
        } = msg
        {
            // Check if this is an eligible tool
            if !ELIGIBLE_TOOLS.contains(name.as_str()) {
                continue;
            }
            // Skip already compacted
            if compact_state.compacted_tool_ids.contains(call_id) {
                continue;
            }

            let info = ToolInfo {
                message_idx: msg_idx,
                content_idx: 0, // FunctionCall is its own message
                name: name.clone(),
                result_tokens: None,
                result_message_idx: None,
                result_content_idx: None,
            };
            let idx = tool_infos.len();
            tool_infos.push((call_id.clone(), info));
            call_id_to_tool.insert(call_id.clone(), idx);
            if let Some(id) = id {
                call_id_to_tool.insert(id.clone(), idx);
            }
        }

        // Look for function call outputs
        if let ResponseItem::FunctionCallOutput { call_id, output } = msg {
            if let Some(&idx) = call_id_to_tool.get(call_id) {
                // Check token cache first, then calculate and cache
                let tokens = if let Some(&cached) = compact_state.tool_token_cache.get(call_id) {
                    cached
                } else {
                    let count = token_counter.approximate(&output.content);
                    compact_state
                        .tool_token_cache
                        .insert(call_id.clone(), count);
                    count
                };
                tool_infos[idx].1.result_tokens = Some(tokens);
                tool_infos[idx].1.result_message_idx = Some(msg_idx);
            }
        }
    }

    // Filter to only tools with found results
    let tool_infos: Vec<(String, ToolInfo)> = tool_infos
        .into_iter()
        .filter(|(_, info)| info.result_tokens.is_some())
        .collect();

    if tool_infos.is_empty() {
        return None;
    }

    // Step 2: Identify which to KEEP (last N - LRU-like behavior)
    let keep_count = config.keep_last_n_tools as usize;
    let to_keep: HashSet<String> = tool_infos
        .iter()
        .rev()
        .take(keep_count)
        .map(|(id, _)| id.clone())
        .collect();

    // Step 3: Identify ALL non-kept tools to compress
    let to_compress: Vec<(String, ToolInfo)> = tool_infos
        .into_iter()
        .filter(|(id, _)| !to_keep.contains(id))
        .collect();

    if to_compress.is_empty() {
        return None;
    }

    // Step 4: Calculate total savings
    let savings: i64 = to_compress
        .iter()
        .filter_map(|(_, info)| info.result_tokens)
        .sum();

    // Step 5: Check if compression is worthwhile
    if savings < config.min_tokens_to_save {
        return None;
    }

    // Step 6: Build compacted message list
    let compress_ids: HashSet<String> = to_compress.iter().map(|(id, _)| id.clone()).collect();
    let compress_msg_indices: HashSet<usize> = to_compress
        .iter()
        .filter_map(|(_, info)| info.result_message_idx)
        .collect();

    let mut compacted_items: Vec<ResponseItem> = Vec::with_capacity(messages.len());

    for (idx, msg) in messages.iter().enumerate() {
        if compress_msg_indices.contains(&idx) {
            // This is a function call output that should be compressed
            if let ResponseItem::FunctionCallOutput { call_id, output } = msg {
                if compress_ids.contains(call_id) {
                    // Replace with placeholder
                    compacted_items.push(ResponseItem::FunctionCallOutput {
                        call_id: call_id.clone(),
                        output: codex_protocol::models::FunctionCallOutputPayload {
                            content: CLEARED_PLACEHOLDER.to_string(),
                            content_items: None,
                            success: output.success,
                        },
                    });
                    continue;
                }
            }
        }
        compacted_items.push(msg.clone());
    }

    // Step 7: Track compacted IDs
    for (id, _) in &to_compress {
        compact_state.compacted_tool_ids.insert(id.clone());
    }

    Some(MicroCompactResult {
        was_effective: true,
        tools_compacted: to_compress.len() as i32,
        tokens_saved: savings,
        compacted_items,
    })
}

/// Check if a tool name is eligible for micro-compact.
#[allow(dead_code)]
pub fn is_eligible_tool(name: &str) -> bool {
    ELIGIBLE_TOOLS.contains(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;

    fn make_function_call(call_id: &str, name: &str) -> ResponseItem {
        ResponseItem::FunctionCall {
            id: Some(format!("fc_{call_id}")),
            name: name.to_string(),
            arguments: "{}".to_string(),
            call_id: call_id.to_string(),
        }
    }

    fn make_function_output(call_id: &str, content: &str) -> ResponseItem {
        ResponseItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                content: content.to_string(),
                content_items: None,
                success: Some(true),
            },
        }
    }

    #[test]
    fn eligible_tools_are_defined() {
        assert!(is_eligible_tool("shell"));
        assert!(is_eligible_tool("read"));
        assert!(is_eligible_tool("grep"));
        assert!(!is_eligible_tool("write"));
        assert!(!is_eligible_tool("unknown"));
    }

    #[test]
    fn micro_compact_skips_when_not_enough_savings() {
        let messages = vec![
            make_function_call("call1", "shell"),
            make_function_output("call1", "short output"),
        ];

        let mut state = CompactState::default();
        let config = MicroCompactConfig {
            min_tokens_to_save: 1000, // High threshold
            keep_last_n_tools: 3,
            tokens_per_image: 2000,
        };
        let token_counter = TokenCounter::default();

        let result = try_micro_compact(&messages, &mut state, &config, &token_counter);
        assert!(result.is_none());
    }

    #[test]
    fn micro_compact_keeps_last_n_tools() {
        // Create 5 tool calls, config keeps last 3
        let long_output = "x".repeat(10000);
        let messages = vec![
            make_function_call("call1", "shell"),
            make_function_output("call1", &long_output),
            make_function_call("call2", "shell"),
            make_function_output("call2", &long_output),
            make_function_call("call3", "shell"),
            make_function_output("call3", &long_output),
            make_function_call("call4", "shell"),
            make_function_output("call4", &long_output),
            make_function_call("call5", "shell"),
            make_function_output("call5", &long_output),
        ];

        let mut state = CompactState::default();
        let config = MicroCompactConfig {
            min_tokens_to_save: 1000,
            keep_last_n_tools: 3,
            tokens_per_image: 2000,
        };
        let token_counter = TokenCounter::default();

        let result = try_micro_compact(&messages, &mut state, &config, &token_counter);
        assert!(result.is_some());

        let result = result.unwrap();
        assert!(result.was_effective);
        assert_eq!(result.tools_compacted, 2); // First 2 compressed, last 3 kept
    }

    #[test]
    fn micro_compact_skips_ineligible_tools() {
        let long_output = "x".repeat(10000);
        let messages = vec![
            make_function_call("call1", "write"), // Not eligible
            make_function_output("call1", &long_output),
        ];

        let mut state = CompactState::default();
        let config = MicroCompactConfig::default();
        let token_counter = TokenCounter::default();

        let result = try_micro_compact(&messages, &mut state, &config, &token_counter);
        assert!(result.is_none());
    }

    #[test]
    fn micro_compact_is_idempotent() {
        let long_output = "x".repeat(10000);
        let messages = vec![
            make_function_call("call1", "shell"),
            make_function_output("call1", &long_output),
            make_function_call("call2", "shell"),
            make_function_output("call2", &long_output),
        ];

        let mut state = CompactState::default();
        let config = MicroCompactConfig {
            min_tokens_to_save: 1000,
            keep_last_n_tools: 1, // Keep only last
            tokens_per_image: 2000,
        };
        let token_counter = TokenCounter::default();

        // First run should compact
        let result1 = try_micro_compact(&messages, &mut state, &config, &token_counter);
        assert!(result1.is_some());
        assert_eq!(state.compacted_tool_ids.len(), 1);

        // Second run should skip already-compacted
        let result2 = try_micro_compact(&messages, &mut state, &config, &token_counter);
        assert!(result2.is_none()); // Nothing new to compact
    }

    #[test]
    fn micro_compact_replaces_with_placeholder() {
        let long_output = "x".repeat(100000);
        let messages = vec![
            make_function_call("call1", "shell"),
            make_function_output("call1", &long_output),
            make_function_call("call2", "shell"),
            make_function_output("call2", "short"),
        ];

        let mut state = CompactState::default();
        let config = MicroCompactConfig {
            min_tokens_to_save: 1000,
            keep_last_n_tools: 1, // Keep only last
            tokens_per_image: 2000,
        };
        let token_counter = TokenCounter::default();

        let result = try_micro_compact(&messages, &mut state, &config, &token_counter);
        assert!(result.is_some());

        let result = result.unwrap();
        // Check that call1's output was replaced
        if let ResponseItem::FunctionCallOutput { call_id, output } = &result.compacted_items[1] {
            assert_eq!(call_id, "call1");
            assert_eq!(output.content, CLEARED_PLACEHOLDER);
        } else {
            panic!("expected FunctionCallOutput");
        }

        // Check that call2's output was preserved
        if let ResponseItem::FunctionCallOutput { call_id, output } = &result.compacted_items[3] {
            assert_eq!(call_id, "call2");
            assert_eq!(output.content, "short");
        } else {
            panic!("expected FunctionCallOutput");
        }
    }

    #[test]
    fn micro_compact_uses_token_cache() {
        let long_output = "x".repeat(10000);
        let messages = vec![
            make_function_call("call1", "shell"),
            make_function_output("call1", &long_output),
            make_function_call("call2", "shell"),
            make_function_output("call2", &long_output),
        ];

        let mut state = CompactState::default();
        let config = MicroCompactConfig {
            min_tokens_to_save: 1000,
            keep_last_n_tools: 1,
            tokens_per_image: 2000,
        };
        let token_counter = TokenCounter::default();

        // After micro-compact, token counts should be cached
        let _ = try_micro_compact(&messages, &mut state, &config, &token_counter);

        // Verify tokens were cached
        assert!(state.tool_token_cache.contains_key("call1"));
        assert!(state.tool_token_cache.contains_key("call2"));
        assert!(state.tool_token_cache.get("call1").unwrap() > &0);
    }
}
