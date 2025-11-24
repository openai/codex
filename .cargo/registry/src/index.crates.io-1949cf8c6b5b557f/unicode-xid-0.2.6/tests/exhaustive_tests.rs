extern crate unicode_xid;
use unicode_xid::UnicodeXID;
/// A `char` in Rust is a Unicode Scalar Value
///
/// See: http://www.unicode.org/glossary/#unicode_scalar_value
fn all_valid_chars() -> impl Iterator<Item = char> {
    (0u32..=0xD7FF).chain(0xE000u32..=0x10FFFF).map(|u| {
        core::convert::TryFrom::try_from(u)
            .expect("The selected range should be infallible if the docs match impl")
    })
}

#[test]
fn all_valid_chars_do_not_panic_for_is_xid_start() {
    for c in all_valid_chars() {
        let _ = UnicodeXID::is_xid_start(c);
    }
}

#[test]
fn all_valid_chars_do_not_panic_for_is_xid_continue() {
    for c in all_valid_chars() {
        let _ = UnicodeXID::is_xid_continue(c);
    }
}
