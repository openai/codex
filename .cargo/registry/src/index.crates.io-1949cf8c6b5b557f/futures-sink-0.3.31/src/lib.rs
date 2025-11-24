//! Asynchronous sinks
//!
//! This crate contains the `Sink` trait which allows values to be sent
//! asynchronously.

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(missing_docs, /* unsafe_op_in_unsafe_fn */)] // unsafe_op_in_unsafe_fn requires Rust 1.52

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::ops::DerefMut;
use core::pin::Pin;
use core::task::{Context, Poll};

/// A `Sink` is a value into which other values can be sent, asynchronously.
///
/// Basic examples of sinks include the sending side of:
///
/// - Channels
/// - Sockets
/// - Pipes
///
/// In addition to such "primitive" sinks, it's typical to layer additional
/// functionality, such as buffering, on top of an existing sink.
///
/// Sending to a sink is "asynchronous" in the sense that the value may not be
/// sent in its entirety immediately. Instead, values are sent in a two-phase
/// way: first by initiating a send, and then by polling for completion. This
/// two-phase setup is analogous to buffered writing in synchronous code, where
/// writes often succeed immediately, but internally are buffered and are
/// *actually* written only upon flushing.
///
/// In addition, the `Sink` may be *full*, in which case it is not even possible
/// to start the sending process.
///
/// As with `Future` and `Stream`, the `Sink` trait is built from a few core
/// required methods, and a host of default methods for working in a
/// higher-level way. The `Sink::send_all` combinator is of particular
/// importance: you can use it to send an entire stream to a sink, which is
/// the simplest way to ultimately consume a stream.
#[must_use = "sinks do nothing unless polled"]
pub trait Sink<Item> {
    /// The type of value produced by the sink when an error occurs.
    type Error;

    /// Attempts to prepare the `Sink` to receive a value.
    ///
    /// This method must be called and return `Poll::Ready(Ok(()))` prior to
    /// each call to `start_send`.
    ///
    /// This method returns `Poll::Ready` once the underlying sink is ready to
    /// receive data. If this method returns `Poll::Pending`, the current task
    /// is registered to be notified (via `cx.waker().wake_by_ref()`) when `poll_ready`
    /// should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Begin the process of sending a value to the sink.
    /// Each call to this function must be preceded by a successful call to
    /// `poll_ready` which returned `Poll::Ready(Ok(()))`.
    ///
    /// As the name suggests, this method only *begins* the process of sending
    /// the item. If the sink employs buffering, the item isn't fully processed
    /// until the buffer is fully flushed. Since sinks are designed to work with
    /// asynchronous I/O, the process of actually writing out the data to an
    /// underlying object takes place asynchronously. **You *must* use
    /// `poll_flush` or `poll_close` in order to guarantee completion of a
    /// send**.
    ///
    /// Implementations of `poll_ready` and `start_send` will usually involve
    /// flushing behind the scenes in order to make room for new messages.
    /// It is only necessary to call `poll_flush` if you need to guarantee that
    /// *all* of the items placed into the `Sink` have been sent.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error>;

    /// Flush any remaining output from this sink.
    ///
    /// Returns `Poll::Ready(Ok(()))` when no buffered items remain. If this
    /// value is returned then it is guaranteed that all previous values sent
    /// via `start_send` have been flushed.
    ///
    /// Returns `Poll::Pending` if there is more work left to do, in which
    /// case the current task is scheduled (via `cx.waker().wake_by_ref()`) to wake up when
    /// `poll_flush` should be called again.
    ///
    /// In most cases, if the sink encounters an error, the sink will
    /// permanently be unable to receive items.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Flush any remaining output and close this sink, if necessary.
    ///
    /// Returns `Poll::Ready(Ok(()))` when no buffered items remain and the sink
    /// has been successfully closed.
    ///
    /// Returns `Poll::Pending` if there is more work left to do, in which
    /// case the current task is scheduled (via `cx.waker().wake_by_ref()`) to wake up when
    /// `poll_close` should be called again.
    ///
    /// If this function encounters an error, the sink should be considered to
    /// have failed permanently, and no more `Sink` methods should be called.
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
}

impl<S: ?Sized + Sink<Item> + Unpin, Item> Sink<Item> for &mut S {
    type Error = S::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        Pin::new(&mut **self).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_close(cx)
    }
}

impl<P, Item> Sink<Item> for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: Sink<Item>,
{
    type Error = <P::Target as Sink<Item>>::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.get_mut().as_mut().start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_close(cx)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use super::*;
    use core::convert::Infallible as Never;

    impl<T> Sink<T> for alloc::vec::Vec<T> {
        type Error = Never;

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { self.get_unchecked_mut() }.push(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<T> Sink<T> for alloc::collections::VecDeque<T> {
        type Error = Never;

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
            // TODO: impl<T> Unpin for Vec<T> {}
            unsafe { self.get_unchecked_mut() }.push_back(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl<S: ?Sized + Sink<Item> + Unpin, Item> Sink<Item> for alloc::boxed::Box<S> {
        type Error = S::Error;

        fn poll_ready(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Pin::new(&mut **self).poll_ready(cx)
        }

        fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
            Pin::new(&mut **self).start_send(item)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Pin::new(&mut **self).poll_flush(cx)
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Pin::new(&mut **self).poll_close(cx)
        }
    }
}
