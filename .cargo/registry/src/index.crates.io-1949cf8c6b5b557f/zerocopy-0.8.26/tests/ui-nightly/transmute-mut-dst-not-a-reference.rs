// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

extern crate zerocopy;

use zerocopy::transmute_mut;

fn main() {}

// `transmute_mut!` does not support transmuting into a non-reference
// destination type.
const DST_NOT_A_REFERENCE: usize = transmute_mut!(&mut 0u8);
