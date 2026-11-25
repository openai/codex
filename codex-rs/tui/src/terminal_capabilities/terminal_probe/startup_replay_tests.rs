#![cfg(test)]

use super::MAX_REPLAYED_OSC_BYTES;
use super::parse_startup_input;
use pretty_assertions::assert_eq;

fn startup_replay_input(input: &[u8]) -> Vec<u8> {
    parse_startup_input(
        input, /*probe_kitty_graphics*/ false, /*probe_bidi*/ false,
    )
    .0
}

#[test]
fn startup_replay_bounds_oversized_osc_without_exposing_its_payload() {
    let mut input = b"typed\x1b]10;".to_vec();
    input.extend(std::iter::repeat_n(b'x', MAX_REPLAYED_OSC_BYTES));
    input.extend_from_slice(b"1\r");

    assert_eq!(
        startup_replay_input(&input),
        input[..b"typed".len() + MAX_REPLAYED_OSC_BYTES]
    );

    input.push(b'\x07');
    input.extend_from_slice(b"suffix");
    assert_eq!(startup_replay_input(&input), b"typedsuffix");
    assert_eq!(
        startup_replay_input(b"typed\x1b]10;partial\x1b"),
        b"typed\x1b]10;partial\x1b"
    );
}

#[test]
fn startup_replay_preserves_osc_looking_bracketed_paste() {
    let mut input = b"prefix\x1b[200~\x1b]10;".to_vec();
    input.extend(std::iter::repeat_n(b'x', 2_048));
    input.extend_from_slice(b"\x1b[201~suffix");

    assert_eq!(startup_replay_input(&input), input);
}

#[test]
fn startup_replay_omits_terminal_color_replies_interleaved_with_bracketed_paste() {
    let input = b"prefix\x1b[200~hello\x1b]10;rgb:eeee/eeee/eeee\x07 \
        \x1b]11;rgb:1111/1111/1111\x1b\\world\x1b[201~suffix";

    assert_eq!(
        startup_replay_input(input),
        b"prefix\x1b[200~hello world\x1b[201~suffix"
    );

    let unfinished_paste = b"\x1b[200~hello\x1b]10;rgb:eeee/eeee/eeee\x07 world";
    assert_eq!(
        startup_replay_input(unfinished_paste),
        b"\x1b[200~hello world"
    );

    let unrelated_osc = b"\x1b[200~\x1b]52;preserve me\x07\x1b[201~";
    assert_eq!(startup_replay_input(unrelated_osc), unrelated_osc);

    let response_after_paste = b"\x1b[200~\x1b]10;unfinished\x1b[201~suffix\x07";
    assert_eq!(
        startup_replay_input(response_after_paste),
        response_after_paste
    );
}

#[test]
fn herdr_graphics_reply_selects_previews_without_replaying_the_response_as_keys() {
    let paste = b"\x1b[200~\x1b_Gi=31;OK\x1b\\\x1b[201~";
    for (reply, expected) in [
        (b"\x1b_Gi=31;OK\x1b\\".as_slice(), true),
        (b"\x1b_Gi=31;EINVAL: unsupported\x1b\\", false),
        (b"", false),
    ] {
        let input = [paste.as_slice(), reply, b"typed"].concat();
        assert_eq!(
            parse_startup_input(
                &input, /*probe_kitty_graphics*/ true, /*probe_bidi*/ false
            ),
            ([paste.as_slice(), b"typed"].concat(), expected, None)
        );
    }
}

#[test]
fn bidi_replies_interleaved_with_paste_are_consumed_only_when_queried() {
    for (mode, implicit) in [
        (0, None),
        (1, Some(true)),
        (2, Some(false)),
        (3, Some(true)),
        (4, Some(false)),
    ] {
        let input = format!("typed\x1b[200~a\x1b[8;{mode}$yb\x1b[201~").into_bytes();
        assert_eq!(
            parse_startup_input(
                &input, /*probe_kitty_graphics*/ false, /*probe_bidi*/ true
            ),
            (b"typed\x1b[200~ab\x1b[201~".to_vec(), false, implicit)
        );
        assert_eq!(startup_replay_input(&input), input);
    }
}
