// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::{pin_project, UnsafeUnpin};

#[pin_project] //~ ERROR E0119
struct Foo<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

unsafe impl<T, U> UnsafeUnpin for Foo<T, U> where T: Unpin {}

#[pin_project] //~ ERROR E0119
struct Bar<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

unsafe impl<T, U> UnsafeUnpin for Bar<T, U> {}

#[pin_project] //~ ERROR E0119
struct Baz<T, U> {
    #[pin]
    f1: T,
    f2: U,
}

unsafe impl<T: Unpin, U: Unpin> UnsafeUnpin for Baz<T, U> {}

fn main() {}
