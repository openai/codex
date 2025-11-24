// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Little-Endian utilities
//!
//! Little-Endian order has been chosen for internal usage; this makes some
//! useful functions available.

use core::convert::TryInto;

/// Reads unsigned 32 bit integers from `src` into `dst`.
#[inline]
pub fn read_u32_into(src: &[u8], dst: &mut [u32]) {
    assert!(src.len() >= 4 * dst.len());
    for (out, chunk) in dst.iter_mut().zip(src.chunks_exact(4)) {
        *out = u32::from_le_bytes(chunk.try_into().unwrap());
    }
}

/// Reads unsigned 64 bit integers from `src` into `dst`.
#[inline]
pub fn read_u64_into(src: &[u8], dst: &mut [u64]) {
    assert!(src.len() >= 8 * dst.len());
    for (out, chunk) in dst.iter_mut().zip(src.chunks_exact(8)) {
        *out = u64::from_le_bytes(chunk.try_into().unwrap());
    }
}

#[test]
fn test_read() {
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    let mut buf = [0u32; 4];
    read_u32_into(&bytes, &mut buf);
    assert_eq!(buf[0], 0x04030201);
    assert_eq!(buf[3], 0x100F0E0D);

    let mut buf = [0u32; 3];
    read_u32_into(&bytes[1..13], &mut buf); // unaligned
    assert_eq!(buf[0], 0x05040302);
    assert_eq!(buf[2], 0x0D0C0B0A);

    let mut buf = [0u64; 2];
    read_u64_into(&bytes, &mut buf);
    assert_eq!(buf[0], 0x0807060504030201);
    assert_eq!(buf[1], 0x100F0E0D0C0B0A09);

    let mut buf = [0u64; 1];
    read_u64_into(&bytes[7..15], &mut buf); // unaligned
    assert_eq!(buf[0], 0x0F0E0D0C0B0A0908);
}
