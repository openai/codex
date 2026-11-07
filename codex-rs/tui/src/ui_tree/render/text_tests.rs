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
