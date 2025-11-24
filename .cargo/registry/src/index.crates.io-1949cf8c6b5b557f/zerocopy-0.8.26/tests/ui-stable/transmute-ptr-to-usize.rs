// Copyright 2022 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

extern crate zerocopy;

use zerocopy::transmute;

fn main() {}

// It is unclear whether we can or should support this transmutation, especially
// in a const context. This test ensures that even if such a transmutation
// becomes valid due to the requisite implementations of `FromBytes` being
// added, that we re-examine whether it should specifically be valid in a const
// context.
const POINTER_VALUE: usize = transmute!(&0usize as *const usize);
