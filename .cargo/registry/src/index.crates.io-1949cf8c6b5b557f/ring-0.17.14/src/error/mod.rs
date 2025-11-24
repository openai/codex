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

//! Error reporting.

pub use self::{key_rejected::KeyRejected, unspecified::Unspecified};

pub(crate) use self::{
    input_too_long::InputTooLongError, len_mismatch_error::LenMismatchError,
    too_much_output_requested::TooMuchOutputRequestedError,
};

mod input_too_long;
mod into_unspecified;
mod key_rejected;
mod unspecified;

#[cold]
#[inline(never)]
pub(crate) fn erase<T>(_: T) -> Unspecified {
    Unspecified
}

cold_exhaustive_error! {
    struct too_much_output_requested::TooMuchOutputRequestedError
        with pub(crate) constructor {
        // Note that this might not actually be the (exact) output length
        // requested, and its units might be lost. For example, it could be any of
        // the following:
        //
        //    * The length in bytes of the entire output.
        //    * The length in bytes of some *part* of the output.
        //    * A bit length.
        //    * A length in terms of "blocks" or other grouping of output values.
        //    * Some intermediate quantity that was used when checking the output
        //      length.
        //    * Some arbitrary value.
        imprecise_output_length: usize
    }
}

cold_exhaustive_error! {
    struct len_mismatch_error::LenMismatchError
        with pub(crate) constructor {
        len: usize
    }
}
