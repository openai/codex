#[cfg(target_feature = "simd128")]
#[allow(dead_code)]
pub(crate) mod simd128;

#[inline]
#[cfg(target_feature = "simd128")]
pub(crate) unsafe fn validate_utf8_basic(input: &[u8]) -> Result<(), crate::basic::Utf8Error> {
    if input.len() < super::helpers::SIMD_CHUNK_SIZE {
        return super::validate_utf8_basic_fallback(input);
    }

    validate_utf8_basic_simd128(input)
}

#[inline(never)]
#[cfg(target_feature = "simd128")]
unsafe fn validate_utf8_basic_simd128(input: &[u8]) -> Result<(), crate::basic::Utf8Error> {
    simd128::validate_utf8_basic(input)
}

#[cfg(not(target_feature = "simd128"))]
pub(crate) use super::validate_utf8_basic_fallback as validate_utf8_basic;

#[inline]
#[cfg(target_feature = "simd128")]
pub(crate) unsafe fn validate_utf8_compat(input: &[u8]) -> Result<(), crate::compat::Utf8Error> {
    if input.len() < super::helpers::SIMD_CHUNK_SIZE {
        return super::validate_utf8_compat_fallback(input);
    }

    validate_utf8_compat_simd128(input)
}

#[inline(never)]
#[cfg(target_feature = "simd128")]
unsafe fn validate_utf8_compat_simd128(input: &[u8]) -> Result<(), crate::compat::Utf8Error> {
    simd128::validate_utf8_compat(input)
}

#[cfg(not(target_feature = "simd128"))]
pub(crate) use super::validate_utf8_compat_fallback as validate_utf8_compat;
