// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::pin::Pin;

use pin_project::pin_project;

#[cfg_attr(not(any()), pin_project)]
struct Foo<T> {
    #[cfg_attr(any(), pin)]
    f: T,
}

#[cfg_attr(not(any()), pin_project)]
struct Bar<T> {
    #[cfg_attr(not(any()), pin)]
    f: T,
}

fn main() {
    let mut x = Foo { f: 0_u8 };
    let x = Pin::new(&mut x).project();
    let _: Pin<&mut u8> = x.f; //~ ERROR E0308

    let mut x = Bar { f: 0_u8 };
    let x = Pin::new(&mut x).project();
    let _: &mut u8 = x.f; //~ ERROR E0308
}
