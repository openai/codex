// SPDX-License-Identifier: Apache-2.0 OR MIT

// https://github.com/taiki-e/pin-project/issues/340#issuecomment-2428002670

pin_project_lite::pin_project! {
    struct Foo<Pinned, Unpinned> {
        #[pin]
        pinned: Pinned,
        unpinned: Unpinned,
    }
}

struct MyPhantomPinned(::core::marker::PhantomPinned);
impl Unpin for MyPhantomPinned where for<'cursed> str: Sized {}
impl Unpin for Foo<MyPhantomPinned, ()> {}

fn is_unpin<T: Unpin>() {}

fn main() {
    is_unpin::<Foo<MyPhantomPinned, ()>>()
}
