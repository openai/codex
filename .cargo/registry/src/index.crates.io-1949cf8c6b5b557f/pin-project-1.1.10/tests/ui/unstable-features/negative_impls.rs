// SPDX-License-Identifier: Apache-2.0 OR MIT

#![feature(negative_impls)]

// https://rust-lang.zulipchat.com/#narrow/stream/213817-t-lang/topic/design.20meeting.3A.20backlog.20bonanza/near/269471299
// https://github.com/taiki-e/pin-project/issues/340

#[pin_project::pin_project]
struct Foo<Pinned, Unpinned> {
    #[pin]
    pinned: Pinned,
    unpinned: Unpinned,
}

struct MyPhantomPinned {}
impl !Unpin for MyPhantomPinned {}
impl Unpin for Foo<MyPhantomPinned, ()> {}

fn is_unpin<T: Unpin>() {}

fn main() {
    is_unpin::<Foo<MyPhantomPinned, ()>>()
}
