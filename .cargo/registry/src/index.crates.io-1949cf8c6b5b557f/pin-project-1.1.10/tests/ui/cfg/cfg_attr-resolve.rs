// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

#[cfg_attr(any(), pin_project::pin_project)]
struct Foo<T> {
    f: T,
}

fn main() {
    let mut x = Foo { f: 0_u8 };
    let _ = Pin::new(&mut x).project(); //~ ERROR E0599
}
