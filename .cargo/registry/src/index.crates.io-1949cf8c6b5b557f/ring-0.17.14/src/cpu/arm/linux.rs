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

use super::Neon;

// Work around a bug in LLVM/rustc where `-C target_cpu=cortex-a72`--
// and `-C target_cpu=native` on Cortex-A72 Raspberry PI devices in
// particular--enables crypto features even though not all Cortex-A72
// CPUs have crypto features:
//
// ```
// $ rustc --print cfg --target=aarch64-unknown-linux-gnu | grep feature
// target_feature="neon"
// $ rustc --print cfg --target=aarch64-unknown-linux-gnu -C target_cpu=cortex-a72 | grep feature
// target_feature="aes"
// target_feature="crc"
// target_feature="neon"
// target_feature="pmuv3"
// target_feature="sha2"
// ```
//
// XXX/TODO(MSRV https://github.com/llvm/llvm-project/issues/90365): This
// workaround is heavy-handed since it forces extra branches for devices that
// have correctly-modeled feature sets, so it should be removed.
pub const FORCE_DYNAMIC_DETECTION: u32 = !Neon::mask();

// `uclibc` does not provide `getauxval` so just use static feature detection
// for it.
#[cfg(target_env = "uclibc")]
pub fn detect_features() -> u32 {
    0
}

#[cfg(all(
    not(target_env = "uclibc"),
    all(target_arch = "aarch64", target_endian = "little")
))]
pub fn detect_features() -> u32 {
    use super::{Aes, PMull, Sha256, Sha512, CAPS_STATIC};
    use libc::{getauxval, AT_HWCAP, HWCAP_AES, HWCAP_PMULL, HWCAP_SHA2, HWCAP_SHA512};

    let mut features = 0;

    // We do not need to check for the presence of NEON, as Armv8-A always has it
    const _ASSERT_NEON_DETECTED: () = assert!((CAPS_STATIC & Neon::mask()) == Neon::mask());

    let caps = unsafe { getauxval(AT_HWCAP) };

    if caps & HWCAP_AES == HWCAP_AES {
        features |= Aes::mask();
    }
    if caps & HWCAP_PMULL == HWCAP_PMULL {
        features |= PMull::mask();
    }
    if caps & HWCAP_SHA2 == HWCAP_SHA2 {
        features |= Sha256::mask();
    }
    if caps & HWCAP_SHA512 == HWCAP_SHA512 {
        features |= Sha512::mask();
    }

    features
}

#[cfg(all(
    not(target_env = "uclibc"),
    all(target_arch = "arm", target_endian = "little")
))]
pub fn detect_features() -> u32 {
    use super::CAPS_STATIC;

    // The `libc` crate doesn't provide this functionality on all
    // 32-bit Linux targets, like Android or -musl. Use this polyfill
    // for all 32-bit ARM targets so that testing on one of them will
    // be more meaningful to the others.
    use libc::c_ulong;
    extern "C" {
        pub fn getauxval(type_: c_ulong) -> c_ulong;
    }
    const AT_HWCAP: c_ulong = 16;
    const HWCAP_NEON: c_ulong = 1 << 12;

    let mut features = 0;

    if CAPS_STATIC & Neon::mask() != Neon::mask() {
        let caps = unsafe { getauxval(AT_HWCAP) };

        // OpenSSL and BoringSSL don't enable any other features if NEON isn't
        // available. We don't enable any hardware implementations for 32-bit ARM.
        if caps & HWCAP_NEON == HWCAP_NEON {
            features |= Neon::mask();
        }
    }

    features
}
