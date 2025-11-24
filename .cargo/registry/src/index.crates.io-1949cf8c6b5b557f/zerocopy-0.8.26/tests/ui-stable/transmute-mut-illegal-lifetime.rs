// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

fn main() {}

fn increase_lifetime() {
    let mut x = 0u64;
    // It is illegal to increase the lifetime scope.
    let _: &'static mut u64 = zerocopy::transmute_mut!(&mut x);
}
