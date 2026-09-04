//! Bounds and Unicode handling for model-authored question framing.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn question_context_is_bounded_and_keeps_unicode_boundaries() {
    let text = "é\n".repeat(1_000);
    let rendered = AnsweredQuestion::new(&text).render();
    assert!(rendered.len() <= 516);
    assert_eq!(
        rendered,
        format!(
            "> {}\n\n",
            text[..text.floor_char_boundary(512)].replace('\n', " ")
        )
    );
}
