// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project_lite::pin_project;

pin_project! {
    struct Foo {
        #[pin]
        inner: u8,
    }
}

impl Unpin for __Origin {} //~ ERROR E0412,E0321

fn main() {}
