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

use super::{Aes, Neon, PMull, Sha256, CAPS_STATIC};
use windows_sys::Win32::System::Threading::{
    IsProcessorFeaturePresent, PF_ARM_V8_CRYPTO_INSTRUCTIONS_AVAILABLE,
};

pub const FORCE_DYNAMIC_DETECTION: u32 = 0;

pub fn detect_features() -> u32 {
    // We do not need to check for the presence of NEON, as Armv8-A always has it
    const _ASSERT_NEON_DETECTED: () = assert!((CAPS_STATIC & Neon::mask()) == Neon::mask());

    let mut features = 0;

    let result = unsafe { IsProcessorFeaturePresent(PF_ARM_V8_CRYPTO_INSTRUCTIONS_AVAILABLE) };

    if result != 0 {
        // These are all covered by one call in Windows
        features |= Aes::mask();
        features |= PMull::mask();
        features |= Sha256::mask();
    }

    features
}
