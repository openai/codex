// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project_lite::pin_project;

pin_project! {
    struct A<T> {
        #[pin()] //~ ERROR no rules expected the token `(`
        pinned: T,
    }
}

pin_project! {
    #[pin] //~ ERROR cannot find attribute `pin` in this scope
    struct B<T> {
        pinned: T,
    }
}

pin_project! {
    struct C<T> {
        #[pin]
        #[pin] //~ ERROR no rules expected the token `#`
        pinned: T,
    }
}

fn main() {}
