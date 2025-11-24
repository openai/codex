use crate::stream::{FuturesUnordered, StreamExt};
use alloc::collections::binary_heap::{BinaryHeap, PeekMut};
use core::cmp::Ordering;
use core::fmt::{self, Debug};
use core::iter::FromIterator;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::{
    task::{Context, Poll},
    FusedStream,
};
use pin_project_lite::pin_project;

pin_project! {
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    struct OrderWrapper<T> {
        #[pin]
        data: T, // A future or a future's output
        // Use i64 for index since isize may overflow in 32-bit targets.
        index: i64,
    }
}

impl<T> PartialEq for OrderWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for OrderWrapper<T> {}

impl<T> PartialOrd for OrderWrapper<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for OrderWrapper<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max heap, so compare backwards here.
        other.index.cmp(&self.index)
    }
}

impl<T> Future for OrderWrapper<T>
where
    T: Future,
{
    type Output = OrderWrapper<T::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let index = self.index;
        self.project().data.poll(cx).map(|output| OrderWrapper { data: output, index })
    }
}

/// An unbounded queue of futures.
///
/// This "combinator" is similar to [`FuturesUnordered`], but it imposes a FIFO
/// order on top of the set of futures. While futures in the set will race to
/// completion in parallel, results will only be returned in the order their
/// originating futures were added to the queue.
///
/// Futures are pushed into this queue and their realized values are yielded in
/// order. This structure is optimized to manage a large number of futures.
/// Futures managed by [`FuturesOrdered`] will only be polled when they generate
/// notifications. This reduces the required amount of work needed to coordinate
/// large numbers of futures.
///
/// When a [`FuturesOrdered`] is first created, it does not contain any futures.
/// Calling [`poll_next`](FuturesOrdered::poll_next) in this state will result
/// in [`Poll::Ready(None)`](Poll::Ready) to be returned. Futures are submitted
/// to the queue using [`push_back`](FuturesOrdered::push_back) (or
/// [`push_front`](FuturesOrdered::push_front)); however, the future will
/// **not** be polled at this point. [`FuturesOrdered`] will only poll managed
/// futures when [`FuturesOrdered::poll_next`] is called. As such, it
/// is important to call [`poll_next`](FuturesOrdered::poll_next) after pushing
/// new futures.
///
/// If [`FuturesOrdered::poll_next`] returns [`Poll::Ready(None)`](Poll::Ready)
/// this means that the queue is currently not managing any futures. A future
/// may be submitted to the queue at a later time. At that point, a call to
/// [`FuturesOrdered::poll_next`] will either return the future's resolved value
/// **or** [`Poll::Pending`] if the future has not yet completed. When
/// multiple futures are submitted to the queue, [`FuturesOrdered::poll_next`]
/// will return [`Poll::Pending`] until the first future completes, even if
/// some of the later futures have already completed.
///
/// Note that you can create a ready-made [`FuturesOrdered`] via the
/// [`collect`](Iterator::collect) method, or you can start with an empty queue
/// with the [`FuturesOrdered::new`] constructor.
///
/// This type is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
#[must_use = "streams do nothing unless polled"]
pub struct FuturesOrdered<T: Future> {
    in_progress_queue: FuturesUnordered<OrderWrapper<T>>,
    queued_outputs: BinaryHeap<OrderWrapper<T::Output>>,
    next_incoming_index: i64,
    next_outgoing_index: i64,
}

impl<T: Future> Unpin for FuturesOrdered<T> {}

impl<Fut: Future> FuturesOrdered<Fut> {
    /// Constructs a new, empty `FuturesOrdered`
    ///
    /// The returned [`FuturesOrdered`] does not contain any futures and, in
    /// this state, [`FuturesOrdered::poll_next`] will return
    /// [`Poll::Ready(None)`](Poll::Ready).
    pub fn new() -> Self {
        Self {
            in_progress_queue: FuturesUnordered::new(),
            queued_outputs: BinaryHeap::new(),
            next_incoming_index: 0,
            next_outgoing_index: 0,
        }
    }

    /// Returns the number of futures contained in the queue.
    ///
    /// This represents the total number of in-flight futures, both
    /// those currently processing and those that have completed but
    /// which are waiting for earlier futures to complete.
    pub fn len(&self) -> usize {
        self.in_progress_queue.len() + self.queued_outputs.len()
    }

    /// Returns `true` if the queue contains no futures
    pub fn is_empty(&self) -> bool {
        self.in_progress_queue.is_empty() && self.queued_outputs.is_empty()
    }

    /// Push a future into the queue.
    ///
    /// This function submits the given future to the internal set for managing.
    /// This function will not call [`poll`](Future::poll) on the submitted
    /// future. The caller must ensure that [`FuturesOrdered::poll_next`] is
    /// called in order to receive task notifications.
    #[deprecated(note = "use `push_back` instead")]
    pub fn push(&mut self, future: Fut) {
        self.push_back(future);
    }

    /// Pushes a future to the back of the queue.
    ///
    /// This function submits the given future to the internal set for managing.
    /// This function will not call [`poll`](Future::poll) on the submitted
    /// future. The caller must ensure that [`FuturesOrdered::poll_next`] is
    /// called in order to receive task notifications.
    pub fn push_back(&mut self, future: Fut) {
        let wrapped = OrderWrapper { data: future, index: self.next_incoming_index };
        self.next_incoming_index += 1;
        self.in_progress_queue.push(wrapped);
    }

    /// Pushes a future to the front of the queue.
    ///
    /// This function submits the given future to the internal set for managing.
    /// This function will not call [`poll`](Future::poll) on the submitted
    /// future. The caller must ensure that [`FuturesOrdered::poll_next`] is
    /// called in order to receive task notifications. This future will be
    /// the next future to be returned complete.
    pub fn push_front(&mut self, future: Fut) {
        let wrapped = OrderWrapper { data: future, index: self.next_outgoing_index - 1 };
        self.next_outgoing_index -= 1;
        self.in_progress_queue.push(wrapped);
    }
}

impl<Fut: Future> Default for FuturesOrdered<Fut> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Fut: Future> Stream for FuturesOrdered<Fut> {
    type Item = Fut::Output;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        // Check to see if we've already received the next value
        if let Some(next_output) = this.queued_outputs.peek_mut() {
            if next_output.index == this.next_outgoing_index {
                this.next_outgoing_index += 1;
                return Poll::Ready(Some(PeekMut::pop(next_output).data));
            }
        }

        loop {
            match ready!(this.in_progress_queue.poll_next_unpin(cx)) {
                Some(output) => {
                    if output.index == this.next_outgoing_index {
                        this.next_outgoing_index += 1;
                        return Poll::Ready(Some(output.data));
                    } else {
                        this.queued_outputs.push(output)
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<Fut: Future> Debug for FuturesOrdered<Fut> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FuturesOrdered {{ ... }}")
    }
}

impl<Fut: Future> FromIterator<Fut> for FuturesOrdered<Fut> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Fut>,
    {
        let acc = Self::new();
        iter.into_iter().fold(acc, |mut acc, item| {
            acc.push_back(item);
            acc
        })
    }
}

impl<Fut: Future> FusedStream for FuturesOrdered<Fut> {
    fn is_terminated(&self) -> bool {
        self.in_progress_queue.is_terminated() && self.queued_outputs.is_empty()
    }
}

impl<Fut: Future> Extend<Fut> for FuturesOrdered<Fut> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = Fut>,
    {
        for item in iter {
            self.push_back(item);
        }
    }
}
