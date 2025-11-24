// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::{pin_project, UnsafeUnpin};

#[pin_project(UnsafeUnpin, project = EnumProj, project_ref = EnumProjRef)]
enum Enum<T, U> {
    Struct {
        #[pin]
        pinned: T,
        unpinned: U,
    },
    Tuple(#[pin] T, U),
    Unit,
}

unsafe impl<T: Unpin, U> UnsafeUnpin for Enum<T, U> {}

fn main() {}
