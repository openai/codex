// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project_lite::pin_project;

pin_project! {
#[project_replace = EnumProjReplace]
enum Enum<T, U> {
    Struct {
        #[pin]
        pinned1: T,
        #[pin]
        pinned2: T,
        unpinned1: U,
        unpinned2: U,
    },
    Unit,
}
}

fn main() {}
