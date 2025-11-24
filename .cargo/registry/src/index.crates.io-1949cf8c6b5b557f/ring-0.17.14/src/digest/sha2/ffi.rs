// Copyright 2024 Brian Smith.
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

use super::CHAINING_WORDS;
use crate::polyfill::slice::AsChunks;
use core::num::{NonZeroUsize, Wrapping};

/// `unsafe { T => f }` means it is safe to call `f` iff we can construct
/// a value of type `T`.
macro_rules! sha2_ffi {
    ( $U:ty, $BLOCK_LEN:expr, unsafe { $Cpu:ty => $f:ident },
      $state:expr, $data:expr, $cpu:expr $(,)? ) => {{
        prefixed_extern! {
            fn $f(
                state: *mut [core::num::Wrapping<$U>; crate::digest::sha2::CHAINING_WORDS],
                data: *const [u8; $BLOCK_LEN],
                num: crate::c::NonZero_size_t);
        }
        // SAFETY: The user asserts that $f has the signature above and is safe
        // to call if additionally we have a value of type `$Cpu`, which we do.
        unsafe {
            crate::digest::sha2::ffi::sha2_ffi::<$U, $Cpu, { $BLOCK_LEN }>($state, $data, $cpu, $f)
        }
    }};
}

macro_rules! sha2_32_ffi {
    ( unsafe { $Cpu:ty => $f:ident }, $state:expr, $data:expr, $cpu:expr $(,)? ) => {
        sha2_ffi!(u32, crate::digest::sha2::SHA256_BLOCK_LEN.into(),
                  unsafe { $Cpu => $f }, $state, $data, $cpu)
    }
}

macro_rules! sha2_64_ffi {
    ( unsafe { $Cpu:ty => $f:ident }, $state:expr, $data:expr, $cpu:expr $(,)? ) => {
        sha2_ffi!(u64, SHA512_BLOCK_LEN.into(), unsafe { $Cpu => $f }, $state, $data, $cpu)
    }
}

pub(super) unsafe fn sha2_ffi<U, Cpu, const BLOCK_LEN: usize>(
    state: &mut [Wrapping<U>; CHAINING_WORDS],
    data: AsChunks<u8, BLOCK_LEN>,
    cpu: Cpu,
    f: unsafe extern "C" fn(
        *mut [Wrapping<U>; CHAINING_WORDS],
        *const [u8; BLOCK_LEN],
        crate::c::NonZero_size_t,
    ),
) {
    if let Some(blocks) = NonZeroUsize::new(data.len()) {
        let data = data.as_ptr();
        let _: Cpu = cpu;
        // SAFETY:
        //   * `blocks` is non-zero.
        //   * `data` is non-NULL and points to `blocks` blocks.
        //   * The caller asserted that `f` meets this contract if we have
        //     an instance of `Cpu`.
        unsafe { f(state, data, blocks) }
    }
}
