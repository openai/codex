/// Decode base64 `input`, writing the result into `output`.
///
/// `input` is treated as secret, so efforts are made to avoid
/// leaking its value via side channels, such as timing,
/// memory accesses, and execution trace.
///
/// The following is deemed non-secret information:
///
/// - Appearance of whitespace in `input`
/// - Erroneous characters in `input` (indeed, the first illegal
///   character is quoted in the error type)
/// - The length of `input`
/// - The length of `output`
///
/// Returns the prefix of `output` that was written to.
pub(crate) fn decode_secret<'a>(input: &[u8], output: &'a mut [u8]) -> Result<&'a [u8], Error> {
    decode(input, output, CodePoint::decode_secret)
}

/// Decode base64 `input`, writing the result into `output`.
///
/// `input` is treated as public information, so its value may
/// be leaked via side channels.
///
/// Returns the prefix of `output` that was written to.
pub(crate) fn decode_public<'a>(input: &[u8], output: &'a mut [u8]) -> Result<&'a [u8], Error> {
    decode(input, output, CodePoint::decode_public)
}

/// Provide an upper limit on how much space could be required
/// to decode a base64 encoding of len `base64_len`.
pub(crate) const fn decoded_length(base64_len: usize) -> usize {
    ((base64_len + 3) / 4) * 3
}

fn decode<'a>(
    input: &[u8],
    output: &'a mut [u8],
    decode_byte: impl Fn(u8) -> CodePoint,
) -> Result<&'a [u8], Error> {
    let mut buffer = 0u64;
    let mut used = 0;
    let mut shift = SHIFT_INITIAL;
    let mut pad_mask = 0;

    let mut output_offset = 0;

    const SHIFT_INITIAL: i32 = (8 - 1) * 6;

    for byte in input.iter().copied() {
        let (item, pad) = match decode_byte(byte) {
            CodePoint::WHITESPACE => continue,
            CodePoint::INVALID => return Err(Error::InvalidCharacter(byte)),
            CodePoint::PAD => (0, 1),
            CodePoint(n) => (n, 0),
        };

        // we collect 8 code points (therefore: 6 output bytes) into
        // `buffer`.  this keeps this loop as tight as possible.
        if used == 8 {
            if pad_mask != 0b0000_0000 {
                return Err(Error::PrematurePadding);
            }

            let chunk = output
                .get_mut(output_offset..output_offset + 6)
                .ok_or(Error::InsufficientOutputSpace)?;

            chunk[0] = (buffer >> 40) as u8;
            chunk[1] = (buffer >> 32) as u8;
            chunk[2] = (buffer >> 24) as u8;
            chunk[3] = (buffer >> 16) as u8;
            chunk[4] = (buffer >> 8) as u8;
            chunk[5] = buffer as u8;

            output_offset += 6;
            buffer = 0;
            used = 0;
            pad_mask = 0;
            shift = SHIFT_INITIAL;
        }

        buffer |= (item as u64) << shift;
        shift -= 6;
        pad_mask |= pad << used;
        used += 1;
    }

    // reduce to final block
    if used > 4 {
        if pad_mask & 0b0000_1111 != 0 {
            return Err(Error::PrematurePadding);
        }
        let chunk = output
            .get_mut(output_offset..output_offset + 3)
            .ok_or(Error::InsufficientOutputSpace)?;
        chunk[0] = (buffer >> 40) as u8;
        chunk[1] = (buffer >> 32) as u8;
        chunk[2] = (buffer >> 24) as u8;

        buffer <<= 24;
        pad_mask >>= 4;
        used -= 4;
        output_offset += 3;
    }

    match (used, pad_mask) {
        // no trailing bytes
        (0, 0b0000) => {}

        // 4 trailing bytes, no padding
        (4, 0b0000) => {
            let chunk = output
                .get_mut(output_offset..output_offset + 3)
                .ok_or(Error::InsufficientOutputSpace)?;
            chunk[0] = (buffer >> 40) as u8;
            chunk[1] = (buffer >> 32) as u8;
            chunk[2] = (buffer >> 24) as u8;
            output_offset += 3;
        }

        // 4 trailing bytes with one padding char, or 3 trailing bytes
        (4, 0b1000) | (3, 0b0000) => {
            let chunk = output
                .get_mut(output_offset..output_offset + 2)
                .ok_or(Error::InsufficientOutputSpace)?;

            chunk[0] = (buffer >> 40) as u8;
            chunk[1] = (buffer >> 32) as u8;
            output_offset += 2;
        }

        // 4 trailing bytes with two padding char, or 2 trailing bytes
        (4, 0b1100) | (2, 0b0000) => {
            let chunk = output
                .get_mut(output_offset..output_offset + 1)
                .ok_or(Error::InsufficientOutputSpace)?;
            chunk[0] = (buffer >> 40) as u8;
            output_offset += 1;
        }

        // everything else is illegal
        _ => return Err(Error::InvalidTrailingPadding),
    }

    Ok(&output[..output_offset])
}

#[derive(Debug, PartialEq)]
pub(crate) enum Error {
    /// Given character is not valid in base64 alphabet.
    InvalidCharacter(u8),

    /// A padding character (`=`) appeared outside the final
    /// block of 4 characters.
    PrematurePadding,

    /// The padding characters at the end of the input were invalid.
    InvalidTrailingPadding,

    /// Not enough space in output buffer.
    ///
    /// Use `decoded_length` to get an upper bound.
    InsufficientOutputSpace,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct CodePoint(u8);

impl CodePoint {
    const WHITESPACE: Self = Self(0xf0);
    const PAD: Self = Self(0xf1);
    const INVALID: Self = Self(0xf2);
}

impl CodePoint {
    /// Side-channel rules:
    ///
    /// - code paths that produce `CodePoint(n)` must not make
    ///   `n` observable via a side channel.
    /// - other code paths -- whitespace, padding or invalid -- need not,
    ///   these are not considered secret conditions.
    fn decode_secret(b: u8) -> Self {
        let is_upper = u8_in_range(b, b'A', b'Z');
        let is_lower = u8_in_range(b, b'a', b'z');
        let is_digit = u8_in_range(b, b'0', b'9');
        let is_plus = u8_equals(b, b'+');
        let is_slash = u8_equals(b, b'/');
        let is_pad = u8_equals(b, b'=');
        let is_space = u8_in_range(b, b'\t', b'\r') | u8_equals(b, b' ');

        let is_invalid = !(is_lower | is_upper | is_digit | is_plus | is_slash | is_pad | is_space);

        Self(
            (is_upper & b.wrapping_sub(b'A'))
                | (is_lower & (b.wrapping_sub(b'a').wrapping_add(26)))
                | (is_digit & (b.wrapping_sub(b'0').wrapping_add(52)))
                | (is_plus & 62)
                | (is_slash & 63)
                | (is_space & Self::WHITESPACE.0)
                | (is_pad & Self::PAD.0)
                | (is_invalid & Self::INVALID.0),
        )
    }

    fn decode_public(a: u8) -> Self {
        const TABLE: [CodePoint; 256] = [
            // 0x00..0x0f
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::WHITESPACE,
            CodePoint::WHITESPACE,
            CodePoint::WHITESPACE,
            CodePoint::WHITESPACE,
            CodePoint::WHITESPACE,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0x10..0x1f
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0x20..0x2f
            CodePoint::WHITESPACE,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint(62),
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint(63),
            // 0x30..0x3f
            CodePoint(52),
            CodePoint(53),
            CodePoint(54),
            CodePoint(55),
            CodePoint(56),
            CodePoint(57),
            CodePoint(58),
            CodePoint(59),
            CodePoint(60),
            CodePoint(61),
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::PAD,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0x40..0x4f
            CodePoint::INVALID,
            CodePoint(0),
            CodePoint(1),
            CodePoint(2),
            CodePoint(3),
            CodePoint(4),
            CodePoint(5),
            CodePoint(6),
            CodePoint(7),
            CodePoint(8),
            CodePoint(9),
            CodePoint(10),
            CodePoint(11),
            CodePoint(12),
            CodePoint(13),
            CodePoint(14),
            // 0x50..0x5f
            CodePoint(15),
            CodePoint(16),
            CodePoint(17),
            CodePoint(18),
            CodePoint(19),
            CodePoint(20),
            CodePoint(21),
            CodePoint(22),
            CodePoint(23),
            CodePoint(24),
            CodePoint(25),
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0x60..0x6f
            CodePoint::INVALID,
            CodePoint(26),
            CodePoint(27),
            CodePoint(28),
            CodePoint(29),
            CodePoint(30),
            CodePoint(31),
            CodePoint(32),
            CodePoint(33),
            CodePoint(34),
            CodePoint(35),
            CodePoint(36),
            CodePoint(37),
            CodePoint(38),
            CodePoint(39),
            CodePoint(40),
            // 0x70..0x7f
            CodePoint(41),
            CodePoint(42),
            CodePoint(43),
            CodePoint(44),
            CodePoint(45),
            CodePoint(46),
            CodePoint(47),
            CodePoint(48),
            CodePoint(49),
            CodePoint(50),
            CodePoint(51),
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0x80..0x8f
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0x90..0x9f
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0xa0..0xaf
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0xb0..0xbf
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0xc0..0xcf
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0xd0..0xdf
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0xe0..0xef
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            // 0xf0..0xff
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
            CodePoint::INVALID,
        ];

        TABLE[a as usize]
    }
}

/// Returns 0xff if `a` in `lo..=hi`.
///
/// lo..=hi must not be 0..=255.  Callers in this file have constant
/// `lo` and `hi`, and this function is private to this file.
fn u8_in_range(a: u8, lo: u8, hi: u8) -> u8 {
    debug_assert!(lo <= hi);
    debug_assert!(hi - lo != 255);
    let a = a.wrapping_sub(lo);
    u8_less_than(a, (hi - lo).wrapping_add(1))
}

/// Returns 0xff if a < b, 0 otherwise.
fn u8_less_than(a: u8, b: u8) -> u8 {
    let a = u16::from(a);
    let b = u16::from(b);
    u8_broadcast16(a.wrapping_sub(b))
}

/// Returns 0xff if a == b, 0 otherwise.
fn u8_equals(a: u8, b: u8) -> u8 {
    let diff = a ^ b;
    u8_nonzero(diff)
}

/// Returns 0xff if a != 0, 0 otherwise.
fn u8_nonzero(x: u8) -> u8 {
    u8_broadcast8(!x & x.wrapping_sub(1))
}

/// Broadcasts the top bit of `x`
///
/// In other words, if the top bit of `x` is set,
/// returns 0xff else 0x00.
fn u8_broadcast8(x: u8) -> u8 {
    let msb = x >> 7;
    0u8.wrapping_sub(msb)
}

/// Broadcasts the top bit of `x`
///
/// In other words, if the top bit of `x` is set,
/// returns 0xff else 0x00.
fn u8_broadcast16(x: u16) -> u8 {
    let msb = x >> 15;
    0u8.wrapping_sub(msb as u8)
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn decode_test() {
        assert_eq!(
            decode(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"),
            b"\x00\x10\x83\x10\x51\x87\x20\x92\x8b\x30\xd3\x8f\x41\x14\x93\x51\x55\x97\
              \x61\x96\x9b\x71\xd7\x9f\x82\x18\xa3\x92\x59\xa7\xa2\x9a\xab\xb2\xdb\xaf\
              \xc3\x1c\xb3\xd3\x5d\xb7\xe3\x9e\xbb\xf3\xdf\xbf"
        );
        assert_eq!(decode(b"aGVsbG8="), b"hello");
        assert_eq!(decode(b"aGVsbG8gd29ybGQ="), b"hello world");
        assert_eq!(decode(b"aGVsbG8gd29ybGQh"), b"hello world!");
        assert_eq!(decode(b"////"), b"\xff\xff\xff");
        assert_eq!(decode(b"++++"), b"\xfb\xef\xbe");
        assert_eq!(decode(b"AAAA"), b"\x00\x00\x00");
        assert_eq!(decode(b"AAA="), b"\x00\x00");
        assert_eq!(decode(b"AA=="), b"\x00");

        // like our previous use of rust-base64, we don't require padding
        // if the encoding is otherwise valid given the length
        assert_eq!(decode(b"AAA"), b"\x00\x00");
        assert_eq!(decode(b"AA"), b"\x00");

        assert_eq!(decode(b""), b"");
    }

    #[test]
    fn decode_errors() {
        let mut buf = [0u8; 6];

        // illegal trailing padding
        assert_eq!(
            decode_both(b"A===", &mut buf),
            Err(Error::InvalidTrailingPadding)
        );
        assert_eq!(
            decode_both(b"====", &mut buf),
            Err(Error::InvalidTrailingPadding)
        );
        assert_eq!(
            decode_both(b"A==", &mut buf),
            Err(Error::InvalidTrailingPadding)
        );
        assert_eq!(
            decode_both(b"AA=", &mut buf),
            Err(Error::InvalidTrailingPadding)
        );
        assert_eq!(
            decode_both(b"A", &mut buf),
            Err(Error::InvalidTrailingPadding)
        );

        // padding before final block
        assert_eq!(
            decode_both(b"=AAAAA==", &mut buf),
            Err(Error::PrematurePadding)
        );
        assert_eq!(
            decode_both(b"A=AAAA==", &mut buf),
            Err(Error::PrematurePadding)
        );
        assert_eq!(
            decode_both(b"AA=AAA==", &mut buf),
            Err(Error::PrematurePadding)
        );
        assert_eq!(
            decode_both(b"AAA=AA==", &mut buf),
            Err(Error::PrematurePadding)
        );

        // illegal inputs
        assert_eq!(
            decode_both(b"%AAA", &mut buf),
            Err(Error::InvalidCharacter(b'%'))
        );
        assert_eq!(
            decode_both(b"A%AA", &mut buf),
            Err(Error::InvalidCharacter(b'%'))
        );
        assert_eq!(
            decode_both(b"AA%A", &mut buf),
            Err(Error::InvalidCharacter(b'%'))
        );
        assert_eq!(
            decode_both(b"AAA%", &mut buf),
            Err(Error::InvalidCharacter(b'%'))
        );

        // output sizing
        assert_eq!(decode_both(b"am9lIGJw", &mut [0u8; 7]), Ok(&b"joe bp"[..]));
        assert_eq!(decode_both(b"am9lIGJw", &mut [0u8; 6]), Ok(&b"joe bp"[..]));
        assert_eq!(
            decode_both(b"am9lIGJw", &mut [0u8; 5]),
            Err(Error::InsufficientOutputSpace)
        );
        assert_eq!(
            decode_both(b"am9lIGJw", &mut [0u8; 4]),
            Err(Error::InsufficientOutputSpace)
        );
        assert_eq!(
            decode_both(b"am9lIGJw", &mut [0u8; 3]),
            Err(Error::InsufficientOutputSpace)
        );

        // output sizing is not pessimistic when padding is valid
        assert_eq!(decode_both(b"am9=", &mut [0u8; 2]), Ok(&b"jo"[..]));
        assert_eq!(decode_both(b"am==", &mut [0u8; 1]), Ok(&b"j"[..]));
        assert_eq!(decode_both(b"am9", &mut [0u8; 2]), Ok(&b"jo"[..]));
        assert_eq!(decode_both(b"am", &mut [0u8; 1]), Ok(&b"j"[..]));
    }

    #[test]
    fn check_models() {
        fn u8_broadcast8_model(x: u8) -> u8 {
            match x & 0x80 {
                0x80 => 0xff,
                _ => 0x00,
            }
        }

        fn u8_broadcast16_model(x: u16) -> u8 {
            match x & 0x8000 {
                0x8000 => 0xff,
                _ => 0x00,
            }
        }

        fn u8_nonzero_model(x: u8) -> u8 {
            match x {
                0 => 0xff,
                _ => 0x00,
            }
        }

        fn u8_equals_model(x: u8, y: u8) -> u8 {
            match x == y {
                true => 0xff,
                false => 0x00,
            }
        }

        fn u8_in_range_model(x: u8, y: u8, z: u8) -> u8 {
            match (y..=z).contains(&x) {
                true => 0xff,
                false => 0x00,
            }
        }

        for x in u8::MIN..=u8::MAX {
            assert_eq!(u8_broadcast8(x), u8_broadcast8_model(x));
            assert_eq!(u8_nonzero(x), u8_nonzero_model(x));
            assert_eq!(CodePoint::decode_secret(x), CodePoint::decode_public(x));

            for y in u8::MIN..=u8::MAX {
                assert_eq!(u8_equals(x, y), u8_equals_model(x, y));

                let v = (x as u16) | ((y as u16) << 8);
                assert_eq!(u8_broadcast16(v), u8_broadcast16_model(v));

                for z in y..=u8::MAX {
                    if z - y == 255 {
                        continue;
                    }
                    assert_eq!(u8_in_range(x, y, z), u8_in_range_model(x, y, z));
                }
            }
        }
    }

    #[cfg(all(feature = "std", target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn codepoint_decode_secret_does_not_branch_or_index_on_secret_input() {
        // this is using the same theory as <https://github.com/agl/ctgrind>
        use crabgrind as cg;

        if matches!(cg::run_mode(), cg::RunMode::Native) {
            std::println!("SKIPPED: must be run under valgrind");
            return;
        }

        let input = [b'a'];
        cg::monitor_command(format!(
            "make_memory undefined {:p} {}",
            input.as_ptr(),
            input.len()
        ))
        .unwrap();

        core::hint::black_box(CodePoint::decode_secret(input[0]));
    }

    #[track_caller]
    fn decode(input: &[u8]) -> alloc::vec::Vec<u8> {
        let length = decoded_length(input.len());

        let mut v = alloc::vec![0u8; length];
        let used = decode_both(input, &mut v).unwrap().len();
        v.truncate(used);

        v
    }

    fn decode_both<'a>(input: &'_ [u8], output: &'a mut [u8]) -> Result<&'a [u8], Error> {
        let mut output_copy = output.to_vec();
        let r_pub = decode_public(input, &mut output_copy);

        let r_sec = decode_secret(input, output);

        assert_eq!(r_pub, r_sec);

        r_sec
    }
}
