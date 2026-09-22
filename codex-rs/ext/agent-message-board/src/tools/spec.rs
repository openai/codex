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

pub(super) fn tool(name: &str, namespace: Option<&str>, namespace_description: &str) -> ToolSpec {
    let (description, fields, required): (&str, &[&str], &[&str]) = match name {
        "create_channel" => (
            "Create a channel shared by this agent tree. Subscribe to new discussion roots by default.",
            &["channel_name", "subscribe"],
            &["channel_name"],
        ),
        "get_channels" => (
            "Find channels by case-insensitive name substring, ordered by activity.",
            &["query", "recent_first", "limit", "cursor"],
            &[],
        ),
        "list_threads" => (
            "List discussion roots with bounded previews; sort by created (default) or activity.",
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
            "Search posts by case-insensitive text substring, newest first. Filters combine. Agent references may be absolute or relative to you.",
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
            "Read a discussion root and newest replies. The root repeats on each page; the cursor advances replies.",
            &["thread_id", "limit", "cursor", "max_chars_per_post"],
            &["thread_id"],
        ),
        "read_post" => (
            "Read part of one post. Offsets and lengths count Unicode characters; continue at next_offset_chars until n_chars.",
            &["message_id", "offset_chars", "limit_chars"],
            &["message_id"],
        ),
        "subscribe" | "unsubscribe" => (
            "Change a channel OR discussion subscription, optionally for another tree member. Channel subscriptions notify new roots; thread subscriptions notify replies. Notices only reach currently running turns, with no later backlog.",
            &["channel_name", "thread_id", "target_agent"],
            &[],
        ),
        "post" => (
            "Post to exactly one existing channel, new channel, or discussion thread. Posting follows the discussion unless you unsubscribed. Explicit agent recipients get a notice without subscribing. Notices never start idle agents. Returns metadata, not the post body.",
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
                json!({"type":"boolean","description":"Subscribe to new roots. Default true."})
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
        description: description.into(),
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
