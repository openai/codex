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
use zerocopy::try_transmute_ref;

// Although this is not a soundness requirement, we currently require that the
// size of the destination type is not smaller than the size of the source type.
fn main() {
    let decrease_size: Result<&u8, _> = try_transmute_ref!(&AU16(0));
}
