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

pub const FORCE_DYNAMIC_DETECTION: u32 = 0;

pub fn detect_features() -> u32 {
    type zx_status_t = i32;

    #[link(name = "zircon")]
    extern "C" {
        fn zx_system_get_features(kind: u32, features: *mut u32) -> zx_status_t;
    }

    const ZX_OK: i32 = 0;
    const ZX_FEATURE_KIND_CPU: u32 = 0;
    const ZX_ARM64_FEATURE_ISA_AES: u32 = 1 << 3;
    const ZX_ARM64_FEATURE_ISA_PMULL: u32 = 1 << 4;
    const ZX_ARM64_FEATURE_ISA_SHA256: u32 = 1 << 6;
    const ZX_ARM64_FEATURE_ISA_SHA512: u32 = 1 << 18;

    let mut caps = 0;
    let rc = unsafe { zx_system_get_features(ZX_FEATURE_KIND_CPU, &mut caps) };

    let mut features = 0;

    // We do not need to check for the presence of NEON, as Armv8-A always has it
    const _ASSERT_NEON_DETECTED: () = assert!((CAPS_STATIC & Neon::mask()) == Neon::mask());

    if rc == ZX_OK {
        if caps & ZX_ARM64_FEATURE_ISA_AES == ZX_ARM64_FEATURE_ISA_AES {
            features |= Aes::mask();
        }
        if caps & ZX_ARM64_FEATURE_ISA_PMULL == ZX_ARM64_FEATURE_ISA_PMULL {
            features |= PMull::mask();
        }
        if caps & ZX_ARM64_FEATURE_ISA_SHA256 == ZX_ARM64_FEATURE_ISA_SHA256 {
            features |= Sha256::mask();
        }
        if caps & ZX_ARM64_FEATURE_ISA_SHA512 == ZX_ARM64_FEATURE_ISA_SHA512 {
            features |= Sha512::mask();
        }
    }

    features
}
