#![cfg(test)]

use pretty_assertions::assert_eq;
use unicode_linebreak::linebreaks;

#[test]
fn styled_runs_have_the_same_breaks_as_contiguous_text() {
    let long = "中日".repeat(200);
    let runs = ["bu", "ild (", " ready)", "— ", "—"] // codespell:ignore bu
        .into_iter()
        .chain(std::iter::repeat_n("🇨", 79))
        .chain([long.as_str(), " done"]);
    let text: String = runs.clone().collect();
    let expected: Vec<_> = linebreaks(&text).map(|(i, _)| i).collect();
    assert_eq!(super::break_offsets(runs), expected[..expected.len() - 1]);
}

#[test]
fn chunks_of_a_clean_leaf_share_its_original_buffer() {
    let leaf = String::from("build ready");
    let start = leaf.as_ptr();
    let mut inline = super::InlineText::default();
    inline.push_leaf(leaf, super::Paint::default());
    let prepared = inline.finish();
    let runs: Vec<_> = prepared.lines[0]
        .iter()
        .map(|chunk| &*chunk.runs[0].text)
        .collect();
    assert_eq!(runs, ["build", "ready"]);
    assert_eq!(
        [runs[0].as_ptr(), runs[1].as_ptr()],
        [start, start.wrapping_add(6)]
    );
}
