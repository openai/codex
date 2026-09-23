//! Schemas for shared discussion tools. The host supplies their namespace.

use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use codex_tools::parse_tool_input_schema;
use serde_json::json;

pub(super) const NAMES: [&str; 9] = [
    "create_channel",
    "get_channels",
    "list_threads",
    "search_posts",
    "read_thread",
    "read_post",
    "subscribe",
    "unsubscribe",
    "post",
];

pub(super) fn tool(
    name: &str,
    namespace: Option<&str>,
    namespace_description: &str,
    description_override: Option<&str>,
) -> ToolSpec {
    let (description, fields, required): (&str, &[&str], &[&str]) = match name {
        "create_channel" => (
            "Create a channel where all agents in this collaboration can read and post messages. You are subscribed to new top-level posts by default.",
            &["channel_name", "subscribe"],
            &["channel_name"],
        ),
        "get_channels" => (
            "List channels, most recently active first, or search by a case-insensitive part of the name. Creating a channel or posting in it counts as activity.",
            &["query", "recent_first", "limit", "cursor"],
            &[],
        ),
        "list_threads" => (
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
        "search_posts" => (
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
        "read_thread" => (
            "Read a thread using its first post's message ID. Every page includes previews of the first post and the newest replies; the cursor advances through replies.",
            &["thread_id", "limit", "cursor", "max_chars_per_post"],
            &["thread_id"],
        ),
        "read_post" => (
            "Read a post or reply by message ID, without needing its channel. Offsets count Unicode characters; continue at next_offset_chars while it is less than n_chars.",
            &["message_id", "offset_chars", "limit_chars"],
            &["message_id"],
        ),
        "subscribe" => (
            "Subscribe yourself or another agent to a channel for new top-level posts, or to a thread for replies. Provide exactly one of channel_name or thread_id. Notifications only reach agents with a running turn; missed notifications are not saved.",
            &["channel_name", "thread_id", "target_agent"],
            &[],
        ),
        "unsubscribe" => (
            "Unsubscribe yourself or another agent from a channel or thread. Provide exactly one of channel_name or thread_id. Posting again does not undo a thread unsubscribe. Agents explicitly named on a post can still receive that notification.",
            &["channel_name", "thread_id", "target_agent"],
            &[],
        ),
        "post" => (
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
        _ => unreachable!("only registered message-board tools have schemas"),
    };
    let mut properties = serde_json::Map::new();
    for field in fields {
        let schema = match *field {
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
        name: name.into(),
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
