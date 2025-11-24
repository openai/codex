// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::marker::PhantomPinned;

use auxiliary_macro::add_pin_attr;
use pin_project::pin_project;

#[pin_project]
#[add_pin_attr(struct)] //~ ERROR expected attribute arguments in parentheses
struct Foo {
    #[pin]
    f: PhantomPinned,
}

#[add_pin_attr(struct)] //~ ERROR #[pin] attribute may only be used on fields of structs or variants
#[pin_project]
struct Bar {
    #[pin]
    f: PhantomPinned,
}

fn main() {}
