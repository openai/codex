use super::WindowsTtyInputNormalizer;
use pretty_assertions::assert_eq;

#[test]
fn normalizes_terminal_input_without_changing_text_or_ctrl_c() {
    let mut normalizer = WindowsTtyInputNormalizer::default();
    assert_eq!(normalizer.normalize(b"first\n"), b"first\r");
    assert_eq!(normalizer.normalize(b"second\r"), b"second\r");
    assert_eq!(normalizer.normalize(b"\nthird\r\n"), b"third\r");

    let mut input_with_controls = "cafeé 漢字".as_bytes().to_vec();
    input_with_controls.extend_from_slice(b"\x08\x03");
    let mut expected = "cafeé 漢字".as_bytes().to_vec();
    expected.extend_from_slice(b"\x7f\x03");
    assert_eq!(normalizer.normalize(&input_with_controls), expected);
}
