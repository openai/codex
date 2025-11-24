use crate::iter::Bytes;

#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn match_uri_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 32 {

        let advance = match_url_char_32_avx(bytes.as_ref());

        bytes.advance(advance);

        if advance != 32 {
            return;
        }
    }
    // NOTE: use SWAR for <32B, more efficient than falling back to SSE4.2
    super::swar::match_uri_vectored(bytes)
}

#[inline(always)]
#[allow(non_snake_case, overflowing_literals)]
#[allow(unused)]
unsafe fn match_url_char_32_avx(buf: &[u8]) -> usize {
    // NOTE: This check might be not necessary since this function is only used in
    // `match_uri_vectored` where buffer overflow is taken care of.
    debug_assert!(buf.len() >= 32);

    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    // pointer to buffer
    let ptr = buf.as_ptr();

    // %x21-%x7e %x80-%xff
    //
    // Character ranges allowed by this function, can also be interpreted as:
    // 33 =< (x != 127) =< 255
    //
    // Create a vector full of DEL (0x7f) characters.
    let DEL: __m256i = _mm256_set1_epi8(0x7f);
    // Create a vector full of exclamation mark (!) (0x21) characters.
    // Used as lower threshold, characters in URLs cannot be smaller than this.
    let LOW: __m256i = _mm256_set1_epi8(0x21);

    // Load a chunk of 32 bytes from `ptr` as a vector.
    // We can check 32 bytes in parallel at most with AVX2 since
    // YMM registers can only have 256 bits most.
    let dat = _mm256_lddqu_si256(ptr as *const _);

    // unsigned comparison dat >= LOW
    //
    // `_mm256_max_epu8` creates a new vector by comparing vectors `dat` and `LOW`
    // and picks the max. values from each for all indices.
    // So if a byte in `dat` is <= 32, it'll be represented as 33
    // which is the smallest valid character.
    //
    // Then, we compare the new vector with `dat` for equality.
    //
    // `_mm256_cmpeq_epi8` returns a new vector where;
    // * matching bytes are set to 0xFF (all bits set),
    // * nonmatching bytes are set to 0 (no bits set).
    let low = _mm256_cmpeq_epi8(_mm256_max_epu8(dat, LOW), dat);
    // Similar to what we did before, but now invalid characters are set to 0xFF.
    let del = _mm256_cmpeq_epi8(dat, DEL);

    // We glue the both comparisons via `_mm256_andnot_si256`.
    //
    // Since the representation of truthiness differ in these comparisons,
    // we are in need of bitwise NOT to convert valid characters of `del`.
    let bit = _mm256_andnot_si256(del, low);
    // This creates a bitmask from the most significant bit of each byte.
    // Simply, we're converting a vector value to scalar value here.
    let res = _mm256_movemask_epi8(bit) as u32;

    // Count trailing zeros to find the first encountered invalid character.
    // Bitwise NOT is required once again to flip truthiness.
    // TODO: use .trailing_ones() once MSRV >= 1.46
    (!res).trailing_zeros() as usize
}

#[target_feature(enable = "avx2")]
pub unsafe fn match_header_value_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 32 {
        let advance = match_header_value_char_32_avx(bytes.as_ref());
        bytes.advance(advance);

        if advance != 32 {
            return;
        }
    }
    // NOTE: use SWAR for <32B, more efficient than falling back to SSE4.2
    super::swar::match_header_value_vectored(bytes)
}

#[inline(always)]
#[allow(non_snake_case)]
#[allow(unused)]
unsafe fn match_header_value_char_32_avx(buf: &[u8]) -> usize {
    debug_assert!(buf.len() >= 32);

    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    let ptr = buf.as_ptr();

    // %x09 %x20-%x7e %x80-%xff
    // Create a vector full of horizontal tab (\t) (0x09) characters.
    let TAB: __m256i = _mm256_set1_epi8(0x09);
    // Create a vector full of DEL (0x7f) characters.
    let DEL: __m256i = _mm256_set1_epi8(0x7f);
    // Create a vector full of space (0x20) characters.
    let LOW: __m256i = _mm256_set1_epi8(0x20);

    // Load a chunk of 32 bytes from `ptr` as a vector.
    let dat = _mm256_lddqu_si256(ptr as *const _);

    // unsigned comparison dat >= LOW
    //
    // Same as what we do in `match_url_char_32_avx`.
    // This time the lower threshold is set to space character though.
    let low = _mm256_cmpeq_epi8(_mm256_max_epu8(dat, LOW), dat);
    // Check if `dat` includes `TAB` characters.
    let tab = _mm256_cmpeq_epi8(dat, TAB);
    // Check if `dat` includes `DEL` characters.
    let del = _mm256_cmpeq_epi8(dat, DEL);

    // Combine all comparisons together, notice that we're also using OR
    // to connect `low` and `tab` but flip bits of `del`.
    //
    // In the end, this is simply:
    // ~del & (low | tab)
    let bit = _mm256_andnot_si256(del, _mm256_or_si256(low, tab));
    // This creates a bitmask from the most significant bit of each byte.
    // Creates a scalar value from vector value.
    let res = _mm256_movemask_epi8(bit) as u32;

    // Count trailing zeros to find the first encountered invalid character.
    // Bitwise NOT is required once again to flip truthiness.
    // TODO: use .trailing_ones() once MSRV >= 1.46
    (!res).trailing_zeros() as usize
}

#[test]
fn avx2_code_matches_uri_chars_table() {
    if !is_x86_feature_detected!("avx2") {
        return;
    }

    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        assert!(byte_is_allowed(b'_', match_uri_vectored));

        for (b, allowed) in crate::URI_MAP.iter().cloned().enumerate() {
            assert_eq!(
                byte_is_allowed(b as u8, match_uri_vectored), allowed,
                "byte_is_allowed({:?}) should be {:?}", b, allowed,
            );
        }
    }
}

#[test]
fn avx2_code_matches_header_value_chars_table() {
    if !is_x86_feature_detected!("avx2") {
        return;
    }

    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
        assert!(byte_is_allowed(b'_', match_header_value_vectored));

        for (b, allowed) in crate::HEADER_VALUE_MAP.iter().cloned().enumerate() {
            assert_eq!(
                byte_is_allowed(b as u8, match_header_value_vectored), allowed,
                "byte_is_allowed({:?}) should be {:?}", b, allowed,
            );
        }
    }
}

#[cfg(test)]
unsafe fn byte_is_allowed(byte: u8, f: unsafe fn(bytes: &mut Bytes<'_>)) -> bool {
    let slice = [
        b'_', b'_', b'_', b'_',
        b'_', b'_', b'_', b'_',
        b'_', b'_', b'_', b'_',
        b'_', b'_', b'_', b'_',
        b'_', b'_', b'_', b'_',
        b'_', b'_', b'_', b'_',
        b'_', b'_', byte, b'_',
        b'_', b'_', b'_', b'_',
    ];
    let mut bytes = Bytes::new(&slice);

    f(&mut bytes);

    match bytes.pos() {
        32 => true,
        26 => false,
        _ => unreachable!(),
    }
}
