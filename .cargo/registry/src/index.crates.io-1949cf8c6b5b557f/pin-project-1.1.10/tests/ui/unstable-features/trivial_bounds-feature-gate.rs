// SPDX-License-Identifier: Apache-2.0 OR MIT

// Note: If you change this test, change 'trivial_bounds.rs' at the same time.

mod phantom_pinned {
    use std::marker::{PhantomData, PhantomPinned};

    struct A(PhantomPinned);

    impl Unpin for A where PhantomPinned: Unpin {} //~ ERROR E0277

    struct Wrapper<T>(T);

    impl<T> Unpin for Wrapper<T> where T: Unpin {}

    struct B(PhantomPinned);

    impl Unpin for B where Wrapper<PhantomPinned>: Unpin {} //~ ERROR E0277

    struct WrapperWithLifetime<'a, T>(PhantomData<&'a ()>, T);

    impl<T> Unpin for WrapperWithLifetime<'_, T> where T: Unpin {}

    struct C(PhantomPinned);

    impl<'a> Unpin for C where WrapperWithLifetime<'a, PhantomPinned>: Unpin {} // Ok
}

mod inner {
    use std::marker::{PhantomData, PhantomPinned};

    struct Inner(PhantomPinned);

    struct A(Inner);

    impl Unpin for A where Inner: Unpin {} //~ ERROR E0277

    struct Wrapper<T>(T);

    impl<T> Unpin for Wrapper<T> where T: Unpin {}

    struct B(Inner);

    impl Unpin for B where Wrapper<Inner>: Unpin {} //~ ERROR E0277

    struct WrapperWithLifetime<'a, T>(PhantomData<&'a ()>, T);

    impl<T> Unpin for WrapperWithLifetime<'_, T> where T: Unpin {}

    struct C(Inner);

    impl<'a> Unpin for C where WrapperWithLifetime<'a, Inner>: Unpin {} // Ok
}

fn main() {}
