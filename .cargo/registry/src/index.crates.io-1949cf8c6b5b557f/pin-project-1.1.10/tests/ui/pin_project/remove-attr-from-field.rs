// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{marker::PhantomPinned, pin::Pin};

use auxiliary_macro::remove_attr;
use pin_project::pin_project;

fn is_unpin<T: Unpin>() {}

#[pin_project]
#[remove_attr(field_all)]
struct A {
    #[pin]
    f: PhantomPinned,
}

#[remove_attr(field_all)]
#[pin_project]
struct B {
    #[pin]
    f: PhantomPinned,
}

fn main() {
    is_unpin::<A>();
    is_unpin::<B>();

    let mut x = A { f: PhantomPinned };
    let x = Pin::new(&mut x).project();
    let _: Pin<&mut PhantomPinned> = x.f; //~ ERROR E0308

    let mut x = B { f: PhantomPinned };
    let x = Pin::new(&mut x).project();
    let _: Pin<&mut PhantomPinned> = x.f; //~ ERROR E0308
}
