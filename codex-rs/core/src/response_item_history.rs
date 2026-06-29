use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SearchToolCallParams;
use serde::Deserialize;
use std::borrow::Cow;
use tracing::warn;

const INVALID_TOOL_SEARCH_QUERY: &str = "[invalid tool_search arguments omitted]";

/// Applies response-item-specific normalization before model-visible history and persistence.
pub(super) fn prepare_for_durable_history<'a>(
    thread_id: &ThreadId,
    items: &'a [ResponseItem],
) -> Cow<'a, [ResponseItem]> {
    if !items.iter().any(|item| {
        matches!(
            item,
            ResponseItem::ToolSearchCall { execution, .. } if execution == "client"
        )
    }) {
        return Cow::Borrowed(items);
    }

    Cow::Owned(
        items
            .iter()
            .filter_map(|item| {
                let ResponseItem::ToolSearchCall {
                    id,
                    call_id,
                    status,
                    execution,
                    arguments,
                    internal_chat_message_metadata_passthrough,
                } = item
                else {
                    return Some(item.clone());
                };
                if execution != "client" {
                    return Some(item.clone());
                }

                let Some(call_id) = call_id.as_deref().filter(|call_id| !call_id.is_empty()) else {
                    warn!(
                        %thread_id,
                        "dropping client tool_search call with missing call_id from durable history"
                    );
                    return None;
                };

                let canonical_arguments = match SearchToolCallParams::deserialize(arguments) {
                    Ok(params) => match serde_json::to_value(params) {
                        Ok(canonical_arguments) => canonical_arguments,
                        Err(err) => {
                            warn!(
                                %thread_id,
                                call_id,
                                %err,
                                "failed to serialize canonical client tool_search arguments"
                            );
                            invalid_tool_search_arguments()
                        }
                    },
                    Err(err) => {
                        warn!(
                            %thread_id,
                            call_id,
                            error_category = ?err.classify(),
                            error_line = err.line(),
                            error_column = err.column(),
                            "replacing malformed client tool_search arguments in durable history"
                        );
                        invalid_tool_search_arguments()
                    }
                };

                Some(ResponseItem::ToolSearchCall {
                    id: id.clone(),
                    call_id: Some(call_id.to_string()),
                    status: status.clone(),
                    execution: execution.clone(),
                    arguments: canonical_arguments,
                    internal_chat_message_metadata_passthrough:
                        internal_chat_message_metadata_passthrough.clone(),
                })
            })
            .collect(),
    )
}

fn invalid_tool_search_arguments() -> serde_json::Value {
    serde_json::json!({ "query": INVALID_TOOL_SEARCH_QUERY })
}
