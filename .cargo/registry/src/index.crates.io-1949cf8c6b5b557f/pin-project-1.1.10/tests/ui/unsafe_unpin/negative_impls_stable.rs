// SPDX-License-Identifier: Apache-2.0 OR MIT

// https://github.com/taiki-e/pin-project/issues/340#issuecomment-2428002670

#[pin_project::pin_project(UnsafeUnpin)]
struct Foo<Pinned, Unpinned> {
    #[pin]
    pinned: Pinned,
    unpinned: Unpinned,
}

unsafe impl<Pinned: Unpin, Unpinned> pin_project::UnsafeUnpin for Foo<Pinned, Unpinned> {}

struct MyPhantomPinned(::core::marker::PhantomPinned);
impl Unpin for MyPhantomPinned where for<'cursed> str: Sized {}
impl Unpin for Foo<MyPhantomPinned, ()> {}

fn is_unpin<T: Unpin>() {}

fn main() {
    is_unpin::<Foo<MyPhantomPinned, ()>>()
}
