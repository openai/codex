//! Cover observed terminal signatures, fragmented replies, and pasted lookalikes.

use super::is_apple_terminal;
use pretty_assertions::assert_eq;

#[test]
fn distinguishes_the_native_terminal_replies() {
    for (primary, secondary, expected) in [
        ("1;2", "1;95;0", true),
        ("62;22;52", "1;10;0", false),
        ("64;1;2;4;6;17;18;21;22;52", "64;2500;0", false),
        ("62;52;", "1;4000;46", false),
        ("65;4;6;18;22", "1;277;0", false),
        ("6", "0;2600;1", false),
        ("1;2", "0;95;0", false),
        ("6", "1;95;0", false),
        ("1;2", "1;95;00", false),
        ("1;2", "1;95;1", false),
    ] {
        let primary = format!("\x1b[?{primary}c");
        let secondary = format!("\x1b[>{secondary}c");
        for input in [
            format!("{primary}{secondary}"),
            format!("{secondary}{primary}"),
        ] {
            assert_eq!(
                is_apple_terminal(input.as_bytes()),
                Some(expected),
                "{input:?}"
            );
        }
    }
}

#[test]
fn unrelated_csi_replies_cannot_consume_typed_text_as_device_attributes() {
    for reply in [b"\x1b[?7u".as_slice(), b"\x1b[>4;2m".as_slice()] {
        let mut input = reply.to_vec();
        input.extend_from_slice(b"c\x1b[>1;95;0c");
        assert_eq!(is_apple_terminal(&input), None);
        input.extend_from_slice(b"\x1b[?1;2c");
        assert_eq!(is_apple_terminal(&input), Some(true));
    }
}

#[test]
fn identity_waits_for_complete_replies_and_ignores_pasted_signatures() {
    let signature = b"\x1b[?1;2c\x1b[>1;95;0c";
    for end in 0..signature.len() {
        assert_eq!(is_apple_terminal(&signature[..end]), None);
    }
    let mut input = b"typed\x1b[200~".to_vec();
    input.extend_from_slice(signature);
    assert_eq!(is_apple_terminal(&input), None);
    input.extend_from_slice(b"\x1b[201~more typed");
    assert_eq!(is_apple_terminal(&input), None);
    input.extend_from_slice(signature);
    assert_eq!(is_apple_terminal(&input), Some(true));
}
