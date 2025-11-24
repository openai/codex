/// Returns true if and only if the given byte is considered a word character.
/// This only applies to ASCII.
pub(crate) fn is_word_byte(b: u8) -> bool {
    const fn mkwordset() -> [bool; 256] {
        // FIXME: Use as_usize() once const functions in traits are stable.
        let mut set = [false; 256];
        set[b'_' as usize] = true;

        let mut byte = b'0';
        while byte <= b'9' {
            set[byte as usize] = true;
            byte += 1;
        }
        byte = b'A';
        while byte <= b'Z' {
            set[byte as usize] = true;
            byte += 1;
        }
        byte = b'a';
        while byte <= b'z' {
            set[byte as usize] = true;
            byte += 1;
        }
        set
    }
    const WORD: [bool; 256] = mkwordset();
    WORD[b as usize]
}

/// The accept state index. When we enter this state, we know we've found a
/// valid Unicode scalar value.
const ACCEPT: usize = 12;
/// The reject state index. When we enter this state, we know that we've found
/// invalid UTF-8.
const REJECT: usize = 0;

/// Like `decode`, but automatically converts the `None` case to the
/// replacement codepoint.
pub(crate) fn decode_lossy<B: AsRef<[u8]>>(slice: B) -> (char, usize) {
    match decode(slice) {
        (Some(ch), size) => (ch, size),
        (None, size) => ('\u{FFFD}', size),
    }
}

/// UTF-8 decode a single Unicode scalar value from the beginning of a slice.
///
/// When successful, the corresponding Unicode scalar value is returned along
/// with the number of bytes it was encoded with. The number of bytes consumed
/// for a successful decode is always between 1 and 4, inclusive.
///
/// When unsuccessful, `None` is returned along with the number of bytes that
/// make up a maximal prefix of a valid UTF-8 code unit sequence. In this case,
/// the number of bytes consumed is always between 0 and 3, inclusive, where
/// 0 is only returned when `slice` is empty.
pub(crate) fn decode<B: AsRef<[u8]>>(slice: B) -> (Option<char>, usize) {
    let slice = slice.as_ref();
    match slice.get(0) {
        None => return (None, 0),
        Some(&b) if b <= 0x7F => return (Some(b as char), 1),
        _ => {}
    }

    let (mut state, mut cp, mut i) = (ACCEPT, 0, 0);
    while i < slice.len() {
        decode_step(&mut state, &mut cp, slice[i]);
        i += 1;

        if state == ACCEPT {
            // OK since `decode_step` guarantees that `cp` is a valid Unicode
            // scalar value in an ACCEPT state.
            //
            // We don't have to use safe code here, but do so because perf
            // isn't our primary objective in regex-lite.
            let ch = char::from_u32(cp).unwrap();
            return (Some(ch), i);
        } else if state == REJECT {
            // At this point, we always want to advance at least one byte.
            return (None, core::cmp::max(1, i.saturating_sub(1)));
        }
    }
    (None, i)
}

/// Transitions to the next state and updates `cp` while it does.
fn decode_step(state: &mut usize, cp: &mut u32, b: u8) {
    // Splits the space of all bytes into equivalence classes, such that
    // any byte in the same class can never discriminate between whether a
    // particular sequence is valid UTF-8 or not.
    #[rustfmt::skip]
    const CLASSES: [u8; 256] = [
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
       1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,  9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
       7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
       8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,  2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
      10,3,3,3,3,3,3,3,3,3,3,3,3,4,3,3, 11,6,6,6,5,8,8,8,8,8,8,8,8,8,8,8,
    ];

    // A state machine taken from `bstr` which was in turn adapted from:
    // https://bjoern.hoehrmann.de/utf-8/decoder/dfa/
    #[rustfmt::skip]
    const STATES_FORWARD: &'static [u8] = &[
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      12, 0, 24, 36, 60, 96, 84, 0, 0, 0, 48, 72,
      0, 12, 0, 0, 0, 0, 0, 12, 0, 12, 0, 0,
      0, 24, 0, 0, 0, 0, 0, 24, 0, 24, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0,
      0, 24, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 36, 0, 36, 0, 0,
      0, 36, 0, 0, 0, 0, 0, 36, 0, 36, 0, 0,
      0, 36, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    let class = CLASSES[usize::from(b)];
    if *state == ACCEPT {
        *cp = (0xFF >> class) & (b as u32);
    } else {
        *cp = (b as u32 & 0b111111) | (*cp << 6);
    }
    *state = usize::from(STATES_FORWARD[*state + usize::from(class)]);
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;

    #[test]
    fn decode_valid() {
        fn d(mut s: &str) -> Vec<char> {
            let mut chars = vec![];
            while !s.is_empty() {
                let (ch, size) = decode(s.as_bytes());
                s = &s[size..];
                chars.push(ch.unwrap());
            }
            chars
        }

        assert_eq!(vec!['☃'], d("☃"));
        assert_eq!(vec!['☃', '☃'], d("☃☃"));
        assert_eq!(vec!['α', 'β', 'γ', 'δ', 'ε'], d("αβγδε"));
        assert_eq!(vec!['☃', '⛄', '⛇'], d("☃⛄⛇"));
        assert_eq!(vec!['𝗮', '𝗯', '𝗰', '𝗱', '𝗲'], d("𝗮𝗯𝗰𝗱𝗲"));
    }

    #[test]
    fn decode_invalid() {
        let (ch, size) = decode(b"");
        assert_eq!(None, ch);
        assert_eq!(0, size);

        let (ch, size) = decode(b"\xFF");
        assert_eq!(None, ch);
        assert_eq!(1, size);

        let (ch, size) = decode(b"\xCE\xF0");
        assert_eq!(None, ch);
        assert_eq!(1, size);

        let (ch, size) = decode(b"\xE2\x98\xF0");
        assert_eq!(None, ch);
        assert_eq!(2, size);

        let (ch, size) = decode(b"\xF0\x9D\x9D");
        assert_eq!(None, ch);
        assert_eq!(3, size);

        let (ch, size) = decode(b"\xF0\x9D\x9D\xF0");
        assert_eq!(None, ch);
        assert_eq!(3, size);

        let (ch, size) = decode(b"\xF0\x82\x82\xAC");
        assert_eq!(None, ch);
        assert_eq!(1, size);

        let (ch, size) = decode(b"\xED\xA0\x80");
        assert_eq!(None, ch);
        assert_eq!(1, size);

        let (ch, size) = decode(b"\xCEa");
        assert_eq!(None, ch);
        assert_eq!(1, size);

        let (ch, size) = decode(b"\xE2\x98a");
        assert_eq!(None, ch);
        assert_eq!(2, size);

        let (ch, size) = decode(b"\xF0\x9D\x9Ca");
        assert_eq!(None, ch);
        assert_eq!(3, size);
    }

    #[test]
    fn decode_lossily() {
        let (ch, size) = decode_lossy(b"");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(0, size);

        let (ch, size) = decode_lossy(b"\xFF");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(1, size);

        let (ch, size) = decode_lossy(b"\xCE\xF0");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(1, size);

        let (ch, size) = decode_lossy(b"\xE2\x98\xF0");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(2, size);

        let (ch, size) = decode_lossy(b"\xF0\x9D\x9D\xF0");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(3, size);

        let (ch, size) = decode_lossy(b"\xF0\x82\x82\xAC");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(1, size);

        let (ch, size) = decode_lossy(b"\xED\xA0\x80");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(1, size);

        let (ch, size) = decode_lossy(b"\xCEa");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(1, size);

        let (ch, size) = decode_lossy(b"\xE2\x98a");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(2, size);

        let (ch, size) = decode_lossy(b"\xF0\x9D\x9Ca");
        assert_eq!('\u{FFFD}', ch);
        assert_eq!(3, size);
    }
}
