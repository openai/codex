// Copyright 2025 Brian Smith.
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

#![doc(hidden)]

/// References a test input file.
#[macro_export]
macro_rules! test_file {
    ($file_name:expr) => {
        $crate::test::File {
            file_name: $file_name,
            contents: include_str!($file_name),
        }
    };
}

pub use crate::testutil::{
    compile_time_assert_clone, compile_time_assert_copy, compile_time_assert_eq,
    compile_time_assert_send, compile_time_assert_sync, from_hex, run, File, TestCase,
};

#[cfg(feature = "std")]
pub use crate::testutil::compile_time_assert_std_error_error;

#[deprecated(note = "internal API that will be removed")]
#[doc(hidden)]
pub mod rand {
    #[deprecated(note = "internal API that will be removed")]
    pub type FixedByteRandom = crate::testutil::rand::FixedByteRandom;
    #[deprecated(note = "internal API that will be removed")]
    pub type FixedSliceRandom<'a> = crate::testutil::rand::FixedSliceRandom<'a>;
    #[deprecated(note = "internal API that will be removed")]
    pub type FixedSliceSequenceRandom<'a> = crate::testutil::rand::FixedSliceSequenceRandom<'a>;
}
