use crate::iter::Bytes;
use core::arch::aarch64::*;

#[inline]
pub fn match_header_name_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 16 {
        // SAFETY: ensured that there are at least 16 bytes remaining 
        unsafe {
            let advance = match_header_name_char_16_neon(bytes.as_ref().as_ptr());
            bytes.advance(advance);

            if advance != 16 {
                return;
            }
        }
    }
    super::swar::match_header_name_vectored(bytes);
}

#[inline]
pub fn match_header_value_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 16 {
        // SAFETY: ensured that there are at least 16 bytes remaining 
        unsafe {
            let advance = match_header_value_char_16_neon(bytes.as_ref().as_ptr());
            bytes.advance(advance);

            if advance != 16 {
                return;
            }
        }
    }
    super::swar::match_header_value_vectored(bytes);
}

#[inline]
pub fn match_uri_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 16 {
        // SAFETY: ensured that there are at least 16 bytes remaining 
        unsafe {
            let advance = match_url_char_16_neon(bytes.as_ref().as_ptr());
            bytes.advance(advance);

            if advance != 16 {
                return;
            }
        }
    }
    super::swar::match_uri_vectored(bytes);
}

const fn bit_set(x: u8) -> bool {
    // Validates if a byte is a valid header name character
    // https://tools.ietf.org/html/rfc7230#section-3.2.6
    matches!(x, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
}

// A 256-bit bitmap, split into two halves
// lower half contains bits whose higher nibble is <= 7
// higher half contains bits whose higher nibble is >= 8
const fn build_bitmap() -> ([u8; 16], [u8; 16]) {
    let mut bitmap_0_7 = [0u8; 16]; // 0x00..0x7F
    let mut bitmap_8_15 = [0u8; 16]; // 0x80..0xFF
    let mut i = 0;
    while i < 256 {
        if bit_set(i as u8) {
            // Nibbles
            let (lo, hi) = (i & 0x0F, i >> 4);
            if i < 128 {
                bitmap_0_7[lo] |= 1 << hi;
            } else {
                bitmap_8_15[lo] |= 1 << hi;
            }
        }
        i += 1;
    }
    (bitmap_0_7, bitmap_8_15)
}

const BITMAPS: ([u8; 16], [u8; 16]) = build_bitmap();

// NOTE: adapted from 256-bit version, with upper 128-bit ops commented out
#[inline]
unsafe fn match_header_name_char_16_neon(ptr: *const u8) -> usize {
    let bitmaps = BITMAPS;
    // NOTE: ideally compile-time constants
    let (bitmap_0_7, _bitmap_8_15) = bitmaps;
    let bitmap_0_7 = vld1q_u8(bitmap_0_7.as_ptr());
    // let bitmap_8_15 = vld1q_u8(bitmap_8_15.as_ptr());

    // Initialize the bitmask_lookup.
    const BITMASK_LOOKUP_DATA: [u8; 16] =
        [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
    let bitmask_lookup = vld1q_u8(BITMASK_LOOKUP_DATA.as_ptr());

    // Load 16 input bytes.
    let input = vld1q_u8(ptr);

    // Extract indices for row_0_7.
    let indices_0_7 = vandq_u8(input, vdupq_n_u8(0x8F)); // 0b1000_1111;

    // Extract indices for row_8_15.
    // let msb = vandq_u8(input, vdupq_n_u8(0x80));
    // let indices_8_15 = veorq_u8(indices_0_7, msb);

    // Fetch row_0_7 and row_8_15.
    let row_0_7 = vqtbl1q_u8(bitmap_0_7, indices_0_7);
    // let row_8_15 = vqtbl1q_u8(bitmap_8_15, indices_8_15);

    // Calculate a bitmask, i.e. (1 << hi_nibble % 8).
    let bitmask = vqtbl1q_u8(bitmask_lookup, vshrq_n_u8(input, 4));

    // Choose rows halves depending on higher nibbles.
    // let bitsets = vorrq_u8(row_0_7, row_8_15);
    let bitsets = row_0_7;

    // Finally check which bytes belong to the set.
    let tmp = vandq_u8(bitsets, bitmask);
    let result = vceqq_u8(tmp, bitmask);

    offsetz(result) as usize
}

#[inline]
unsafe fn match_url_char_16_neon(ptr: *const u8) -> usize {
    let input = vld1q_u8(ptr);

    // Check that b'!' <= and b != 127
    let result = vcleq_u8(vdupq_n_u8(b'!'), input);

    // Disallow del
    let del = vceqq_u8(input, vdupq_n_u8(0x7F));
    let result = vbicq_u8(result, del);

    offsetz(result) as usize
}

#[inline]
unsafe fn match_header_value_char_16_neon(ptr: *const u8) -> usize {
    let input = vld1q_u8(ptr);

    // Check that b' ' <= and b != 127 or b == 9
    let result = vcleq_u8(vdupq_n_u8(b' '), input);

    // Allow tab
    let tab = vceqq_u8(input, vdupq_n_u8(0x09));
    let result = vorrq_u8(result, tab);

    // Disallow del
    let del = vceqq_u8(input, vdupq_n_u8(0x7F));
    let result = vbicq_u8(result, del);

    offsetz(result) as usize
}

#[inline]
unsafe fn offsetz(x: uint8x16_t) -> u32 {
    // NOT the vector since it's faster to operate with zeros instead
    offsetnz(vmvnq_u8(x))
}

#[inline]
unsafe fn offsetnz(x: uint8x16_t) -> u32 {
    // Extract two u64
    let x = vreinterpretq_u64_u8(x);
    // Extract to general purpose registers to perform clz
    let low: u64 = vgetq_lane_u64::<0>(x);
    let high: u64 = vgetq_lane_u64::<1>(x);

    #[inline]
    fn clz(x: u64) -> u32 {
        // perf: rust will unroll this loop
        // and it's much faster than rbit + clz so voila
        for (i, b) in x.to_ne_bytes().iter().copied().enumerate() {
            if b != 0 {
                return i as u32;
            }
        }
        8 // Technically not reachable since zero-guarded
    }

    if low != 0 {
        clz(low)
    } else if high != 0 {
        return 8 + clz(high);
    } else {
        return 16;
    }
}

#[test]
fn neon_code_matches_uri_chars_table() {
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        assert!(byte_is_allowed(b'_', match_uri_vectored));

        for (b, allowed) in crate::URI_MAP.iter().cloned().enumerate() {
            assert_eq!(
                byte_is_allowed(b as u8, match_uri_vectored),
                allowed,
                "byte_is_allowed({:?}) should be {:?}",
                b,
                allowed,
            );
        }
    }
}

#[test]
fn neon_code_matches_header_value_chars_table() {
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        assert!(byte_is_allowed(b'_', match_header_value_vectored));

        for (b, allowed) in crate::HEADER_VALUE_MAP.iter().cloned().enumerate() {
            assert_eq!(
                byte_is_allowed(b as u8, match_header_value_vectored),
                allowed,
                "byte_is_allowed({:?}) should be {:?}",
                b,
                allowed,
            );
        }
    }
}

#[test]
fn neon_code_matches_header_name_chars_table() {
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        assert!(byte_is_allowed(b'_', match_header_name_vectored));

        for (b, allowed) in crate::TOKEN_MAP.iter().cloned().enumerate() {
            assert_eq!(
                byte_is_allowed(b as u8, match_header_name_vectored),
                allowed,
                "byte_is_allowed({:?}) should be {:?}",
                b,
                allowed,
            );
        }
    }
}

#[cfg(test)]
unsafe fn byte_is_allowed(byte: u8, f: unsafe fn(bytes: &mut Bytes<'_>)) -> bool {
    let mut slice = [b'_'; 16];
    slice[10] = byte;
    let mut bytes = Bytes::new(&slice);

    f(&mut bytes);

    match bytes.pos() {
        16 => true,
        10 => false,
        x => panic!("unexpected pos: {}", x),
    }
}
