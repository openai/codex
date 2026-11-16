//! Preserve startup terminal input while filtering replies to terminal probes.

const MAX_REPLAYED_OSC_BYTES: usize = 1024;

/// Return the input to replay and whether Herdr acknowledged the Kitty graphics query.
///
/// Keep bounded incomplete OSC prefixes so unread continuations stay parser-framed.
pub(super) fn parse_startup_input(input: &[u8], probe_herdr: bool) -> (Vec<u8>, bool) {
    const PASTE_START: &[u8] = b"\x1b[200~";
    const PASTE_END: &[u8] = b"\x1b[201~";
    const KITTY_REPLY: &[u8] = b"\x1b_Gi=31;";

    let mut replay = Vec::with_capacity(input.len());
    let mut kitty_graphics = false;
    let mut cursor = 0;
    let mut inside_paste = false;
    while cursor < input.len() {
        if input
            .get(cursor..cursor + 11)
            .is_some_and(|report| super::alternate_scroll_report(report).is_some())
        {
            cursor += 11;
            continue;
        }
        if inside_paste {
            if input[cursor..].starts_with(PASTE_END) {
                replay.extend_from_slice(PASTE_END);
                cursor += PASTE_END.len();
                inside_paste = false;
            } else if let Some((slot, _)) = [(10, b"\x1b]10;"), (11, b"\x1b]11;")]
                .into_iter()
                .find(|(_, prefix)| input[cursor..].starts_with(*prefix))
                && let Some((end, terminator_len)) = super::osc_payload_end(
                    &input[cursor + 2..input.len().min(cursor + MAX_REPLAYED_OSC_BYTES)],
                )
                && let response = &input[cursor..cursor + 2 + end + terminator_len]
                && !response
                    .windows(PASTE_END.len())
                    .any(|window| window == PASTE_END)
                && super::parse_osc_color(response, slot).is_some()
            {
                cursor += 2 + end + terminator_len;
            } else {
                replay.push(input[cursor]);
                cursor += 1;
            }
            continue;
        }

        if input[cursor..].starts_with(PASTE_START) {
            replay.extend_from_slice(PASTE_START);
            cursor += PASTE_START.len();
            inside_paste = true;
            continue;
        }

        // Only consume replies to our query. Bracketed paste above remains user input.
        if probe_herdr
            && let Some(message) = input[cursor..].strip_prefix(KITTY_REPLY)
            && let Some(end) = message.iter().position(|b| !(0x20..=0x7e).contains(b))
            && message[end..].starts_with(b"\x1b\\")
        {
            kitty_graphics = &message[..end] == b"OK";
            cursor += KITTY_REPLY.len() + end + 2;
            continue;
        }

        if input[cursor..].starts_with(b"\x1b]") {
            if let Some((end, terminator_len)) = super::osc_payload_end(&input[cursor + 2..]) {
                cursor += 2 + end + terminator_len;
                continue;
            }
            let end = input.len().min(cursor + MAX_REPLAYED_OSC_BYTES);
            replay.extend_from_slice(&input[cursor..end]);
            break;
        }

        replay.push(input[cursor]);
        cursor += 1;
    }
    (replay, kitty_graphics)
}

#[cfg(test)]
#[path = "startup_replay_tests.rs"]
mod tests;
