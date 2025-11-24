// SPDX-License-Identifier: Apache-2.0 OR MIT

// Note: If you change this test, change 'trivial_bounds-feature-gate.rs' at the same time.

// trivial_bounds
// Tracking issue: https://github.com/rust-lang/rust/issues/48214
#![feature(trivial_bounds)]
#![deny(trivial_bounds)]
#![allow(dead_code)]

use std::marker::{PhantomData, PhantomPinned};

fn inner() {
    struct Inner(PhantomPinned);

    struct A(PhantomPinned);

    impl Unpin for A where PhantomPinned: Unpin {} //~ ERROR Unpin does not depend on any type or lifetime parameters

    struct B(Inner);

    impl Unpin for B where Inner: Unpin {} //~ ERROR Unpin does not depend on any type or lifetime parameters

    struct Wrapper<T>(T);

    impl<T> Unpin for Wrapper<T> where T: Unpin {}

    struct C(Inner);

    impl Unpin for C where Wrapper<Inner>: Unpin {} //~ ERROR Unpin does not depend on any type or lifetime parameters

    struct WrapperWithLifetime<'a, T>(PhantomData<&'a ()>, T);

    impl<T> Unpin for WrapperWithLifetime<'_, T> where T: Unpin {}

    struct D(Inner);

    impl<'a> Unpin for D where WrapperWithLifetime<'a, Inner>: Unpin {} // Ok
}

fn main() {}
