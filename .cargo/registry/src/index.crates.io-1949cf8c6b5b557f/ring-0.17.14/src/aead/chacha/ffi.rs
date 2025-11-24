// Copyright 2016-2025 Brian Smith.
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

use super::{super::overlapping::Overlapping, Counter, Key};

// `unsafe { (N, C, InOut) => f }` means that the function `f` is safe to call
// iff the in/out length is at least `N`, the CPU features `C` are available,
// and the input type is `InOut`. If `f` supports overlapping input/output then
// `InOut` should be `Overlapping<'_, u8>`; otherwise it should be `&mut [u8]`.
macro_rules! chacha20_ctr32_ffi {
    ( unsafe { ($MIN_LEN:expr, $Cpu:ty, $InOut:ty) => $f:ident },
      $key:expr, $counter:expr, $in_out:expr, $cpu:expr ) => {{
        prefixed_extern! {
            fn $f(
                out: *mut u8,
                in_: *const u8,
                in_len: crate::c::size_t,
                key: &[u32; 8],
                counter: &crate::aead::chacha::Counter,
            );
        }
        // SAFETY: The user asserts that $f has the signature above and is safe
        // to call if additionally we have a value of type `$Cpu` and an in/out
        // value of the indicated type, which we do.
        unsafe {
            crate::aead::chacha::ffi::chacha20_ctr32_ffi::<$InOut, $Cpu, $MIN_LEN>(
                $key, $counter, $in_out, $cpu, $f,
            )
        }
    }};
}

// Panics if `in_out.len() < MIN_LEN`. The caller should have guarded against
// that so that the assertion gets optimized away.
pub(super) unsafe fn chacha20_ctr32_ffi<
    'o,
    InOut: 'o + Into<Overlapping<'o, u8>>,
    Cpu,
    const MIN_LEN: usize,
>(
    key: &Key,
    counter: Counter,
    in_out: InOut,
    cpu: Cpu,
    f: unsafe extern "C" fn(*mut u8, *const u8, crate::c::size_t, &[u32; 8], &Counter),
) {
    assert!(MIN_LEN > 0);
    let in_out: Overlapping<'_, u8> = in_out.into();
    in_out.with_input_output_len(|input, output, len| {
        assert!(len >= MIN_LEN);
        let key = key.words_less_safe();
        let _: Cpu = cpu;
        unsafe { f(output, input, len, key, &counter) }
    });
}
