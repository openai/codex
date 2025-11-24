// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::marker::PhantomPinned;

use pin_project_lite::pin_project;

pin_project! {
    struct Foo<T> {
        #[pin]
        inner: T,
    }
}

struct __Origin {}

impl Unpin for __Origin {}

fn is_unpin<T: Unpin>() {}

fn main() {
    is_unpin::<Foo<PhantomPinned>>(); //~ ERROR E0277
}
