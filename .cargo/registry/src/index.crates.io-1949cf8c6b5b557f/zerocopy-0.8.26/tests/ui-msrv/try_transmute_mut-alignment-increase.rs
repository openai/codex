// Copyright 2024 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

include!("../../zerocopy-derive/tests/include.rs");

extern crate zerocopy;

use util::AU16;
use zerocopy::try_transmute_mut;

// `try_transmute_mut!` does not support transmuting from a type of smaller
// alignment to one of larger alignment.
fn main() {
    let src = &mut [0u8; 2];
    let increase_size: Result<&mut AU16, _> = try_transmute_mut!(src);
}
