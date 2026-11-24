//! Coverage for exact matching and a lossless projection of the envelope.

use super::without_fallback;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn preserves_the_serialized_fields_around_the_fallback() {
    let raw = r#"{"extra":1e+27,"content":[{"text":" [\"\\u0061\", true] ","type":"text"}],"structuredContent":["a",true],"last":42}"#;
    assert_eq!(
        without_fallback(raw).as_deref(),
        Some(r#"{"extra":1e+27,"structuredContent":["a",true],"last":42}"#)
    );
}

#[test]
fn equivalent_decimal_spellings_keep_the_structured_numbers() {
    let raw = r#"{"content":[{"type":"text","text":"[1.0,1e0,-12.300E+2,0.0120,1000000000000000000000,0.0]"}],"structuredContent":[1,1,-1230,12e-3,1e+21,0]}"#;
    assert_eq!(
        without_fallback(raw).as_deref(),
        Some(r#"{"structuredContent":[1,1,-1230,12e-3,1e+21,0]}"#)
    );
}

#[test]
fn distinct_or_invalid_fallbacks_are_preserved() {
    for text in ["note", "null", "1.1", "1 trailing", "+1", "1,"] {
        let raw = json!({"structuredContent": 1, "content": [{"type":"text","text":text}]});
        assert_eq!(without_fallback(&raw.to_string()), None, "{raw}");
    }
    let raw = r#"{"structuredContent":null,"content":[{"type":"text","text":"null"}]}"#;
    assert_eq!(without_fallback(raw), None);
}

#[test]
fn errors_and_richer_blocks_are_preserved() {
    let text = json!({"type":"text", "text": "{}"});
    for (content, is_error) in [
        (json!([text]), true),
        (json!([text, {"type":"image","data":"abc"}]), false),
        (json!([{"type":"text","text":"{}","annotations":{}}]), false),
        (json!([{"type":"text","text":"{}","_meta":{}}]), false),
        (json!([["text", "{}"]]), false),
    ] {
        let raw = json!({"structuredContent": {}, "content": content, "isError": is_error});
        assert_eq!(without_fallback(&raw.to_string()), None, "{raw}");
    }
}
