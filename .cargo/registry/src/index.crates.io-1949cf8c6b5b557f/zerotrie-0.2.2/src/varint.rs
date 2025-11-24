// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Varint spec for ZeroTrie:
//!
//! - Lead byte: top M (2 or 3) bits are metadata; next is varint extender; rest is value
//! - Trail bytes: top bit is varint extender; rest are low bits of value
//! - Guaranteed uniqueness of varint by adding "latent value" for each extender byte
//! - No maximum, but high bits will be dropped if they don't fit in the platform's `usize`
//!
//! This is best shown by examples.
//!
//! ```txt
//! xxx0'1010 = 10
//! xxx0'1111 = 15 (largest single-byte value with M=3)
//! xxx1'0000 0000'0000 must be 16 (smallest two-byte value with M=3)
//! xxx1'0000 0000'0001 = 17
//! xxx1'1111 0111'1111 = 2063 (largest two-byte value with M=3)
//! xxx1'0000 1000'0000 0000'0000 must be 2064 (smallest three-byte value with M=3)
//! xxx1'0000 1000'0000 0000'0001 = 2065
//! ```
//!
//! The latent values by number of bytes for M=3 are:
//!
//! - 1 byte: 0
//! - 2 bytes: 16 = 0x10 = 0b10000
//! - 3 bytes: 2064 = 0x810 = 0b100000010000
//! - 4 bytes: 264208 = 0x40810 = 0b1000000100000010000
//! - 5 bytes: 33818640 = 0x2040810 = 0b10000001000000100000010000
//! - …
//!
//! For M=2, the latent values are:
//!
//! - 1 byte: 0
//! - 2 bytes: 32 = 0x20 = 0b100000
//! - 3 bytes: 4128 = 0x1020 = 0b1000000100000
//! - 4 bytes: 524320 = 0x81020 = 0b10000001000000100000
//! - 5 bytes: 67637280 = 0x4081020 = 0b100000010000001000000100000
//! - …

use crate::builder::konst::ConstArrayBuilder;

#[cfg(feature = "alloc")]
use crate::builder::nonconst::TrieBuilderStore;

/// Reads a varint with 2 bits of metadata in the lead byte.
///
/// Returns the varint value and a subslice of `remainder` with the varint bytes removed.
///
/// If the varint spills off the end of the slice, a debug assertion will fail,
/// and the function will return the value up to that point.
pub const fn read_varint_meta2(start: u8, remainder: &[u8]) -> (usize, &[u8]) {
    let mut value = (start & 0b00011111) as usize;
    let mut remainder = remainder;
    if (start & 0b00100000) != 0 {
        loop {
            let next;
            (next, remainder) = debug_unwrap!(remainder.split_first(), break, "invalid varint");
            // Note: value << 7 could drop high bits. The first addition can't overflow.
            // The second addition could overflow; in such a case we just inform the
            // developer via the debug assertion.
            value = (value << 7) + ((*next & 0b01111111) as usize) + 32;
            if (*next & 0b10000000) == 0 {
                break;
            }
        }
    }
    (value, remainder)
}

/// Reads a varint with 3 bits of metadata in the lead byte.
///
/// Returns the varint value and a subslice of `remainder` with the varint bytes removed.
///
/// If the varint spills off the end of the slice, a debug assertion will fail,
/// and the function will return the value up to that point.
pub const fn read_varint_meta3(start: u8, remainder: &[u8]) -> (usize, &[u8]) {
    let mut value = (start & 0b00001111) as usize;
    let mut remainder = remainder;
    if (start & 0b00010000) != 0 {
        loop {
            let next;
            (next, remainder) = debug_unwrap!(remainder.split_first(), break, "invalid varint");
            // Note: value << 7 could drop high bits. The first addition can't overflow.
            // The second addition could overflow; in such a case we just inform the
            // developer via the debug assertion.
            value = (value << 7) + ((*next & 0b01111111) as usize) + 16;
            if (*next & 0b10000000) == 0 {
                break;
            }
        }
    }
    (value, remainder)
}

/// Reads and removes a varint with 3 bits of metadata from a [`TrieBuilderStore`].
///
/// Returns the varint value.
#[cfg(feature = "alloc")]
pub(crate) fn try_read_varint_meta3_from_tstore<S: TrieBuilderStore>(
    start: u8,
    remainder: &mut S,
) -> Option<usize> {
    let mut value = (start & 0b00001111) as usize;
    if (start & 0b00010000) != 0 {
        loop {
            let next = remainder.atbs_pop_front()?;
            // Note: value << 7 could drop high bits. The first addition can't overflow.
            // The second addition could overflow; in such a case we just inform the
            // developer via the debug assertion.
            value = (value << 7) + ((next & 0b01111111) as usize) + 16;
            if (next & 0b10000000) == 0 {
                break;
            }
        }
    }
    Some(value)
}

#[cfg(test)]
const MAX_VARINT: usize = usize::MAX;

// *Upper Bound:* Each trail byte stores 7 bits of data, plus the latent value.
// Add an extra 1 since the lead byte holds only 5 bits of data.
const MAX_VARINT_LENGTH: usize = 1 + core::mem::size_of::<usize>() * 8 / 7;

/// Returns a new [`ConstArrayBuilder`] containing a varint with 2 bits of metadata.
pub(crate) const fn write_varint_meta2(value: usize) -> ConstArrayBuilder<MAX_VARINT_LENGTH, u8> {
    let mut result = [0; MAX_VARINT_LENGTH];
    let mut i = MAX_VARINT_LENGTH - 1;
    let mut value = value;
    let mut last = true;
    loop {
        if value < 32 {
            result[i] = value as u8;
            if !last {
                result[i] |= 0b00100000;
            }
            break;
        }
        value -= 32;
        result[i] = (value as u8) & 0b01111111;
        if !last {
            result[i] |= 0b10000000;
        } else {
            last = false;
        }
        value >>= 7;
        i -= 1;
    }
    // The bytes are from i to the end.
    ConstArrayBuilder::from_manual_slice(result, i, MAX_VARINT_LENGTH)
}

/// Returns a new [`ConstArrayBuilder`] containing a varint with 3 bits of metadata.
pub(crate) const fn write_varint_meta3(value: usize) -> ConstArrayBuilder<MAX_VARINT_LENGTH, u8> {
    let mut result = [0; MAX_VARINT_LENGTH];
    let mut i = MAX_VARINT_LENGTH - 1;
    let mut value = value;
    let mut last = true;
    loop {
        if value < 16 {
            result[i] = value as u8;
            if !last {
                result[i] |= 0b00010000;
            }
            break;
        }
        value -= 16;
        result[i] = (value as u8) & 0b01111111;
        if !last {
            result[i] |= 0b10000000;
        } else {
            last = false;
        }
        value >>= 7;
        i -= 1;
    }
    // The bytes are from i to the end.
    ConstArrayBuilder::from_manual_slice(result, i, MAX_VARINT_LENGTH)
}

/// A secondary implementation that separates the latent value while computing the varint.
#[cfg(test)]
pub(crate) const fn write_varint_reference(
    value: usize,
) -> ConstArrayBuilder<MAX_VARINT_LENGTH, u8> {
    let mut result = [0; MAX_VARINT_LENGTH];
    if value < 32 {
        result[0] = value as u8;
        return ConstArrayBuilder::from_manual_slice(result, 0, 1);
    }
    result[0] = 32;
    let mut latent = 32;
    let mut steps = 2;
    loop {
        let next_latent = (latent << 7) + 32;
        if value < next_latent || next_latent == latent {
            break;
        }
        latent = next_latent;
        steps += 1;
    }
    let mut value = value - latent;
    let mut i = steps;
    while i > 0 {
        i -= 1;
        result[i] |= (value as u8) & 0b01111111;
        value >>= 7;
        if i > 0 && i < steps - 1 {
            result[i] |= 0b10000000;
        }
    }
    // The bytes are from 0 to `steps`.
    ConstArrayBuilder::from_manual_slice(result, 0, steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestCase<'a> {
        bytes: &'a [u8],
        remainder: &'a [u8],
        value: usize,
    }
    static CASES: &[TestCase] = &[
        TestCase {
            bytes: &[0b00000000],
            remainder: &[],
            value: 0,
        },
        TestCase {
            bytes: &[0b00001010],
            remainder: &[],
            value: 10,
        },
        TestCase {
            bytes: &[0b00011111],
            remainder: &[],
            value: 31,
        },
        TestCase {
            bytes: &[0b00011111, 0b10101010],
            remainder: &[0b10101010],
            value: 31,
        },
        TestCase {
            bytes: &[0b00100000, 0b00000000],
            remainder: &[],
            value: 32,
        },
        TestCase {
            bytes: &[0b00100000, 0b00000001],
            remainder: &[],
            value: 33,
        },
        TestCase {
            bytes: &[0b00100000, 0b00100000],
            remainder: &[],
            value: 64,
        },
        TestCase {
            bytes: &[0x20, 0x44],
            remainder: &[],
            value: 100,
        },
        TestCase {
            bytes: &[0b00100000, 0b01111111],
            remainder: &[],
            value: 159,
        },
        TestCase {
            bytes: &[0b00100001, 0b00000000],
            remainder: &[],
            value: 160,
        },
        TestCase {
            bytes: &[0b00100001, 0b00000001],
            remainder: &[],
            value: 161,
        },
        TestCase {
            bytes: &[0x23, 0x54],
            remainder: &[],
            value: 500,
        },
        TestCase {
            bytes: &[0b00111111, 0b01111111],
            remainder: &[],
            value: 4127, // 32 + (1 << 12) - 1
        },
        TestCase {
            bytes: &[0b00100000, 0b10000000, 0b00000000],
            remainder: &[],
            value: 4128, // 32 + (1 << 12)
        },
        TestCase {
            bytes: &[0b00100000, 0b10000000, 0b00000001],
            remainder: &[],
            value: 4129, // 32 + (1 << 12) + 1
        },
        TestCase {
            bytes: &[0b00100000, 0b10000000, 0b01111111],
            remainder: &[],
            value: 4255, // 32 + (1 << 12) + 127
        },
        TestCase {
            bytes: &[0b00100000, 0b10000001, 0b00000000],
            remainder: &[],
            value: 4256, // 32 + (1 << 12) + 128
        },
        TestCase {
            bytes: &[0b00100000, 0b10000001, 0b00000001],
            remainder: &[],
            value: 4257, // 32 + (1 << 12) + 129
        },
        TestCase {
            bytes: &[0x20, 0x86, 0x68],
            remainder: &[],
            value: 5000,
        },
        TestCase {
            bytes: &[0b00100000, 0b11111111, 0b01111111],
            remainder: &[],
            value: 20511, // 32 + (1 << 12) + (1 << 14) - 1
        },
        TestCase {
            bytes: &[0b00100001, 0b10000000, 0b00000000],
            remainder: &[],
            value: 20512, // 32 + (1 << 12) + (1 << 14)
        },
        TestCase {
            bytes: &[0b00111111, 0b11111111, 0b01111111],
            remainder: &[],
            value: 528415, // 32 + (1 << 12) + (1 << 19) - 1
        },
        TestCase {
            bytes: &[0b00100000, 0b10000000, 0b10000000, 0b00000000],
            remainder: &[],
            value: 528416, // 32 + (1 << 12) + (1 << 19)
        },
        TestCase {
            bytes: &[0b00100000, 0b10000000, 0b10000000, 0b00000001],
            remainder: &[],
            value: 528417, // 32 + (1 << 12) + (1 << 19) + 1
        },
        TestCase {
            bytes: &[0b00111111, 0b11111111, 0b11111111, 0b01111111],
            remainder: &[],
            value: 67637279, // 32 + (1 << 12) + (1 << 19) + (1 << 26) - 1
        },
        TestCase {
            bytes: &[0b00100000, 0b10000000, 0b10000000, 0b10000000, 0b00000000],
            remainder: &[],
            value: 67637280, // 32 + (1 << 12) + (1 << 19) + (1 << 26)
        },
    ];

    #[test]
    fn test_read() {
        for cas in CASES {
            let recovered = read_varint_meta2(cas.bytes[0], &cas.bytes[1..]);
            assert_eq!(recovered, (cas.value, cas.remainder), "{:?}", cas);
        }
    }

    #[test]
    fn test_read_write() {
        for cas in CASES {
            let reference_bytes = write_varint_reference(cas.value);
            assert_eq!(
                reference_bytes.len(),
                cas.bytes.len() - cas.remainder.len(),
                "{:?}",
                cas
            );
            assert_eq!(
                reference_bytes.as_slice(),
                &cas.bytes[0..reference_bytes.len()],
                "{:?}",
                cas
            );
            let recovered = read_varint_meta2(cas.bytes[0], &cas.bytes[1..]);
            assert_eq!(recovered, (cas.value, cas.remainder), "{:?}", cas);
            let write_bytes = write_varint_meta2(cas.value);
            assert_eq!(
                reference_bytes.as_slice(),
                write_bytes.as_slice(),
                "{:?}",
                cas
            );
        }
    }

    #[test]
    fn test_roundtrip() {
        let mut i = 0usize;
        while i < MAX_VARINT {
            let bytes = write_varint_meta2(i);
            let recovered = read_varint_meta2(bytes.as_slice()[0], &bytes.as_slice()[1..]);
            assert_eq!(i, recovered.0, "{:?}", bytes.as_slice());
            i <<= 1;
            i += 1;
        }
    }

    #[test]
    fn test_extended_roundtrip() {
        let mut i = 0usize;
        while i < MAX_VARINT {
            let bytes = write_varint_meta3(i);
            let recovered = read_varint_meta3(bytes.as_slice()[0], &bytes.as_slice()[1..]);
            assert_eq!(i, recovered.0, "{:?}", bytes.as_slice());
            i <<= 1;
            i += 1;
        }
    }

    #[test]
    fn test_max() {
        let reference_bytes = write_varint_reference(MAX_VARINT);
        let write_bytes = write_varint_meta2(MAX_VARINT);
        assert_eq!(reference_bytes.len(), MAX_VARINT_LENGTH);
        assert_eq!(reference_bytes.as_slice(), write_bytes.as_slice());
        let subarray = write_bytes
            .as_const_slice()
            .get_subslice_or_panic(1, write_bytes.len());
        let (recovered_value, remainder) = read_varint_meta2(
            *write_bytes.as_const_slice().first().unwrap(),
            subarray.as_slice(),
        );
        assert!(remainder.is_empty());
        assert_eq!(recovered_value, MAX_VARINT);
        assert_eq!(
            write_bytes.as_slice(),
            &[
                0b00100001, //
                0b11011111, //
                0b11011111, //
                0b11011111, //
                0b11011111, //
                0b11011111, //
                0b11011111, //
                0b11011111, //
                0b11011111, //
                0b01011111, //
            ]
        );
    }

    #[test]
    fn text_extended_max() {
        let write_bytes = write_varint_meta3(MAX_VARINT);
        assert_eq!(write_bytes.len(), MAX_VARINT_LENGTH);
        let (lead, trailing) = write_bytes.as_slice().split_first().unwrap();
        let (recovered_value, remainder) = read_varint_meta3(*lead, trailing);
        assert!(remainder.is_empty());
        assert_eq!(recovered_value, MAX_VARINT);
        assert_eq!(
            write_bytes.as_slice(),
            &[
                0b00010001, //
                0b11101111, //
                0b11101111, //
                0b11101111, //
                0b11101111, //
                0b11101111, //
                0b11101111, //
                0b11101111, //
                0b11101111, //
                0b01101111, //
            ]
        );
    }

    #[test]
    fn test_latent_values() {
        // Same values documented in the module docs: M=2
        let m2 = read_varint_meta2;
        assert_eq!(m2(0, &[]).0, 0);
        assert_eq!(m2(0x20, &[0x00]).0, 32);
        assert_eq!(m2(0x20, &[0x80, 0x00]).0, 4128);
        assert_eq!(m2(0x20, &[0x80, 0x80, 0x00]).0, 528416);
        assert_eq!(m2(0x20, &[0x80, 0x80, 0x80, 0x00]).0, 67637280);

        // Same values documented in the module docs: M=3
        let m3 = read_varint_meta3;
        assert_eq!(m3(0, &[]).0, 0);
        assert_eq!(m3(0x10, &[0x00]).0, 16);
        assert_eq!(m3(0x10, &[0x80, 0x00]).0, 2064);
        assert_eq!(m3(0x10, &[0x80, 0x80, 0x00]).0, 264208);
        assert_eq!(m3(0x10, &[0x80, 0x80, 0x80, 0x00]).0, 33818640);
    }
}
