// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

include!("../../zerocopy-derive/tests/include.rs");

#[macro_use]
extern crate zerocopy;

use util::NotZerocopy;

fn main() {}

// Should fail because `NotZerocopy<u32>: !FromBytes`.
const NOT_FROM_BYTES: NotZerocopy<u32> = include_value!("../../testdata/include_value/data");
