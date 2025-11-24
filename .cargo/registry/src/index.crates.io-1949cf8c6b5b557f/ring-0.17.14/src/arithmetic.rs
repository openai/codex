// Copyright 2017-2023 Brian Smith.
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

pub(crate) use self::{constant::limbs_from_hex, limb_slice_error::LimbSliceError};
use crate::{error::LenMismatchError, limb::LIMB_BITS};

#[macro_use]
mod ffi;

mod constant;

#[cfg(feature = "alloc")]
pub mod bigint;

pub(crate) mod inout;
mod limbs;
mod limbs512;
pub mod montgomery;

mod n0;

// The minimum number of limbs allowed for any `&[Limb]` operation.
//
// TODO: Use `256 / LIMB_BITS` so that the limit is independent of limb size.
pub const MIN_LIMBS: usize = 4;

// The maximum number of limbs allowed for any `&[Limb]` operation.
pub const MAX_LIMBS: usize = 8192 / LIMB_BITS;

cold_exhaustive_error! {
    enum limb_slice_error::LimbSliceError {
        len_mismatch => LenMismatch(LenMismatchError),
        too_short => TooShort(usize),
        too_long => TooLong(usize),
    }
}
