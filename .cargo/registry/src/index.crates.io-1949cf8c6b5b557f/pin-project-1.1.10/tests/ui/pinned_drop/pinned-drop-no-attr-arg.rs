// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

#[pin_project]
struct S {
    #[pin]
    f: u8,
}

#[pinned_drop]
impl PinnedDrop for S {
    //~^ ERROR E0119
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
