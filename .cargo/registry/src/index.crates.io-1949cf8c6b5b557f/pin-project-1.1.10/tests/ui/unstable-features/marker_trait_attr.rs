// SPDX-License-Identifier: Apache-2.0 OR MIT

// Note: If you change this test, change 'marker_trait_attr-feature-gate.rs' at the same time.

// marker_trait_attr
// Tracking issue: https://github.com/rust-lang/rust/issues/29864
#![feature(marker_trait_attr)]

// See https://github.com/taiki-e/pin-project/issues/105#issuecomment-535355974

use std::marker::PhantomPinned;

use pin_project::pin_project;

#[pin_project] //~ ERROR E0119
struct Struct<T> {
    #[pin]
    f: T,
}

// unsound Unpin impl
impl<T> Unpin for Struct<T> {}

fn is_unpin<T: Unpin>() {}

fn main() {
    is_unpin::<Struct<PhantomPinned>>()
}
