use crate::loom::sync::Arc;
use crate::sync::batch_semaphore::{self as semaphore, TryAcquireError};
use crate::sync::mpsc::chan;
use crate::sync::mpsc::error::{SendError, TryRecvError, TrySendError};

cfg_time! {
    use crate::sync::mpsc::error::SendTimeoutError;
    use crate::time::Duration;
}

use std::fmt;
use std::task::{Context, Poll};

/// Sends values to the associated `Receiver`.
///
/// Instances are created by the [`channel`] function.
///
/// To convert the `Sender` into a `Sink` or use it in a poll function, you can
/// use the [`PollSender`] utility.
///
/// [`PollSender`]: https://docs.rs/tokio-util/latest/tokio_util/sync/struct.PollSender.html
pub struct Sender<T> {
    chan: chan::Tx<T, Semaphore>,
}

/// A sender that does not prevent the channel from being closed.
///
/// If all [`Sender`] instances of a channel were dropped and only `WeakSender`
/// instances remain, the channel is closed.
///
/// In order to send messages, the `WeakSender` needs to be upgraded using
/// [`WeakSender::upgrade`], which returns `Option<Sender>`. It returns `None`
/// if all `Sender`s have been dropped, and otherwise it returns a `Sender`.
///
/// [`Sender`]: Sender
/// [`WeakSender::upgrade`]: WeakSender::upgrade
///
/// # Examples
///
/// ```
/// use tokio::sync::mpsc::channel;
///
/// #[tokio::main]
/// async fn main() {
///     let (tx, _rx) = channel::<i32>(15);
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
pub struct WeakSender<T> {
    chan: Arc<chan::Chan<T, Semaphore>>,
}

/// Permits to send one value into the channel.
///
/// `Permit` values are returned by [`Sender::reserve()`] and [`Sender::try_reserve()`]
/// and are used to guarantee channel capacity before generating a message to send.
///
/// [`Sender::reserve()`]: Sender::reserve
/// [`Sender::try_reserve()`]: Sender::try_reserve
pub struct Permit<'a, T> {
    chan: &'a chan::Tx<T, Semaphore>,
}

/// An [`Iterator`] of [`Permit`] that can be used to hold `n` slots in the channel.
///
/// `PermitIterator` values are returned by [`Sender::reserve_many()`] and [`Sender::try_reserve_many()`]
/// and are used to guarantee channel capacity before generating `n` messages to send.
///
/// [`Sender::reserve_many()`]: Sender::reserve_many
/// [`Sender::try_reserve_many()`]: Sender::try_reserve_many
pub struct PermitIterator<'a, T> {
    chan: &'a chan::Tx<T, Semaphore>,
    n: usize,
}

/// Owned permit to send one value into the channel.
///
/// This is identical to the [`Permit`] type, except that it moves the sender
/// rather than borrowing it.
///
/// `OwnedPermit` values are returned by [`Sender::reserve_owned()`] and
/// [`Sender::try_reserve_owned()`] and are used to guarantee channel capacity
/// before generating a message to send.
///
/// [`Permit`]: Permit
/// [`Sender::reserve_owned()`]: Sender::reserve_owned
/// [`Sender::try_reserve_owned()`]: Sender::try_reserve_owned
pub struct OwnedPermit<T> {
    chan: Option<chan::Tx<T, Semaphore>>,
}

/// Receives values from the associated `Sender`.
///
/// Instances are created by the [`channel`] function.
///
/// This receiver can be turned into a `Stream` using [`ReceiverStream`].
///
/// [`ReceiverStream`]: https://docs.rs/tokio-stream/0.1/tokio_stream/wrappers/struct.ReceiverStream.html
pub struct Receiver<T> {
    /// The channel receiver.
    chan: chan::Rx<T, Semaphore>,
}

/// Creates a bounded mpsc channel for communicating between asynchronous tasks
/// with backpressure.
///
/// The channel will buffer up to the provided number of messages.  Once the
/// buffer is full, attempts to send new messages will wait until a message is
/// received from the channel. The provided buffer capacity must be at least 1.
///
/// All data sent on `Sender` will become available on `Receiver` in the same
/// order as it was sent.
///
/// The `Sender` can be cloned to `send` to the same channel from multiple code
/// locations. Only one `Receiver` is supported.
///
/// If the `Receiver` is disconnected while trying to `send`, the `send` method
/// will return a `SendError`. Similarly, if `Sender` is disconnected while
/// trying to `recv`, the `recv` method will return `None`.
///
/// # Panics
///
/// Panics if the buffer capacity is 0.
///
/// # Examples
///
/// ```rust
/// use tokio::sync::mpsc;
///
/// #[tokio::main]
/// async fn main() {
///     let (tx, mut rx) = mpsc::channel(100);
///
///     tokio::spawn(async move {
///         for i in 0..10 {
///             if let Err(_) = tx.send(i).await {
///                 println!("receiver dropped");
///                 return;
///             }
///         }
///     });
///
///     while let Some(i) = rx.recv().await {
///         println!("got = {}", i);
///     }
/// }
/// ```
#[track_caller]
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    assert!(buffer > 0, "mpsc bounded channel requires buffer > 0");
    let semaphore = Semaphore {
        semaphore: semaphore::Semaphore::new(buffer),
        bound: buffer,
    };
    let (tx, rx) = chan::channel(semaphore);

    let tx = Sender::new(tx);
    let rx = Receiver::new(rx);

    (tx, rx)
}

/// Channel semaphore is a tuple of the semaphore implementation and a `usize`
/// representing the channel bound.
#[derive(Debug)]
pub(crate) struct Semaphore {
    pub(crate) semaphore: semaphore::Semaphore,
    pub(crate) bound: usize,
}

impl<T> Receiver<T> {
    pub(crate) fn new(chan: chan::Rx<T, Semaphore>) -> Receiver<T> {
        Receiver { chan }
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
    /// the channel is closed.  Note that if [`close`] is called, but there are
    /// still outstanding [`Permits`] from before it was closed, the channel is
    /// not considered closed by `recv` until the permits are released.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `recv` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, it is guaranteed that no messages were received on this
    /// channel.
    ///
    /// [`close`]: Self::close
    /// [`Permits`]: struct@crate::sync::mpsc::Permit
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(100);
    ///
    ///     tokio::spawn(async move {
    ///         tx.send("hello").await.unwrap();
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
    ///     let (tx, mut rx) = mpsc::channel(100);
    ///
    ///     tx.send("hello").await.unwrap();
    ///     tx.send("world").await.unwrap();
    ///
    ///     assert_eq!(Some("hello"), rx.recv().await);
    ///     assert_eq!(Some("world"), rx.recv().await);
    /// }
    /// ```
    pub async fn recv(&mut self) -> Option<T> {
        use std::future::poll_fn;
        poll_fn(|cx| self.chan.recv(cx)).await
    }

    /// Receives the next values for this receiver and extends `buffer`.
    ///
    /// This method extends `buffer` by no more than a fixed number of values
    /// as specified by `limit`. If `limit` is zero, the function immediately
    /// returns `0`. The return value is the number of values added to `buffer`.
    ///
    /// For `limit > 0`, if there are no messages in the channel's queue, but
    /// the channel has not yet been closed, this method will sleep until a
    /// message is sent or the channel is closed. Note that if [`close`] is
    /// called, but there are still outstanding [`Permits`] from before it was
    /// closed, the channel is not considered closed by `recv_many` until the
    /// permits are released.
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
    /// [`Permits`]: struct@crate::sync::mpsc::Permit
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
    ///     let (tx, mut rx) = mpsc::channel(100);
    ///     let tx2 = tx.clone();
    ///     tx2.send("first").await.unwrap();
    ///     tx2.send("second").await.unwrap();
    ///     tx2.send("third").await.unwrap();
    ///
    ///     // Call `recv_many` to receive up to `limit` (2) values.
    ///     assert_eq!(2, rx.recv_many(&mut buffer, limit).await);
    ///     assert_eq!(vec!["first", "second"], buffer);
    ///
    ///     // If the buffer is full, the next call to `recv_many`
    ///     // reserves additional capacity.
    ///     assert_eq!(1, rx.recv_many(&mut buffer, 1).await);
    ///
    ///     tokio::spawn(async move {
    ///         tx.send("fourth").await.unwrap();
    ///     });
    ///
    ///     // 'tx' is dropped, but `recv_many`
    ///     // is guaranteed not to return 0 as the channel
    ///     // is not yet closed.
    ///     assert_eq!(1, rx.recv_many(&mut buffer, 1).await);
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
    ///     let (tx, mut rx) = mpsc::channel(100);
    ///
    ///     tx.send("hello").await.unwrap();
    ///
    ///     assert_eq!(Ok("hello"), rx.try_recv());
    ///     assert_eq!(Err(TryRecvError::Empty), rx.try_recv());
    ///
    ///     tx.send("hello").await.unwrap();
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
    /// This method returns `None` if the channel has been closed and there are
    /// no remaining messages in the channel's buffer. This indicates that no
    /// further values can ever be received from this `Receiver`. The channel is
    /// closed when all senders have been dropped, or when [`close`] is called.
    ///
    /// If there are no messages in the channel's buffer, but the channel has
    /// not yet been closed, this method will block until a message is sent or
    /// the channel is closed.
    ///
    /// This method is intended for use cases where you are sending from
    /// asynchronous code to synchronous code, and will work even if the sender
    /// is not using [`blocking_send`] to send the message.
    ///
    /// Note that if [`close`] is called, but there are still outstanding
    /// [`Permits`] from before it was closed, the channel is not considered
    /// closed by `blocking_recv` until the permits are released.
    ///
    /// [`close`]: Self::close
    /// [`Permits`]: struct@crate::sync::mpsc::Permit
    /// [`blocking_send`]: fn@crate::sync::mpsc::Sender::blocking_send
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
    /// use tokio::runtime::Runtime;
    /// use tokio::sync::mpsc;
    ///
    /// fn main() {
    ///     let (tx, mut rx) = mpsc::channel::<u8>(10);
    ///
    ///     let sync_code = thread::spawn(move || {
    ///         assert_eq!(Some(10), rx.blocking_recv());
    ///     });
    ///
    ///     Runtime::new()
    ///         .unwrap()
    ///         .block_on(async move {
    ///             let _ = tx.send(10).await;
    ///         });
    ///     sync_code.join().unwrap()
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

    /// Closes the receiving half of a channel without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered. Any
    /// outstanding [`Permit`] values will still be able to send messages.
    ///
    /// To guarantee that no messages are dropped, after calling `close()`,
    /// `recv()` must be called until `None` is returned. If there are
    /// outstanding [`Permit`] or [`OwnedPermit`] values, the `recv` method will
    /// not return `None` until those are released.
    ///
    /// [`Permit`]: Permit
    /// [`OwnedPermit`]: OwnedPermit
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(20);
    ///
    ///     tokio::spawn(async move {
    ///         let mut i = 0;
    ///         while let Ok(permit) = tx.reserve().await {
    ///             permit.send(i);
    ///             i += 1;
    ///         }
    ///     });
    ///
    ///     rx.close();
    ///
    ///     while let Some(msg) = rx.recv().await {
    ///         println!("got {}", msg);
    ///     }
    ///
    ///     // Channel closed and no messages are lost.
    /// }
    /// ```
    pub fn close(&mut self) {
        self.chan.close();
    }

    /// Checks if a channel is closed.
    ///
    /// This method returns `true` if the channel has been closed. The channel is closed
    /// when all [`Sender`] have been dropped, or when [`Receiver::close`] is called.
    ///
    /// [`Sender`]: crate::sync::mpsc::Sender
    /// [`Receiver::close`]: crate::sync::mpsc::Receiver::close
    ///
    /// # Examples
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (_tx, mut rx) = mpsc::channel::<()>(10);
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
    ///     let (tx, rx) = mpsc::channel(10);
    ///     assert!(rx.is_empty());
    ///
    ///     tx.send(0).await.unwrap();
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
    ///     let (tx, rx) = mpsc::channel(10);
    ///     assert_eq!(0, rx.len());
    ///
    ///     tx.send(0).await.unwrap();
    ///     assert_eq!(1, rx.len());
    /// }
    /// ```
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    /// Returns the current capacity of the channel.
    ///
    /// The capacity goes down when the sender sends a value by calling [`Sender::send`] or by reserving
    /// capacity with [`Sender::reserve`]. The capacity goes up when values are received.
    /// This is distinct from [`max_capacity`], which always returns buffer capacity initially
    /// specified when calling [`channel`].
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel::<()>(5);
    ///
    ///     assert_eq!(rx.capacity(), 5);
    ///
    ///     // Making a reservation drops the capacity by one.
    ///     let permit = tx.reserve().await.unwrap();
    ///     assert_eq!(rx.capacity(), 4);
    ///     assert_eq!(rx.len(), 0);
    ///
    ///     // Sending and receiving a value increases the capacity by one.
    ///     permit.send(());
    ///     assert_eq!(rx.len(), 1);
    ///     rx.recv().await.unwrap();
    ///     assert_eq!(rx.capacity(), 5);
    ///
    ///     // Directly sending a message drops the capacity by one.
    ///     tx.send(()).await.unwrap();
    ///     assert_eq!(rx.capacity(), 4);
    ///     assert_eq!(rx.len(), 1);
    ///
    ///     // Receiving the message increases the capacity by one.
    ///     rx.recv().await.unwrap();
    ///     assert_eq!(rx.capacity(), 5);
    ///     assert_eq!(rx.len(), 0);
    /// }
    /// ```
    /// [`capacity`]: Receiver::capacity
    /// [`max_capacity`]: Receiver::max_capacity
    pub fn capacity(&self) -> usize {
        self.chan.semaphore().semaphore.available_permits()
    }

    /// Returns the maximum buffer capacity of the channel.
    ///
    /// The maximum capacity is the buffer capacity initially specified when calling
    /// [`channel`]. This is distinct from [`capacity`], which returns the *current*
    /// available buffer capacity: as messages are sent and received, the value
    /// returned by [`capacity`] will go up or down, whereas the value
    /// returned by [`max_capacity`] will remain constant.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::channel::<()>(5);
    ///
    ///     // both max capacity and capacity are the same at first
    ///     assert_eq!(rx.max_capacity(), 5);
    ///     assert_eq!(rx.capacity(), 5);
    ///
    ///     // Making a reservation doesn't change the max capacity.
    ///     let permit = tx.reserve().await.unwrap();
    ///     assert_eq!(rx.max_capacity(), 5);
    ///     // but drops the capacity by one
    ///     assert_eq!(rx.capacity(), 4);
    /// }
    /// ```
    /// [`capacity`]: Receiver::capacity
    /// [`max_capacity`]: Receiver::max_capacity
    pub fn max_capacity(&self) -> usize {
        self.chan.semaphore().bound
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
    ///     receiver: mpsc::Receiver<i32>,
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
    ///     let (tx, rx) = mpsc::channel(32);
    ///     let mut buffer = Vec::new();
    ///
    ///     let my_receiver_future = MyReceiverFuture {
    ///         receiver: rx,
    ///         buffer: &mut buffer,
    ///         limit: 3,
    ///     };
    ///
    ///     for i in 0..10 {
    ///         tx.send(i).await.unwrap();
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

    /// Returns the number of [`Sender`] handles.
    pub fn sender_strong_count(&self) -> usize {
        self.chan.sender_strong_count()
    }

    /// Returns the number of [`WeakSender`] handles.
    pub fn sender_weak_count(&self) -> usize {
        self.chan.sender_weak_count()
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Receiver")
            .field("chan", &self.chan)
            .finish()
    }
}

impl<T> Unpin for Receiver<T> {}

impl<T> Sender<T> {
    pub(crate) fn new(chan: chan::Tx<T, Semaphore>) -> Sender<T> {
        Sender { chan }
    }

    /// Sends a value, waiting until there is capacity.
    ///
    /// A successful send occurs when it is determined that the other end of the
    /// channel has not hung up already. An unsuccessful send would be one where
    /// the corresponding receiver has already been closed. Note that a return
    /// value of `Err` means that the data will never be received, but a return
    /// value of `Ok` does not mean that the data will be received. It is
    /// possible for the corresponding receiver to hang up immediately after
    /// this function returns `Ok`.
    ///
    /// # Errors
    ///
    /// If the receive half of the channel is closed, either due to [`close`]
    /// being called or the [`Receiver`] handle dropping, the function returns
    /// an error. The error includes the value passed to `send`.
    ///
    /// [`close`]: Receiver::close
    /// [`Receiver`]: Receiver
    ///
    /// # Cancel safety
    ///
    /// If `send` is used as the event in a [`tokio::select!`](crate::select)
    /// statement and some other branch completes first, then it is guaranteed
    /// that the message was not sent. **However, in that case, the message
    /// is dropped and will be lost.**
    ///
    /// To avoid losing messages, use [`reserve`](Self::reserve) to reserve
    /// capacity, then use the returned [`Permit`] to send the message.
    ///
    /// This channel uses a queue to ensure that calls to `send` and `reserve`
    /// complete in the order they were requested.  Cancelling a call to
    /// `send` makes you lose your place in the queue.
    ///
    /// # Examples
    ///
    /// In the following example, each call to `send` will block until the
    /// previously sent value was received.
    ///
    /// ```rust
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     tokio::spawn(async move {
    ///         for i in 0..10 {
    ///             if let Err(_) = tx.send(i).await {
    ///                 println!("receiver dropped");
    ///                 return;
    ///             }
    ///         }
    ///     });
    ///
    ///     while let Some(i) = rx.recv().await {
    ///         println!("got = {}", i);
    ///     }
    /// }
    /// ```
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        match self.reserve().await {
            Ok(permit) => {
                permit.send(value);
                Ok(())
            }
            Err(_) => Err(SendError(value)),
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
    ///     let (tx1, rx) = mpsc::channel::<()>(1);
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
    ///     println!("Receiver dropped");
    /// }
    /// ```
    pub async fn closed(&self) {
        self.chan.closed().await;
    }

    /// Attempts to immediately send a message on this `Sender`
    ///
    /// This method differs from [`send`] by returning immediately if the channel's
    /// buffer is full or no receiver is waiting to acquire some data. Compared
    /// with [`send`], this function has two failure cases instead of one (one for
    /// disconnection, one for a full buffer).
    ///
    /// # Errors
    ///
    /// If the channel capacity has been reached, i.e., the channel has `n`
    /// buffered values where `n` is the argument passed to [`channel`], then an
    /// error is returned.
    ///
    /// If the receive half of the channel is closed, either due to [`close`]
    /// being called or the [`Receiver`] handle dropping, the function returns
    /// an error. The error includes the value passed to `send`.
    ///
    /// [`send`]: Sender::send
    /// [`channel`]: channel
    /// [`close`]: Receiver::close
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Create a channel with buffer size 1
    ///     let (tx1, mut rx) = mpsc::channel(1);
    ///     let tx2 = tx1.clone();
    ///
    ///     tokio::spawn(async move {
    ///         tx1.send(1).await.unwrap();
    ///         tx1.send(2).await.unwrap();
    ///         // task waits until the receiver receives a value.
    ///     });
    ///
    ///     tokio::spawn(async move {
    ///         // This will return an error and send
    ///         // no message if the buffer is full
    ///         let _ = tx2.try_send(3);
    ///     });
    ///
    ///     let mut msg;
    ///     msg = rx.recv().await.unwrap();
    ///     println!("message {} received", msg);
    ///
    ///     msg = rx.recv().await.unwrap();
    ///     println!("message {} received", msg);
    ///
    ///     // Third message may have never been sent
    ///     match rx.recv().await {
    ///         Some(msg) => println!("message {} received", msg),
    ///         None => println!("the third message was never sent"),
    ///     }
    /// }
    /// ```
    pub fn try_send(&self, message: T) -> Result<(), TrySendError<T>> {
        match self.chan.semaphore().semaphore.try_acquire(1) {
            Ok(()) => {}
            Err(TryAcquireError::Closed) => return Err(TrySendError::Closed(message)),
            Err(TryAcquireError::NoPermits) => return Err(TrySendError::Full(message)),
        }

        // Send the message
        self.chan.send(message);
        Ok(())
    }

    /// Sends a value, waiting until there is capacity, but only for a limited time.
    ///
    /// Shares the same success and error conditions as [`send`], adding one more
    /// condition for an unsuccessful send, which is when the provided timeout has
    /// elapsed, and there is no capacity available.
    ///
    /// [`send`]: Sender::send
    ///
    /// # Errors
    ///
    /// If the receive half of the channel is closed, either due to [`close`]
    /// being called or the [`Receiver`] having been dropped,
    /// the function returns an error. The error includes the value passed to `send`.
    ///
    /// [`close`]: Receiver::close
    /// [`Receiver`]: Receiver
    ///
    /// # Panics
    ///
    /// This function panics if it is called outside the context of a Tokio
    /// runtime [with time enabled](crate::runtime::Builder::enable_time).
    ///
    /// # Examples
    ///
    /// In the following example, each call to `send_timeout` will block until the
    /// previously sent value was received, unless the timeout has elapsed.
    ///
    /// ```rust
    /// use tokio::sync::mpsc;
    /// use tokio::time::{sleep, Duration};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     tokio::spawn(async move {
    ///         for i in 0..10 {
    ///             if let Err(e) = tx.send_timeout(i, Duration::from_millis(100)).await {
    ///                 println!("send error: #{:?}", e);
    ///                 return;
    ///             }
    ///         }
    ///     });
    ///
    ///     while let Some(i) = rx.recv().await {
    ///         println!("got = {}", i);
    ///         sleep(Duration::from_millis(200)).await;
    ///     }
    /// }
    /// ```
    #[cfg(feature = "time")]
    #[cfg_attr(docsrs, doc(cfg(feature = "time")))]
    pub async fn send_timeout(
        &self,
        value: T,
        timeout: Duration,
    ) -> Result<(), SendTimeoutError<T>> {
        let permit = match crate::time::timeout(timeout, self.reserve()).await {
            Err(_) => {
                return Err(SendTimeoutError::Timeout(value));
            }
            Ok(Err(_)) => {
                return Err(SendTimeoutError::Closed(value));
            }
            Ok(Ok(permit)) => permit,
        };

        permit.send(value);
        Ok(())
    }

    /// Blocking send to call outside of asynchronous contexts.
    ///
    /// This method is intended for use cases where you are sending from
    /// synchronous code to asynchronous code, and will work even if the
    /// receiver is not using [`blocking_recv`] to receive the message.
    ///
    /// [`blocking_recv`]: fn@crate::sync::mpsc::Receiver::blocking_recv
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
    /// use tokio::runtime::Runtime;
    /// use tokio::sync::mpsc;
    ///
    /// fn main() {
    ///     let (tx, mut rx) = mpsc::channel::<u8>(1);
    ///
    ///     let sync_code = thread::spawn(move || {
    ///         tx.blocking_send(10).unwrap();
    ///     });
    ///
    ///     Runtime::new().unwrap().block_on(async move {
    ///         assert_eq!(Some(10), rx.recv().await);
    ///     });
    ///     sync_code.join().unwrap()
    /// }
    /// ```
    #[track_caller]
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(alias = "send_blocking"))]
    pub fn blocking_send(&self, value: T) -> Result<(), SendError<T>> {
        crate::future::block_on(self.send(value))
    }

    /// Checks if the channel has been closed. This happens when the
    /// [`Receiver`] is dropped, or when the [`Receiver::close`] method is
    /// called.
    ///
    /// [`Receiver`]: crate::sync::mpsc::Receiver
    /// [`Receiver::close`]: crate::sync::mpsc::Receiver::close
    ///
    /// ```
    /// let (tx, rx) = tokio::sync::mpsc::channel::<()>(42);
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

    /// Waits for channel capacity. Once capacity to send one message is
    /// available, it is reserved for the caller.
    ///
    /// If the channel is full, the function waits for the number of unreceived
    /// messages to become less than the channel capacity. Capacity to send one
    /// message is reserved for the caller. A [`Permit`] is returned to track
    /// the reserved capacity. The [`send`] function on [`Permit`] consumes the
    /// reserved capacity.
    ///
    /// Dropping [`Permit`] without sending a message releases the capacity back
    /// to the channel.
    ///
    /// [`Permit`]: Permit
    /// [`send`]: Permit::send
    ///
    /// # Cancel safety
    ///
    /// This channel uses a queue to ensure that calls to `send` and `reserve`
    /// complete in the order they were requested.  Cancelling a call to
    /// `reserve` makes you lose your place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Reserve capacity
    ///     let permit = tx.reserve().await.unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Sending on the permit succeeds
    ///     permit.send(456);
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    /// }
    /// ```
    pub async fn reserve(&self) -> Result<Permit<'_, T>, SendError<()>> {
        self.reserve_inner(1).await?;
        Ok(Permit { chan: &self.chan })
    }

    /// Waits for channel capacity. Once capacity to send `n` messages is
    /// available, it is reserved for the caller.
    ///
    /// If the channel is full or if there are fewer than `n` permits available, the function waits
    /// for the number of unreceived messages to become `n` less than the channel capacity.
    /// Capacity to send `n` message is then reserved for the caller.
    ///
    /// A [`PermitIterator`] is returned to track the reserved capacity.
    /// You can call this [`Iterator`] until it is exhausted to
    /// get a [`Permit`] and then call [`Permit::send`]. This function is similar to
    /// [`try_reserve_many`] except it awaits for the slots to become available.
    ///
    /// If the channel is closed, the function returns a [`SendError`].
    ///
    /// Dropping [`PermitIterator`] without consuming it entirely releases the remaining
    /// permits back to the channel.
    ///
    /// [`PermitIterator`]: PermitIterator
    /// [`Permit`]: Permit
    /// [`send`]: Permit::send
    /// [`try_reserve_many`]: Sender::try_reserve_many
    ///
    /// # Cancel safety
    ///
    /// This channel uses a queue to ensure that calls to `send` and `reserve_many`
    /// complete in the order they were requested. Cancelling a call to
    /// `reserve_many` makes you lose your place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(2);
    ///
    ///     // Reserve capacity
    ///     let mut permit = tx.reserve_many(2).await.unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Sending with the permit iterator succeeds
    ///     permit.next().unwrap().send(456);
    ///     permit.next().unwrap().send(457);
    ///
    ///     // The iterator should now be exhausted
    ///     assert!(permit.next().is_none());
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    ///     assert_eq!(rx.recv().await.unwrap(), 457);
    /// }
    /// ```
    pub async fn reserve_many(&self, n: usize) -> Result<PermitIterator<'_, T>, SendError<()>> {
        self.reserve_inner(n).await?;
        Ok(PermitIterator {
            chan: &self.chan,
            n,
        })
    }

    /// Waits for channel capacity, moving the `Sender` and returning an owned
    /// permit. Once capacity to send one message is available, it is reserved
    /// for the caller.
    ///
    /// This moves the sender _by value_, and returns an owned permit that can
    /// be used to send a message into the channel. Unlike [`Sender::reserve`],
    /// this method may be used in cases where the permit must be valid for the
    /// `'static` lifetime. `Sender`s may be cloned cheaply (`Sender::clone` is
    /// essentially a reference count increment, comparable to [`Arc::clone`]),
    /// so when multiple [`OwnedPermit`]s are needed or the `Sender` cannot be
    /// moved, it can be cloned prior to calling `reserve_owned`.
    ///
    /// If the channel is full, the function waits for the number of unreceived
    /// messages to become less than the channel capacity. Capacity to send one
    /// message is reserved for the caller. An [`OwnedPermit`] is returned to
    /// track the reserved capacity. The [`send`] function on [`OwnedPermit`]
    /// consumes the reserved capacity.
    ///
    /// Dropping the [`OwnedPermit`] without sending a message releases the
    /// capacity back to the channel.
    ///
    /// # Cancel safety
    ///
    /// This channel uses a queue to ensure that calls to `send` and `reserve`
    /// complete in the order they were requested.  Cancelling a call to
    /// `reserve_owned` makes you lose your place in the queue.
    ///
    /// # Examples
    /// Sending a message using an [`OwnedPermit`]:
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Reserve capacity, moving the sender.
    ///     let permit = tx.reserve_owned().await.unwrap();
    ///
    ///     // Send a message, consuming the permit and returning
    ///     // the moved sender.
    ///     let tx = permit.send(123);
    ///
    ///     // The value sent on the permit is received.
    ///     assert_eq!(rx.recv().await.unwrap(), 123);
    ///
    ///     // The sender can now be used again.
    ///     tx.send(456).await.unwrap();
    /// }
    /// ```
    ///
    /// When multiple [`OwnedPermit`]s are needed, or the sender cannot be moved
    /// by value, it can be inexpensively cloned before calling `reserve_owned`:
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Clone the sender and reserve capacity.
    ///     let permit = tx.clone().reserve_owned().await.unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Sending on the permit succeeds.
    ///     permit.send(456);
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    /// }
    /// ```
    ///
    /// [`Sender::reserve`]: Sender::reserve
    /// [`OwnedPermit`]: OwnedPermit
    /// [`send`]: OwnedPermit::send
    /// [`Arc::clone`]: std::sync::Arc::clone
    pub async fn reserve_owned(self) -> Result<OwnedPermit<T>, SendError<()>> {
        self.reserve_inner(1).await?;
        Ok(OwnedPermit {
            chan: Some(self.chan),
        })
    }

    async fn reserve_inner(&self, n: usize) -> Result<(), SendError<()>> {
        crate::trace::async_trace_leaf().await;

        if n > self.max_capacity() {
            return Err(SendError(()));
        }
        match self.chan.semaphore().semaphore.acquire(n).await {
            Ok(()) => Ok(()),
            Err(_) => Err(SendError(())),
        }
    }

    /// Tries to acquire a slot in the channel without waiting for the slot to become
    /// available.
    ///
    /// If the channel is full this function will return [`TrySendError`], otherwise
    /// if there is a slot available it will return a [`Permit`] that will then allow you
    /// to [`send`] on the channel with a guaranteed slot. This function is similar to
    /// [`reserve`] except it does not await for the slot to become available.
    ///
    /// Dropping [`Permit`] without sending a message releases the capacity back
    /// to the channel.
    ///
    /// [`Permit`]: Permit
    /// [`send`]: Permit::send
    /// [`reserve`]: Sender::reserve
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Reserve capacity
    ///     let permit = tx.try_reserve().unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Trying to reserve an additional slot on the `tx` will
    ///     // fail because there is no capacity.
    ///     assert!(tx.try_reserve().is_err());
    ///
    ///     // Sending on the permit succeeds
    ///     permit.send(456);
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    ///
    /// }
    /// ```
    pub fn try_reserve(&self) -> Result<Permit<'_, T>, TrySendError<()>> {
        match self.chan.semaphore().semaphore.try_acquire(1) {
            Ok(()) => {}
            Err(TryAcquireError::Closed) => return Err(TrySendError::Closed(())),
            Err(TryAcquireError::NoPermits) => return Err(TrySendError::Full(())),
        }

        Ok(Permit { chan: &self.chan })
    }

    /// Tries to acquire `n` slots in the channel without waiting for the slot to become
    /// available.
    ///
    /// A [`PermitIterator`] is returned to track the reserved capacity.
    /// You can call this [`Iterator`] until it is exhausted to
    /// get a [`Permit`] and then call [`Permit::send`]. This function is similar to
    /// [`reserve_many`] except it does not await for the slots to become available.
    ///
    /// If there are fewer than `n` permits available on the channel, then
    /// this function will return a [`TrySendError::Full`]. If the channel is closed
    /// this function will return a [`TrySendError::Closed`].
    ///
    /// Dropping [`PermitIterator`] without consuming it entirely releases the remaining
    /// permits back to the channel.
    ///
    /// [`PermitIterator`]: PermitIterator
    /// [`send`]: Permit::send
    /// [`reserve_many`]: Sender::reserve_many
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(2);
    ///
    ///     // Reserve capacity
    ///     let mut permit = tx.try_reserve_many(2).unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Trying to reserve an additional slot on the `tx` will
    ///     // fail because there is no capacity.
    ///     assert!(tx.try_reserve().is_err());
    ///
    ///     // Sending with the permit iterator succeeds
    ///     permit.next().unwrap().send(456);
    ///     permit.next().unwrap().send(457);
    ///
    ///     // The iterator should now be exhausted
    ///     assert!(permit.next().is_none());
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    ///     assert_eq!(rx.recv().await.unwrap(), 457);
    ///
    ///     // Trying to call try_reserve_many with 0 will return an empty iterator
    ///     let mut permit = tx.try_reserve_many(0).unwrap();
    ///     assert!(permit.next().is_none());
    ///
    ///     // Trying to call try_reserve_many with a number greater than the channel
    ///     // capacity will return an error
    ///     let permit = tx.try_reserve_many(3);
    ///     assert!(permit.is_err());
    ///
    ///     // Trying to call try_reserve_many on a closed channel will return an error
    ///     drop(rx);
    ///     let permit = tx.try_reserve_many(1);
    ///     assert!(permit.is_err());
    ///
    ///     let permit = tx.try_reserve_many(0);
    ///     assert!(permit.is_err());
    /// }
    /// ```
    pub fn try_reserve_many(&self, n: usize) -> Result<PermitIterator<'_, T>, TrySendError<()>> {
        if n > self.max_capacity() {
            return Err(TrySendError::Full(()));
        }

        match self.chan.semaphore().semaphore.try_acquire(n) {
            Ok(()) => {}
            Err(TryAcquireError::Closed) => return Err(TrySendError::Closed(())),
            Err(TryAcquireError::NoPermits) => return Err(TrySendError::Full(())),
        }

        Ok(PermitIterator {
            chan: &self.chan,
            n,
        })
    }

    /// Tries to acquire a slot in the channel without waiting for the slot to become
    /// available, returning an owned permit.
    ///
    /// This moves the sender _by value_, and returns an owned permit that can
    /// be used to send a message into the channel. Unlike [`Sender::try_reserve`],
    /// this method may be used in cases where the permit must be valid for the
    /// `'static` lifetime.  `Sender`s may be cloned cheaply (`Sender::clone` is
    /// essentially a reference count increment, comparable to [`Arc::clone`]),
    /// so when multiple [`OwnedPermit`]s are needed or the `Sender` cannot be
    /// moved, it can be cloned prior to calling `try_reserve_owned`.
    ///
    /// If the channel is full this function will return a [`TrySendError`].
    /// Since the sender is taken by value, the `TrySendError` returned in this
    /// case contains the sender, so that it may be used again. Otherwise, if
    /// there is a slot available, this method will return an [`OwnedPermit`]
    /// that can then be used to [`send`] on the channel with a guaranteed slot.
    /// This function is similar to  [`reserve_owned`] except it does not await
    /// for the slot to become available.
    ///
    /// Dropping the [`OwnedPermit`] without sending a message releases the capacity back
    /// to the channel.
    ///
    /// [`OwnedPermit`]: OwnedPermit
    /// [`send`]: OwnedPermit::send
    /// [`reserve_owned`]: Sender::reserve_owned
    /// [`Arc::clone`]: std::sync::Arc::clone
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Reserve capacity
    ///     let permit = tx.clone().try_reserve_owned().unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Trying to reserve an additional slot on the `tx` will
    ///     // fail because there is no capacity.
    ///     assert!(tx.try_reserve().is_err());
    ///
    ///     // Sending on the permit succeeds
    ///     permit.send(456);
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    ///
    /// }
    /// ```
    pub fn try_reserve_owned(self) -> Result<OwnedPermit<T>, TrySendError<Self>> {
        match self.chan.semaphore().semaphore.try_acquire(1) {
            Ok(()) => {}
            Err(TryAcquireError::Closed) => return Err(TrySendError::Closed(self)),
            Err(TryAcquireError::NoPermits) => return Err(TrySendError::Full(self)),
        }

        Ok(OwnedPermit {
            chan: Some(self.chan),
        })
    }

    /// Returns `true` if senders belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```
    /// let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);
    /// let  tx2 = tx.clone();
    /// assert!(tx.same_channel(&tx2));
    ///
    /// let (tx3, rx3) = tokio::sync::mpsc::channel::<()>(1);
    /// assert!(!tx3.same_channel(&tx2));
    /// ```
    pub fn same_channel(&self, other: &Self) -> bool {
        self.chan.same_channel(&other.chan)
    }

    /// Returns the current capacity of the channel.
    ///
    /// The capacity goes down when sending a value by calling [`send`] or by reserving capacity
    /// with [`reserve`]. The capacity goes up when values are received by the [`Receiver`].
    /// This is distinct from [`max_capacity`], which always returns buffer capacity initially
    /// specified when calling [`channel`]
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel::<()>(5);
    ///
    ///     assert_eq!(tx.capacity(), 5);
    ///
    ///     // Making a reservation drops the capacity by one.
    ///     let permit = tx.reserve().await.unwrap();
    ///     assert_eq!(tx.capacity(), 4);
    ///
    ///     // Sending and receiving a value increases the capacity by one.
    ///     permit.send(());
    ///     rx.recv().await.unwrap();
    ///     assert_eq!(tx.capacity(), 5);
    /// }
    /// ```
    ///
    /// [`send`]: Sender::send
    /// [`reserve`]: Sender::reserve
    /// [`channel`]: channel
    /// [`max_capacity`]: Sender::max_capacity
    pub fn capacity(&self) -> usize {
        self.chan.semaphore().semaphore.available_permits()
    }

    /// Converts the `Sender` to a [`WeakSender`] that does not count
    /// towards RAII semantics, i.e. if all `Sender` instances of the
    /// channel were dropped and only `WeakSender` instances remain,
    /// the channel is closed.
    #[must_use = "Downgrade creates a WeakSender without destroying the original non-weak sender."]
    pub fn downgrade(&self) -> WeakSender<T> {
        WeakSender {
            chan: self.chan.downgrade(),
        }
    }

    /// Returns the maximum buffer capacity of the channel.
    ///
    /// The maximum capacity is the buffer capacity initially specified when calling
    /// [`channel`]. This is distinct from [`capacity`], which returns the *current*
    /// available buffer capacity: as messages are sent and received, the
    /// value returned by [`capacity`] will go up or down, whereas the value
    /// returned by [`max_capacity`] will remain constant.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, _rx) = mpsc::channel::<()>(5);
    ///
    ///     // both max capacity and capacity are the same at first
    ///     assert_eq!(tx.max_capacity(), 5);
    ///     assert_eq!(tx.capacity(), 5);
    ///
    ///     // Making a reservation doesn't change the max capacity.
    ///     let permit = tx.reserve().await.unwrap();
    ///     assert_eq!(tx.max_capacity(), 5);
    ///     // but drops the capacity by one
    ///     assert_eq!(tx.capacity(), 4);
    /// }
    /// ```
    ///
    /// [`channel`]: channel
    /// [`max_capacity`]: Sender::max_capacity
    /// [`capacity`]: Sender::capacity
    pub fn max_capacity(&self) -> usize {
        self.chan.semaphore().bound
    }

    /// Returns the number of [`Sender`] handles.
    pub fn strong_count(&self) -> usize {
        self.chan.strong_count()
    }

    /// Returns the number of [`WeakSender`] handles.
    pub fn weak_count(&self) -> usize {
        self.chan.weak_count()
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            chan: self.chan.clone(),
        }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Sender")
            .field("chan", &self.chan)
            .finish()
    }
}

impl<T> Clone for WeakSender<T> {
    fn clone(&self) -> Self {
        self.chan.increment_weak_count();

        WeakSender {
            chan: self.chan.clone(),
        }
    }
}

impl<T> Drop for WeakSender<T> {
    fn drop(&mut self) {
        self.chan.decrement_weak_count();
    }
}

impl<T> WeakSender<T> {
    /// Tries to convert a `WeakSender` into a [`Sender`]. This will return `Some`
    /// if there are other `Sender` instances alive and the channel wasn't
    /// previously dropped, otherwise `None` is returned.
    pub fn upgrade(&self) -> Option<Sender<T>> {
        chan::Tx::upgrade(self.chan.clone()).map(Sender::new)
    }

    /// Returns the number of [`Sender`] handles.
    pub fn strong_count(&self) -> usize {
        self.chan.strong_count()
    }

    /// Returns the number of [`WeakSender`] handles.
    pub fn weak_count(&self) -> usize {
        self.chan.weak_count()
    }
}

impl<T> fmt::Debug for WeakSender<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("WeakSender").finish()
    }
}

// ===== impl Permit =====

impl<T> Permit<'_, T> {
    /// Sends a value using the reserved capacity.
    ///
    /// Capacity for the message has already been reserved. The message is sent
    /// to the receiver and the permit is consumed. The operation will succeed
    /// even if the receiver half has been closed. See [`Receiver::close`] for
    /// more details on performing a clean shutdown.
    ///
    /// [`Receiver::close`]: Receiver::close
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Reserve capacity
    ///     let permit = tx.reserve().await.unwrap();
    ///
    ///     // Trying to send directly on the `tx` will fail due to no
    ///     // available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Send a message on the permit
    ///     permit.send(456);
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    /// }
    /// ```
    pub fn send(self, value: T) {
        use std::mem;

        self.chan.send(value);

        // Avoid the drop logic
        mem::forget(self);
    }
}

impl<T> Drop for Permit<'_, T> {
    fn drop(&mut self) {
        use chan::Semaphore;

        let semaphore = self.chan.semaphore();

        // Add the permit back to the semaphore
        semaphore.add_permit();

        // If this is the last sender for this channel, wake the receiver so
        // that it can be notified that the channel is closed.
        if semaphore.is_closed() && semaphore.is_idle() {
            self.chan.wake_rx();
        }
    }
}

impl<T> fmt::Debug for Permit<'_, T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Permit")
            .field("chan", &self.chan)
            .finish()
    }
}

// ===== impl PermitIterator =====

impl<'a, T> Iterator for PermitIterator<'a, T> {
    type Item = Permit<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n == 0 {
            return None;
        }

        self.n -= 1;
        Some(Permit { chan: self.chan })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.n;
        (n, Some(n))
    }
}
impl<T> ExactSizeIterator for PermitIterator<'_, T> {}
impl<T> std::iter::FusedIterator for PermitIterator<'_, T> {}

impl<T> Drop for PermitIterator<'_, T> {
    fn drop(&mut self) {
        use chan::Semaphore;

        if self.n == 0 {
            return;
        }

        let semaphore = self.chan.semaphore();

        // Add the remaining permits back to the semaphore
        semaphore.add_permits(self.n);

        // If this is the last sender for this channel, wake the receiver so
        // that it can be notified that the channel is closed.
        if semaphore.is_closed() && semaphore.is_idle() {
            self.chan.wake_rx();
        }
    }
}

impl<T> fmt::Debug for PermitIterator<'_, T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("PermitIterator")
            .field("chan", &self.chan)
            .field("capacity", &self.n)
            .finish()
    }
}

// ===== impl Permit =====

impl<T> OwnedPermit<T> {
    /// Sends a value using the reserved capacity.
    ///
    /// Capacity for the message has already been reserved. The message is sent
    /// to the receiver and the permit is consumed. The operation will succeed
    /// even if the receiver half has been closed. See [`Receiver::close`] for
    /// more details on performing a clean shutdown.
    ///
    /// Unlike [`Permit::send`], this method returns the [`Sender`] from which
    /// the `OwnedPermit` was reserved.
    ///
    /// [`Receiver::close`]: Receiver::close
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, mut rx) = mpsc::channel(1);
    ///
    ///     // Reserve capacity
    ///     let permit = tx.reserve_owned().await.unwrap();
    ///
    ///     // Send a message on the permit, returning the sender.
    ///     let tx = permit.send(456);
    ///
    ///     // The value sent on the permit is received
    ///     assert_eq!(rx.recv().await.unwrap(), 456);
    ///
    ///     // We may now reuse `tx` to send another message.
    ///     tx.send(789).await.unwrap();
    /// }
    /// ```
    pub fn send(mut self, value: T) -> Sender<T> {
        let chan = self.chan.take().unwrap_or_else(|| {
            unreachable!("OwnedPermit channel is only taken when the permit is moved")
        });
        chan.send(value);

        Sender { chan }
    }

    /// Releases the reserved capacity *without* sending a message, returning the
    /// [`Sender`].
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::channel(1);
    ///
    ///     // Clone the sender and reserve capacity
    ///     let permit = tx.clone().reserve_owned().await.unwrap();
    ///
    ///     // Trying to send on the original `tx` will fail, since the `permit`
    ///     // has reserved all the available capacity.
    ///     assert!(tx.try_send(123).is_err());
    ///
    ///     // Release the permit without sending a message, returning the clone
    ///     // of the sender.
    ///     let tx2 = permit.release();
    ///
    ///     // We may now reuse `tx` to send another message.
    ///     tx.send(789).await.unwrap();
    ///     # drop(rx); drop(tx2);
    /// }
    /// ```
    ///
    /// [`Sender`]: Sender
    pub fn release(mut self) -> Sender<T> {
        use chan::Semaphore;

        let chan = self.chan.take().unwrap_or_else(|| {
            unreachable!("OwnedPermit channel is only taken when the permit is moved")
        });

        // Add the permit back to the semaphore
        chan.semaphore().add_permit();
        Sender { chan }
    }

    /// Returns `true` if permits belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::channel::<()>(2);
    ///
    ///     let permit1 = tx.clone().reserve_owned().await.unwrap();
    ///     let permit2 = tx.clone().reserve_owned().await.unwrap();
    ///     assert!(permit1.same_channel(&permit2));
    ///
    ///     let (tx2, rx2) = mpsc::channel::<()>(1);
    ///
    ///     let permit3 = tx2.clone().reserve_owned().await.unwrap();
    ///     assert!(!permit3.same_channel(&permit2));
    /// }
    /// ```
    pub fn same_channel(&self, other: &Self) -> bool {
        self.chan
            .as_ref()
            .zip(other.chan.as_ref())
            .is_some_and(|(a, b)| a.same_channel(b))
    }

    /// Returns `true` if this permit belongs to the same channel as the given [`Sender`].
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::mpsc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (tx, rx) = mpsc::channel::<()>(1);
    ///
    ///     let permit = tx.clone().reserve_owned().await.unwrap();
    ///     assert!(permit.same_channel_as_sender(&tx));
    ///
    ///     let (tx2, rx2) = mpsc::channel::<()>(1);
    ///     assert!(!permit.same_channel_as_sender(&tx2));
    /// }
    /// ```
    pub fn same_channel_as_sender(&self, sender: &Sender<T>) -> bool {
        self.chan
            .as_ref()
            .is_some_and(|chan| chan.same_channel(&sender.chan))
    }
}

impl<T> Drop for OwnedPermit<T> {
    fn drop(&mut self) {
        use chan::Semaphore;

        // Are we still holding onto the sender?
        if let Some(chan) = self.chan.take() {
            let semaphore = chan.semaphore();

            // Add the permit back to the semaphore
            semaphore.add_permit();

            // If this `OwnedPermit` is holding the last sender for this
            // channel, wake the receiver so that it can be notified that the
            // channel is closed.
            if semaphore.is_closed() && semaphore.is_idle() {
                chan.wake_rx();
            }
        }

        // Otherwise, do nothing.
    }
}

impl<T> fmt::Debug for OwnedPermit<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("OwnedPermit")
            .field("chan", &self.chan)
            .finish()
    }
}
