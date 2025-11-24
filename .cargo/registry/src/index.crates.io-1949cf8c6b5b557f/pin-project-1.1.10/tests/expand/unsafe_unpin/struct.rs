// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::{pin_project, UnsafeUnpin};

#[pin_project(UnsafeUnpin)]
struct Struct<T, U> {
    #[pin]
    pinned: T,
    unpinned: U,
}

unsafe impl<T: Unpin, U> UnsafeUnpin for Struct<T, U> {}

fn main() {}
