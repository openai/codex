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
use zerocopy::try_transmute;

fn main() {
    // `try_transmute` requires that the source type implements `IntoBytes`
    let src_not_into_bytes: Result<AU16, _> = try_transmute!(NotZerocopy(AU16(0)));
}
