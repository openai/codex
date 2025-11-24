// SPDX-License-Identifier: Apache-2.0 OR MIT

use auxiliary_macro::{hidden_repr_macro, HiddenRepr};
use pin_project::pin_project;

hidden_repr_macro! {} //~ ERROR expected item after attributes
#[pin_project]
struct S1 {
    #[pin]
    f: u32,
}

macro_rules! hidden_repr_macro2 {
    () => {
        #[repr(packed)] //~ ERROR expected item after attributes
    };
}

hidden_repr_macro2! {}
#[pin_project]
struct S2 {
    #[pin]
    f: u32,
}

#[derive(HiddenRepr)] //~ ERROR expected item after attributes
struct S3 {}
#[pin_project]
struct S4 {
    #[pin]
    f: u32,
}

fn main() {}
