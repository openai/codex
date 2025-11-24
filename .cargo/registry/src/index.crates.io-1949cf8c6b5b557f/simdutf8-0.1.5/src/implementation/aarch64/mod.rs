#[cfg(any(feature = "aarch64_neon", target_feature = "neon"))]
#[allow(dead_code)]
pub(crate) mod neon;

#[inline]
#[cfg(any(feature = "aarch64_neon", target_feature = "neon"))]
pub(crate) unsafe fn validate_utf8_basic(input: &[u8]) -> Result<(), crate::basic::Utf8Error> {
    if input.len() < super::helpers::SIMD_CHUNK_SIZE {
        return super::validate_utf8_basic_fallback(input);
    }

    validate_utf8_basic_neon(input)
}

#[inline(never)]
#[cfg(any(feature = "aarch64_neon", target_feature = "neon"))]
unsafe fn validate_utf8_basic_neon(input: &[u8]) -> Result<(), crate::basic::Utf8Error> {
    neon::validate_utf8_basic(input)
}

#[cfg(not(any(feature = "aarch64_neon", target_feature = "neon")))]
pub(crate) use super::validate_utf8_basic_fallback as validate_utf8_basic;

#[inline]
#[cfg(any(feature = "aarch64_neon", target_feature = "neon"))]
pub(crate) unsafe fn validate_utf8_compat(input: &[u8]) -> Result<(), crate::compat::Utf8Error> {
    if input.len() < super::helpers::SIMD_CHUNK_SIZE {
        return super::validate_utf8_compat_fallback(input);
    }

    validate_utf8_compat_neon(input)
}

#[inline(never)]
#[cfg(any(feature = "aarch64_neon", target_feature = "neon"))]
unsafe fn validate_utf8_compat_neon(input: &[u8]) -> Result<(), crate::compat::Utf8Error> {
    neon::validate_utf8_compat(input)
}

#[cfg(not(any(feature = "aarch64_neon", target_feature = "neon")))]
pub(crate) use super::validate_utf8_compat_fallback as validate_utf8_compat;
