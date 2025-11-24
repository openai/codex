// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

#[pin_project]
struct S {
    //~^ ERROR may not be used on structs with zero fields
    #[cfg(any())]
    #[pin]
    f: u8,
}

fn main() {}
