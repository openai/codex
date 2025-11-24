// Copyright 2015 Brian Smith.
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

use crate::error;
use core::ffi::c_int;

/// An `int` returned from a foreign function containing **1** if the function
/// was successful or **0** if an error occurred. This is the convention used by
/// C code in `ring`.
#[must_use]
#[repr(transparent)]
pub struct Result(c_int);

impl From<Result> for core::result::Result<(), error::Unspecified> {
    fn from(ret: Result) -> Self {
        match ret.0 {
            1 => Ok(()),
            c => {
                debug_assert_eq!(c, 0, "`bssl::Result` value must be 0 or 1");
                Err(error::Unspecified)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    mod result {
        use crate::bssl;
        use core::{
            ffi::c_int,
            mem::{align_of, size_of},
        };

        #[test]
        fn size_and_alignment() {
            type Underlying = c_int;
            assert_eq!(size_of::<bssl::Result>(), size_of::<Underlying>());
            assert_eq!(align_of::<bssl::Result>(), align_of::<Underlying>());
        }

        #[test]
        fn semantics() {
            assert!(Result::from(bssl::Result(0)).is_err());
            assert!(Result::from(bssl::Result(1)).is_ok());
        }
    }
}
