// SPDX-License-Identifier: Apache-2.0 OR MIT

use auxiliary_macro::hidden_repr_macro;
use pin_project::pin_project;

hidden_repr_macro! { //~ ERROR may not be used on #[repr(packed)] types
    #[pin_project]
    struct B {
        #[pin]
        f: u32,
    }
}

fn main() {}
