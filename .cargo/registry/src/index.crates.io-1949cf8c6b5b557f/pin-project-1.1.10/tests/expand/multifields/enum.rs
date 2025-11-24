// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

#[pin_project(project = EnumProj, project_ref = EnumProjRef, project_replace = EnumProjOwn)]
enum Enum<T, U> {
    Struct {
        #[pin]
        pinned1: T,
        #[pin]
        pinned2: T,
        unpinned1: U,
        unpinned2: U,
    },
    Tuple(#[pin] T, #[pin] T, U, U),
    Unit,
}

fn main() {}
