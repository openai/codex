// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use auxiliary_macro::{hidden_repr, hidden_repr2};
use pin_project::{pin_project, pinned_drop, UnsafeUnpin};

#[pin_project] //~ ERROR may not be used on #[repr(packed)] types
#[hidden_repr(packed)]
struct A {
    #[pin]
    f: u32,
}

#[hidden_repr2]
#[pin_project] //~ ERROR may not be used on #[repr(packed)] types
struct B {
    #[pin]
    f: u32,
}

#[pin_project(UnsafeUnpin)] //~ ERROR may not be used on #[repr(packed)] types
#[hidden_repr(packed)]
struct C {
    #[pin]
    f: u32,
}

unsafe impl UnsafeUnpin for C {}

#[pin_project(PinnedDrop)] //~ ERROR may not be used on #[repr(packed)] types
#[hidden_repr(packed)]
struct D {
    #[pin]
    f: u32,
}

#[pinned_drop]
impl PinnedDrop for D {
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
