// SPDX-License-Identifier: Apache-2.0 OR MIT

// Note: If you change this test, change 'overlapping_marker_traits-feature-gate.rs' at the same time.

// This feature could break the guarantee for Unpin provided by pin-project,
// but was removed in https://github.com/rust-lang/rust/pull/68544 (nightly-2020-02-06).
// Refs:
// - https://github.com/rust-lang/rust/issues/29864#issuecomment-515780867
// - https://github.com/taiki-e/pin-project/issues/105

// overlapping_marker_traits
// Tracking issue: https://github.com/rust-lang/rust/issues/29864
#![feature(overlapping_marker_traits)]

use std::marker::PhantomPinned;

use pin_project::pin_project;

#[pin_project]
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
