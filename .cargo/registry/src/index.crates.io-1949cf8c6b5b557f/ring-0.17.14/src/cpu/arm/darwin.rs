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

use super::{Aes, Neon, PMull, Sha256, Sha512, CAPS_STATIC};
use crate::polyfill::cstr;

// ```
// $ rustc +1.61.0 --print cfg --target=aarch64-apple-ios | grep -E "neon|aes|sha|pmull"
// target_feature="aes"
// target_feature="neon"
// target_feature="sha2"
// $ rustc +1.61.0 --print cfg --target=aarch64-apple-darwin | grep -E "neon|aes|sha|pmull"
// target_feature="aes"
// target_feature="neon"
// target_feature="sha2"
// target_feature="sha3"
// ```
//
// XXX/TODO(coverage)/TODO(size): aarch64-apple-darwin is statically guaranteed to have "sha3" but
// other aarch64-apple-* targets require dynamic detection. Since we don't have test coverage for
// the other targets yet, we wouldn't have a way of testing the dynamic detection if we statically
// enabled `Sha512` for -darwin. So instead, temporarily, we statically ignore the static
// availability of the feature on -darwin so that it runs the dynamic detection.
pub const MIN_STATIC_FEATURES: u32 = Neon::mask() | Aes::mask() | Sha256::mask() | PMull::mask();
pub const FORCE_DYNAMIC_DETECTION: u32 = !MIN_STATIC_FEATURES;

// MSRV: Enforce 1.61.0 onaarch64-apple-*, in particular) prior to. Earlier
// versions of Rust before did not report the AAarch64 CPU features correctly
// for these targets. Cargo.toml specifies `rust-version` but versions before
// Rust 1.56 don't know about it.
#[allow(clippy::assertions_on_constants)]
const _AARCH64_APPLE_TARGETS_EXPECTED_FEATURES: () =
    assert!((CAPS_STATIC & MIN_STATIC_FEATURES) == MIN_STATIC_FEATURES);

// Ensure we don't accidentally allow features statically beyond
// `MIN_STATIC_FEATURES` so that dynamic detection is done uniformly for
// all of these targets.
#[allow(clippy::assertions_on_constants)]
const _AARCH64_APPLE_DARWIN_TARGETS_EXPECTED_FEATURES: () =
    assert!(CAPS_STATIC == MIN_STATIC_FEATURES);

pub fn detect_features() -> u32 {
    fn detect_feature(name: cstr::Ref) -> bool {
        use crate::polyfill;
        use core::mem;
        use libc::{c_int, c_void};

        let mut value: c_int = 0;
        let mut len = mem::size_of_val(&value);
        let value_ptr = polyfill::ptr::from_mut(&mut value).cast::<c_void>();
        // SAFETY: `value_ptr` is a valid pointer to `value` and `len` is the size of `value`.
        let rc = unsafe {
            libc::sysctlbyname(name.as_ptr(), value_ptr, &mut len, core::ptr::null_mut(), 0)
        };
        // All the conditions are separated so we can observe them in code coverage.
        if rc != 0 {
            return false;
        }
        debug_assert_eq!(len, mem::size_of_val(&value));
        if len != mem::size_of_val(&value) {
            return false;
        }
        value != 0
    }

    // We do not need to check for the presence of NEON, as Armv8-A always has it
    const _ASSERT_NEON_DETECTED: () = assert!((CAPS_STATIC & Neon::mask()) == Neon::mask());

    let mut features = 0;

    // TODO(MSRV 1.77): Use c"..." literal.
    const SHA512_NAME: cstr::Ref =
        cstr::unwrap_const_from_bytes_with_nul(b"hw.optional.armv8_2_sha512\0");
    if detect_feature(SHA512_NAME) {
        features |= Sha512::mask();
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu;

    #[test]
    fn sha512_detection() {
        // We intentionally disable static feature detection for SHA-512.
        const _SHA512_NOT_STATICALLY_DETECTED: () = assert!((CAPS_STATIC & Sha512::mask()) == 0);

        if cfg!(target_os = "macos") {
            use crate::cpu::{arm::Sha512, GetFeature as _};

            // All aarch64-apple-darwin targets have SHA3 enabled statically...
            assert!(cfg!(target_feature = "sha3"));

            // ...so we should detect it.
            let cpu = cpu::features();
            assert!(matches!(cpu.get_feature(), Some(Sha512 { .. })));
        }
    }
}
