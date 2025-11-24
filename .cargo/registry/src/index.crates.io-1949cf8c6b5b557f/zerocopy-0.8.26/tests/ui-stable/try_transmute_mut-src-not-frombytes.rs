// Copyright 2024 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

extern crate zerocopy;

use zerocopy::transmute_mut;

#[derive(zerocopy::IntoBytes)]
#[repr(C)]
struct Src;

#[derive(zerocopy::TryFromBytes)]
#[repr(C)]
struct Dst;

fn main() {
    // `try_transmute_mut` requires that the source type implements `FromBytes`
    let src_not_from_bytes: &mut Dst = transmute_mut!(&mut Src);
}
