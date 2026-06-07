use codex_protocol::models::ResponseItem;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Zstd,
}

/// Reattaches the item IDs historically sent with stored/stateful Responses requests.
///
/// `ResponseItem` IDs are skipped by serde so request modes can choose their attachment policy
/// after serializing the common request body. Stateful requests use the legacy narrower policy:
/// attach IDs for prompt items the stateful Responses path may treat as existing stored items,
/// but do not opt every Codex-owned output item into the newer stateless stable-ID behavior.
pub(crate) fn attach_stateful_response_item_ids(
    payload_json: &mut Value,
    original_items: &[ResponseItem],
) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let Value::Array(items) = input_value else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let ResponseItem::Reasoning { id, .. }
        | ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::ToolSearchCall { id: Some(id), .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. } = item
        {
            if id.is_empty() {
                continue;
            }

            if let Some(obj) = value.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
        }
    }
}

/// Reattaches every non-empty stable item ID before sending a stateless Responses request.
///
/// Stateless requests resend the full prompt history by value, so the server only sees stable
/// item identity when Codex explicitly puts each known ID back into the serialized `input` array.
/// This includes Codex-generated IDs for new local prompt items and IDs preserved from server
/// output, compaction, and rollout replay.
pub(crate) fn attach_stateless_response_item_ids(
    payload_json: &mut Value,
    original_items: &[ResponseItem],
) {
    let Some(Value::Array(items)) = payload_json.get_mut("input") else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        item.attach_id_to_json(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn attaches_stateless_response_item_ids_to_input_json() {
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

        attach_stateless_response_item_ids(&mut payload, &items);

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
    fn attach_stateless_response_item_ids_leaves_missing_ids_unchanged() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: Vec::new(),
            phase: None,
        }];
        let mut payload = serde_json::json!({
            "input": serde_json::to_value(&items).expect("serialize input"),
        });

        attach_stateless_response_item_ids(&mut payload, &items);

        assert_eq!(payload["input"][0].get("id"), None);
    }
}
