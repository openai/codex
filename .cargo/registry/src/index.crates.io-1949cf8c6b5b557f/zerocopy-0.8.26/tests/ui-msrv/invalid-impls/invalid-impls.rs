// Copyright 2022 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

// Since some macros from `macros.rs` are unused.
#![allow(unused)]

extern crate zerocopy;
extern crate zerocopy_derive;

include!("../../../src/util/macros.rs");

use zerocopy::*;
use zerocopy_derive::*;

fn main() {}

#[derive(FromBytes, IntoBytes, Unaligned)]
#[repr(transparent)]
struct Foo<T>(T);

const _: () = unsafe {
    impl_or_verify!(T => TryFromBytes for Foo<T>);
    impl_or_verify!(T => FromZeros for Foo<T>);
    impl_or_verify!(T => FromBytes for Foo<T>);
    impl_or_verify!(T => IntoBytes for Foo<T>);
    impl_or_verify!(T => Unaligned for Foo<T>);
};
