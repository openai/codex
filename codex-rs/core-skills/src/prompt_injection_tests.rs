use pretty_assertions::assert_eq;

use super::MAX_SKILL_PROMPT_BYTES;
use super::bounded_skill_prompt_contents;

#[test]
fn skill_prompt_contents_are_bounded_at_utf8_boundaries() {
    let contents = format!("{}é", "a".repeat(MAX_SKILL_PROMPT_BYTES - 1));

    let (bounded, truncated) = bounded_skill_prompt_contents(&contents);

    assert_eq!(bounded.len(), MAX_SKILL_PROMPT_BYTES - 1);
    assert_eq!(truncated, true);
}
