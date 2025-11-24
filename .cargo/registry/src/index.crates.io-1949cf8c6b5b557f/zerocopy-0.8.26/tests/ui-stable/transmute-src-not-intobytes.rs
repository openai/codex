// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

include!("../../zerocopy-derive/tests/include.rs");

extern crate zerocopy;

use util::{NotZerocopy, AU16};
use zerocopy::transmute;

fn main() {}

// `transmute` requires that the source type implements `IntoBytes`
const SRC_NOT_AS_BYTES: AU16 = transmute!(NotZerocopy(AU16(0)));
