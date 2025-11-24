// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

#[pin_project(PinnedDrop)]
struct TupleStruct<T, U>(#[pin] T, U);

#[pinned_drop]
impl<T, U> PinnedDrop for TupleStruct<T, U> {
    fn drop(self: Pin<&mut Self>) {
        let _ = self;
    }
}

fn main() {}
