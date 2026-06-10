use codex_protocol::models::ResponseItem;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Zstd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseItemIdPolicy {
    KeepAll,
    KeepStatefulStoredOnly,
    OmitAll,
}

/// Applies the request-specific Responses API item ID policy after common serialization.
///
/// `ResponseItem` normally serializes every known ID so rollout, app-server, and other internal
/// JSON surfaces preserve item identity. For now, Responses API request modes are the exception
/// to preserve historical behavior: stateless OpenAI requests keep every item ID, whereas
/// stateful Responses API usage keep only the narrower legacy set (only item IDs returned by the
/// server). We can simplify this in the future and always send all IDs once we validate that
/// doing so is safe for stateful usage of Responses API (i.e. Azure).
pub(crate) fn apply_response_item_id_policy(
    payload_json: &mut Value,
    original_items: &[ResponseItem],
    policy: ResponseItemIdPolicy,
) {
    let Some(Value::Array(items)) = payload_json.get_mut("input") else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if !should_keep_response_item_id(item, policy)
            && let Some(obj) = value.as_object_mut()
        {
            obj.remove("id");
        }
    }
}

fn should_keep_response_item_id(item: &ResponseItem, policy: ResponseItemIdPolicy) -> bool {
    if !item.id().is_some_and(|id| !id.is_empty()) {
        return false;
    }

    match policy {
        ResponseItemIdPolicy::KeepAll => true,
        ResponseItemIdPolicy::KeepStatefulStoredOnly => matches!(
            item,
            ResponseItem::Reasoning { .. }
                | ResponseItem::Message { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::ToolSearchCall { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::CustomToolCall { .. }
        ),
        ResponseItemIdPolicy::OmitAll => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn keeps_all_response_item_ids_in_input_json() {
        let items = serde_json::from_str::<Vec<ResponseItem>>(
            r#"[
                {"type":"message","id":"msg_1","role":"user","content":[]},
                {"type":"agent_message","id":"amsg_1","author":"/root","recipient":"/root/worker","content":[]},
                {"type":"reasoning","id":"rs_1","summary":[]},
                {"type":"local_shell_call","id":"lsh_1","call_id":"call_shell","status":"completed","action":{"type":"exec","command":["echo","ok"]}},
                {"type":"function_call","id":"fc_1","name":"shell","arguments":"{}","call_id":"call_function"},
                {"type":"tool_search_call","id":"tsc_1","call_id":"call_search","status":"completed","execution":"client","arguments":{}},
                {"type":"function_call_output","id":"fco_1","call_id":"call_1","output":"ok"},
                {"type":"custom_tool_call","id":"ctc_1","status":"completed","call_id":"call_custom","name":"apply_patch","input":"{}"},
                {"type":"custom_tool_call_output","id":"ctco_1","call_id":"call_custom","name":"apply_patch","output":"ok"},
                {"type":"tool_search_output","id":"tso_1","call_id":"call_search","status":"completed","execution":"client","tools":[]},
                {"type":"web_search_call","id":"ws_1","status":"completed","action":{"type":"search","query":"weather"}},
                {"type":"image_generation_call","id":"ig_1","status":"completed","result":"image"},
                {"type":"compaction","id":"cmp_1","encrypted_content":"opaque"}
            ]"#,
        )
        .expect("deserialize response items");
        let mut payload = serde_json::json!({
            "input": serde_json::to_value(&items).expect("serialize input"),
        });

        apply_response_item_id_policy(&mut payload, &items, ResponseItemIdPolicy::KeepAll);

        let ids = payload["input"]
            .as_array()
            .expect("input array")
            .iter()
            .map(|item| item["id"].as_str().expect("item id"))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "msg_1", "amsg_1", "rs_1", "lsh_1", "fc_1", "tsc_1", "fco_1", "ctc_1", "ctco_1",
                "tso_1", "ws_1", "ig_1", "cmp_1",
            ]
        );
    }

    #[test]
    fn omits_response_item_ids_without_request_opt_in() {
        let items = vec![ResponseItem::Message {
            id: Some("msg_1".to_string()),
            role: "user".to_string(),
            content: Vec::new(),
            phase: None,
        }];
        let mut payload = serde_json::json!({
            "input": serde_json::to_value(&items).expect("serialize input"),
        });

        apply_response_item_id_policy(&mut payload, &items, ResponseItemIdPolicy::OmitAll);

        assert_eq!(payload["input"][0].get("id"), None);
    }
}
