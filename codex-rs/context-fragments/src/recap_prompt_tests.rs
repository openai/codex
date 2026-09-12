use super::RecapPrompt;
use crate::ContextualUserFragment;
use codex_utils_string::approx_token_count;
use pretty_assertions::assert_eq;

#[test]
fn recap_prompt_bounds_the_entire_utf8_fragment() {
    for history in ["a".repeat(/*n*/ 40_000), "進捗🦀".repeat(/*n*/ 10_000)] {
        let prompt = RecapPrompt::new(&history).render();
        assert!(prompt.len() <= RecapPrompt::MAX_BYTES);
        assert!(approx_token_count(&prompt) <= RecapPrompt::MAX_ESTIMATED_TOKENS);
        let retained = prompt.split_once("Conversation:\n").unwrap().1;
        assert!(history.starts_with(retained));
        assert!(RecapPrompt::MAX_BYTES - prompt.len() < 4);
    }
}

#[test]
fn recap_prompt_preserves_history_that_fits() {
    let history = "User: Fix the parser.\n\nAssistant: Done. What should happen on empty input?";
    let prompt = RecapPrompt::new(history).render();
    assert_eq!(prompt.split_once("Conversation:\n").unwrap().1, history);
}
