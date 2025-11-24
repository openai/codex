use crate::loom::sync::{atomic::AtomicUsize, Arc};
use crate::sync::mpsc::chan;
use crate::sync::mpsc::error::{SendError, TryRecvError};

use std::fmt;
use std::task::{Context, Poll};

/// Send values to the associated `UnboundedReceiver`.
///
/// Instances are created by the [`unbounded_channel`] function.
pub struct UnboundedSender<T> {
    chan: chan::Tx<T, Semaphore>,
}

/// An unbounded sender that does not prevent the channel from being closed.
///
/// If all [`UnboundedSender`] instances of a channel were dropped and only
/// `WeakUnboundedSender` instances remain, the channel is closed.
///
/// In order to send messages, the `WeakUnboundedSender` needs to be upgraded using
/// [`WeakUnboundedSender::upgrade`], which returns `Option<UnboundedSender>`. It returns `None`
/// if all `UnboundedSender`s have been dropped, and otherwise it returns an `UnboundedSender`.
///
/// [`UnboundedSender`]: UnboundedSender
/// [`WeakUnboundedSender::upgrade`]: WeakUnboundedSender::upgrade
///
/// # Examples
///
/// ```
/// use tokio::sync::mpsc::unbounded_channel;
///
/// #[tokio::main]
/// async fn main() {
///     let (tx, _rx) = unbounded_channel::<i32>();
///     let tx_weak = tx.downgrade();
///
///     // Upgrading will succeed because `tx` still exists.
///     assert!(tx_weak.upgrade().is_some());
///
///     // If we drop `tx`, then it will fail.
///     drop(tx);
///     assert!(tx_weak.clone().upgrade().is_none());
/// }
/// ```
pub struct WeakUnboundedSender<T> {
    chan: Arc<chan::Chan<T, Semaphore>>,
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        UnboundedSender {
            chan: self.chan.clone(),
        }
    }
}

impl<T> fmt::Debug for UnboundedSender<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("UnboundedSender")
            .field("chan", &self.chan)
            .finish()
    }
}

/// Receive values from the associated `UnboundedSender`.
///
/// Instances are created by the [`unbounded_channel`] function.
///
/// This receiver can be turned into a `Stream` using [`UnboundedReceiverStream`].
///
/// [`UnboundedReceiverStream`]: https://docs.rs/tokio-stream/0.1/tokio_stream/wrappers/struct.UnboundedReceiverStream.html
pub struct UnboundedReceiver<T> {
    /// The channel receiver
    chan: chan::Rx<T, Semaphore>,
}

impl<T> fmt::Debug for UnboundedReceiver<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("UnboundedReceiver")
            .field("chan", &self.chan)
            .finish()
    }
}

/// Creates an unbounded mpsc channel for communicating between asynchronous
/// tasks without backpressure.
///
/// A `send` on this channel will always succeed as long as the receive half has
/// not been closed. If the receiver falls behind, messages will be arbitrarily
/// buffered.
///
/// **Note** that the amount of available system memory is an implicit bound to
/// the channel. Using an `unbounded` channel has the ability of causing the
/// process to run out of memory. In this case, the process will be aborted.
pub fn unbounded_channel<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = chan::channel(Semaphore(AtomicUsize::new(0)));

    let tx = UnboundedSender::new(tx);
    let rx = UnboundedReceiver::new(rx);

    (tx, rx)
}

/// No capacity
#[derive(Debug)]
pub(crate) struct Semaphore(pub(crate) AtomicUsize);

impl<T> UnboundedReceiver<T> {
    pub(crate) fn new(chan: chan::Rx<T, Semaphore>) -> UnboundedReceiver<T> {
        UnboundedReceiver { chan }
    }

    /// Receives the next value for this receiver.
    ///
    /// This method returns `None` if the channel has been closed and there are
    /// no remaining messages in the channel's buffer. This indicates that no
    /// further values can ever be received from this `Receiver`. The channel is
    /// closed when all senders have been dropped, or when [`close`] is called.
    ///
    /// If there are no messages in the channel's buffer, but the channel has
    /// not yet been closed, this method will sleep until a message is sent or
    /// the channel is closed.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `recv` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, it is guaranteed that no messages were received on this
    /// channel.
    ///
    /// [`close`]: Self::close
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::unbounded_channel();
    ///
    ///     tokio::spawn(async move {
    ///         tx.send("hello").unwrap();
    ///     });
    ///
    ///     assert_eq!(Some("hello"), rx.recv().await);
    ///     assert_eq!(None, rx.recv().await);
    /// }
    /// ```
    ///
    /// Values are buffered:
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::unbounded_channel();
    ///
    ///     tx.send("hello").unwrap();
    ///     tx.send("world").unwrap();
    ///
    ///     assert_eq!(Some("hello"), rx.recv().await);
    ///     assert_eq!(Some("world"), rx.recv().await);
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<T> {
        use std::future::poll_fn;

        poll_fn(|cx| self.poll_recv(cx)).await
    }

    /// Receives the next values for this receiver and extends `buffer`.
    ///
    /// This method extends `buffer` by no more than a fixed number of values
    /// as specified by `limit`. If `limit` is zero, the function returns
    /// immediately with `0`. The return value is the number of values added to
    /// `buffer`.
    ///
    /// For `limit > 0`, if there are no messages in the channel's queue,
    /// but the channel has not yet been closed, this method will sleep
    /// until a message is sent or the channel is closed.
    ///
    /// For non-zero values of `limit`, this method will never return `0` unless
    /// the channel has been closed and there are no remaining messages in the
    /// channel's queue. This indicates that no further values can ever be
    /// received from this `Receiver`. The channel is closed when all senders
    /// have been dropped, or when [`close`] is called.
    ///
    /// The capacity of `buffer` is increased as needed.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `recv_many` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, it is guaranteed that no messages were received on this
    /// channel.
    ///
    /// [`close`]: Self::close
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut buffer: Vec<&str> = Vec::with_capacity(2);
    ///     let limit = 2;
    ///     let (tx, mut rx) = mpsc::unbounded_channel();
    ///     let tx2 = tx.clone();
    ///     tx2.send("first").unwrap();
    ///     tx2.send("second").unwrap();
    ///     tx2.send("third").unwrap();
    ///
    ///     // Call `recv_many` to receive up to `limit` (2) values.
    ///     assert_eq!(2, rx.recv_many(&mut buffer, limit).await);
    ///     assert_eq!(vec!["first", "second"], buffer);
    ///
    ///     // If the buffer is full, the next call to `recv_many`
    ///     // reserves additional capacity.
    ///     assert_eq!(1, rx.recv_many(&mut buffer, limit).await);
    ///
    ///     tokio::spawn(async move {
    ///         tx.send("fourth").unwrap();
    ///     });
    ///
    ///     // 'tx' is dropped, but `recv_many`
    ///     // is guaranteed not to return 0 as the channel
    ///     // is not yet closed.
    ///     assert_eq!(1, rx.recv_many(&mut buffer, limit).await);
    ///     assert_eq!(vec!["first", "second", "third", "fourth"], buffer);
    ///
    ///     // Once the last sender is dropped, the channel is
    ///     // closed and `recv_many` returns 0, capacity unchanged.
    ///     drop(tx2);
    ///     assert_eq!(0, rx.recv_many(&mut buffer, limit).await);
    ///     assert_eq!(vec!["first", "second", "third", "fourth"], buffer);
    /// }
    /// ```
    pub async fn recv_many(&mut self, buffer: &mut Vec<T>, limit: usize) -> usize {
        use std::future::poll_fn;
        poll_fn(|cx| self.chan.recv_many(cx, buffer, limit)).await
    }

    /// Tries to receive the next value for this receiver.
    ///
    /// This method returns the [`Empty`] error if the channel is currently
    /// empty, but there are still outstanding [senders] or [permits].
    ///
    /// This method returns the [`Disconnected`] error if the channel is
    /// currently empty, and there are no outstanding [senders] or [permits].
    ///
    /// Unlike the [`poll_recv`] method, this method will never return an
    /// [`Empty`] error spuriously.
    ///
    /// [`Empty`]: crate::sync::mpsc::error::TryRecvError::Empty
    /// [`Disconnected`]: crate::sync::mpsc::error::TryRecvError::Disconnected
    /// [`poll_recv`]: Self::poll_recv
    /// [senders]: crate::sync::mpsc::Sender
    /// [permits]: crate::sync::mpsc::Permit
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    /// use tokio::sync::mpsc::error::TryRecvError;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::unbounded_channel();
    ///
    ///     tx.send("hello").unwrap();
    ///
    ///     assert_eq!(Ok("hello"), rx.try_recv());
    ///     assert_eq!(Err(TryRecvError::Empty), rx.try_recv());
    ///
    ///     tx.send("hello").unwrap();
    ///     // Drop the last sender, closing the channel.
    ///     drop(tx);
    ///
    ///     assert_eq!(Ok("hello"), rx.try_recv());
    ///     assert_eq!(Err(TryRecvError::Disconnected), rx.try_recv());
    /// }
    /// ```
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        self.chan.try_recv()
    }

    /// Blocking receive to call outside of asynchronous contexts.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution
    /// context.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::unbounded_channel::<u8>();
    ///
    ///     let sync_code = thread::spawn(move || {
    ///         assert_eq!(Some(10), rx.blocking_recv());
    ///     });
    ///
    ///     let _ = tx.send(10);
    ///     sync_code.join().unwrap();
    /// }
    /// ```
    #[track_caller]
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(alias = "recv_blocking"))]
    pub fn blocking_recv(&mut self) -> Option<T> {
        crate::future::block_on(self.recv())
    }

    /// Variant of [`Self::recv_many`] for blocking contexts.
    ///
    /// The same conditions as in [`Self::blocking_recv`] apply.
    #[track_caller]
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(alias = "recv_many_blocking"))]
    pub fn blocking_recv_many(&mut self, buffer: &mut Vec<T>, limit: usize) -> usize {
        crate::future::block_on(self.recv_many(buffer, limit))
    }

    /// Closes the receiving half of a channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    ///
    /// To guarantee that no messages are dropped, after calling `close()`,
    /// `recv()` must be called until `None` is returned.
    pub fn close(&mut self) {
        self.chan.close();
    }

    /// Checks if a channel is closed.
    ///
    /// This method returns `true` if the channel has been closed. The channel is closed
    /// when all [`UnboundedSender`] have been dropped, or when [`UnboundedReceiver::close`] is called.
    ///
    /// [`UnboundedSender`]: crate::sync::mpsc::UnboundedSender
    /// [`UnboundedReceiver::close`]: crate::sync::mpsc::UnboundedReceiver::close
    ///
    /// # Examples
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (_tx, mut rx) = mpsc::unbounded_channel::<()>();
    ///     assert!(!rx.is_closed());
    ///
    ///     rx.close();
    ///
    ///     assert!(rx.is_closed());
    /// }
    /// ```
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    /// Checks if a channel is empty.
    ///
    /// This method returns `true` if the channel has no messages.
    ///
    /// # Examples
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::unbounded_channel();
    ///     assert!(rx.is_empty());
    ///
    ///     tx.send(0).unwrap();
    ///     assert!(!rx.is_empty());
    /// }
    ///
    /// ```
    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }

    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::unbounded_channel();
    ///     assert_eq!(0, rx.len());
    ///
    ///     tx.send(0).unwrap();
    ///     assert_eq!(1, rx.len());
    /// }
    /// ```
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    /// Polls to receive the next message on this channel.
    ///
    /// This method returns:
    ///
    ///  * `Poll::Pending` if no messages are available but the channel is not
    ///    closed, or if a spurious failure happens.
    ///  * `Poll::Ready(Some(message))` if a message is available.
    ///  * `Poll::Ready(None)` if the channel has been closed and all messages
    ///    sent before it was closed have been received.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided
    /// `Context` is scheduled to receive a wakeup when a message is sent on any
    /// receiver, or when the channel is closed.  Note that on multiple calls to
    /// `poll_recv` or `poll_recv_many`, only the `Waker` from the `Context`
    /// passed to the most recent call is scheduled to receive a wakeup.
    ///
    /// If this method returns `Poll::Pending` due to a spurious failure, then
    /// the `Waker` will be notified when the situation causing the spurious
    /// failure has been resolved. Note that receiving such a wakeup does not
    /// guarantee that the next call will succeed — it could fail with another
    /// spurious failure.
    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.chan.recv(cx)
    }

    /// Polls to receive multiple messages on this channel, extending the provided buffer.
    ///
    /// This method returns:
    /// * `Poll::Pending` if no messages are available but the channel is not closed, or if a
    ///   spurious failure happens.
    /// * `Poll::Ready(count)` where `count` is the number of messages successfully received and
    ///   stored in `buffer`. This can be less than, or equal to, `limit`.
    /// * `Poll::Ready(0)` if `limit` is set to zero or when the channel is closed.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided
    /// `Context` is scheduled to receive a wakeup when a message is sent on any
    /// receiver, or when the channel is closed.  Note that on multiple calls to
    /// `poll_recv` or `poll_recv_many`, only the `Waker` from the `Context`
    /// passed to the most recent call is scheduled to receive a wakeup.
    ///
    /// Note that this method does not guarantee that exactly `limit` messages
    /// are received. Rather, if at least one message is available, it returns
    /// as many messages as it can up to the given limit. This method returns
    /// zero only if the channel is closed (or if `limit` is zero).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::task::{Context, Poll};
    /// use std::pin::Pin;
    /// use tokio::sync::mpsc;
    /// use futures::Future;
    ///
    /// struct MyReceiverFuture<'a> {
    ///     receiver: mpsc::UnboundedReceiver<i32>,
    ///     buffer: &'a mut Vec<i32>,
    ///     limit: usize,
    /// }
    ///
    /// impl<'a> Future for MyReceiverFuture<'a> {
    ///     type Output = usize; // Number of messages received
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    ///         let MyReceiverFuture { receiver, buffer, limit } = &mut *self;
    ///
    ///         // Now `receiver` and `buffer` are mutable references, and `limit` is copied
    ///         match receiver.poll_recv_many(cx, *buffer, *limit) {
    ///             Poll::Pending => Poll::Pending,
    ///             Poll::Ready(count) => Poll::Ready(count),
    ///         }
    ///     }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::unbounded_channel::<i32>();
    ///     let mut buffer = Vec::new();
    ///
    ///     let my_receiver_future = MyReceiverFuture {
    ///         receiver: rx,
    ///         buffer: &mut buffer,
    ///         limit: 3,
    ///     };
    ///
    ///     for i in 0..10 {
    ///         tx.send(i).expect("Unable to send integer");
    ///     }
    ///
    ///     let count = my_receiver_future.await;
    ///     assert_eq!(count, 3);
    ///     assert_eq!(buffer, vec![0,1,2])
    /// }
    /// ```
    pub fn poll_recv_many(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &mut Vec<T>,
        limit: usize,
    ) -> Poll<usize> {
        self.chan.recv_many(cx, buffer, limit)
    }

    /// Returns the number of [`UnboundedSender`] handles.
    pub fn sender_strong_count(&self) -> usize {
        self.chan.sender_strong_count()
    }

    /// Returns the number of [`WeakUnboundedSender`] handles.
    pub fn sender_weak_count(&self) -> usize {
        self.chan.sender_weak_count()
    }
}

impl<T> UnboundedSender<T> {
    pub(crate) fn new(chan: chan::Tx<T, Semaphore>) -> UnboundedSender<T> {
        UnboundedSender { chan }
    }

    /// Attempts to send a message on this `UnboundedSender` without blocking.
    ///
    /// This method is not marked async because sending a message to an unbounded channel
    /// never requires any form of waiting. Because of this, the `send` method can be
    /// used in both synchronous and asynchronous code without problems.
    ///
    /// If the receive half of the channel is closed, either due to [`close`]
    /// being called or the [`UnboundedReceiver`] having been dropped, this
    /// function returns an error. The error includes the value passed to `send`.
    ///
    /// [`close`]: UnboundedReceiver::close
    /// [`UnboundedReceiver`]: UnboundedReceiver
    pub fn send(&self, message: T) -> Result<(), SendError<T>> {
        if !self.inc_num_messages() {
            return Err(SendError(message));
        }

        self.chan.send(message);
        Ok(())
    }

    fn inc_num_messages(&self) -> bool {
        use std::process;
        use std::sync::atomic::Ordering::{AcqRel, Acquire};

        let mut curr = self.chan.semaphore().0.load(Acquire);

        loop {
            if curr & 1 == 1 {
                return false;
            }

            if curr == usize::MAX ^ 1 {
                // Overflowed the ref count. There is no safe way to recover, so
                // abort the process. In practice, this should never happen.
                process::abort()
            }

            match self
                .chan
                .semaphore()
                .0
                .compare_exchange(curr, curr + 2, AcqRel, Acquire)
            {
                Ok(_) => return true,
                Err(actual) => {
                    curr = actual;
                }
            }
        }
    }

    /// Completes when the receiver has dropped.
    ///
    /// This allows the producers to get notified when interest in the produced
    /// values is canceled and immediately stop doing work.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once the channel is closed, it stays closed
    /// forever and all future calls to `closed` will return immediately.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx1, rx) = mpsc::unbounded_channel::<()>();
    ///     let tx2 = tx1.clone();
    ///     let tx3 = tx1.clone();
    ///     let tx4 = tx1.clone();
    ///     let tx5 = tx1.clone();
    ///     tokio::spawn(async move {
    ///         drop(rx);
    ///     });
    ///
    ///     futures::join!(
    ///         tx1.closed(),
    ///         tx2.closed(),
    ///         tx3.closed(),
    ///         tx4.closed(),
    ///         tx5.closed()
    ///     );
    ////     println!("Receiver dropped");
    /// }
    /// ```
    pub async fn closed(&self) {
        self.chan.closed().await;
    }

    /// Checks if the channel has been closed. This happens when the
    /// [`UnboundedReceiver`] is dropped, or when the
    /// [`UnboundedReceiver::close`] method is called.
    ///
    /// [`UnboundedReceiver`]: crate::sync::mpsc::UnboundedReceiver
    /// [`UnboundedReceiver::close`]: crate::sync::mpsc::UnboundedReceiver::close
    ///
    /// ```
    /// let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    /// assert!(!tx.is_closed());
    ///
    /// let tx2 = tx.clone();
    /// assert!(!tx2.is_closed());
    ///
    /// drop(rx);
    /// assert!(tx.is_closed());
    /// assert!(tx2.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    /// Returns `true` if senders belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```
    /// let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    /// let  tx2 = tx.clone();
    /// assert!(tx.same_channel(&tx2));
    ///
    /// let (tx3, rx3) = tokio::sync::mpsc::unbounded_channel::<()>();
    /// assert!(!tx3.same_channel(&tx2));
    /// ```
    pub fn same_channel(&self, other: &Self) -> bool {
        self.chan.same_channel(&other.chan)
    }

    /// Converts the `UnboundedSender` to a [`WeakUnboundedSender`] that does not count
    /// towards RAII semantics, i.e. if all `UnboundedSender` instances of the
    /// channel were dropped and only `WeakUnboundedSender` instances remain,
    /// the channel is closed.
    #[must_use = "Downgrade creates a WeakSender without destroying the original non-weak sender."]
    pub fn downgrade(&self) -> WeakUnboundedSender<T> {
        WeakUnboundedSender {
            chan: self.chan.downgrade(),
        }
    }

    /// Returns the number of [`UnboundedSender`] handles.
    pub fn strong_count(&self) -> usize {
        self.chan.strong_count()
    }

    /// Returns the number of [`WeakUnboundedSender`] handles.
    pub fn weak_count(&self) -> usize {
        self.chan.weak_count()
    }
}

impl<T> Clone for WeakUnboundedSender<T> {
    fn clone(&self) -> Self {
        self.chan.increment_weak_count();

        WeakUnboundedSender {
            chan: self.chan.clone(),
        }
    }
}

impl<T> Drop for WeakUnboundedSender<T> {
    fn drop(&mut self) {
        self.chan.decrement_weak_count();
    }
}

impl<T> WeakUnboundedSender<T> {
    /// Tries to convert a `WeakUnboundedSender` into an [`UnboundedSender`].
    /// This will return `Some` if there are other `Sender` instances alive and
    /// the channel wasn't previously dropped, otherwise `None` is returned.
    pub fn upgrade(&self) -> Option<UnboundedSender<T>> {
        chan::Tx::upgrade(self.chan.clone()).map(UnboundedSender::new)
    }

    /// Returns the number of [`UnboundedSender`] handles.
    pub fn strong_count(&self) -> usize {
        self.chan.strong_count()
    }

    /// Returns the number of [`WeakUnboundedSender`] handles.
    pub fn weak_count(&self) -> usize {
        self.chan.weak_count()
    }
}

impl<T> fmt::Debug for WeakUnboundedSender<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("WeakUnboundedSender").finish()
    }
}
