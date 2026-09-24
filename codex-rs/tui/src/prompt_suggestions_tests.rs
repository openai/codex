//! Exercise suggestion parsing and composer input behavior.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn prompt_suggestion_requires_bounded_single_line_json() {
    for response in [
        "null",
        "{}",
        r#"{"suggestion":null}"#,
        r#"{"suggestion":3}"#,
        r#"{"suggestion":""}"#,
        r#"{"suggestion":"a\nb"}"#,
        r#"{"suggestion":"ok","extra":true}"#,
    ] {
        assert_eq!(parse_suggestion(response), None, "{response}");
    }
    for length in [240, 241] {
        let text = "é".repeat(length);
        let response = serde_json::json!({"suggestion":text}).to_string();
        assert_eq!(parse_suggestion(&response), (length == 240).then_some(text));
    }
    assert_eq!(
        parse_suggestion(r#"{"suggestion":"  Add a regression test  "}"#),
        Some("Add a regression test".into())
    );
}
