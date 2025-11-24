use ascii_char::{AsciiChar, ToAsciiChar};

/// Terminals use [caret notation](https://en.wikipedia.org/wiki/Caret_notation)
/// to display some typed control codes, such as ^D for EOT and ^Z for SUB.
///
/// This function returns the caret notation letter for control codes,
/// or `None` for printable characters.
///
/// # Examples
/// ```
/// # use ascii::{AsciiChar, caret_encode};
/// assert_eq!(caret_encode(b'\0'), Some(AsciiChar::At));
/// assert_eq!(caret_encode(AsciiChar::DEL), Some(AsciiChar::Question));
/// assert_eq!(caret_encode(b'E'), None);
/// assert_eq!(caret_encode(b'\n'), Some(AsciiChar::J));
/// ```
pub fn caret_encode<C: Copy + Into<u8>>(c: C) -> Option<AsciiChar> {
    // The formula is explained in the Wikipedia article.
    let c = c.into() ^ 0b0100_0000;
    if (b'?'..=b'_').contains(&c) {
        // SAFETY: All bytes between '?' (0x3F) and '_' (0x5f) are valid ascii characters.
        Some(unsafe { c.to_ascii_char_unchecked() })
    } else {
        None
    }
}

/// Returns the control code represented by a [caret notation](https://en.wikipedia.org/wiki/Caret_notation)
/// letter, or `None` if the letter is not used in caret notation.
///
/// This function is the inverse of `caret_encode()`.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// # use ascii::{AsciiChar, caret_decode};
/// assert_eq!(caret_decode(b'?'), Some(AsciiChar::DEL));
/// assert_eq!(caret_decode(AsciiChar::D), Some(AsciiChar::EOT));
/// assert_eq!(caret_decode(b'\0'), None);
/// ```
///
/// Symmetry:
///
/// ```
/// # use ascii::{AsciiChar, caret_encode, caret_decode};
/// assert_eq!(caret_encode(AsciiChar::US).and_then(caret_decode), Some(AsciiChar::US));
/// assert_eq!(caret_decode(b'@').and_then(caret_encode), Some(AsciiChar::At));
/// ```
pub fn caret_decode<C: Copy + Into<u8>>(c: C) -> Option<AsciiChar> {
    // The formula is explained in the Wikipedia article.
    match c.into() {
        // SAFETY: All bytes between '?' (0x3F) and '_' (0x5f) after `xoring` with `0b0100_0000` are
        //         valid bytes, as they represent characters between '␀' (0x0) and '␠' (0x1f) + '␡' (0x7f)
        b'?'..=b'_' => Some(unsafe { AsciiChar::from_ascii_unchecked(c.into() ^ 0b0100_0000) }),
        _ => None,
    }
}
