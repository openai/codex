// Copyright 2016-2024 Brian Smith.
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

use super::CAPS_STATIC;

mod abi_assumptions {
    use core::mem::size_of;

    // TODO: Support ARM64_32; see
    // https://github.com/briansmith/ring/issues/1832#issuecomment-1892928147. This also requires
    // replacing all `cfg(target_pointer_width)` logic for non-pointer/reference things
    // (`N0`, `Limb`, `LimbMask`, `crypto_word_t` etc.).
    #[cfg(target_arch = "aarch64")]
    const _ASSUMED_POINTER_SIZE: usize = 8;
    #[cfg(target_arch = "arm")]
    const _ASSUMED_POINTER_SIZE: usize = 4;
    const _ASSUMED_USIZE_SIZE: () = assert!(size_of::<usize>() == _ASSUMED_POINTER_SIZE);
    const _ASSUMED_REF_SIZE: () = assert!(size_of::<&'static u8>() == _ASSUMED_POINTER_SIZE);

    // To support big-endian, we'd need to make several changes as described in
    // https://github.com/briansmith/ring/issues/1832.
    const _ASSUMED_ENDIANNESS: () = assert!(cfg!(target_endian = "little"));
}

// uclibc: When linked statically, uclibc doesn't provide getauxval.
// When linked dynamically, recent versions do provide it, but we
// want to support older versions too. Assume that if uclibc is being
// used, this is an embedded target where the user cares a lot about
// minimizing code size and also that they know in advance exactly
// what target features are supported, so rely only on static feature
// detection.

cfg_if::cfg_if! {
    if #[cfg(all(all(target_arch = "aarch64", target_endian = "little"),
                 any(target_os = "ios", target_os = "macos", target_os = "tvos", target_os = "visionos", target_os = "watchos")))] {
        mod darwin;
        use darwin as detect;
    } else if #[cfg(all(all(target_arch = "aarch64", target_endian = "little"), target_os = "fuchsia"))] {
        mod fuchsia;
        use fuchsia as detect;
    } else if #[cfg(any(target_os = "android", target_os = "linux"))] {
        mod linux;
        use linux as detect;
    } else if #[cfg(all(all(target_arch = "aarch64", target_endian = "little"), target_os = "windows"))] {
        mod windows;
        use windows as detect;
    } else {
        mod detect {
            pub const FORCE_DYNAMIC_DETECTION: u32 = 0;
            pub fn detect_features() -> u32 { 0 }
        }
    }
}

impl_get_feature! {
    features: [
        // TODO(MSRV): 32-bit ARM doesn't have `target_feature = "neon"` yet.
        { ("aarch64", "arm") => Neon },

        // TODO(MSRV): There is no "pmull" feature listed from
        // `rustc --print cfg --target=aarch64-apple-darwin`. Originally ARMv8 tied
        // PMULL detection into AES detection, but later versions split it; see
        // https://developer.arm.com/downloads/-/exploration-tools/feature-names-for-a-profile
        // "Features introduced prior to 2020." Change this to use "pmull" when
        // that is supported.
        { ("aarch64") => PMull },

        { ("aarch64") => Aes },

        { ("aarch64") => Sha256 },

        // Keep in sync with `ARMV8_SHA512`.

        // "sha3" is overloaded for both SHA-3 and SHA-512.
        { ("aarch64") => Sha512 },
    ],
}

pub(super) mod featureflags {
    pub(in super::super) use super::detect::FORCE_DYNAMIC_DETECTION;
    use super::*;
    use crate::{
        cpu,
        polyfill::{once_cell::race, usize_from_u32},
    };
    use core::num::NonZeroUsize;
    #[cfg(all(target_arch = "arm", target_endian = "little"))]
    use core::sync::atomic::{AtomicU32, Ordering};

    pub(in super::super) fn get_or_init() -> cpu::Features {
        fn init() -> NonZeroUsize {
            let detected = detect::detect_features();
            let filtered = (if cfg!(feature = "unstable-testing-arm-no-hw") {
                !Neon::mask()
            } else {
                0
            }) | (if cfg!(feature = "unstable-testing-arm-no-neon") {
                Neon::mask()
            } else {
                0
            });
            let detected = detected & !filtered;
            let merged = CAPS_STATIC | detected;

            #[cfg(all(
                target_arch = "arm",
                target_endian = "little",
                target_has_atomic = "32"
            ))]
            if (merged & Neon::mask()) == Neon::mask() {
                // `neon_available` is declared as `alignas(4) uint32_t` in the C code.
                // AtomicU32 is `#[repr(C, align(4))]`.
                prefixed_extern! {
                    static neon_available: AtomicU32;
                }
                // SAFETY: The C code only reads `neon_available`, and its
                // reads are synchronized through the `OnceNonZeroUsize`
                // Acquire/Release semantics as we ensure we have a
                // `cpu::Features` instance before calling into the C code.
                let p = unsafe { &neon_available };
                p.store(1, Ordering::Relaxed);
            }

            let merged = usize_from_u32(merged) | (1 << (Shift::Initialized as u32));
            NonZeroUsize::new(merged).unwrap() // Can't fail because we just set a bit.
        }

        // SAFETY: This is the only caller. Any concurrent reading doesn't
        // affect the safety of the writing.
        let _: NonZeroUsize = FEATURES.get_or_init(init);

        // SAFETY: We initialized the CPU features as required.
        unsafe { cpu::Features::new_after_feature_flags_written_and_synced_unchecked() }
    }

    pub(in super::super) fn get(_cpu_features: cpu::Features) -> u32 {
        // SAFETY: Since only `get_or_init()` could have created
        // `_cpu_features`, and it only does so after `FEATURES.get_or_init()`,
        // we know we are reading from `FEATURES` after initializing it.
        //
        // Also, 0 means "no features detected" to users, which is designed to
        // be a safe configuration.
        let features = FEATURES.get().map(NonZeroUsize::get).unwrap_or(0);

        // The truncation is lossless, as we set the value with a u32.
        #[allow(clippy::cast_possible_truncation)]
        let features = features as u32;

        features
    }

    static FEATURES: race::OnceNonZeroUsize = race::OnceNonZeroUsize::new();

    // TODO(MSRV): There is no "pmull" feature listed from
    // `rustc --print cfg --target=aarch64-apple-darwin`. Originally ARMv8 tied
    // PMULL detection into AES detection, but later versions split it; see
    // https://developer.arm.com/downloads/-/exploration-tools/feature-names-for-a-profile
    // "Features introduced prior to 2020." Change this to use "pmull" when
    // that is supported.
    //
    // "sha3" is overloaded for both SHA-3 and SHA-512.
    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    #[rustfmt::skip]
    pub(in super::super) const STATIC_DETECTED: u32 = 0
        | (if cfg!(target_feature = "neon") { Neon::mask() } else { 0 })
        | (if cfg!(target_feature = "aes") { Aes::mask() } else { 0 })
        | (if cfg!(target_feature = "aes") { PMull::mask() } else { 0 })
        | (if cfg!(target_feature = "sha2") { Sha256::mask() } else { 0 })
        | (if cfg!(target_feature = "sha3") { Sha512::mask() } else { 0 })
        ;

    // TODO(MSRV): 32-bit ARM doesn't support any static feature detection yet.
    #[cfg(all(target_arch = "arm", target_endian = "little"))]
    pub(in super::super) const STATIC_DETECTED: u32 = 0;
}

#[allow(clippy::assertions_on_constants)]
const _AARCH64_HAS_NEON: () = assert!(
    ((CAPS_STATIC & Neon::mask()) == Neon::mask())
        || !cfg!(all(target_arch = "aarch64", target_endian = "little"))
);
