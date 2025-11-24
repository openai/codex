// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

// In `Drop` impl, the implementor must specify the same requirement as type definition.

struct DropImpl<T> {
    f: T,
}

impl<T: Unpin> Drop for DropImpl<T> {
    //~^ ERROR E0367
    fn drop(&mut self) {}
}

#[pin_project(PinnedDrop)] //~ ERROR E0277
struct PinnedDropImpl<T> {
    #[pin]
    f: T,
}

#[pinned_drop]
impl<T: Unpin> PinnedDrop for PinnedDropImpl<T> {
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
