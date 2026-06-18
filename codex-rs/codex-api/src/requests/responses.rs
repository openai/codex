use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Zstd,
}

pub(crate) fn strip_response_item_ids(payload_json: &mut Value) {
    let Some(Value::Array(items)) = payload_json.get_mut("input") else {
        return;
    };

    for item in items {
        if let Value::Object(object) = item {
            object.remove("id");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn strip_response_item_ids_removes_ids_from_input_items() {
        let mut payload = json!({
            "model": "gpt-test",
            "id": "request-id",
            "input": [
                {
                    "type": "message",
                    "id": "msg_1",
                    "content": [
                        {"type": "input_text", "text": "hello"},
                        {"type": "input_image", "id": "img_1", "image_url": "https://example.com/image.png"}
                    ]
                },
                {"type": "function_call_output", "id": "fco_1", "call_id": "call_1", "output": "done"}
            ]
        });

        strip_response_item_ids(&mut payload);

        assert_eq!(
            payload,
            json!({
                "model": "gpt-test",
                "id": "request-id",
                "input": [
                    {
                        "type": "message",
                        "content": [
                            {"type": "input_text", "text": "hello"},
                            {"type": "input_image", "id": "img_1", "image_url": "https://example.com/image.png"}
                        ]
                    },
                    {"type": "function_call_output", "call_id": "call_1", "output": "done"}
                ]
            })
        );
    }

    #[test]
    fn strip_response_item_ids_ignores_missing_input() {
        let mut payload = json!({"id": "request-id"});

        strip_response_item_ids(&mut payload);

        assert_eq!(payload, json!({"id": "request-id"}));
    }
}
