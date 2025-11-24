//! Characters.

/// Properties of ASCII characters.
///
/// About `'` (single quote) being considered as a literal: see
/// [Errata ID 6937](https://www.rfc-editor.org/errata/eid6937).
const CHARS_TABLE: [u8; 128] = [
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
    0b_0000_0001, // !
    0b_0000_0000, // "
    0b_0000_0001, // #
    0b_0000_0001, // $
    0b_0000_0000, // %
    0b_0000_0001, // &
    0b_0000_0001, // '
    0b_0000_0001, // (
    0b_0000_0001, // )
    0b_0000_0001, // *
    0b_0000_0001, // +
    0b_0000_0001, // ,
    0b_0000_0001, // -
    0b_0000_0101, // .
    0b_0000_0001, // /
    0b_0000_0111, // 0
    0b_0000_0111, // 1
    0b_0000_0111, // 2
    0b_0000_0111, // 3
    0b_0000_0111, // 4
    0b_0000_0111, // 5
    0b_0000_0111, // 6
    0b_0000_0111, // 7
    0b_0000_0111, // 8
    0b_0000_0111, // 9
    0b_0000_0001, // :
    0b_0000_0001, // ;
    0b_0000_0000, // <
    0b_0000_0001, // =
    0b_0000_0000, // >
    0b_0000_0001, // ?
    0b_0000_0001, // @
    0b_0000_0111, // A
    0b_0000_0111, // B
    0b_0000_0111, // C
    0b_0000_0111, // D
    0b_0000_0111, // E
    0b_0000_0111, // F
    0b_0000_0111, // G
    0b_0000_0111, // H
    0b_0000_0111, // I
    0b_0000_0111, // J
    0b_0000_0111, // K
    0b_0000_0111, // L
    0b_0000_0111, // M
    0b_0000_0111, // N
    0b_0000_0111, // O
    0b_0000_0111, // P
    0b_0000_0111, // Q
    0b_0000_0111, // R
    0b_0000_0111, // S
    0b_0000_0111, // T
    0b_0000_0111, // U
    0b_0000_0111, // V
    0b_0000_0111, // W
    0b_0000_0111, // X
    0b_0000_0111, // Y
    0b_0000_0111, // Z
    0b_0000_0001, // [
    0b_0000_0000, // \
    0b_0000_0001, // ]
    0b_0000_0000, // ^
    0b_0000_0111, // _
    0b_0000_0000, // `
    0b_0000_0111, // a
    0b_0000_0111, // b
    0b_0000_0111, // c
    0b_0000_0111, // d
    0b_0000_0111, // e
    0b_0000_0111, // f
    0b_0000_0111, // g
    0b_0000_0111, // h
    0b_0000_0111, // i
    0b_0000_0111, // j
    0b_0000_0111, // k
    0b_0000_0111, // l
    0b_0000_0111, // m
    0b_0000_0111, // n
    0b_0000_0111, // o
    0b_0000_0111, // p
    0b_0000_0111, // q
    0b_0000_0111, // r
    0b_0000_0111, // s
    0b_0000_0111, // t
    0b_0000_0111, // u
    0b_0000_0111, // v
    0b_0000_0111, // w
    0b_0000_0111, // x
    0b_0000_0111, // y
    0b_0000_0111, // z
    0b_0000_0000, // {
    0b_0000_0000, // |
    0b_0000_0000, // }
    0b_0000_0001, // ~
    0b_0000_0000, // DEL
];

/// A mask to test whether the character matches `literals` rule defined in [RFC 6570].
///
/// [RFC 6570]: https://www.rfc-editor.org/rfc/rfc6570.html#section-2.1
const CHARS_TABLE_MASK_LITERAL: u8 = 1 << 0;

/// A mask to test whether the character matches `varchar` rule defined in [RFC 6570].
///
/// [RFC 6570]: https://www.rfc-editor.org/rfc/rfc6570.html#section-2.3
const CHARS_TABLE_MASK_VARCHAR_START: u8 = 1 << 1;

/// A mask to test whether the character matches `varchar` rule defined in [RFC 6570] or a period.
///
/// [RFC 6570]: https://www.rfc-editor.org/rfc/rfc6570.html#section-2.3
const CHARS_TABLE_MASK_VARCHAR_CONTINUE: u8 = 1 << 2;

/// Returns true if the given ASCII character is allowed in a literal string.
///
/// # Precondition
///
/// The given byte should be an ASCII character, i.e. should be less than 128.
#[inline]
#[must_use]
pub(super) const fn is_ascii_literal_char(c: u8) -> bool {
    (CHARS_TABLE[c as usize] & CHARS_TABLE_MASK_LITERAL) != 0
}

/// Returns true if the given ASCII character is allowed as the beginning of the `varname`.
///
/// Note that this does not return true for `%` character. It is caller's
/// responsibility to test validity of percent-encoded triplets.
///
/// # Precondition
///
/// The given byte should be an ASCII character, i.e. should be less than 128.
#[inline]
#[must_use]
pub(super) const fn is_ascii_varchar_start(c: u8) -> bool {
    (CHARS_TABLE[c as usize] & CHARS_TABLE_MASK_VARCHAR_START) != 0
}

/// Returns true if the given ASCII character is allowed as the non-beginning of the `varname`.
///
/// Note that this does not return true for `%` character. It is caller's
/// responsibility to test validity of percent-encoded triplets.
///
/// # Precondition
///
/// The given byte should be an ASCII character, i.e. should be less than 128.
#[inline]
#[must_use]
pub(super) const fn is_ascii_varchar_continue(c: u8) -> bool {
    (CHARS_TABLE[c as usize] & CHARS_TABLE_MASK_VARCHAR_CONTINUE) != 0
}
