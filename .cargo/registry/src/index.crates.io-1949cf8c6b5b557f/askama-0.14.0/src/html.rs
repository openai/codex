// The file is shared across many crates, not all have this feature.
// If they don't then the tests won't be compiled in, but that's OK, because they are executed at
// least in the crate `askama`. There's no need to run the test multiple times.
#![allow(unexpected_cfgs)]

use core::{fmt, str};

use crate::ascii_str::{AsciiChar, AsciiStr};

#[allow(unused)]
pub(crate) fn write_escaped_str(mut dest: impl fmt::Write, src: &str) -> fmt::Result {
    // This implementation reads one byte after another.
    // It's not very fast, but should work well enough until portable SIMD gets stabilized.

    let mut escaped_buf = ESCAPED_BUF_INIT;
    let mut last = 0;

    for (index, byte) in src.bytes().enumerate() {
        if let Some(escaped) = get_escaped(byte) {
            [escaped_buf[2], escaped_buf[3]] = escaped;
            write_str_if_nonempty(&mut dest, &src[last..index])?;
            dest.write_str(AsciiStr::from_slice(&escaped_buf[..ESCAPED_BUF_LEN]))?;
            last = index + 1;
        }
    }
    write_str_if_nonempty(&mut dest, &src[last..])
}

#[allow(unused)]
pub(crate) fn write_escaped_char(mut dest: impl fmt::Write, c: char) -> fmt::Result {
    if !c.is_ascii() {
        dest.write_char(c)
    } else if let Some(escaped) = get_escaped(c as u8) {
        let mut escaped_buf = ESCAPED_BUF_INIT;
        [escaped_buf[2], escaped_buf[3]] = escaped;
        dest.write_str(AsciiStr::from_slice(&escaped_buf[..ESCAPED_BUF_LEN]))
    } else {
        // RATIONALE: `write_char(c)` gets optimized if it is known that `c.is_ascii()`
        dest.write_char(c)
    }
}

/// Returns the decimal representation of the codepoint if the character needs HTML escaping.
#[inline]
fn get_escaped(byte: u8) -> Option<[AsciiChar; 2]> {
    if let MIN_CHAR..=MAX_CHAR = byte {
        let entry = TABLE.0[(byte - MIN_CHAR) as usize];
        (entry != UNESCAPED).then_some(entry)
    } else {
        None
    }
}

#[inline(always)]
fn write_str_if_nonempty(output: &mut impl fmt::Write, input: &str) -> fmt::Result {
    if !input.is_empty() {
        output.write_str(input)
    } else {
        Ok(())
    }
}

/// List of characters that need HTML escaping, not necessarily in ordinal order.
const CHARS: &[u8] = br#""&'<>"#;

/// The character with the lowest codepoint that needs HTML escaping.
const MIN_CHAR: u8 = {
    let mut v = u8::MAX;
    let mut i = 0;
    while i < CHARS.len() {
        if v > CHARS[i] {
            v = CHARS[i];
        }
        i += 1;
    }
    v
};

/// The character with the highest codepoint that needs HTML escaping.
const MAX_CHAR: u8 = {
    let mut v = u8::MIN;
    let mut i = 0;
    while i < CHARS.len() {
        if v < CHARS[i] {
            v = CHARS[i];
        }
        i += 1;
    }
    v
};

/// Number of codepoints between the lowest and highest character that needs escaping, incl.
const CHAR_RANGE: usize = (MAX_CHAR - MIN_CHAR + 1) as usize;

#[repr(align(64))]
struct Table([[AsciiChar; 2]; CHAR_RANGE]);

/// For characters that need HTML escaping, the codepoint is formatted as decimal digits,
/// otherwise `b"\0\0"`. Starting at [`MIN_CHAR`].
const TABLE: &Table = &{
    let mut table = Table([UNESCAPED; CHAR_RANGE]);
    let mut i = 0;
    while i < CHARS.len() {
        let c = CHARS[i];
        table.0[c as u32 as usize - MIN_CHAR as usize] = AsciiChar::two_digits(c as u32);
        i += 1;
    }
    table
};

const UNESCAPED: [AsciiChar; 2] = AsciiStr::new_sized("");

const ESCAPED_BUF_INIT_UNPADDED: &str = "&#__;";
// RATIONALE: llvm generates better code if the buffer is register sized
const ESCAPED_BUF_INIT: [AsciiChar; 8] = AsciiStr::new_sized(ESCAPED_BUF_INIT_UNPADDED);
const ESCAPED_BUF_LEN: usize = ESCAPED_BUF_INIT_UNPADDED.len();

#[test]
#[cfg(feature = "alloc")]
fn test_simple_html_string_escaping() {
    extern crate alloc;

    let mut buf = alloc::string::String::new();
    write_escaped_str(&mut buf, "<script>").unwrap();
    assert_eq!(buf, "&#60;script&#62;");

    buf.clear();
    write_escaped_str(&mut buf, "s<crip>t").unwrap();
    assert_eq!(buf, "s&#60;crip&#62;t");

    buf.clear();
    write_escaped_str(&mut buf, "s<cripcripcripcripcripcripcripcripcripcrip>t").unwrap();
    assert_eq!(buf, "s&#60;cripcripcripcripcripcripcripcripcripcrip&#62;t");
}
