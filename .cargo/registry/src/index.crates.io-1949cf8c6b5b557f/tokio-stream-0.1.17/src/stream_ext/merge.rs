use crate::stream_ext::Fuse;
use crate::Stream;

use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream returned by the [`merge`](super::StreamExt::merge) method.
    pub struct Merge<T, U> {
        #[pin]
        a: Fuse<T>,
        #[pin]
        b: Fuse<U>,
        // When `true`, poll `a` first, otherwise, `poll` b`.
        a_first: bool,
    }
}

impl<T, U> Merge<T, U> {
    pub(super) fn new(a: T, b: U) -> Merge<T, U>
    where
        T: Stream,
        U: Stream,
    {
        Merge {
            a: Fuse::new(a),
            b: Fuse::new(b),
            a_first: true,
        }
    }
}

impl<T, U> Stream for Merge<T, U>
where
    T: Stream,
    U: Stream<Item = T::Item>,
{
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T::Item>> {
        let me = self.project();
        let a_first = *me.a_first;

        // Toggle the flag
        *me.a_first = !a_first;

        if a_first {
            poll_next(me.a, me.b, cx)
        } else {
            poll_next(me.b, me.a, cx)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        super::merge_size_hints(self.a.size_hint(), self.b.size_hint())
    }
}

fn poll_next<T, U>(
    first: Pin<&mut T>,
    second: Pin<&mut U>,
    cx: &mut Context<'_>,
) -> Poll<Option<T::Item>>
where
    T: Stream,
    U: Stream<Item = T::Item>,
{
    let mut done = true;

    match first.poll_next(cx) {
        Poll::Ready(Some(val)) => return Poll::Ready(Some(val)),
        Poll::Ready(None) => {}
        Poll::Pending => done = false,
    }

    match second.poll_next(cx) {
        Poll::Ready(Some(val)) => return Poll::Ready(Some(val)),
        Poll::Ready(None) => {}
        Poll::Pending => done = false,
    }

    if done {
        Poll::Ready(None)
    } else {
        Poll::Pending
    }
}
