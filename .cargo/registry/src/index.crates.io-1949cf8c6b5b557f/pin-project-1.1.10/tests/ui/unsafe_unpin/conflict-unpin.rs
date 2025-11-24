// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

#[pin_project(UnsafeUnpin)] //~ ERROR E0119
struct Foo<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

impl<T, U> Unpin for Foo<T, U> where T: Unpin {}

#[pin_project(UnsafeUnpin)] //~ ERROR E0119
struct Bar<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

impl<T, U> Unpin for Bar<T, U> {}

#[pin_project(UnsafeUnpin)] //~ ERROR E0119
struct Baz<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

impl<T: Unpin, U: Unpin> Unpin for Baz<T, U> {}

fn main() {}
