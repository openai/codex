//! Stable "polyfills" for unstable `core::arch::aarch64` intrinsics which use
//! `asm!` internally to allow use on stable Rust.
// TODO(tarcieri): remove when these intrinsics have been stabilized

use core::arch::{aarch64::uint8x16_t, asm};

/// AES single round encryption.
#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn vaeseq_u8(mut data: uint8x16_t, key: uint8x16_t) -> uint8x16_t {
    asm!(
        "AESE {d:v}.16B, {k:v}.16B",
        d = inout(vreg) data,
        k = in(vreg) key,
        options(pure, nomem, nostack, preserves_flags)
    );
    data
}

/// AES single round decryption.
#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn vaesdq_u8(mut data: uint8x16_t, key: uint8x16_t) -> uint8x16_t {
    asm!(
        "AESD {d:v}.16B, {k:v}.16B",
        d = inout(vreg) data,
        k = in(vreg) key,
        options(pure, nomem, nostack, preserves_flags)
    );
    data
}

/// AES mix columns.
#[cfg(feature = "hazmat")]
#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn vaesmcq_u8(mut data: uint8x16_t) -> uint8x16_t {
    asm!(
        "AESMC {d:v}.16B, {d:v}.16B",
        d = inout(vreg) data,
        options(pure, nomem, nostack, preserves_flags)
    );
    data
}

/// AES inverse mix columns.
#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn vaesimcq_u8(mut data: uint8x16_t) -> uint8x16_t {
    asm!(
        "AESIMC {d:v}.16B, {d:v}.16B",
        d = inout(vreg) data,
        options(pure, nomem, nostack, preserves_flags)
    );
    data
}

/// AES single round encryption combined with mix columns.
///
/// These two instructions are combined into a single assembly block to ensure
/// that instructions fuse properly.
#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn vaeseq_u8_and_vaesmcq_u8(mut data: uint8x16_t, key: uint8x16_t) -> uint8x16_t {
    asm!(
        "AESE {d:v}.16B, {k:v}.16B",
        "AESMC {d:v}.16B, {d:v}.16B",
        d = inout(vreg) data,
        k = in(vreg) key,
        options(pure, nomem, nostack, preserves_flags)
    );
    data
}

/// AES single round decryption combined with mix columns.
///
/// These two instructions are combined into a single assembly block to ensure
/// that instructions fuse properly.
#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn vaesdq_u8_and_vaesimcq_u8(
    mut data: uint8x16_t,
    key: uint8x16_t,
) -> uint8x16_t {
    asm!(
        "AESD {d:v}.16B, {k:v}.16B",
        "AESIMC {d:v}.16B, {d:v}.16B",
        d = inout(vreg) data,
        k = in(vreg) key,
        options(pure, nomem, nostack, preserves_flags)
    );
    data
}
