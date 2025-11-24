// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project_lite::pin_project;

// In `Drop` impl, the implementor must specify the same requirement as type definition.

struct DropImpl<T> {
    f: T,
}

impl<T: Unpin> Drop for DropImpl<T> {
    //~^ ERROR E0367
    fn drop(&mut self) {}
}

pin_project! {
    //~^ ERROR E0367
    struct PinnedDropImpl<T> {
        #[pin]
        f: T,
    }

    impl<T: Unpin> PinnedDrop for PinnedDropImpl<T> {
        fn drop(_this: Pin<&mut Self>) {}
    }
}

fn main() {}
