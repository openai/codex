#![no_std]
#![doc = include_str!("../README.md")]

use core::pin::Pin;
use core::task::{Context, Poll};

/// A stream that produces items that are ordered according to some token.
///
/// The main advantage of this trait over the standard `Stream` trait is the ability to implement a
/// [`join`](join()) function that does not either block until both source streams produce an item
/// or contain a race condition when rejoining streams that originated from a common well-ordered
/// source.
pub trait OrderedStream {
    /// The type ordered by this stream.
    ///
    /// Each stream must produce values that are in ascending order according to this function,
    /// although there is no requirement that the values be strictly ascending.
    type Ordering: Ord;

    /// The unordered data carried by this stream
    ///
    /// This is split from the `Ordering` type to allow specifying a smaller or cheaper-to-generate
    /// type as the ordering key.  This is especially useful if you generate values to pass in to
    /// `before`.
    type Data;

    /// Attempt to pull out the next value of this stream, registering the current task for wakeup
    /// if needed, and returning `NoneBefore` if it is known that the stream will not produce any
    /// more values ordered before the given point.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct stream state depending
    /// on the value passed in `before`:
    ///
    /// - If `before` was `None`, `Poll::Pending` means that this stream's next value is not ready
    /// yet. Implementations will ensure that the current task is notified when the next value may
    /// be ready.
    ///
    /// - If `before` was `Some`, `Poll::Pending` means that this stream's next value is not ready
    /// and that it is not yet known if the stream will produce a value ordered prior to the given
    /// ordering value.  Implementations will ensure that the current task is notified when either
    /// the next value is ready or once it is known that no such value will be produced.
    ///
    /// - `Poll::Ready(PollResult::Item)` means that the stream has successfully produced
    /// an item.  The stream may produce further values on subsequent `poll_next_before` calls.
    /// The returned ordering value must not be less than any prior ordering value returned by this
    /// stream.  The returned ordering value **may** be greater than the value passed to `before`.
    ///
    /// - `Poll::Ready(PollResult::Terminated)` means that the stream has terminated, and
    /// `poll_next_before` should not be invoked again.
    ///
    /// - `Poll::Ready(PollResult::NoneBefore)` means that the stream will not produce
    /// any further ordering tokens less than the given token.  Subsequent `poll_next_before` calls
    /// may still produce additional items, but their tokens will be greater than or equal to the
    /// given token.  It does not make sense to return this value if `before` was `None`.
    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>>;

    /// The minimum value of the ordering for any future items.
    ///
    /// If this does not return `None`, the returned ordering must be less than or equal to the
    /// ordering of any future item returned from [`Self::poll_next_before`].  This value should
    /// (but is not required to) be greater than or equal to the ordering of the most recent item
    /// returned.
    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        None
    }

    /// Returns the bounds on the remaining length of the stream.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

/// A value that is either borrowed or owned.
///
/// This is similar to `std::borrow::Cow`, but does not require the ability to convert from
/// borrowed to owned.
#[derive(Debug)]
pub enum MaybeBorrowed<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<'a, T> AsRef<T> for MaybeBorrowed<'a, T> {
    fn as_ref(&self) -> &T {
        match self {
            Self::Borrowed(t) => t,
            Self::Owned(t) => t,
        }
    }
}

impl<'a, T> core::ops::Deref for MaybeBorrowed<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Self::Borrowed(t) => t,
            Self::Owned(t) => t,
        }
    }
}

impl<P> OrderedStream for Pin<P>
where
    P: core::ops::DerefMut + Unpin,
    P::Target: OrderedStream,
{
    type Data = <P::Target as OrderedStream>::Data;
    type Ordering = <P::Target as OrderedStream>::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        self.get_mut().as_mut().poll_next_before(cx, before)
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        (**self).position_hint()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

impl<S> OrderedStream for Option<S>
where
    S: OrderedStream,
{
    type Data = S::Data;
    type Ordering = S::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        match self.as_pin_mut() {
            Some(s) => s.poll_next_before(cx, before),
            None => Poll::Ready(PollResult::Terminated),
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.as_ref().and_then(|s| s.position_hint())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.as_ref().map_or((0, Some(0)), |s| s.size_hint())
    }
}

/// An [`OrderedStream`] that tracks if the underlying stream should be polled.
pub trait FusedOrderedStream: OrderedStream {
    /// Returns `true` if the stream should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<P> FusedOrderedStream for Pin<P>
where
    P: core::ops::DerefMut + Unpin,
    P::Target: FusedOrderedStream,
{
    fn is_terminated(&self) -> bool {
        (**self).is_terminated()
    }
}

impl<S> FusedOrderedStream for Option<S>
where
    S: FusedOrderedStream,
{
    fn is_terminated(&self) -> bool {
        self.as_ref().map_or(true, |s| s.is_terminated())
    }
}

/// The result of a [`OrderedStream::poll_next_before`] operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PollResult<Ordering, Data> {
    /// An item with a corresponding ordering token.
    Item { data: Data, ordering: Ordering },
    /// This stream will not return any items prior to the given point.
    NoneBefore,
    /// This stream is terminated and should not be polled again.
    Terminated,
}

impl<D, T> PollResult<T, D> {
    /// Extract the data from the result.
    pub fn into_data(self) -> Option<D> {
        match self {
            Self::Item { data, .. } => Some(data),
            _ => None,
        }
    }

    /// Extract the item from the result.
    pub fn into_tuple(self) -> Option<(T, D)> {
        match self {
            Self::Item { data, ordering } => Some((ordering, data)),
            _ => None,
        }
    }

    /// Apply a closure to the data.
    pub fn map_data<R>(self, f: impl FnOnce(D) -> R) -> PollResult<T, R> {
        match self {
            Self::Item { data, ordering } => PollResult::Item {
                data: f(data),
                ordering,
            },
            Self::NoneBefore => PollResult::NoneBefore,
            Self::Terminated => PollResult::Terminated,
        }
    }
}

impl<T, D, E> PollResult<T, Result<D, E>> {
    /// Extract the error of a [`Result`] item.
    pub fn transpose_result(self) -> Result<PollResult<T, D>, E> {
        self.transpose_result_item().map_err(|(_, e)| e)
    }

    /// Extract the error and ordering from a [`Result`] item.
    pub fn transpose_result_item(self) -> Result<PollResult<T, D>, (T, E)> {
        match self {
            Self::Item {
                data: Ok(data),
                ordering,
            } => Ok(PollResult::Item { data, ordering }),
            Self::Item {
                data: Err(data),
                ordering,
            } => Err((ordering, data)),
            Self::NoneBefore => Ok(PollResult::NoneBefore),
            Self::Terminated => Ok(PollResult::Terminated),
        }
    }
}

/// A [`Future`](core::future::Future) that produces an item with an associated ordering.
///
/// This is equivalent to an [`OrderedStream`] that always produces exactly one item.  This trait
/// is not very useful on its own; see [`FromFuture`] to convert it to a stream.
///
/// It is valid to implement both [`Future`](core::future::Future) and [`OrderedFuture`] on the
/// same type.  In this case, unless otherwise documented by the implementing type, neither poll
/// function should be invoked after either returns an output value.
pub trait OrderedFuture {
    /// See [`OrderedStream::Ordering`].
    type Ordering: Ord;

    /// See [`OrderedStream::Data`].
    type Output;

    /// Attempt to pull out the value of this future, registering the current task for wakeup if
    /// needed, and returning `None` if it is known that the future will not produce a value
    /// ordered before the given point.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct state depending on the
    /// value passed in `before`:
    ///
    /// - If `before` was `None`, `Poll::Pending` means that this future's value is not ready yet.
    /// Implementations will ensure that the current task is notified when the next value may be
    /// ready.
    ///
    /// - If `before` was `Some`, `Poll::Pending` means that this future's value is not ready and
    /// that it is not yet known if the value will be ordered prior to the given ordering value.
    /// Implementations will ensure that the current task is notified when either the next value is
    /// ready or once it is known that no such value will be produced.
    ///
    /// - `Poll::Ready(Some(Data))` means that the future has successfully terminated.  The
    /// returned ordering value **may** be greater than the value passed to `before`.  The
    /// `poll_before` function should not be invoked again.
    ///
    /// - `Poll::Ready(None)` means that this future will not produce an ordering token less than
    /// the given token.  It is an error to return `None` if `before` was `None`.
    fn poll_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<Option<(Self::Ordering, Self::Output)>>;

    /// The minimum value of the ordering of the item.
    ///
    /// See [`OrderedStream::position_hint`] for details.
    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        None
    }
}

mod adapters;
pub use adapters::*;
mod join;
pub use join::*;
mod multi;
pub use multi::*;
