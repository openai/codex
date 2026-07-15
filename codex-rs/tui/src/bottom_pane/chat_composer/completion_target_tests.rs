use super::*;
use crate::bottom_pane::textarea::TextArea;
use pretty_assertions::assert_eq;

#[test]
fn current_prefixed_token_affinity_does_not_cross_line_break() {
    for (text, prefix, allow_empty) in [
        ("@file\n  continue", '@', false),
        ("continue  \n@file", '@', false),
        ("$skill\n  continue", '$', true),
        ("continue  \n$skill", '$', true),
    ] {
        let mut textarea = TextArea::new();
        textarea.insert_str(text);
        textarea.set_cursor(text.find("  ").expect("indentation present") + 1);

        assert_eq!(
            current_prefixed_token_range(&textarea, prefix, allow_empty),
            None
        );
    }
}

#[test]
fn current_prefixed_token_affinity_does_not_cross_separator_before_plain_text() {
    for (text, prefix, allow_empty) in [("@old  word", '@', false), ("$old  word", '$', true)] {
        let mut textarea = TextArea::new();
        textarea.insert_str(text);
        textarea.set_cursor(text.find("  ").expect("separator present") + 1);

        assert_eq!(
            current_prefixed_token_range(&textarea, prefix, allow_empty),
            None
        );
    }
}

#[test]
fn current_prefixed_token_prefers_right_after_adjacent_target() {
    let bound = "@bound";
    let left = "@bound@old";
    let text = "@bound@old  @new";
    let right_start = left.len() + 2;
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.add_element_range(0..bound.len());
    textarea.set_cursor(left.len());

    assert_eq!(
        current_prefixed_token_range(&textarea, '@', /*allow_empty*/ false),
        Some((right_start..text.len(), "new".to_string()))
    );
}

#[test]
fn current_prefixed_token_prefers_right_at_token_start() {
    let text = "$old $new";
    let right_start = "$old ".len();
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_cursor(right_start);

    assert_eq!(
        current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true),
        Some((right_start..text.len(), "new".to_string()))
    );
}

#[test]
fn current_prefixed_token_falls_back_from_bound_at_token_on_right() {
    let text = "@left  @bound";
    let bound_start = text.find("@bound").expect("bound mention");
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.add_element_range(bound_start..text.len());
    textarea.set_cursor("@left ".len());

    assert_eq!(
        current_prefixed_token_range(&textarea, '@', /*allow_empty*/ true),
        Some((0.."@left".len(), "left".to_string()))
    );
}

#[test]
fn current_prefixed_token_falls_back_from_bound_dollar_suffix_on_right() {
    for suffix in ["/path", ".config"] {
        let text = format!("$left  $bound{suffix}");
        let bound_start = text.find("$bound").expect("bound skill mention");
        let bound_end = bound_start + "$bound".len();
        let mut textarea = TextArea::new();
        textarea.insert_str(&text);
        textarea.add_element_range(bound_start..bound_end);
        textarea.set_cursor("$left  ".len());

        assert_eq!(
            current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true),
            Some((0.."$left".len(), "left".to_string()))
        );
    }
}

#[test]
fn current_prefixed_token_targets_later_segment_between_bound_elements() {
    let text = "$bound1$one$bound2$two$bound3";
    let bound2_start = text.find("$bound2").expect("second bound mention");
    let two_start = text.find("$two").expect("later plaintext target");
    let bound3_start = text.find("$bound3").expect("third bound mention");
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.add_element_range(0.."$bound1".len());
    textarea.add_element_range(bound2_start..two_start);
    textarea.add_element_range(bound3_start..text.len());
    textarea.set_cursor(two_start + "$t".len());

    assert_eq!(
        current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true),
        Some((two_start..bound3_start, "two".to_string()))
    );
}

#[test]
fn current_prefixed_token_bounds_right_candidate_before_bound_element() {
    let text = "continue @other@bound";
    let other_start = text.find("@other").expect("editable mention");
    let bound_start = text.find("@bound").expect("bound mention");
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.add_element_range(bound_start..text.len());
    textarea.set_cursor(other_start);

    assert_eq!(
        current_prefixed_token_range(&textarea, '@', /*allow_empty*/ false),
        Some((other_start..bound_start, "other".to_string()))
    );
}

#[test]
fn current_prefixed_token_stops_before_overlapping_element() {
    let editable = "$other";
    let placeholder = "[Pasted Content 1001 chars]";
    let text = format!("{editable}{placeholder}");
    let mut textarea = TextArea::new();
    textarea.insert_str(&text);
    textarea.add_element_range(editable.len()..text.len());
    textarea.set_cursor(editable.len());

    assert_eq!(
        current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true),
        Some((0..editable.len(), "other".to_string()))
    );
}

#[test]
fn current_prefixed_token_stops_at_trailing_space_before_line_break() {
    let text = "$figma \nnext";
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_cursor("$figma ".len());

    assert_eq!(
        current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true),
        None
    );
}

#[test]
fn current_prefixed_token_keeps_nested_dollar_prefix_in_same_token() {
    let text = "$HOME/$USER";
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_cursor(text.find("$USER").expect("nested prefix present"));

    assert_eq!(
        current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true),
        Some((0..text.len(), "HOME/$USER".to_string()))
    );
}

#[test]
fn typed_qualified_suffix_preserves_bound_plain_mention() {
    // Qualified skill bindings and history persistence are intentionally deferred. Typing a
    // suffix after a bound plain mention must not reinterpret it as an editable qualified one.
    let bound = "$google-calendar";
    let text = "$google-calendar:availability";
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.add_element_range(0..bound.len());
    textarea.set_cursor(text.len());

    let (range, token) = current_prefixed_token_range(&textarea, '$', /*allow_empty*/ true)
        .expect("dollar token should be present");
    assert_eq!((range.clone(), token.as_str()), (0..text.len(), &text[1..]));
    assert!(!prefixed_token_range_is_editable(
        &textarea, '$', &range, &token,
    ));
}

#[test]
fn dollar_query_classifies_shell_and_skill_syntax() {
    assert!(!dollar_query_is_completable("{CODEX_HOME}/config.toml"));
    assert!(!dollar_query_is_completable("(pwd)"));
    assert!(dollar_query_is_completable("home:search"));
    assert!(dollar_query_is_completable("home"));
    assert!(!dollar_query_is_completable("HOME"));
    for query in ["0", "1", "12", "-", "_"] {
        assert!(!dollar_query_is_completable(query));
        assert_eq!(
            dollar_query_kind(query),
            DollarQueryKind::DefiniteShellParameter
        );
    }
    for query in ["1_suffix", "1foo", "-x"] {
        assert!(!dollar_query_is_completable(query));
        assert_eq!(
            dollar_query_kind(query),
            DollarQueryKind::AmbiguousShellParameter
        );
    }
}
