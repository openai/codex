// Copyright 2018-2024 Brian Smith.
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

#![cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    target_arch = "x86",
    target_arch = "x86_64"
))]

use super::{Block, Counter, EncryptBlock, EncryptCtr32, Iv, KeyBytes, Overlapping, AES_KEY};
use crate::{cpu, error};
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(all(target_arch = "aarch64", target_endian = "little"))] {
        pub(in super::super) type RequiredCpuFeatures = cpu::arm::Aes;
        pub(in super::super) type OptionalCpuFeatures = ();
    } else if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        use cpu::intel::{Aes, Avx, Ssse3};
        // Some functions seem to have been written to require only SSE/SSE2
        // but there seem to be no SSSE3-less CPUs with AES-NI, and we don't
        // have feature detection for SSE2.
        pub(in super::super) type RequiredCpuFeatures = (Aes, Ssse3);
        pub(in super::super) type OptionalCpuFeatures = Avx;
    }
}

#[derive(Clone)]
pub struct Key {
    inner: AES_KEY,
}

impl Key {
    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    pub(in super::super) fn new(
        bytes: KeyBytes<'_>,
        _required_cpu_features: RequiredCpuFeatures,
        _optional_cpu_features: Option<OptionalCpuFeatures>,
    ) -> Result<Self, error::Unspecified> {
        let inner = unsafe { set_encrypt_key!(aes_hw_set_encrypt_key, bytes) }?;
        Ok(Self { inner })
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub(in super::super) fn new(
        bytes: KeyBytes<'_>,
        (Aes { .. }, Ssse3 { .. }): RequiredCpuFeatures,
        optional_cpu_features: Option<OptionalCpuFeatures>,
    ) -> Result<Self, error::Unspecified> {
        // Ssse3 is required, but upstream only uses this if there is also Avx;
        // presumably the base version is faster on pre-AVX CPUs.
        let inner = if let Some(Avx { .. }) = optional_cpu_features {
            unsafe { set_encrypt_key!(aes_hw_set_encrypt_key_alt, bytes) }?
        } else {
            unsafe { set_encrypt_key!(aes_hw_set_encrypt_key_base, bytes) }?
        };
        Ok(Self { inner })
    }

    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86_64"
    ))]
    #[must_use]
    pub(in super::super) fn inner_less_safe(&self) -> &AES_KEY {
        &self.inner
    }
}

impl EncryptBlock for Key {
    fn encrypt_block(&self, block: Block) -> Block {
        super::encrypt_block_using_encrypt_iv_xor_block(self, block)
    }

    fn encrypt_iv_xor_block(&self, iv: Iv, block: Block) -> Block {
        super::encrypt_iv_xor_block_using_ctr32(self, iv, block)
    }
}

impl EncryptCtr32 for Key {
    fn ctr32_encrypt_within(&self, in_out: Overlapping<'_>, ctr: &mut Counter) {
        unsafe { ctr32_encrypt_blocks!(aes_hw_ctr32_encrypt_blocks, in_out, &self.inner, ctr) }
    }
}
