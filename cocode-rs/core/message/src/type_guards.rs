//! Type guards for content blocks and messages.
//!
//! These utilities help identify and extract specific content types from
//! messages, similar to Claude Code's `type-guards.ts`.

use hyper_sdk::ContentBlock;
use hyper_sdk::Message;
use hyper_sdk::Role;
use hyper_sdk::ToolCall;

/// Check if a content block is a text block.
pub fn is_text_block(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::Text { .. })
}

/// Check if a content block is a tool use block.
pub fn is_tool_use_block(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::ToolUse { .. })
}

/// Check if a content block is a tool result block.
pub fn is_tool_result_block(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::ToolResult { .. })
}

/// Check if a content block is a thinking block.
pub fn is_thinking_block(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::Thinking { .. })
}

/// Check if a content block is an image block.
pub fn is_image_block(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::Image { .. })
}

/// Extract text from a text block.
pub fn extract_text(block: &ContentBlock) -> Option<&str> {
    match block {
        ContentBlock::Text { text } => Some(text),
        _ => None,
    }
}

/// Extract thinking content from a thinking block.
pub fn extract_thinking(block: &ContentBlock) -> Option<&str> {
    match block {
        ContentBlock::Thinking { content, .. } => Some(content),
        _ => None,
    }
}

/// Extract tool use details from a tool use block.
pub fn extract_tool_use(block: &ContentBlock) -> Option<(&str, &str, &serde_json::Value)> {
    match block {
        ContentBlock::ToolUse { id, name, input } => Some((id, name, input)),
        _ => None,
    }
}

/// Extract tool result details from a tool result block.
pub fn extract_tool_result(
    block: &ContentBlock,
) -> Option<(&str, &hyper_sdk::ToolResultContent, bool)> {
    match block {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => Some((tool_use_id, content, *is_error)),
        _ => None,
    }
}

/// Check if a message contains any tool use blocks.
pub fn has_tool_use(message: &Message) -> bool {
    message.content.iter().any(is_tool_use_block)
}

/// Check if a message contains any tool result blocks.
pub fn has_tool_result(message: &Message) -> bool {
    message.content.iter().any(is_tool_result_block)
}

/// Check if a message contains any thinking blocks.
pub fn has_thinking(message: &Message) -> bool {
    message.content.iter().any(is_thinking_block)
}

/// Check if a message is empty (no content blocks).
pub fn is_empty_message(message: &Message) -> bool {
    message.content.is_empty()
}

/// Check if a message is a user message.
pub fn is_user_message(message: &Message) -> bool {
    message.role == Role::User
}

/// Check if a message is an assistant message.
pub fn is_assistant_message(message: &Message) -> bool {
    message.role == Role::Assistant
}

/// Check if a message is a system message.
pub fn is_system_message(message: &Message) -> bool {
    message.role == Role::System
}

/// Check if a message is a tool message.
pub fn is_tool_message(message: &Message) -> bool {
    message.role == Role::Tool
}

/// Get all text content from a message.
pub fn get_text_content(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(extract_text)
        .collect::<Vec<_>>()
        .join("")
}

/// Get all tool calls from a message.
pub fn get_tool_calls(message: &Message) -> Vec<ToolCall> {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, name, input } => {
                Some(ToolCall::new(id, name, input.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Get the thinking content from a message if present.
pub fn get_thinking_content(message: &Message) -> Option<String> {
    message.content.iter().find_map(|b| match b {
        ContentBlock::Thinking { content, .. } => Some(content.clone()),
        _ => None,
    })
}

/// Count the number of tool use blocks in a message.
pub fn count_tool_uses(message: &Message) -> usize {
    message
        .content
        .iter()
        .filter(|b| is_tool_use_block(b))
        .count()
}

/// Count the number of tool result blocks in a message.
pub fn count_tool_results(message: &Message) -> usize {
    message
        .content
        .iter()
        .filter(|b| is_tool_result_block(b))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_block(text: &str) -> ContentBlock {
        ContentBlock::text(text)
    }

    fn make_tool_use_block(id: &str, name: &str) -> ContentBlock {
        ContentBlock::tool_use(id, name, serde_json::json!({}))
    }

    fn make_thinking_block(content: &str) -> ContentBlock {
        ContentBlock::Thinking {
            content: content.to_string(),
            signature: None,
        }
    }

    #[test]
    fn test_is_text_block() {
        assert!(is_text_block(&make_text_block("hello")));
        assert!(!is_text_block(&make_tool_use_block("id", "name")));
    }

    #[test]
    fn test_is_tool_use_block() {
        assert!(is_tool_use_block(&make_tool_use_block("id", "name")));
        assert!(!is_tool_use_block(&make_text_block("hello")));
    }

    #[test]
    fn test_is_thinking_block() {
        assert!(is_thinking_block(&make_thinking_block("thinking...")));
        assert!(!is_thinking_block(&make_text_block("hello")));
    }

    #[test]
    fn test_extract_text() {
        assert_eq!(extract_text(&make_text_block("hello")), Some("hello"));
        assert_eq!(extract_text(&make_tool_use_block("id", "name")), None);
    }

    #[test]
    fn test_extract_tool_use() {
        let block = make_tool_use_block("call_1", "get_weather");
        let (id, name, _input) = extract_tool_use(&block).unwrap();
        assert_eq!(id, "call_1");
        assert_eq!(name, "get_weather");
    }

    #[test]
    fn test_has_tool_use() {
        let msg_with_tool = Message::new(
            Role::Assistant,
            vec![
                make_text_block("Let me help"),
                make_tool_use_block("call_1", "get_weather"),
            ],
        );
        assert!(has_tool_use(&msg_with_tool));

        let msg_without_tool = Message::assistant("Just text");
        assert!(!has_tool_use(&msg_without_tool));
    }

    #[test]
    fn test_get_text_content() {
        let msg = Message::new(
            Role::Assistant,
            vec![
                make_text_block("Hello "),
                make_tool_use_block("call_1", "test"),
                make_text_block("world"),
            ],
        );
        assert_eq!(get_text_content(&msg), "Hello world");
    }

    #[test]
    fn test_get_tool_calls() {
        let msg = Message::new(
            Role::Assistant,
            vec![
                make_text_block("Let me check"),
                make_tool_use_block("call_1", "get_weather"),
                make_tool_use_block("call_2", "get_time"),
            ],
        );
        let calls = get_tool_calls(&msg);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[1].name, "get_time");
    }

    #[test]
    fn test_message_role_checks() {
        assert!(is_user_message(&Message::user("hello")));
        assert!(is_assistant_message(&Message::assistant("hi")));
        assert!(is_system_message(&Message::system("instructions")));
    }

    #[test]
    fn test_count_tool_uses() {
        let msg = Message::new(
            Role::Assistant,
            vec![
                make_tool_use_block("call_1", "tool1"),
                make_text_block("text"),
                make_tool_use_block("call_2", "tool2"),
            ],
        );
        assert_eq!(count_tool_uses(&msg), 2);
    }
}
