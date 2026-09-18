//! Bounds, Unicode handling, and escaping for async question replies.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn question_context_is_bounded_and_keeps_unicode_boundaries() {
    let text = "é\n".repeat(1_000);
    let id = r#"["request_user_input_async","message",1]"#;
    let answer = "A \"quoted\" answer\nwith a second line";
    let fragment = AnsweredQuestion::new(id, &text, answer);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fragment.body()).unwrap(),
        serde_json::json!([{
            "questionItemId": id,
            "question": text[..text.floor_char_boundary(512)].replace('\n', " "),
            "answer": answer,
        }]),
    );
}
