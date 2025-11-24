// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

include!("../../zerocopy-derive/tests/include.rs");

extern crate zerocopy;

use util::AU16;
use zerocopy::transmute_ref;

fn main() {}

#[derive(zerocopy::Immutable)]
#[repr(transparent)]
struct Dst(AU16);

// `transmute_ref` requires that the destination type implements `FromBytes`
const DST_NOT_FROM_BYTES: &Dst = transmute_ref!(&AU16(0));
