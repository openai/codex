// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

extern crate zerocopy;

use zerocopy::transmute_ref;

fn main() {}

fn ref_dst_mutable() {
    // `transmute_ref!` requires that its destination type be an immutable
    // reference.
    let _: &mut u8 = transmute_ref!(&0u8);
}
