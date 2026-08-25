//! Identify terminal-color replies without changing unrelated Windows console input.

use std::ops::Range;

const MAX_TERMINAL_COLOR_RESPONSE_BYTES: usize = 1_024;
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// Return the byte ranges occupied by complete, valid OSC 10/11 color responses.
///
/// The Windows probe maps these byte ranges back to their original `INPUT_RECORD`s so keyboard
/// modifiers, UTF-16 input, mouse events, and focus changes can be replayed without reconstruction.
pub(super) fn terminal_color_response_ranges(input: &[u8]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = 0;

    while cursor < input.len() {
        if input[cursor..].starts_with(PASTE_START) {
            let payload_start = cursor + PASTE_START.len();
            let Some(payload_end) = super::find_subslice(&input[payload_start..], PASTE_END) else {
                break;
            };
            cursor = payload_start + payload_end + PASTE_END.len();
            continue;
        }
        if !input[cursor..].starts_with(b"\x1b]") {
            cursor += 1;
            continue;
        }

        let start = cursor;
        let response = &input[start..];
        let Some((slot, prefix)) = [(10_u8, b"\x1b]10;"), (11_u8, b"\x1b]11;")]
            .into_iter()
            .find(|(_, prefix)| response.starts_with(*prefix))
        else {
            cursor = start + b"\x1b]".len();
            continue;
        };

        let payload_start = start + prefix.len();
        let bounded_end = input
            .len()
            .min(start.saturating_add(MAX_TERMINAL_COLOR_RESPONSE_BYTES));
        let Some((payload_len, terminator_len)) =
            super::osc_payload_end(&input[payload_start..bounded_end])
        else {
            cursor = payload_start;
            continue;
        };
        let end = payload_start + payload_len + terminator_len;
        if super::parse_osc_color(&input[start..end], slot).is_some() {
            ranges.push(start..end);
        }
        cursor = end;
    }

    ranges
}

/// Parse only real probe replies, never OSC-looking content inside a bracketed paste.
pub(super) fn terminal_default_colors(input: &[u8]) -> Option<super::DefaultColors> {
    let mut foreground = None;
    let mut background = None;

    for range in terminal_color_response_ranges(input) {
        let response = &input[range];
        if foreground.is_none() {
            foreground = super::parse_osc_color(response, /*slot*/ 10);
        }
        if background.is_none() {
            background = super::parse_osc_color(response, /*slot*/ 11);
        }
    }

    Some(super::DefaultColors {
        fg: foreground?,
        bg: background?,
    })
}

#[cfg(test)]
#[path = "windows_replay_tests.rs"]
mod tests;
