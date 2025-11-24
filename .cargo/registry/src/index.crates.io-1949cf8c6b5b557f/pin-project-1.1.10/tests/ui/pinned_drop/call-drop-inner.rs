// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

#[pin_project(PinnedDrop)]
struct Struct {
    f: bool,
}

#[pinned_drop]
impl PinnedDrop for Struct {
    fn drop(mut self: Pin<&mut Self>) {
        __drop_inner(__self);
    }
}

fn main() {}
