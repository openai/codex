//! Asynchronous streams.

use core::ops::DerefMut;
use core::pin::Pin;
use core::task::{Context, Poll};

/// An owned dynamically typed [`Stream`] for use in cases where you can't
/// statically type your result or need to add some indirection.
///
/// This type is often created by the [`boxed`] method on [`StreamExt`]. See its documentation for more.
///
/// [`boxed`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.boxed
/// [`StreamExt`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html
#[cfg(feature = "alloc")]
pub type BoxStream<'a, T> = Pin<alloc::boxed::Box<dyn Stream<Item = T> + Send + 'a>>;

/// `BoxStream`, but without the `Send` requirement.
///
/// This type is often created by the [`boxed_local`] method on [`StreamExt`]. See its documentation for more.
///
/// [`boxed_local`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.boxed_local
/// [`StreamExt`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html
#[cfg(feature = "alloc")]
pub type LocalBoxStream<'a, T> = Pin<alloc::boxed::Box<dyn Stream<Item = T> + 'a>>;

/// A stream of values produced asynchronously.
///
/// If `Future<Output = T>` is an asynchronous version of `T`, then `Stream<Item
/// = T>` is an asynchronous version of `Iterator<Item = T>`. A stream
/// represents a sequence of value-producing events that occur asynchronously to
/// the caller.
///
/// The trait is modeled after `Future`, but allows `poll_next` to be called
/// even after a value has been produced, yielding `None` once the stream has
/// been fully exhausted.
#[must_use = "streams do nothing unless polled"]
pub trait Stream {
    /// Values yielded by the stream.
    type Item;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - `Poll::Pending` means that this stream's next value is not ready
    ///   yet. Implementations will ensure that the current task will be notified
    ///   when the next value may be ready.
    ///
    /// - `Poll::Ready(Some(val))` means that the stream has successfully
    ///   produced a value, `val`, and may produce further values on subsequent
    ///   `poll_next` calls.
    ///
    /// - `Poll::Ready(None)` means that the stream has terminated, and
    ///   `poll_next` should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream has finished (returned `Ready(None)` from `poll_next`), calling its
    /// `poll_next` method again may panic, block forever, or cause other kinds of
    /// problems; the `Stream` trait places no requirements on the effects of
    /// such a call. However, as the `poll_next` method is not marked `unsafe`,
    /// Rust's usual rules apply: calls must never cause undefined behavior
    /// (memory corruption, incorrect use of `unsafe` functions, or the like),
    /// regardless of the stream's state.
    ///
    /// If this is difficult to guard against then the [`fuse`] adapter can be used
    /// to ensure that `poll_next` always returns `Ready(None)` in subsequent
    /// calls.
    ///
    /// [`fuse`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.fuse
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;

    /// Returns the bounds on the remaining length of the stream.
    ///
    /// Specifically, `size_hint()` returns a tuple where the first element
    /// is the lower bound, and the second element is the upper bound.
    ///
    /// The second half of the tuple that is returned is an [`Option`]`<`[`usize`]`>`.
    /// A [`None`] here means that either there is no known upper bound, or the
    /// upper bound is larger than [`usize`].
    ///
    /// # Implementation notes
    ///
    /// It is not enforced that a stream implementation yields the declared
    /// number of elements. A buggy stream may yield less than the lower bound
    /// or more than the upper bound of elements.
    ///
    /// `size_hint()` is primarily intended to be used for optimizations such as
    /// reserving space for the elements of the stream, but must not be
    /// trusted to e.g., omit bounds checks in unsafe code. An incorrect
    /// implementation of `size_hint()` should not lead to memory safety
    /// violations.
    ///
    /// That said, the implementation should provide a correct estimation,
    /// because otherwise it would be a violation of the trait's protocol.
    ///
    /// The default implementation returns `(0, `[`None`]`)` which is correct for any
    /// stream.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<S: ?Sized + Stream + Unpin> Stream for &mut S {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        S::poll_next(Pin::new(&mut **self), cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

impl<P> Stream for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: Stream,
{
    type Item = <P::Target as Stream>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

/// A stream which tracks whether or not the underlying stream
/// should no longer be polled.
///
/// `is_terminated` will return `true` if a future should no longer be polled.
/// Usually, this state occurs after `poll_next` (or `try_poll_next`) returned
/// `Poll::Ready(None)`. However, `is_terminated` may also return `true` if a
/// stream has become inactive and can no longer make progress and should be
/// ignored or dropped rather than being polled again.
pub trait FusedStream: Stream {
    /// Returns `true` if the stream should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<F: ?Sized + FusedStream + Unpin> FusedStream for &mut F {
    fn is_terminated(&self) -> bool {
        <F as FusedStream>::is_terminated(&**self)
    }
}

impl<P> FusedStream for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: FusedStream,
{
    fn is_terminated(&self) -> bool {
        <P::Target as FusedStream>::is_terminated(&**self)
    }
}

mod private_try_stream {
    use super::Stream;

    pub trait Sealed {}

    impl<S, T, E> Sealed for S where S: ?Sized + Stream<Item = Result<T, E>> {}
}

/// A convenience for streams that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryStream: Stream + private_try_stream::Sealed {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryStream` as if it were a `Stream`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Stream` trait; in the future it won't be
    /// needed.
    fn try_poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Ok, Self::Error>>>;
}

impl<S, T, E> TryStream for S
where
    S: ?Sized + Stream<Item = Result<T, E>>,
{
    type Ok = T;
    type Error = E;

    fn try_poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Ok, Self::Error>>> {
        self.poll_next(cx)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use alloc::boxed::Box;

    impl<S: ?Sized + Stream + Unpin> Stream for Box<S> {
        type Item = S::Item;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Pin::new(&mut **self).poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (**self).size_hint()
        }
    }

    #[cfg(feature = "std")]
    impl<S: Stream> Stream for std::panic::AssertUnwindSafe<S> {
        type Item = S::Item;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
            unsafe { self.map_unchecked_mut(|x| &mut x.0) }.poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.0.size_hint()
        }
    }

    impl<S: ?Sized + FusedStream + Unpin> FusedStream for Box<S> {
        fn is_terminated(&self) -> bool {
            <S as FusedStream>::is_terminated(&**self)
        }
    }
}
