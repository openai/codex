// Copyright 2024 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

extern crate zerocopy;

use zerocopy::try_transmute_ref;

fn main() {}

fn ref_dst_mutable() {
    // `try_transmute_ref!` requires that its destination type be an immutable
    // reference.
    let _: Result<&mut u8, _> = try_transmute_ref!(&0u8);
}
