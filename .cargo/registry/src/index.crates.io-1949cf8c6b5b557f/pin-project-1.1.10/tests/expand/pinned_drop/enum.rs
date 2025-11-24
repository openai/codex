// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::{pin_project, pinned_drop};

#[pin_project(PinnedDrop, project = EnumProj, project_ref = EnumProjRef)]
enum Enum<T, U> {
    Struct {
        #[pin]
        pinned: T,
        unpinned: U,
    },
    Tuple(#[pin] T, U),
    Unit,
}

#[pinned_drop]
impl<T, U> PinnedDrop for Enum<T, U> {
    fn drop(self: Pin<&mut Self>) {
        let _ = self;
    }
}

fn main() {}
