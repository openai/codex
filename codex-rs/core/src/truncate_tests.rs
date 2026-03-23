use super::TruncationPolicy;
use super::approx_token_count;
use super::formatted_truncate_text;
use super::formatted_truncate_text_content_items_with_policy;
use super::split_string;
use super::truncate_function_output_items_with_policy;
use super::truncate_text;
use super::truncate_with_token_budget;
use codex_protocol::models::FunctionCallOutputContentItem;
use pretty_assertions::assert_eq;

#[test]
fn split_string_works() {
    assert_eq!(split_string("hello world", 5, 5), (1, "hello", "world"));
    assert_eq!(split_string("abc", 0, 0), (3, "", ""));
}

#[test]
fn split_string_handles_empty_string() {
    assert_eq!(split_string("", 4, 4), (0, "", ""));
}

#[test]
fn split_string_only_keeps_prefix_when_tail_budget_is_zero() {
    assert_eq!(split_string("abcdef", 3, 0), (3, "abc", ""));
}

#[test]
fn split_string_only_keeps_suffix_when_prefix_budget_is_zero() {
    assert_eq!(split_string("abcdef", 0, 3), (3, "", "def"));
}

#[test]
fn split_string_handles_overlapping_budgets_without_removal() {
    assert_eq!(split_string("abcdef", 4, 4), (0, "abcd", "ef"));
}

#[test]
fn split_string_respects_utf8_boundaries() {
    assert_eq!(split_string("😀abc😀", 5, 5), (1, "😀a", "c😀"));

    assert_eq!(split_string("😀😀😀😀😀", 1, 1), (5, "", ""));
    assert_eq!(split_string("😀😀😀😀😀", 7, 7), (3, "😀", "😀"));
    assert_eq!(split_string("😀😀😀😀😀", 8, 8), (1, "😀😀", "😀😀"));
}

#[test]
fn truncate_bytes_less_than_placeholder_returns_placeholder() {
    let content = "example output";

    assert_eq!(
        "Total output lines: 1\n\n…13 chars truncated…t",
        formatted_truncate_text(content, TruncationPolicy::Bytes(1)),
    );
}

#[test]
fn truncate_tokens_less_than_placeholder_returns_placeholder() {
    let content = "example output";

    assert_eq!(
        "Total output lines: 1\n\nex…3 tokens truncated…ut",
        formatted_truncate_text(content, TruncationPolicy::Tokens(1)),
    );
}

#[test]
fn truncate_tokens_under_limit_returns_original() {
    let content = "example output";

    assert_eq!(
        content,
        formatted_truncate_text(content, TruncationPolicy::Tokens(10)),
    );
}

#[test]
fn truncate_bytes_under_limit_returns_original() {
    let content = "example output";

    assert_eq!(
        content,
        formatted_truncate_text(content, TruncationPolicy::Bytes(20)),
    );
}

#[test]
fn truncate_tokens_over_limit_returns_truncated() {
    let content = "this is an example of a long output that should be truncated";

    assert_eq!(
        "Total output lines: 1\n\nthis is an…10 tokens truncated… truncated",
        formatted_truncate_text(content, TruncationPolicy::Tokens(5)),
    );
}

#[test]
fn truncate_bytes_over_limit_returns_truncated() {
    let content = "this is an example of a long output that should be truncated";

    assert_eq!(
        "Total output lines: 1\n\nthis is an exam…30 chars truncated…ld be truncated",
        formatted_truncate_text(content, TruncationPolicy::Bytes(30)),
    );
}

#[test]
fn truncate_bytes_reports_original_line_count_when_truncated() {
    let content =
        "this is an example of a long output that should be truncated\nalso some other line";

    assert_eq!(
        "Total output lines: 2\n\nthis is an exam…51 chars truncated…some other line",
        formatted_truncate_text(content, TruncationPolicy::Bytes(30)),
    );
}

#[test]
fn truncate_tokens_reports_original_line_count_when_truncated() {
    let content =
        "this is an example of a long output that should be truncated\nalso some other line";

    assert_eq!(
        "Total output lines: 2\n\nthis is an example o…11 tokens truncated…also some other line",
        formatted_truncate_text(content, TruncationPolicy::Tokens(10)),
    );
}

#[test]
fn truncate_with_token_budget_returns_original_when_under_limit() {
    let s = "short output";
    let limit = 100;
    let (out, original) = truncate_with_token_budget(s, TruncationPolicy::Tokens(limit));
    assert_eq!(out, s);
    assert_eq!(original, None);
}

#[test]
fn truncate_with_token_budget_reports_truncation_at_zero_limit() {
    let s = "abcdef";
    let (out, original) = truncate_with_token_budget(s, TruncationPolicy::Tokens(0));
    assert_eq!(out, "…2 tokens truncated…");
    assert_eq!(original, Some(2));
}

#[test]
fn truncate_middle_tokens_handles_utf8_content() {
    let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
    let (out, tokens) = truncate_with_token_budget(s, TruncationPolicy::Tokens(8));
    assert_eq!(out, "😀😀😀😀…8 tokens truncated… line with text\n");
    assert_eq!(tokens, Some(16));
}

#[test]
fn truncate_middle_bytes_handles_utf8_content() {
    let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
    let out = truncate_text(s, TruncationPolicy::Bytes(20));
    assert_eq!(out, "😀😀…21 chars truncated…with text\n");
}

#[test]
fn truncates_across_multiple_under_limit_texts_and_reports_omitted() {
    let chunk = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega.\n";
    let chunk_tokens = approx_token_count(chunk);
    assert!(chunk_tokens > 0, "chunk must consume tokens");
    let limit = chunk_tokens * 3;
    let t1 = chunk.to_string();
    let t2 = chunk.to_string();
    let t3 = chunk.repeat(10);
    let t4 = chunk.to_string();
    let t5 = chunk.to_string();

    let items = vec![
        FunctionCallOutputContentItem::InputText { text: t1.clone() },
        FunctionCallOutputContentItem::InputText { text: t2.clone() },
        FunctionCallOutputContentItem::InputImage {
            image_url: "img:mid".to_string(),
            detail: None,
        },
        FunctionCallOutputContentItem::InputText { text: t3 },
        FunctionCallOutputContentItem::InputText { text: t4 },
        FunctionCallOutputContentItem::InputText { text: t5 },
    ];

    let output =
        truncate_function_output_items_with_policy(&items, TruncationPolicy::Tokens(limit));

    // Expect: t1 (full), t2 (full), image, t3 (truncated), summary mentioning 2 omitted.
    assert_eq!(output.len(), 5);

    let first_text = match &output[0] {
        FunctionCallOutputContentItem::InputText { text } => text,
        other => panic!("unexpected first item: {other:?}"),
    };
    assert_eq!(first_text, &t1);

    let second_text = match &output[1] {
        FunctionCallOutputContentItem::InputText { text } => text,
        other => panic!("unexpected second item: {other:?}"),
    };
    assert_eq!(second_text, &t2);

    assert_eq!(
        output[2],
        FunctionCallOutputContentItem::InputImage {
            image_url: "img:mid".to_string(),
            detail: None,
        }
    );

    let fourth_text = match &output[3] {
        FunctionCallOutputContentItem::InputText { text } => text,
        other => panic!("unexpected fourth item: {other:?}"),
    };
    assert!(
        fourth_text.contains("tokens truncated"),
        "expected marker in truncated snippet: {fourth_text}"
    );

    let summary_text = match &output[4] {
        FunctionCallOutputContentItem::InputText { text } => text,
        other => panic!("unexpected summary item: {other:?}"),
    };
    assert!(summary_text.contains("omitted 2 text items"));
}

#[test]
fn formatted_truncate_text_content_items_with_policy_returns_original_under_limit() {
    let items = vec![
        FunctionCallOutputContentItem::InputText {
            text: "alpha".to_string(),
        },
        FunctionCallOutputContentItem::InputText {
            text: String::new(),
        },
        FunctionCallOutputContentItem::InputText {
            text: "beta".to_string(),
        },
    ];

    let (output, original_token_count) =
        formatted_truncate_text_content_items_with_policy(&items, TruncationPolicy::Bytes(32));

    assert_eq!(output, items);
    assert_eq!(original_token_count, None);
}

#[test]
fn formatted_truncate_text_content_items_with_policy_merges_text_and_appends_images() {
    let items = vec![
        FunctionCallOutputContentItem::InputText {
            text: "abcd".to_string(),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "img:one".to_string(),
            detail: None,
        },
        FunctionCallOutputContentItem::InputText {
            text: "efgh".to_string(),
        },
        FunctionCallOutputContentItem::InputText {
            text: "ijkl".to_string(),
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: "img:two".to_string(),
            detail: None,
        },
    ];

    let (output, original_token_count) =
        formatted_truncate_text_content_items_with_policy(&items, TruncationPolicy::Bytes(8));

    assert_eq!(
        output,
        vec![
            FunctionCallOutputContentItem::InputText {
                text: "Total output lines: 3\n\nabcd…6 chars truncated…ijkl".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "img:one".to_string(),
                detail: None,
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "img:two".to_string(),
                detail: None,
            },
        ]
    );
    assert_eq!(original_token_count, Some(4));
}

#[test]
fn formatted_truncate_text_content_items_with_policy_merges_all_text_for_token_budget() {
    let items = vec![
        FunctionCallOutputContentItem::InputText {
            text: "abcdefgh".to_string(),
        },
        FunctionCallOutputContentItem::InputText {
            text: "ijklmnop".to_string(),
        },
    ];

    let (output, original_token_count) =
        formatted_truncate_text_content_items_with_policy(&items, TruncationPolicy::Tokens(2));

    assert_eq!(
        output,
        vec![FunctionCallOutputContentItem::InputText {
            text: "Total output lines: 2\n\nabcd…3 tokens truncated…mnop".to_string(),
        }]
    );
    assert_eq!(original_token_count, Some(5));
}
