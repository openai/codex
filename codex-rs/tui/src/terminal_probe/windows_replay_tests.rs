use super::MAX_TERMINAL_COLOR_RESPONSE_BYTES;
use super::terminal_color_response_ranges;
use super::terminal_default_colors;
use crate::terminal_probe::DefaultColors;
use pretty_assertions::assert_eq;

fn preserved_input(input: &[u8]) -> Vec<u8> {
    let ranges = terminal_color_response_ranges(input);
    input
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| {
            (!ranges.iter().any(|range| range.contains(&index))).then_some(*byte)
        })
        .collect()
}

#[test]
fn preserves_typeahead_before_between_and_after_terminal_color_replies() {
    let input = b"before\x1b]10;rgb:5555/5757/5353\x1b\\between\x1b]11;rgb:ffff/ffff/ffff\x07after";

    assert_eq!(preserved_input(input), b"beforebetweenafter");
}

#[test]
fn preserves_unrelated_malformed_and_partial_osc_sequences() {
    let input = b"\x1b]52;clipboard\x07\x1b]10;not-a-color\x07typed\x1b]11;rgb:ffff/ffff";

    assert_eq!(preserved_input(input), input);
}

#[test]
fn preserves_terminal_color_response_that_exceeds_the_byte_limit() {
    let mut input = b"prefix\x1b]10;".to_vec();
    input.extend(std::iter::repeat_n(b'x', MAX_TERMINAL_COLOR_RESPONSE_BYTES));
    input.extend_from_slice(b"\x07suffix");

    assert_eq!(preserved_input(&input), input);
}

#[test]
fn finds_terminal_color_replies_after_unrelated_escape_sequences() {
    let input = b"\x1b]52;clipboard\x07typed\x1b]11;rgba:ffff/eeee/dddd/ffff\x1b\\suffix";

    assert_eq!(preserved_input(input), b"\x1b]52;clipboard\x07typedsuffix");
}

#[test]
fn consumes_one_and_three_digit_terminal_color_replies() {
    let input = b"before\x1b]10;rgb:f/e/d\x07middle\x1b]11;rgb:fff/800/000\x1b\\after";

    assert_eq!(preserved_input(input), b"beforemiddleafter");
    assert_eq!(
        terminal_default_colors(input),
        Some(DefaultColors {
            fg: (255, 238, 221),
            bg: (255, 127, 0),
        })
    );
}

#[test]
fn preserves_complete_osc_looking_bracketed_paste() {
    let input = b"before\x1b[200~\x1b]10;rgb:f/e/d\x07\x1b]11;rgb:fff/800/000\x07\x1b[201~after";

    assert_eq!(preserved_input(input), input);
    assert_eq!(terminal_default_colors(input), None);
}

#[test]
fn preserves_unfinished_osc_looking_bracketed_paste() {
    let input = b"before\x1b[200~\x1b]10;rgb:f/e/d\x07\x1b]11;rgb:fff/800/000\x07";

    assert_eq!(preserved_input(input), input);
    assert_eq!(terminal_default_colors(input), None);
}

#[test]
fn parses_real_color_replies_around_bracketed_paste() {
    let input =
        b"\x1b]10;rgb:f/e/d\x07\x1b[200~\x1b]11;rgb:0/0/0\x07\x1b[201~\x1b]11;rgb:fff/800/000\x07";

    assert_eq!(
        preserved_input(input),
        b"\x1b[200~\x1b]11;rgb:0/0/0\x07\x1b[201~"
    );
    assert_eq!(
        terminal_default_colors(input),
        Some(DefaultColors {
            fg: (255, 238, 221),
            bg: (255, 127, 0),
        })
    );
}
