// SPDX-License-Identifier: Apache-2.0 OR MIT

// See ./not_unpin-expanded.rs for generated code.

#![allow(dead_code)]

use pin_project::pin_project;

#[pin_project(!Unpin)]
struct Struct<T, U> {
    #[pin]
    pinned: T,
    unpinned: U,
}

fn main() {
    fn _is_unpin<T: Unpin>() {}
    // _is_unpin::<Struct<(), ()>>(); //~ ERROR `std::marker::PhantomPinned` cannot be unpinned
}
