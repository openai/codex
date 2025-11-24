use rmcp::model::{CallToolResult, Content, Meta};
use serde_json::{Value, json};

#[test]
fn serialize_tool_result_with_meta() {
    let content = vec![Content::text("ok")];
    let mut meta = Meta::new();
    meta.insert("foo".to_string(), json!("bar"));
    let result = CallToolResult {
        content,
        structured_content: None,
        is_error: Some(false),
        meta: Some(meta),
    };
    let v = serde_json::to_value(&result).unwrap();
    let expected = json!({
        "content": [{"type":"text","text":"ok"}],
        "isError": false,
        "_meta": {"foo":"bar"}
    });
    assert_eq!(v, expected);
}

#[test]
fn deserialize_tool_result_with_meta() {
    let raw: Value = json!({
        "content": [{"type":"text","text":"hello"}],
        "isError": true,
        "_meta": {"a": 1, "b": "two"}
    });
    let result: CallToolResult = serde_json::from_value(raw).unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.content.len(), 1);
    let meta = result.meta.expect("meta should exist");
    assert_eq!(meta.get("a").unwrap(), &json!(1));
    assert_eq!(meta.get("b").unwrap(), &json!("two"));
}

#[test]
fn serialize_tool_result_without_meta_omits_field() {
    let result = CallToolResult::success(vec![Content::text("no meta")]);
    let v = serde_json::to_value(&result).unwrap();
    // Ensure _meta is omitted
    assert!(v.get("_meta").is_none());
}
