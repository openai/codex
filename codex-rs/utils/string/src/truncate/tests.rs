use super::split_string;
use super::truncate_middle_chars;
use super::truncate_middle_with_token_budget;
use pretty_assertions::assert_eq;

#[test]
fn split_string_works() {
    assert_eq!(
        split_string(
            "hello world",
            /*beginning_bytes*/ 5,
            /*end_bytes*/ 5
        ),
        (1, "hello", "world")
    );
    assert_eq!(
        split_string("abc", /*beginning_bytes*/ 0, /*end_bytes*/ 0),
        (3, "", "")
    );
}

#[test]
fn split_string_handles_empty_string() {
    assert_eq!(
        split_string("", /*beginning_bytes*/ 4, /*end_bytes*/ 4),
        (0, "", "")
    );
}

#[test]
fn split_string_only_keeps_prefix_when_tail_budget_is_zero() {
    assert_eq!(
        split_string("abcdef", /*beginning_bytes*/ 3, /*end_bytes*/ 0),
        (3, "abc", "")
    );
}

#[test]
fn split_string_only_keeps_suffix_when_prefix_budget_is_zero() {
    assert_eq!(
        split_string("abcdef", /*beginning_bytes*/ 0, /*end_bytes*/ 3),
        (3, "", "def")
    );
}

#[test]
fn split_string_handles_overlapping_budgets_without_removal() {
    assert_eq!(
        split_string("abcdef", /*beginning_bytes*/ 4, /*end_bytes*/ 4),
        (0, "abcd", "ef")
    );
}

#[test]
fn split_string_respects_utf8_boundaries() {
    assert_eq!(
        split_string("😀abc😀", /*beginning_bytes*/ 5, /*end_bytes*/ 5),
        (1, "😀a", "c😀")
    );

    assert_eq!(
        split_string(
            "😀😀😀😀😀",
            /*beginning_bytes*/ 1,
            /*end_bytes*/ 1
        ),
        (5, "", "")
    );
    assert_eq!(
        split_string(
            "😀😀😀😀😀",
            /*beginning_bytes*/ 7,
            /*end_bytes*/ 7
        ),
        (3, "😀", "😀")
    );
    assert_eq!(
        split_string(
            "😀😀😀😀😀",
            /*beginning_bytes*/ 8,
            /*end_bytes*/ 8
        ),
        (1, "😀😀", "😀😀")
    );
}

#[test]
fn truncate_with_token_budget_returns_original_when_under_limit() {
    let s = "short output";
    let limit = 100;
    let (out, original) = truncate_middle_with_token_budget(s, limit);
    assert_eq!(out, s);
    assert_eq!(original, None);
}

#[test]
fn truncate_with_token_budget_reports_truncation_at_zero_limit() {
    let s = "abcdef";
    let (out, original) = truncate_middle_with_token_budget(s, /*max_tokens*/ 0);
    assert_eq!(out, "…2 tokens truncated…");
    assert_eq!(original, Some(2));
}

#[test]
fn truncate_middle_tokens_handles_utf8_content() {
    let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
    let (out, tokens) = truncate_middle_with_token_budget(s, /*max_tokens*/ 8);
    assert_eq!(out, "😀😀😀😀…8 tokens truncated… line with text\n");
    assert_eq!(tokens, Some(16));
}

#[test]
fn truncate_middle_bytes_handles_utf8_content() {
    let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
    let out = truncate_middle_chars(s, /*max_bytes*/ 20);
    assert_eq!(out, "😀😀…21 chars truncated…with text\n");
}
