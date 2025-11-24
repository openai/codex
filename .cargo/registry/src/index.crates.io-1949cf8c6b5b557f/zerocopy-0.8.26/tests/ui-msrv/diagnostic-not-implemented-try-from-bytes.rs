// Copyright 2022 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

include!("../../zerocopy-derive/tests/include.rs");

extern crate zerocopy;

use util::NotZerocopy;
use zerocopy::TryFromBytes;

fn main() {
    // We expect the proper diagnostic to be emitted on Rust 1.78.0 and later.
    takes_try_from_bytes::<NotZerocopy>();
}

fn takes_try_from_bytes<T: TryFromBytes>() {}
