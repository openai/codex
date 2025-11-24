//! Characters.

use crate::spec::Spec;

/// A mask to test whether the character is continue character of `scheme`.
// `ALPHA / DIGIT / "+" / "-" / "."`
const MASK_SCHEME_CONTINUE: u8 = 1 << 0;

/// A mask to test whether the character matches `unreserved`.
// `unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"`
const MASK_UNRESERVED: u8 = 1 << 1;

/// A mask to test whether the character matches `gen-delims`.
// `gen-delims = ":" / "/" / "?" / "#" / "[" / "]" / "@"`
const MASK_GEN_DELIMS: u8 = 1 << 2;

/// A mask to test whether the character matches `sub-delims`.
// `sub-delims = "!" / "$" / "&" / "'" / "(" / ")" / "*" / "+" / "," / ";" / "="`
const MASK_SUB_DELIMS: u8 = 1 << 3;

/// A mask to test whether the character matches `pchar` (modulo percent-encoded bytes).
// `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`
const MASK_PCHAR: u8 = 1 << 4;

/// A mask to test whether the character can appear in `query` and `fragment`.
// `query = *( pchar / "/" / "?" )`
// `fragment = *( pchar / "/" / "?" )`
const MASK_FRAG_QUERY: u8 = 1 << 5;

/// A mask to test whether the character can appear in `userinfo` and address of `IPvFuture`.
// `userinfo = *( unreserved / pct-encoded / sub-delims / ":" )`
const MASK_USERINFO_IPVFUTUREADDR: u8 = 1 << 6;

/// A mask to test whether the character matches `pchar` (modulo percent-encoded bytes) or slash.
const MASK_PCHAR_SLASH: u8 = 1 << 7;

/// ASCII characters' properties.
const TABLE: [u8; 128] = [
    0b_0000_0000, // NUL
    0b_0000_0000, // SOH
    0b_0000_0000, // STX
    0b_0000_0000, // ETX
    0b_0000_0000, // EOT
    0b_0000_0000, // ENQ
    0b_0000_0000, // ACK
    0b_0000_0000, // BEL
    0b_0000_0000, // BS
    0b_0000_0000, // HT
    0b_0000_0000, // LF
    0b_0000_0000, // VT
    0b_0000_0000, // FF
    0b_0000_0000, // CR
    0b_0000_0000, // SO
    0b_0000_0000, // SI
    0b_0000_0000, // DLE
    0b_0000_0000, // DC1
    0b_0000_0000, // DC2
    0b_0000_0000, // DC3
    0b_0000_0000, // DC4
    0b_0000_0000, // NAK
    0b_0000_0000, // SYN
    0b_0000_0000, // ETB
    0b_0000_0000, // CAN
    0b_0000_0000, // EM
    0b_0000_0000, // SUB
    0b_0000_0000, // ESC
    0b_0000_0000, // FS
    0b_0000_0000, // GS
    0b_0000_0000, // RS
    0b_0000_0000, // US
    0b_0000_0000, // SPACE
    0b_1111_1000, // !
    0b_0000_0000, // "
    0b_0000_0100, // #
    0b_1111_1000, // $
    0b_0000_0000, // %
    0b_1111_1000, // &
    0b_1111_1000, // '
    0b_1111_1000, // (
    0b_1111_1000, // )
    0b_1111_1000, // *
    0b_1111_1001, // +
    0b_1111_1000, // ,
    0b_1111_0011, // -
    0b_1111_0011, // .
    0b_1010_0100, // /
    0b_1111_0011, // 0
    0b_1111_0011, // 1
    0b_1111_0011, // 2
    0b_1111_0011, // 3
    0b_1111_0011, // 4
    0b_1111_0011, // 5
    0b_1111_0011, // 6
    0b_1111_0011, // 7
    0b_1111_0011, // 8
    0b_1111_0011, // 9
    0b_1111_0100, // :
    0b_1111_1000, // ;
    0b_0000_0000, // <
    0b_1111_1000, // =
    0b_0000_0000, // >
    0b_0010_0100, // ?
    0b_1011_0100, // @
    0b_1111_0011, // A
    0b_1111_0011, // B
    0b_1111_0011, // C
    0b_1111_0011, // D
    0b_1111_0011, // E
    0b_1111_0011, // F
    0b_1111_0011, // G
    0b_1111_0011, // H
    0b_1111_0011, // I
    0b_1111_0011, // J
    0b_1111_0011, // K
    0b_1111_0011, // L
    0b_1111_0011, // M
    0b_1111_0011, // N
    0b_1111_0011, // O
    0b_1111_0011, // P
    0b_1111_0011, // Q
    0b_1111_0011, // R
    0b_1111_0011, // S
    0b_1111_0011, // T
    0b_1111_0011, // U
    0b_1111_0011, // V
    0b_1111_0011, // W
    0b_1111_0011, // X
    0b_1111_0011, // Y
    0b_1111_0011, // Z
    0b_0000_0100, // [
    0b_0000_0000, // \
    0b_0000_0100, // ]
    0b_0000_0000, // ^
    0b_1111_0010, // _
    0b_0000_0000, // `
    0b_1111_0011, // a
    0b_1111_0011, // b
    0b_1111_0011, // c
    0b_1111_0011, // d
    0b_1111_0011, // e
    0b_1111_0011, // f
    0b_1111_0011, // g
    0b_1111_0011, // h
    0b_1111_0011, // i
    0b_1111_0011, // j
    0b_1111_0011, // k
    0b_1111_0011, // l
    0b_1111_0011, // m
    0b_1111_0011, // n
    0b_1111_0011, // o
    0b_1111_0011, // p
    0b_1111_0011, // q
    0b_1111_0011, // r
    0b_1111_0011, // s
    0b_1111_0011, // t
    0b_1111_0011, // u
    0b_1111_0011, // v
    0b_1111_0011, // w
    0b_1111_0011, // x
    0b_1111_0011, // y
    0b_1111_0011, // z
    0b_0000_0000, // {
    0b_0000_0000, // |
    0b_0000_0000, // }
    0b_1111_0010, // ~
    0b_0000_0000, // DEL
];

/// Returns `true` if the given ASCII character is allowed as continue character of `scheme` part.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_scheme_continue(c: u8) -> bool {
    (TABLE[c as usize] & MASK_SCHEME_CONTINUE) != 0
}

/// Returns `true` if the given ASCII character matches `unreserved`.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_unreserved(c: u8) -> bool {
    (TABLE[c as usize] & MASK_UNRESERVED) != 0
}

/// Returns true if the character is unreserved.
#[inline]
#[must_use]
pub(crate) fn is_unreserved<S: Spec>(c: char) -> bool {
    if c.is_ascii() {
        is_ascii_unreserved(c as u8)
    } else {
        S::is_nonascii_char_unreserved(c)
    }
}

///// Returns `true` if the given ASCII character matches `gen-delims`.
//#[inline]
//#[must_use]
//pub(crate) const fn is_ascii_gen_delims(c: u8) -> bool {
//    (TABLE[c as usize] & MASK_GEN_DELIMS) != 0
//}

///// Returns `true` if the given ASCII character matches `sub-delims`.
//#[inline]
//#[must_use]
//pub(crate) const fn is_ascii_sub_delims(c: u8) -> bool {
//    (TABLE[c as usize] & MASK_SUB_DELIMS) != 0
//}

///// Returns `true` if the given ASCII character matches `reserved`.
//#[inline]
//#[must_use]
//pub(crate) const fn is_ascii_reserved(c: u8) -> bool {
//    (TABLE[c as usize] & (MASK_GEN_DELIMS | MASK_SUB_DELIMS)) != 0
//}

/// Returns `true` if the given ASCII character matches `pchar` modulo `pct-encoded`.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_pchar(c: u8) -> bool {
    (TABLE[c as usize] & MASK_PCHAR) != 0
}

/// Returns `true` if the given ASCII character is allowed to appear in `query` and `fragment`.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_frag_query(c: u8) -> bool {
    (TABLE[c as usize] & MASK_FRAG_QUERY) != 0
}

/// Returns `true` if the given non-ASCII character is allowed to appear in `iquery`.
#[inline]
#[must_use]
pub(crate) fn is_nonascii_query<S: Spec>(c: char) -> bool {
    S::is_nonascii_char_unreserved(c) || S::is_nonascii_char_private(c)
}

/// Returns `true` if the given non-ASCII character is allowed to appear in `ifragment`.
#[inline]
#[must_use]
pub(crate) fn is_nonascii_fragment<S: Spec>(c: char) -> bool {
    S::is_nonascii_char_unreserved(c)
}

/// Returns `true` if the given ASCII character is allowed to appear in `userinfo` and `IPvFuture`.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_userinfo_ipvfutureaddr(c: u8) -> bool {
    (TABLE[c as usize] & MASK_USERINFO_IPVFUTUREADDR) != 0
}

/// Returns `true` if the given non-ASCII character is allowed to appear in `iuserinfo`.
#[inline]
#[must_use]
pub(crate) fn is_nonascii_userinfo<S: Spec>(c: char) -> bool {
    S::is_nonascii_char_unreserved(c)
}

/// Returns `true` if the given ASCII character is allowed to appear in `reg-name`
#[inline]
#[must_use]
pub(crate) const fn is_ascii_regname(c: u8) -> bool {
    (TABLE[c as usize] & (MASK_UNRESERVED | MASK_SUB_DELIMS)) != 0
}

/// Returns `true` if the given non-ASCII character is allowed to appear in `ireg-name`.
#[inline]
#[must_use]
pub(crate) fn is_nonascii_regname<S: Spec>(c: char) -> bool {
    S::is_nonascii_char_unreserved(c)
}

/// Returns `true` if the given ASCII character matches `pchar` modulo `pct-encoded` or a slash.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_pchar_slash(c: u8) -> bool {
    (TABLE[c as usize] & MASK_PCHAR_SLASH) != 0
}

/// Checks if the given character matches `ucschar` rule.
#[must_use]
pub(crate) fn is_ucschar(c: char) -> bool {
    matches!(
        u32::from(c),
        0xA0..=0xD7FF |
        0xF900..=0xFDCF |
        0xFDF0..=0xFFEF |
        0x1_0000..=0x1_FFFD |
        0x2_0000..=0x2_FFFD |
        0x3_0000..=0x3_FFFD |
        0x4_0000..=0x4_FFFD |
        0x5_0000..=0x5_FFFD |
        0x6_0000..=0x6_FFFD |
        0x7_0000..=0x7_FFFD |
        0x8_0000..=0x8_FFFD |
        0x9_0000..=0x9_FFFD |
        0xA_0000..=0xA_FFFD |
        0xB_0000..=0xB_FFFD |
        0xC_0000..=0xC_FFFD |
        0xD_0000..=0xD_FFFD |
        0xE_1000..=0xE_FFFD
    )
}

/// Returns true if the given value is a continue byte of UTF-8.
#[inline(always)]
#[must_use]
pub(crate) fn is_utf8_byte_continue(byte: u8) -> bool {
    // `0x80..=0xbf` (i.e. `0b_1000_0000..=0b_1011_1111`) is not the first byte,
    // and `0xc0..=0xc1` (i.e. `0b_1100_0000..=0b_1100_0001` shouldn't appear
    // anywhere in UTF-8 byte sequence.
    // `0x80 as i8` is -128, and `0xc0 as i8` is -96.
    //
    // The first byte of the UTF-8 character is not `0b10xx_xxxx`, and
    // the continue bytes is `0b10xx_xxxx`.
    // `0b1011_1111 as i8` is -65, and `0b1000_0000 as i8` is -128.
    (byte as i8) < -64
}

/// Returns true if the given ASCII character is `unreserved` or `reserved`.
#[inline]
#[must_use]
pub(crate) const fn is_ascii_unreserved_or_reserved(c: u8) -> bool {
    (TABLE[c as usize] & (MASK_UNRESERVED | MASK_GEN_DELIMS | MASK_SUB_DELIMS)) != 0
}
