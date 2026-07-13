use super::*;
use crate::bottom_pane::textarea::TextArea;
use pretty_assertions::assert_eq;

#[test]
fn dismissed_tokens_after_adjacent_elements_are_occurrence_scoped() {
    let first_bound = "$bound1";
    let second_bound = "$bound2";
    let token = "$other";
    let text = format!("{first_bound}{token} {second_bound}{token}");
    let first_start = first_bound.len();
    let second_start = text.rfind(token).expect("second token");
    let second_bound_start = text.find(second_bound).expect("second bound mention");
    let mut textarea = TextArea::new();
    textarea.insert_str(&text);
    textarea.add_element_range(0..first_start);
    textarea.add_element_range(second_bound_start..second_bound_start + second_bound.len());
    let dismissed = DismissedToken::new(
        &textarea,
        first_start..first_start + token.len(),
        "other".to_string(),
    );

    assert!(!dismissed.matches(
        &textarea,
        &(second_start..second_start + token.len()),
        "other",
    ));
}

#[test]
fn nested_at_query_does_not_count_as_complete_dismissed_token() {
    let text = "@ma@latest @ma";
    let second_start = text.rfind("@ma").expect("second token");
    let mut textarea = TextArea::new();
    textarea.insert_str(text);

    assert_eq!(
        complete_token_occurrences_before(&textarea, "@ma", second_start),
        0
    );
}

#[test]
fn non_sigil_elements_delimit_dismissed_token_occurrences() {
    let token = "$other";
    let first_element = "[one]";
    let text = format!("{token}{first_element} {token}");
    let second_start = text.rfind(token).expect("second token");
    let mut textarea = TextArea::new();
    textarea.insert_str(&text);
    textarea.add_element_range(token.len()..token.len() + first_element.len());
    let dismissed = DismissedToken::new(&textarea, 0..token.len(), "other".to_string());

    assert!(!dismissed.matches(
        &textarea,
        &(second_start..second_start + token.len()),
        "other",
    ));
}
