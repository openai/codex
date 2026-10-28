#![cfg(test)]

use super::guardian_truncate_text;

#[test]
fn guardian_truncate_text_keeps_prefix_suffix_and_xml_marker() {
    let content = "prefix ".repeat(200) + &" suffix".repeat(200);

    let (truncated, was_truncated) = guardian_truncate_text(&content, /*token_cap*/ 20);

    assert!(truncated.starts_with("prefix"));
    assert!(truncated.contains("<truncated omitted_approx_tokens=\""));
    assert!(truncated.ends_with("suffix"));
    assert!(was_truncated);
}
