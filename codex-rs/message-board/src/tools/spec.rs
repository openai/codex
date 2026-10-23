//! Schemas for shared board tools. The host supplies their selection and namespace.

use super::selection::MessageBoardTool;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use codex_tools::parse_tool_input_schema;
use serde_json::json;

pub(super) fn tool(
    tool: MessageBoardTool,
    namespace: Option<&str>,
    namespace_description: &str,
    description_override: Option<&str>,
) -> ToolSpec {
    let (description, fields, required): (&str, &[&str], &[&str]) = match tool {
        MessageBoardTool::CreateChannel => (
            "Create a channel where all agents in this collaboration can read and post messages. You are subscribed to new top-level posts by default.",
            &["channel_name", "description", "subscribe"],
            &["channel_name"],
        ),
        MessageBoardTool::UpsertChannel => (
            "Create a channel or update only its description. Existing posts, author, creation time and subscriptions are preserved. Does not subscribe anyone or send notifications. Returns null on success.",
            &["channel_name", "description"],
            &["channel_name", "description"],
        ),
        MessageBoardTool::GetChannels => (
            "List channels, most recently active first, or search by a case-insensitive part of the name. Creating a channel or posting in it counts as activity.",
            &["query", "recent_first", "limit", "cursor"],
            &[],
        ),
        MessageBoardTool::ListThreads => (
            "List a channel's threads with previews of the first post and latest reply. New threads come first by default; sorting by activity brings threads with recent replies to the top.",
            &[
                "channel_name",
                "sort",
                "recent_first",
                "limit",
                "cursor",
                "max_chars_per_post",
            ],
            &["channel_name"],
        ),
        MessageBoardTool::SearchPosts => (
            "Search top-level posts and replies, newest first. Text matches are case-insensitive substrings; omit the query to see recent activity. You can narrow by channel, author, or posts after a message ID. Results are previews; read_post can retrieve the full text.",
            &[
                "channel_name",
                "query",
                "after_message_id",
                "author",
                "limit",
                "cursor",
                "max_chars_per_post",
            ],
            &[],
        ),
        MessageBoardTool::ReadThread => (
            "Read a thread using its first post's message ID. Every page includes previews of the first post and the newest replies; the cursor advances through replies.",
            &["thread_id", "limit", "cursor", "max_chars_per_post"],
            &["thread_id"],
        ),
        MessageBoardTool::ReadPost => (
            "Read a post or reply by message ID, without needing its channel. Offsets count Unicode characters; continue at next_offset_chars while it is less than n_chars.",
            &["message_id", "offset_chars", "limit_chars"],
            &["message_id"],
        ),
        MessageBoardTool::Subscribe => (
            "Subscribe yourself or another agent to a channel for new top-level posts, or to a thread for replies. Provide exactly one of channel_name or thread_id. Notifications only reach agents with a running turn; missed notifications are not saved.",
            &["channel_name", "thread_id", "target_agent"],
            &[],
        ),
        MessageBoardTool::Unsubscribe => (
            "Unsubscribe yourself or another agent from a channel or thread. Provide exactly one of channel_name or thread_id. Posting again does not undo a thread unsubscribe. Agents explicitly named on a post can still receive that notification.",
            &["channel_name", "thread_id", "target_agent"],
            &[],
        ),
        MessageBoardTool::Post => (
            "Start a thread in an existing or new channel, or reply using the first post's message ID as thread_id. Exactly one destination is required. Posting subscribes you to the thread unless you previously unsubscribed. agents_to_notify sends a one-time notification without subscribing recipients or starting idle agents. Returns metadata, not the post text.",
            &[
                "text",
                "channel_name",
                "new_channel_name",
                "thread_id",
                "agents_to_notify",
            ],
            &["text"],
        ),
        MessageBoardTool::Append => (
            "Store a post in an existing or new channel, or reply using the first post's message ID as thread_id. Exactly one destination is required. Does not subscribe anyone or send notifications, including to existing subscribers. Returns metadata, not the post text.",
            &["text", "channel_name", "new_channel_name", "thread_id"],
            &["text"],
        ),
    };
    let mut properties = serde_json::Map::new();
    for field in fields {
        let schema = match *field {
            "description" if tool == MessageBoardTool::UpsertChannel => {
                json!({"type":"string","description":"Channel purpose, at most 512 UTF-8 bytes."})
            }
            "description" => {
                json!({"type":"string","description":"Optional channel purpose, at most 512 UTF-8 bytes."})
            }
            "new_channel_name" if tool == MessageBoardTool::Append => {
                json!({"type":"string","description":"Create this channel without subscribing anyone."})
            }
            "new_channel_name" => {
                json!({"type":"string","description":"Create and subscribe to this channel."})
            }
            "subscribe" => {
                json!({"type":"boolean","description":"Subscribe to new top-level posts. Default true."})
            }
            "author" | "target_agent" => {
                json!({"type":"string","description":"An absolute agent path or a reference relative to you."})
            }
            "recent_first" => json!({"type":"boolean","description":"Newest first by default."}),
            "limit" => {
                json!({"type":"integer","minimum":1,"description":"Maximum results, default 20; output budgets may return fewer. Continue with next_cursor."})
            }
            "offset_chars" => json!({"type":"integer","minimum":0,"description":"Default 0."}),
            "limit_chars" | "max_chars_per_post" => {
                json!({"type":"integer","minimum":1,"description":"Maximum characters; further capped by output budget. Defaults: limit_chars 20000, max_chars_per_post 1000."})
            }
            "sort" => json!({"type":"string","enum":["created","activity"]}),
            "agents_to_notify" => {
                json!({"type":"array","items":{"type":"string"},"description":"Absolute agent paths or references relative to you; maximum 256."})
            }
            "cursor" => {
                json!({"type":"string","description":"Opaque next_cursor from the same query. Keep filters and sorting unchanged. Concurrent posts may shift pages; omit the cursor to refresh."})
            }
            _ => json!({"type":"string"}),
        };
        properties.insert((*field).into(), schema);
    }
    let parameters = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let tool = ResponsesApiTool {
        name: tool.name().into(),
        description: description_override.unwrap_or(description).into(),
        strict: false,
        defer_loading: None,
        parameters: parse_tool_input_schema(&parameters)
            .unwrap_or_else(|error| panic!("message-board schema must parse: {error}")),
        output_schema: None,
    };
    match namespace {
        Some(namespace) => ToolSpec::Namespace(ResponsesApiNamespace {
            name: namespace.into(),
            description: namespace_description.into(),
            tools: vec![ResponsesApiNamespaceTool::Function(tool)],
        }),
        None => ToolSpec::Function(tool),
    }
}
