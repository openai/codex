// Copyright 2024 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use crate::{
    bits::{BitLength, FromByteLen as _},
    polyfill::u64_from_usize,
};

#[test]
fn test_from_byte_len_overflow() {
    const USIZE_MAX_VALID_BYTES: usize = usize::MAX / 8;

    // Maximum valid input for BitLength<usize>.
    match BitLength::<usize>::from_byte_len(USIZE_MAX_VALID_BYTES) {
        Ok(bits) => {
            assert_eq!(bits.as_usize_bytes_rounded_up(), USIZE_MAX_VALID_BYTES);
            assert_eq!(bits.as_bits(), usize::MAX & !0b111);
        }
        Err(_) => unreachable!(),
    }

    // Minimum invalid usize input for BitLength<usize>.
    assert!(BitLength::<usize>::from_byte_len(USIZE_MAX_VALID_BYTES + 1).is_err());

    // Minimum invalid usize input for BitLength<u64> on 64-bit targets.
    {
        let r = BitLength::<u64>::from_byte_len(USIZE_MAX_VALID_BYTES + 1);
        if cfg!(target_pointer_width = "64") {
            assert!(r.is_err());
        } else {
            match r {
                Ok(bits) => {
                    assert_eq!(
                        bits.as_bits(),
                        (u64_from_usize(USIZE_MAX_VALID_BYTES) + 1) * 8
                    );
                }
                Err(_) => unreachable!(),
            }
        }
    }

    const U64_MAX_VALID_BYTES: u64 = u64::MAX / 8;

    // Maximum valid u64 input for BitLength<u64>.
    match BitLength::<u64>::from_byte_len(U64_MAX_VALID_BYTES) {
        Ok(bits) => assert_eq!(bits.as_bits(), u64::MAX & !0b111),
        Err(_) => unreachable!(),
    };

    // Minimum invalid usize input for BitLength<u64> on 64-bit targets.
    assert!(BitLength::<u64>::from_byte_len(U64_MAX_VALID_BYTES + 1).is_err());
}
