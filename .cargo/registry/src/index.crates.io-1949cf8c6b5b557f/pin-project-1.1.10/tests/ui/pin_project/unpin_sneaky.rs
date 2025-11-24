// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

#[pin_project]
struct S {
    #[pin]
    f: u8,
}

impl Unpin for __S {} //~ ERROR E0412,E0321

fn main() {}
