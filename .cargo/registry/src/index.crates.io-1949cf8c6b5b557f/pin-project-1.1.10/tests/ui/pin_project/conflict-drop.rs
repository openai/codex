// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

#[pin_project] //~ ERROR E0119
struct Foo<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

impl<T, U> Drop for Foo<T, U> {
    fn drop(&mut self) {}
}

#[pin_project(PinnedDrop)] //~ ERROR E0119
struct Bar<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

#[pinned_drop]
impl<T, U> PinnedDrop for Bar<T, U> {
    fn drop(self: Pin<&mut Self>) {}
}

impl<T, U> Drop for Bar<T, U> {
    fn drop(&mut self) {}
}

fn main() {}
