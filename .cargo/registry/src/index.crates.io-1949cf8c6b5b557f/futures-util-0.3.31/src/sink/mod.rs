//! Asynchronous sinks.
//!
//! This module contains:
//!
//! - The [`Sink`] trait, which allows you to asynchronously write data.
//! - The [`SinkExt`] trait, which provides adapters for chaining and composing
//!   sinks.

use crate::future::{assert_future, Either};
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};

#[cfg(feature = "compat")]
use crate::compat::CompatSink;

pub use futures_sink::Sink;

mod close;
pub use self::close::Close;

mod drain;
pub use self::drain::{drain, Drain};

mod fanout;
pub use self::fanout::Fanout;

mod feed;
pub use self::feed::Feed;

mod flush;
pub use self::flush::Flush;

mod err_into;
pub use self::err_into::SinkErrInto;

mod map_err;
pub use self::map_err::SinkMapErr;

mod send;
pub use self::send::Send;

mod send_all;
pub use self::send_all::SendAll;

mod unfold;
pub use self::unfold::{unfold, Unfold};

mod with;
pub use self::with::With;

mod with_flat_map;
pub use self::with_flat_map::WithFlatMap;

#[cfg(feature = "alloc")]
mod buffer;
#[cfg(feature = "alloc")]
pub use self::buffer::Buffer;

impl<T: ?Sized, Item> SinkExt<Item> for T where T: Sink<Item> {}

/// An extension trait for `Sink`s that provides a variety of convenient
/// combinator functions.
pub trait SinkExt<Item>: Sink<Item> {
    /// Composes a function *in front of* the sink.
    ///
    /// This adapter produces a new sink that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// To process each value, `f` produces a *future*, which is then polled to
    /// completion before passing its result down to the underlying sink. If the
    /// future produces an error, that error is returned by the new sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    fn with<U, Fut, F, E>(self, f: F) -> With<Self, Item, U, Fut, F>
    where
        F: FnMut(U) -> Fut,
        Fut: Future<Output = Result<Item, E>>,
        E: From<Self::Error>,
        Self: Sized,
    {
        assert_sink::<U, E, _>(With::new(self, f))
    }

    /// Composes a function *in front of* the sink.
    ///
    /// This adapter produces a new sink that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// To process each value, `f` produces a *stream*, of which each value
    /// is passed to the underlying sink. A new value will not be accepted until
    /// the stream has been drained
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::flat_map`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::sink::SinkExt;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let (tx, rx) = mpsc::channel(5);
    ///
    /// let mut tx = tx.with_flat_map(|x| {
    ///     stream::iter(vec![Ok(42); x])
    /// });
    ///
    /// tx.send(5).await.unwrap();
    /// drop(tx);
    /// let received: Vec<i32> = rx.collect().await;
    /// assert_eq!(received, vec![42, 42, 42, 42, 42]);
    /// # });
    /// ```
    fn with_flat_map<U, St, F>(self, f: F) -> WithFlatMap<Self, Item, U, St, F>
    where
        F: FnMut(U) -> St,
        St: Stream<Item = Result<Item, Self::Error>>,
        Self: Sized,
    {
        assert_sink::<U, Self::Error, _>(WithFlatMap::new(self, f))
    }

    /*
    fn with_map<U, F>(self, f: F) -> WithMap<Self, U, F>
        where F: FnMut(U) -> Self::SinkItem,
              Self: Sized;

    fn with_filter<F>(self, f: F) -> WithFilter<Self, F>
        where F: FnMut(Self::SinkItem) -> bool,
              Self: Sized;

    fn with_filter_map<U, F>(self, f: F) -> WithFilterMap<Self, U, F>
        where F: FnMut(U) -> Option<Self::SinkItem>,
              Self: Sized;
     */

    /// Transforms the error returned by the sink.
    fn sink_map_err<E, F>(self, f: F) -> SinkMapErr<Self, F>
    where
        F: FnOnce(Self::Error) -> E,
        Self: Sized,
    {
        assert_sink::<Item, E, _>(SinkMapErr::new(self, f))
    }

    /// Map this sink's error to a different error type using the `Into` trait.
    ///
    /// If wanting to map errors of a `Sink + Stream`, use `.sink_err_into().err_into()`.
    fn sink_err_into<E>(self) -> err_into::SinkErrInto<Self, Item, E>
    where
        Self: Sized,
        Self::Error: Into<E>,
    {
        assert_sink::<Item, E, _>(SinkErrInto::new(self))
    }

    /// Adds a fixed-size buffer to the current sink.
    ///
    /// The resulting sink will buffer up to `capacity` items when the
    /// underlying sink is unwilling to accept additional items. Calling `flush`
    /// on the buffered sink will attempt to both empty the buffer and complete
    /// processing on the underlying sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "alloc")]
    fn buffer(self, capacity: usize) -> Buffer<Self, Item>
    where
        Self: Sized,
    {
        assert_sink::<Item, Self::Error, _>(Buffer::new(self, capacity))
    }

    /// Close the sink.
    fn close(&mut self) -> Close<'_, Self, Item>
    where
        Self: Unpin,
    {
        assert_future::<Result<(), Self::Error>, _>(Close::new(self))
    }

    /// Fanout items to multiple sinks.
    ///
    /// This adapter clones each incoming item and forwards it to both this as well as
    /// the other sink at the same time.
    fn fanout<Si>(self, other: Si) -> Fanout<Self, Si>
    where
        Self: Sized,
        Item: Clone,
        Si: Sink<Item, Error = Self::Error>,
    {
        assert_sink::<Item, Self::Error, _>(Fanout::new(self, other))
    }

    /// Flush the sink, processing all pending items.
    ///
    /// This adapter is intended to be used when you want to stop sending to the sink
    /// until all current requests are processed.
    fn flush(&mut self) -> Flush<'_, Self, Item>
    where
        Self: Unpin,
    {
        assert_future::<Result<(), Self::Error>, _>(Flush::new(self))
    }

    /// A future that completes after the given item has been fully processed
    /// into the sink, including flushing.
    ///
    /// Note that, **because of the flushing requirement, it is usually better
    /// to batch together items to send via `feed` or `send_all`,
    /// rather than flushing between each item.**
    fn send(&mut self, item: Item) -> Send<'_, Self, Item>
    where
        Self: Unpin,
    {
        assert_future::<Result<(), Self::Error>, _>(Send::new(self, item))
    }

    /// A future that completes after the given item has been received
    /// by the sink.
    ///
    /// Unlike `send`, the returned future does not flush the sink.
    /// It is the caller's responsibility to ensure all pending items
    /// are processed, which can be done via `flush` or `close`.
    fn feed(&mut self, item: Item) -> Feed<'_, Self, Item>
    where
        Self: Unpin,
    {
        assert_future::<Result<(), Self::Error>, _>(Feed::new(self, item))
    }

    /// A future that completes after the given stream has been fully processed
    /// into the sink, including flushing.
    ///
    /// This future will drive the stream to keep producing items until it is
    /// exhausted, sending each item to the sink. It will complete once both the
    /// stream is exhausted, the sink has received all items, and the sink has
    /// been flushed. Note that the sink is **not** closed. If the stream produces
    /// an error, that error will be returned by this future without flushing the sink.
    ///
    /// Doing `sink.send_all(stream)` is roughly equivalent to
    /// `stream.forward(sink)`. The returned future will exhaust all items from
    /// `stream` and send them to `self`.
    fn send_all<'a, St>(&'a mut self, stream: &'a mut St) -> SendAll<'a, Self, St>
    where
        St: TryStream<Ok = Item, Error = Self::Error> + Stream + Unpin + ?Sized,
        // St: Stream<Item = Result<Item, Self::Error>> + Unpin + ?Sized,
        Self: Unpin,
    {
        // TODO: type mismatch resolving `<St as Stream>::Item == std::result::Result<Item, <Self as futures_sink::Sink<Item>>::Error>`
        // assert_future::<Result<(), Self::Error>, _>(SendAll::new(self, stream))
        SendAll::new(self, stream)
    }

    /// Wrap this sink in an `Either` sink, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right_sink` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn left_sink<Si2>(self) -> Either<Self, Si2>
    where
        Si2: Sink<Item, Error = Self::Error>,
        Self: Sized,
    {
        assert_sink::<Item, Self::Error, _>(Either::Left(self))
    }

    /// Wrap this stream in an `Either` stream, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left_sink` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn right_sink<Si1>(self) -> Either<Si1, Self>
    where
        Si1: Sink<Item, Error = Self::Error>,
        Self: Sized,
    {
        assert_sink::<Item, Self::Error, _>(Either::Right(self))
    }

    /// Wraps a [`Sink`] into a sink compatible with libraries using
    /// futures 0.1 `Sink`. Requires the `compat` feature to be enabled.
    #[cfg(feature = "compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compat")))]
    fn compat(self) -> CompatSink<Self, Item>
    where
        Self: Sized + Unpin,
    {
        CompatSink::new(self)
    }

    /// A convenience method for calling [`Sink::poll_ready`] on [`Unpin`]
    /// sink types.
    fn poll_ready_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_ready(cx)
    }

    /// A convenience method for calling [`Sink::start_send`] on [`Unpin`]
    /// sink types.
    fn start_send_unpin(&mut self, item: Item) -> Result<(), Self::Error>
    where
        Self: Unpin,
    {
        Pin::new(self).start_send(item)
    }

    /// A convenience method for calling [`Sink::poll_flush`] on [`Unpin`]
    /// sink types.
    fn poll_flush_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_flush(cx)
    }

    /// A convenience method for calling [`Sink::poll_close`] on [`Unpin`]
    /// sink types.
    fn poll_close_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_close(cx)
    }
}

// Just a helper function to ensure the sinks we're returning all have the
// right implementations.
pub(crate) fn assert_sink<T, E, S>(sink: S) -> S
where
    S: Sink<T, Error = E>,
{
    sink
}
