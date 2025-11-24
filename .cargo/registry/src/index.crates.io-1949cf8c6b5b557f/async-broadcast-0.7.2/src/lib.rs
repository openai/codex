//! Async broadcast channel
//!
//! An async multi-producer multi-consumer broadcast channel, where each consumer gets a clone of every
//! message sent on the channel. For obvious reasons, the channel can only be used to broadcast types
//! that implement [`Clone`].
//!
//! A channel has the [`Sender`] and [`Receiver`] side. Both sides are cloneable and can be shared
//! among multiple threads.
//!
//! When all `Sender`s or all `Receiver`s are dropped, the channel becomes closed. When a channel is
//! closed, no more messages can be sent, but remaining messages can still be received.
//!
//! The channel can also be closed manually by calling [`Sender::close()`] or [`Receiver::close()`].
//!
//! ## Examples
//!
//! ```rust
//! use async_broadcast::{broadcast, TryRecvError};
//! use futures_lite::{future::block_on, stream::StreamExt};
//!
//! block_on(async move {
//!     let (s1, mut r1) = broadcast(2);
//!     let s2 = s1.clone();
//!     let mut r2 = r1.clone();
//!
//!     // Send 2 messages from two different senders.
//!     s1.broadcast(7).await.unwrap();
//!     s2.broadcast(8).await.unwrap();
//!
//!     // Channel is now at capacity so sending more messages will result in an error.
//!     assert!(s2.try_broadcast(9).unwrap_err().is_full());
//!     assert!(s1.try_broadcast(10).unwrap_err().is_full());
//!
//!     // We can use `recv` method of the `Stream` implementation to receive messages.
//!     assert_eq!(r1.next().await.unwrap(), 7);
//!     assert_eq!(r1.recv().await.unwrap(), 8);
//!     assert_eq!(r2.next().await.unwrap(), 7);
//!     assert_eq!(r2.recv().await.unwrap(), 8);
//!
//!     // All receiver got all messages so channel is now empty.
//!     assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
//!     assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));
//!
//!     // Drop both senders, which closes the channel.
//!     drop(s1);
//!     drop(s2);
//!
//!     assert_eq!(r1.try_recv(), Err(TryRecvError::Closed));
//!     assert_eq!(r2.try_recv(), Err(TryRecvError::Closed));
//! })
//! ```
//!
//! ## Difference with `async-channel`
//!
//! This crate is similar to [`async-channel`] in that they both provide an MPMC channel but the
//! main difference being that in `async-channel`, each message sent on the channel is only received
//! by one of the receivers. `async-broadcast` on the other hand, delivers each message to every
//! receiver (IOW broadcast) by cloning it for each receiver.
//!
//! [`async-channel`]: https://crates.io/crates/async-channel
//!
//! ## Difference with other broadcast crates
//!
//! * [`broadcaster`]: The main difference would be that `broadcaster` doesn't have a sender and
//!   receiver split and both sides use clones of the same BroadcastChannel instance. The messages
//!   are sent are sent to all channel clones. While this can work for many cases, the lack of
//!   sender and receiver split, means that often times, you'll find yourself having to drain the
//!   channel on the sending side yourself.
//!
//! * [`postage`]: this crate provides a [broadcast API][pba] similar to `async_broadcast`. However,
//!   it:
//!   - (at the time of this writing) duplicates [futures] API, which isn't ideal.
//!   - Does not support overflow mode nor has the concept of inactive receivers, so a slow or
//!     inactive receiver blocking the whole channel is not a solvable problem.
//!   - Provides all kinds of channels, which is generally good but if you just need a broadcast
//!     channel, `async_broadcast` is probably a better choice.
//!
//! * [`tokio::sync`]: Tokio's `sync` module provides a [broadcast channel][tbc] API. The differences
//!    here are:
//!   - While this implementation does provide [overflow mode][tom], it is the default behavior and not
//!     opt-in.
//!   - There is no equivalent of inactive receivers.
//!   - While it's possible to build tokio with only the `sync` module, it comes with other APIs that
//!     you may not need.
//!
//! [`broadcaster`]: https://crates.io/crates/broadcaster
//! [`postage`]: https://crates.io/crates/postage
//! [pba]: https://docs.rs/postage/0.4.1/postage/broadcast/fn.channel.html
//! [futures]: https://crates.io/crates/futures
//! [`tokio::sync`]: https://docs.rs/tokio/1.6.0/tokio/sync
//! [tbc]: https://docs.rs/tokio/1.6.0/tokio/sync/broadcast/index.html
//! [tom]: https://docs.rs/tokio/1.6.0/tokio/sync/broadcast/index.html#lagging
//!
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations, nonstandard_style, rust_2018_idioms)]
#![warn(rustdoc::missing_doc_code_examples, unreachable_pub)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

#[cfg(doctest)]
mod doctests {
    doc_comment::doctest!("../README.md");
}

use std::collections::VecDeque;
use std::convert::TryInto;
use std::error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use event_listener::{Event, EventListener};
use event_listener_strategy::{easy_wrapper, EventListenerFuture};
use futures_core::{ready, stream::Stream};
use pin_project_lite::pin_project;

/// Create a new broadcast channel.
///
/// The created channel has space to hold at most `cap` messages at a time.
///
/// # Panics
///
/// Capacity must be a positive number. If `cap` is zero, this function will panic.
///
/// # Examples
///
/// ```
/// # futures_lite::future::block_on(async {
/// use async_broadcast::{broadcast, TryRecvError, TrySendError};
///
/// let (s, mut r1) = broadcast(1);
/// let mut r2 = r1.clone();
///
/// assert_eq!(s.broadcast(10).await, Ok(None));
/// assert_eq!(s.try_broadcast(20), Err(TrySendError::Full(20)));
///
/// assert_eq!(r1.recv().await, Ok(10));
/// assert_eq!(r2.recv().await, Ok(10));
/// assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
/// assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));
/// # });
/// ```
pub fn broadcast<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    assert!(cap > 0, "capacity cannot be zero");

    let inner = Arc::new(Mutex::new(Inner {
        queue: VecDeque::with_capacity(cap),
        capacity: cap,
        overflow: false,
        await_active: true,
        receiver_count: 1,
        inactive_receiver_count: 0,
        sender_count: 1,
        head_pos: 0,
        is_closed: false,
        send_ops: Event::new(),
        recv_ops: Event::new(),
    }));

    let s = Sender {
        inner: inner.clone(),
    };
    let r = Receiver {
        inner,
        pos: 0,
        listener: None,
    };

    (s, r)
}

#[derive(Debug)]
struct Inner<T> {
    queue: VecDeque<(T, usize)>,
    // We assign the same capacity to the queue but that's just specifying the minimum capacity and
    // the actual capacity could be anything. Hence the need to keep track of our own set capacity.
    capacity: usize,
    receiver_count: usize,
    inactive_receiver_count: usize,
    sender_count: usize,
    /// Send sequence number of the front of the queue
    head_pos: u64,
    overflow: bool,
    await_active: bool,

    is_closed: bool,

    /// Send operations waiting while the channel is full.
    send_ops: Event,

    /// Receive operations waiting while the channel is empty and not closed.
    recv_ops: Event,
}

impl<T> Inner<T> {
    /// Try receiving at the given position, returning either the element or a reference to it.
    ///
    /// Result is used here instead of Cow because we don't have a Clone bound on T.
    fn try_recv_at(&mut self, pos: &mut u64) -> Result<Result<T, &T>, TryRecvError> {
        let i = match pos.checked_sub(self.head_pos) {
            Some(i) => i
                .try_into()
                .expect("Head position more than usize::MAX behind a receiver"),
            None => {
                let count = self.head_pos - *pos;
                *pos = self.head_pos;
                return Err(TryRecvError::Overflowed(count));
            }
        };

        let last_waiter;
        if let Some((_elt, waiters)) = self.queue.get_mut(i) {
            *pos += 1;
            *waiters -= 1;
            last_waiter = *waiters == 0;
        } else {
            debug_assert_eq!(i, self.queue.len());
            if self.is_closed {
                return Err(TryRecvError::Closed);
            } else {
                return Err(TryRecvError::Empty);
            }
        }

        // If we read from the front of the queue and this is the last receiver reading it
        // we can pop the queue instead of cloning the message
        if last_waiter {
            // Only the first element of the queue should have 0 waiters
            assert_eq!(i, 0);

            // Remove the element from the queue, adjust space, and notify senders
            let elt = self.queue.pop_front().unwrap().0;
            self.head_pos += 1;
            if !self.overflow {
                // Notify 1 awaiting senders that there is now room. If there is still room in the
                // queue, the notified operation will notify another awaiting sender.
                self.send_ops.notify(1);
            }

            Ok(Ok(elt))
        } else {
            Ok(Err(&self.queue[i].0))
        }
    }

    /// Closes the channel and notifies all waiting operations.
    ///
    /// Returns `true` if this call has closed the channel and it was not closed already.
    fn close(&mut self) -> bool {
        if self.is_closed {
            return false;
        }

        self.is_closed = true;
        // Notify all waiting senders and receivers.
        self.send_ops.notify(usize::MAX);
        self.recv_ops.notify(usize::MAX);

        true
    }

    /// Set the channel capacity.
    ///
    /// There are times when you need to change the channel's capacity after creating it. If the
    /// `new_cap` is less than the number of messages in the channel, the oldest messages will be
    /// dropped to shrink the channel.
    fn set_capacity(&mut self, new_cap: usize) {
        self.capacity = new_cap;
        if new_cap > self.queue.capacity() {
            let diff = new_cap - self.queue.capacity();
            self.queue.reserve(diff);
        }

        // Ensure queue doesn't have more than `new_cap` messages.
        if new_cap < self.queue.len() {
            let diff = self.queue.len() - new_cap;
            self.queue.drain(0..diff);
            self.head_pos += diff as u64;
        }
    }

    /// Close the channel if there aren't any receivers present anymore
    fn close_channel(&mut self) {
        if self.receiver_count == 0 && self.inactive_receiver_count == 0 {
            self.close();
        }
    }
}

/// The sending side of the broadcast channel.
///
/// Senders can be cloned and shared among threads. When all senders associated with a channel are
/// dropped, the channel becomes closed.
///
/// The channel can also be closed manually by calling [`Sender::close()`].
#[derive(Debug)]
pub struct Sender<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Sender<T> {
    /// Returns the channel capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<i32>(5);
    /// assert_eq!(s.capacity(), 5);
    /// ```
    pub fn capacity(&self) -> usize {
        self.inner.lock().unwrap().capacity
    }

    /// Set the channel capacity.
    ///
    /// There are times when you need to change the channel's capacity after creating it. If the
    /// `new_cap` is less than the number of messages in the channel, the oldest messages will be
    /// dropped to shrink the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError, TryRecvError};
    ///
    /// let (mut s, mut r) = broadcast::<i32>(3);
    /// assert_eq!(s.capacity(), 3);
    /// s.try_broadcast(1).unwrap();
    /// s.try_broadcast(2).unwrap();
    /// s.try_broadcast(3).unwrap();
    ///
    /// s.set_capacity(1);
    /// assert_eq!(s.capacity(), 1);
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Overflowed(2)));
    /// assert_eq!(r.try_recv().unwrap(), 3);
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    /// s.try_broadcast(1).unwrap();
    /// assert_eq!(s.try_broadcast(2), Err(TrySendError::Full(2)));
    ///
    /// s.set_capacity(2);
    /// assert_eq!(s.capacity(), 2);
    /// s.try_broadcast(2).unwrap();
    /// assert_eq!(s.try_broadcast(2), Err(TrySendError::Full(2)));
    /// ```
    pub fn set_capacity(&mut self, new_cap: usize) {
        self.inner.lock().unwrap().set_capacity(new_cap);
    }

    /// If overflow mode is enabled on this channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<i32>(5);
    /// assert!(!s.overflow());
    /// ```
    pub fn overflow(&self) -> bool {
        self.inner.lock().unwrap().overflow
    }

    /// Set overflow mode on the channel.
    ///
    /// When overflow mode is set, broadcasting to the channel will succeed even if the channel is
    /// full. It achieves that by removing the oldest message from the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError, TryRecvError};
    ///
    /// let (mut s, mut r) = broadcast::<i32>(2);
    /// s.try_broadcast(1).unwrap();
    /// s.try_broadcast(2).unwrap();
    /// assert_eq!(s.try_broadcast(3), Err(TrySendError::Full(3)));
    /// s.set_overflow(true);
    /// assert_eq!(s.try_broadcast(3).unwrap(), Some(1));
    /// assert_eq!(s.try_broadcast(4).unwrap(), Some(2));
    ///
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Overflowed(2)));
    /// assert_eq!(r.try_recv().unwrap(), 3);
    /// assert_eq!(r.try_recv().unwrap(), 4);
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    /// ```
    pub fn set_overflow(&mut self, overflow: bool) {
        self.inner.lock().unwrap().overflow = overflow;
    }

    /// If sender will wait for active receivers.
    ///
    /// If set to `false`, [`Send`] will resolve immediately with a [`SendError`]. Defaults to
    /// `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, _) = broadcast::<i32>(5);
    /// assert!(s.await_active());
    /// ```
    pub fn await_active(&self) -> bool {
        self.inner.lock().unwrap().await_active
    }

    /// Specify if sender will wait for active receivers.
    ///
    /// If set to `false`, [`Send`] will resolve immediately with a [`SendError`]. Defaults to
    /// `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (mut s, mut r) = broadcast::<i32>(2);
    /// s.broadcast(1).await.unwrap();
    ///
    /// let _ = r.deactivate();
    /// s.set_await_active(false);
    /// assert!(s.broadcast(2).await.is_err());
    /// # });
    /// ```
    pub fn set_await_active(&mut self, await_active: bool) {
        self.inner.lock().unwrap().await_active = await_active;
    }

    /// Closes the channel.
    ///
    /// Returns `true` if this call has closed the channel and it was not closed already.
    ///
    /// The remaining messages can still be received.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r) = broadcast(1);
    /// s.broadcast(1).await.unwrap();
    /// assert!(s.close());
    ///
    /// assert_eq!(r.recv().await.unwrap(), 1);
    /// assert_eq!(r.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn close(&self) -> bool {
        self.inner.lock().unwrap().close()
    }

    /// Returns `true` if the channel is closed.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert!(!s.is_closed());
    ///
    /// drop(r);
    /// assert!(s.is_closed());
    /// # });
    /// ```
    pub fn is_closed(&self) -> bool {
        self.inner.lock().unwrap().is_closed
    }

    /// Returns `true` if the channel is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert!(s.is_empty());
    /// s.broadcast(1).await;
    /// assert!(!s.is_empty());
    /// # });
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().queue.is_empty()
    }

    /// Returns `true` if the channel is full.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert!(!s.is_full());
    /// s.broadcast(1).await;
    /// assert!(s.is_full());
    /// # });
    /// ```
    pub fn is_full(&self) -> bool {
        let inner = self.inner.lock().unwrap();

        inner.queue.len() == inner.capacity
    }

    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast(2);
    /// assert_eq!(s.len(), 0);
    ///
    /// s.broadcast(1).await;
    /// s.broadcast(2).await;
    /// assert_eq!(s.len(), 2);
    /// # });
    /// ```
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().queue.len()
    }

    /// Returns the number of receivers for the channel.
    ///
    /// This does not include inactive receivers. Use [`Sender::inactive_receiver_count`] if you
    /// are interested in that.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.receiver_count(), 1);
    /// let r = r.deactivate();
    /// assert_eq!(s.receiver_count(), 0);
    ///
    /// let r2 = r.activate_cloned();
    /// assert_eq!(r.receiver_count(), 1);
    /// assert_eq!(r.inactive_receiver_count(), 1);
    /// ```
    pub fn receiver_count(&self) -> usize {
        self.inner.lock().unwrap().receiver_count
    }

    /// Returns the number of inactive receivers for the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.receiver_count(), 1);
    /// let r = r.deactivate();
    /// assert_eq!(s.receiver_count(), 0);
    ///
    /// let r2 = r.activate_cloned();
    /// assert_eq!(r.receiver_count(), 1);
    /// assert_eq!(r.inactive_receiver_count(), 1);
    /// ```
    pub fn inactive_receiver_count(&self) -> usize {
        self.inner.lock().unwrap().inactive_receiver_count
    }

    /// Returns the number of senders for the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.sender_count(), 1);
    ///
    /// let s2 = s.clone();
    /// assert_eq!(s.sender_count(), 2);
    /// # });
    /// ```
    pub fn sender_count(&self) -> usize {
        self.inner.lock().unwrap().sender_count
    }

    /// Produce a new Receiver for this channel.
    ///
    /// The new receiver starts with zero messages available.  This will not re-open the channel if
    /// it was closed due to all receivers being dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r1) = broadcast(2);
    ///
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    ///
    /// let mut r2 = s.new_receiver();
    ///
    /// assert_eq!(s.broadcast(2).await, Ok(None));
    /// drop(s);
    ///
    /// assert_eq!(r1.recv().await, Ok(1));
    /// assert_eq!(r1.recv().await, Ok(2));
    /// assert_eq!(r1.recv().await, Err(RecvError::Closed));
    ///
    /// assert_eq!(r2.recv().await, Ok(2));
    /// assert_eq!(r2.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn new_receiver(&self) -> Receiver<T> {
        let mut inner = self.inner.lock().unwrap();
        inner.receiver_count += 1;
        Receiver {
            inner: self.inner.clone(),
            pos: inner.head_pos + inner.queue.len() as u64,
            listener: None,
        }
    }
}

impl<T: Clone> Sender<T> {
    /// Broadcasts a message on the channel.
    ///
    /// If the channel is full, this method waits until there is space for a message unless:
    ///
    /// 1. overflow mode (set through [`Sender::set_overflow`]) is enabled, in which case it removes
    ///    the oldest message from the channel to make room for the new message. The removed message
    ///    is returned to the caller.
    /// 2. this behavior is disabled using [`Sender::set_await_active`], in which case, it returns
    ///    [`SendError`] immediately.
    ///
    /// If the channel is closed, this method returns an error.
    ///
    /// The future returned by this function is pinned to the heap. If the future being `Unpin` is
    /// not important to you, or if you just `.await` this future, use the [`broadcast_direct`] method
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, SendError};
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    /// drop(r);
    /// assert_eq!(s.broadcast(2).await, Err(SendError(2)));
    /// # });
    /// ```
    pub fn broadcast(&self, msg: T) -> Pin<Box<Send<'_, T>>> {
        Box::pin(self.broadcast_direct(msg))
    }

    /// Broadcasts a message on the channel without pinning the future to the heap.
    ///
    /// The future returned by this method is not `Unpin` and must be pinned before use. This is
    /// the desired behavior if you just `.await` on the future. For other uses cases, use the
    /// [`broadcast`] method instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, SendError};
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert_eq!(s.broadcast_direct(1).await, Ok(None));
    /// drop(r);
    /// assert_eq!(s.broadcast_direct(2).await, Err(SendError(2)));
    /// # });
    /// ```
    pub fn broadcast_direct(&self, msg: T) -> Send<'_, T> {
        Send::_new(SendInner {
            sender: self,
            listener: None,
            msg: Some(msg),
            _pin: PhantomPinned,
        })
    }

    /// Attempts to broadcast a message on the channel.
    ///
    /// If the channel is full, this method returns an error unless overflow mode (set through
    /// [`Sender::set_overflow`]) is enabled. If the overflow mode is enabled, it removes the
    /// oldest message from the channel to make room for the new message. The removed message
    /// is returned to the caller.
    ///
    /// If the channel is closed, this method returns an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError};
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert_eq!(s.try_broadcast(1), Ok(None));
    /// assert_eq!(s.try_broadcast(2), Err(TrySendError::Full(2)));
    ///
    /// drop(r);
    /// assert_eq!(s.try_broadcast(3), Err(TrySendError::Closed(3)));
    /// ```
    pub fn try_broadcast(&self, msg: T) -> Result<Option<T>, TrySendError<T>> {
        let mut ret = None;
        let mut inner = self.inner.lock().unwrap();

        if inner.is_closed {
            return Err(TrySendError::Closed(msg));
        } else if inner.receiver_count == 0 {
            assert!(inner.inactive_receiver_count != 0);

            return Err(TrySendError::Inactive(msg));
        } else if inner.queue.len() == inner.capacity {
            if inner.overflow {
                // Make room by popping a message.
                ret = inner.queue.pop_front().map(|(m, _)| m);
            } else {
                return Err(TrySendError::Full(msg));
            }
        }
        let receiver_count = inner.receiver_count;
        inner.queue.push_back((msg, receiver_count));
        if ret.is_some() {
            inner.head_pos += 1;
        }

        // Notify all awaiting receive operations.
        inner.recv_ops.notify(usize::MAX);

        Ok(ret)
    }

    /// Broadcasts a message on the channel using the blocking strategy.
    ///
    /// If the channel is full, this method will block until there is room.
    ///
    /// If the channel is closed, this method returns an error.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`send`](Self::broadcast) method,
    /// this method will block the current thread until the message is sent.
    ///
    /// This method should not be used in an asynchronous context. It is intended
    /// to be used such that a channel can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in deadlocks.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, SendError};
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert_eq!(s.broadcast_blocking(1), Ok(None));
    /// drop(r);
    /// assert_eq!(s.broadcast_blocking(2), Err(SendError(2)));
    /// ```
    #[cfg(not(target_family = "wasm"))]
    pub fn broadcast_blocking(&self, msg: T) -> Result<Option<T>, SendError<T>> {
        self.broadcast_direct(msg).wait()
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();

        inner.sender_count -= 1;

        if inner.sender_count == 0 {
            inner.close();
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.inner.lock().unwrap().sender_count += 1;

        Sender {
            inner: self.inner.clone(),
        }
    }
}

/// The receiving side of a channel.
///
/// Receivers can be cloned and shared among threads. When all (active) receivers associated with a
/// channel are dropped, the channel becomes closed. You can deactivate a receiver using
/// [`Receiver::deactivate`] if you would like the channel to remain open without keeping active
/// receivers around.
#[derive(Debug)]
pub struct Receiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
    pos: u64,

    /// Listens for a send or close event to unblock this stream.
    listener: Option<EventListener>,
}

impl<T> Receiver<T> {
    /// Returns the channel capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (_s, r) = broadcast::<i32>(5);
    /// assert_eq!(r.capacity(), 5);
    /// ```
    pub fn capacity(&self) -> usize {
        self.inner.lock().unwrap().capacity
    }

    /// Set the channel capacity.
    ///
    /// There are times when you need to change the channel's capacity after creating it. If the
    /// `new_cap` is less than the number of messages in the channel, the oldest messages will be
    /// dropped to shrink the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError, TryRecvError};
    ///
    /// let (s, mut r) = broadcast::<i32>(3);
    /// assert_eq!(r.capacity(), 3);
    /// s.try_broadcast(1).unwrap();
    /// s.try_broadcast(2).unwrap();
    /// s.try_broadcast(3).unwrap();
    ///
    /// r.set_capacity(1);
    /// assert_eq!(r.capacity(), 1);
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Overflowed(2)));
    /// assert_eq!(r.try_recv().unwrap(), 3);
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    /// s.try_broadcast(1).unwrap();
    /// assert_eq!(s.try_broadcast(2), Err(TrySendError::Full(2)));
    ///
    /// r.set_capacity(2);
    /// assert_eq!(r.capacity(), 2);
    /// s.try_broadcast(2).unwrap();
    /// assert_eq!(s.try_broadcast(2), Err(TrySendError::Full(2)));
    /// ```
    pub fn set_capacity(&mut self, new_cap: usize) {
        self.inner.lock().unwrap().set_capacity(new_cap);
    }

    /// If overflow mode is enabled on this channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (_s, r) = broadcast::<i32>(5);
    /// assert!(!r.overflow());
    /// ```
    pub fn overflow(&self) -> bool {
        self.inner.lock().unwrap().overflow
    }

    /// Set overflow mode on the channel.
    ///
    /// When overflow mode is set, broadcasting to the channel will succeed even if the channel is
    /// full. It achieves that by removing the oldest message from the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError, TryRecvError};
    ///
    /// let (s, mut r) = broadcast::<i32>(2);
    /// s.try_broadcast(1).unwrap();
    /// s.try_broadcast(2).unwrap();
    /// assert_eq!(s.try_broadcast(3), Err(TrySendError::Full(3)));
    /// r.set_overflow(true);
    /// assert_eq!(s.try_broadcast(3).unwrap(), Some(1));
    /// assert_eq!(s.try_broadcast(4).unwrap(), Some(2));
    ///
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Overflowed(2)));
    /// assert_eq!(r.try_recv().unwrap(), 3);
    /// assert_eq!(r.try_recv().unwrap(), 4);
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    /// ```
    pub fn set_overflow(&mut self, overflow: bool) {
        self.inner.lock().unwrap().overflow = overflow;
    }

    /// If sender will wait for active receivers.
    ///
    /// If set to `false`, [`Send`] will resolve immediately with a [`SendError`]. Defaults to
    /// `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (_, r) = broadcast::<i32>(5);
    /// assert!(r.await_active());
    /// ```
    pub fn await_active(&self) -> bool {
        self.inner.lock().unwrap().await_active
    }

    /// Specify if sender will wait for active receivers.
    ///
    /// If set to `false`, [`Send`] will resolve immediately with a [`SendError`]. Defaults to
    /// `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, mut r) = broadcast::<i32>(2);
    /// s.broadcast(1).await.unwrap();
    ///
    /// r.set_await_active(false);
    /// let _ = r.deactivate();
    /// assert!(s.broadcast(2).await.is_err());
    /// # });
    /// ```
    pub fn set_await_active(&mut self, await_active: bool) {
        self.inner.lock().unwrap().await_active = await_active;
    }

    /// Closes the channel.
    ///
    /// Returns `true` if this call has closed the channel and it was not closed already.
    ///
    /// The remaining messages can still be received.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r) = broadcast(1);
    /// s.broadcast(1).await.unwrap();
    /// assert!(s.close());
    ///
    /// assert_eq!(r.recv().await.unwrap(), 1);
    /// assert_eq!(r.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn close(&self) -> bool {
        self.inner.lock().unwrap().close()
    }

    /// Returns `true` if the channel is closed.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert!(!s.is_closed());
    ///
    /// drop(r);
    /// assert!(s.is_closed());
    /// # });
    /// ```
    pub fn is_closed(&self) -> bool {
        self.inner.lock().unwrap().is_closed
    }

    /// Returns `true` if the channel is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert!(s.is_empty());
    /// s.broadcast(1).await;
    /// assert!(!s.is_empty());
    /// # });
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().queue.is_empty()
    }

    /// Returns `true` if the channel is full.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast(1);
    ///
    /// assert!(!s.is_full());
    /// s.broadcast(1).await;
    /// assert!(s.is_full());
    /// # });
    /// ```
    pub fn is_full(&self) -> bool {
        let inner = self.inner.lock().unwrap();

        inner.queue.len() == inner.capacity
    }

    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast(2);
    /// assert_eq!(s.len(), 0);
    ///
    /// s.broadcast(1).await;
    /// s.broadcast(2).await;
    /// assert_eq!(s.len(), 2);
    /// # });
    /// ```
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().queue.len()
    }

    /// Returns the number of receivers for the channel.
    ///
    /// This does not include inactive receivers. Use [`Receiver::inactive_receiver_count`] if you
    /// are interested in that.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.receiver_count(), 1);
    /// let r = r.deactivate();
    /// assert_eq!(s.receiver_count(), 0);
    ///
    /// let r2 = r.activate_cloned();
    /// assert_eq!(r.receiver_count(), 1);
    /// assert_eq!(r.inactive_receiver_count(), 1);
    /// ```
    pub fn receiver_count(&self) -> usize {
        self.inner.lock().unwrap().receiver_count
    }

    /// Returns the number of inactive receivers for the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.receiver_count(), 1);
    /// let r = r.deactivate();
    /// assert_eq!(s.receiver_count(), 0);
    ///
    /// let r2 = r.activate_cloned();
    /// assert_eq!(r.receiver_count(), 1);
    /// assert_eq!(r.inactive_receiver_count(), 1);
    /// ```
    pub fn inactive_receiver_count(&self) -> usize {
        self.inner.lock().unwrap().inactive_receiver_count
    }

    /// Returns the number of senders for the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.sender_count(), 1);
    ///
    /// let s2 = s.clone();
    /// assert_eq!(s.sender_count(), 2);
    /// # });
    /// ```
    pub fn sender_count(&self) -> usize {
        self.inner.lock().unwrap().sender_count
    }

    /// Downgrade to a [`InactiveReceiver`].
    ///
    /// An inactive receiver is one that can not and does not receive any messages. Its only purpose
    /// is keep the associated channel open even when there are no (active) receivers. An inactive
    /// receiver can be upgraded into a [`Receiver`] using [`InactiveReceiver::activate`] or
    /// [`InactiveReceiver::activate_cloned`].
    ///
    /// [`Sender::try_broadcast`] will return [`TrySendError::Inactive`] if only inactive
    /// receivers exists for the associated channel and [`Sender::broadcast`] will wait until an
    /// active receiver is available.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, TrySendError};
    ///
    /// let (s, r) = broadcast(1);
    /// let inactive = r.deactivate();
    /// assert_eq!(s.try_broadcast(10), Err(TrySendError::Inactive(10)));
    ///
    /// let mut r = inactive.activate();
    /// assert_eq!(s.broadcast(10).await, Ok(None));
    /// assert_eq!(r.recv().await, Ok(10));
    /// # });
    /// ```
    pub fn deactivate(self) -> InactiveReceiver<T> {
        // Drop::drop impl of Receiver will take care of `receiver_count`.
        self.inner.lock().unwrap().inactive_receiver_count += 1;

        InactiveReceiver {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone> Receiver<T> {
    /// Receives a message from the channel.
    ///
    /// If the channel is empty, this method waits until there is a message.
    ///
    /// If the channel is closed, this method receives a message or returns an error if there are
    /// no more messages.
    ///
    /// If this receiver has missed a message (only possible if overflow mode is enabled), then
    /// this method returns an error and readjusts its cursor to point to the first available
    /// message.
    ///
    /// The future returned by this function is pinned to the heap. If the future being `Unpin` is
    /// not important to you, or if you just `.await` this future, use the [`recv_direct`] method
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r1) = broadcast(1);
    /// let mut r2 = r1.clone();
    ///
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    /// drop(s);
    ///
    /// assert_eq!(r1.recv().await, Ok(1));
    /// assert_eq!(r1.recv().await, Err(RecvError::Closed));
    /// assert_eq!(r2.recv().await, Ok(1));
    /// assert_eq!(r2.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn recv(&mut self) -> Pin<Box<Recv<'_, T>>> {
        Box::pin(self.recv_direct())
    }

    /// Receives a message from the channel without pinning the future to the heap.
    ///
    /// The future returned by this method is not `Unpin` and must be pinned before use. This is
    /// the desired behavior if you just `.await` on the future. For other uses cases, use the
    /// [`recv`] method instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r1) = broadcast(1);
    /// let mut r2 = r1.clone();
    ///
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    /// drop(s);
    ///
    /// assert_eq!(r1.recv_direct().await, Ok(1));
    /// assert_eq!(r1.recv_direct().await, Err(RecvError::Closed));
    /// assert_eq!(r2.recv_direct().await, Ok(1));
    /// assert_eq!(r2.recv_direct().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn recv_direct(&mut self) -> Recv<'_, T> {
        Recv::_new(RecvInner {
            receiver: self,
            listener: None,
            _pin: PhantomPinned,
        })
    }

    /// Attempts to receive a message from the channel.
    ///
    /// If the channel is empty or closed, this method returns an error.
    ///
    /// If this receiver has missed a message (only possible if overflow mode is enabled), then
    /// this method returns an error and readjusts its cursor to point to the first available
    /// message.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, TryRecvError};
    ///
    /// let (s, mut r1) = broadcast(1);
    /// let mut r2 = r1.clone();
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    ///
    /// assert_eq!(r1.try_recv(), Ok(1));
    /// assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
    /// assert_eq!(r2.try_recv(), Ok(1));
    /// assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));
    ///
    /// drop(s);
    /// assert_eq!(r1.try_recv(), Err(TryRecvError::Closed));
    /// assert_eq!(r2.try_recv(), Err(TryRecvError::Closed));
    /// # });
    /// ```
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        self.inner
            .lock()
            .unwrap()
            .try_recv_at(&mut self.pos)
            .map(|cow| cow.unwrap_or_else(T::clone))
    }

    /// Receives a message from the channel using the blocking strategy.
    ///
    /// If the channel is empty, this method will block until there is a message.
    ///
    /// If the channel is closed, this method receives a message or returns an error if there are
    /// no more messages.
    ///
    /// If this receiver has missed a message (only possible if overflow mode is enabled), then
    /// this method returns an error and readjusts its cursor to point to the first available
    /// message.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`recv`](Self::recv) method,
    /// this method will block the current thread until the message is sent.
    ///
    /// This method should not be used in an asynchronous context. It is intended
    /// to be used such that a channel can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in deadlocks.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r) = broadcast(1);
    ///
    /// assert_eq!(s.broadcast_blocking(1), Ok(None));
    /// drop(s);
    ///
    /// assert_eq!(r.recv_blocking(), Ok(1));
    /// assert_eq!(r.recv_blocking(), Err(RecvError::Closed));
    /// ```
    #[cfg(not(target_family = "wasm"))]
    pub fn recv_blocking(&mut self) -> Result<T, RecvError> {
        self.recv_direct().wait()
    }

    /// Produce a new Sender for this channel.
    ///
    /// This will not re-open the channel if it was closed due to all senders being dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s1, mut r) = broadcast(2);
    ///
    /// assert_eq!(s1.broadcast(1).await, Ok(None));
    ///
    /// let mut s2 = r.new_sender();
    ///
    /// assert_eq!(s2.broadcast(2).await, Ok(None));
    /// drop(s1);
    /// drop(s2);
    ///
    /// assert_eq!(r.recv().await, Ok(1));
    /// assert_eq!(r.recv().await, Ok(2));
    /// assert_eq!(r.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn new_sender(&self) -> Sender<T> {
        self.inner.lock().unwrap().sender_count += 1;

        Sender {
            inner: self.inner.clone(),
        }
    }

    /// Produce a new Receiver for this channel.
    ///
    /// Unlike [`Receiver::clone`], this method creates a new receiver that starts with zero
    /// messages available.  This is slightly faster than a real clone.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r1) = broadcast(2);
    ///
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    ///
    /// let mut r2 = r1.new_receiver();
    ///
    /// assert_eq!(s.broadcast(2).await, Ok(None));
    /// drop(s);
    ///
    /// assert_eq!(r1.recv().await, Ok(1));
    /// assert_eq!(r1.recv().await, Ok(2));
    /// assert_eq!(r1.recv().await, Err(RecvError::Closed));
    ///
    /// assert_eq!(r2.recv().await, Ok(2));
    /// assert_eq!(r2.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    pub fn new_receiver(&self) -> Self {
        let mut inner = self.inner.lock().unwrap();
        inner.receiver_count += 1;
        Receiver {
            inner: self.inner.clone(),
            pos: inner.head_pos + inner.queue.len() as u64,
            listener: None,
        }
    }

    /// A low level poll method that is similar to [`Receiver::recv()`] or
    /// [`Receiver::recv_direct()`], and can be useful for building stream implementations which
    /// use a [`Receiver`] under the hood and want to know if the stream has overflowed.
    ///
    /// Prefer to use [`Receiver::recv()`] or [`Receiver::recv_direct()`] when otherwise possible.
    ///
    /// # Errors
    ///
    /// If the number of messages that have been sent has overflowed the channel capacity, a
    /// [`RecvError::Overflowed`] variant is returned containing the number of items that
    /// overflowed and were lost.
    ///
    /// # Examples
    ///
    /// This example shows how the [`Receiver::poll_recv`] method can be used to allow a custom
    /// stream implementation to internally make use of a [`Receiver`]. This example implementation
    /// differs from the stream implementation of [`Receiver`] because it returns an error if
    /// the channel capacity overflows, which the built in [`Receiver`] stream doesn't do.
    ///
    /// ```
    /// use futures_core::Stream;
    /// use async_broadcast::{Receiver, RecvError};
    /// use std::{pin::Pin, task::{Poll, Context}};
    ///
    /// struct MyStream(Receiver<i32>);
    ///
    /// impl futures_core::Stream for MyStream {
    ///     type Item = Result<i32, RecvError>;
    ///     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    ///         Pin::new(&mut self.0).poll_recv(cx)
    ///     }
    /// }
    /// ```
    pub fn poll_recv(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<T, RecvError>>> {
        loop {
            // If this stream is listening for events, first wait for a notification.
            if let Some(listener) = self.listener.as_mut() {
                ready!(Pin::new(listener).poll(cx));
                self.listener = None;
            }

            loop {
                // Attempt to receive a message.
                match self.try_recv() {
                    Ok(msg) => {
                        // The stream is not blocked on an event - drop the listener.
                        self.listener = None;
                        return Poll::Ready(Some(Ok(msg)));
                    }
                    Err(TryRecvError::Closed) => {
                        // The stream is not blocked on an event - drop the listener.
                        self.listener = None;
                        return Poll::Ready(None);
                    }
                    Err(TryRecvError::Overflowed(n)) => {
                        // The stream is not blocked on an event - drop the listener.
                        self.listener = None;
                        return Poll::Ready(Some(Err(RecvError::Overflowed(n))));
                    }
                    Err(TryRecvError::Empty) => {}
                }

                // Receiving failed - now start listening for notifications or wait for one.
                match self.listener.as_mut() {
                    None => {
                        // Start listening and then try receiving again.
                        self.listener = {
                            let inner = self.inner.lock().unwrap();
                            Some(inner.recv_ops.listen())
                        };
                    }
                    Some(_) => {
                        // Go back to the outer loop to poll the listener.
                        break;
                    }
                }
            }
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();

        // Remove ourself from each item's counter
        loop {
            match inner.try_recv_at(&mut self.pos) {
                Ok(_) => continue,
                Err(TryRecvError::Overflowed(_)) => continue,
                Err(TryRecvError::Closed) => break,
                Err(TryRecvError::Empty) => break,
            }
        }

        inner.receiver_count -= 1;

        inner.close_channel();
    }
}

impl<T> Clone for Receiver<T> {
    /// Produce a clone of this Receiver that has the same messages queued.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::{broadcast, RecvError};
    ///
    /// let (s, mut r1) = broadcast(1);
    ///
    /// assert_eq!(s.broadcast(1).await, Ok(None));
    /// drop(s);
    ///
    /// let mut r2 = r1.clone();
    ///
    /// assert_eq!(r1.recv().await, Ok(1));
    /// assert_eq!(r1.recv().await, Err(RecvError::Closed));
    /// assert_eq!(r2.recv().await, Ok(1));
    /// assert_eq!(r2.recv().await, Err(RecvError::Closed));
    /// # });
    /// ```
    fn clone(&self) -> Self {
        let mut inner = self.inner.lock().unwrap();
        inner.receiver_count += 1;
        // increment the waiter count on all items not yet received by this object
        let n = self.pos.saturating_sub(inner.head_pos) as usize;
        for (_elt, waiters) in inner.queue.iter_mut().skip(n) {
            *waiters += 1;
        }
        Receiver {
            inner: self.inner.clone(),
            pos: self.pos,
            listener: None,
        }
    }
}

impl<T: Clone> Stream for Receiver<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match ready!(self.as_mut().poll_recv(cx)) {
                Some(Ok(val)) => return Poll::Ready(Some(val)),
                // If overflowed, we expect future operations to succeed so try again.
                Some(Err(RecvError::Overflowed(_))) => continue,
                // RecvError::Closed should never appear here, but handle it anyway.
                None | Some(Err(RecvError::Closed)) => return Poll::Ready(None),
            }
        }
    }
}

impl<T: Clone> futures_core::stream::FusedStream for Receiver<T> {
    fn is_terminated(&self) -> bool {
        let inner = self.inner.lock().unwrap();

        inner.is_closed && inner.queue.is_empty()
    }
}

/// An error returned from [`Sender::broadcast()`].
///
/// Received because the channel is closed or no active receivers were present while `await-active`
/// was set to `false` (See [`Sender::set_await_active`] for details).
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct SendError<T>(pub T);

impl<T> SendError<T> {
    /// Unwraps the message that couldn't be sent.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> error::Error for SendError<T> {}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SendError(..)")
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sending into a closed channel")
    }
}

/// An error returned from [`Sender::try_broadcast()`].
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TrySendError<T> {
    /// The channel is full but not closed.
    Full(T),

    /// The channel is closed.
    Closed(T),

    /// There are currently no active receivers, only inactive ones.
    Inactive(T),
}

impl<T> TrySendError<T> {
    /// Unwraps the message that couldn't be sent.
    pub fn into_inner(self) -> T {
        match self {
            TrySendError::Full(t) => t,
            TrySendError::Closed(t) => t,
            TrySendError::Inactive(t) => t,
        }
    }

    /// Returns `true` if the channel is full but not closed.
    pub fn is_full(&self) -> bool {
        match self {
            TrySendError::Full(_) => true,
            TrySendError::Closed(_) | TrySendError::Inactive(_) => false,
        }
    }

    /// Returns `true` if the channel is closed.
    pub fn is_closed(&self) -> bool {
        match self {
            TrySendError::Full(_) | TrySendError::Inactive(_) => false,
            TrySendError::Closed(_) => true,
        }
    }

    /// Returns `true` if there are currently no active receivers, only inactive ones.
    pub fn is_disconnected(&self) -> bool {
        match self {
            TrySendError::Full(_) | TrySendError::Closed(_) => false,
            TrySendError::Inactive(_) => true,
        }
    }
}

impl<T> error::Error for TrySendError<T> {}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => write!(f, "Full(..)"),
            TrySendError::Closed(..) => write!(f, "Closed(..)"),
            TrySendError::Inactive(..) => write!(f, "Inactive(..)"),
        }
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => write!(f, "sending into a full channel"),
            TrySendError::Closed(..) => write!(f, "sending into a closed channel"),
            TrySendError::Inactive(..) => write!(f, "sending into the void (no active receivers)"),
        }
    }
}

/// An error returned from [`Receiver::recv()`].
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum RecvError {
    /// The channel has overflowed since the last element was seen.  Future recv operations will
    /// succeed, but some messages have been skipped.
    ///
    /// Contains the number of messages missed.
    Overflowed(u64),

    /// The channel is empty and closed.
    Closed,
}

impl error::Error for RecvError {}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflowed(n) => write!(f, "receiving skipped {} messages", n),
            Self::Closed => write!(f, "receiving from an empty and closed channel"),
        }
    }
}

/// An error returned from [`Receiver::try_recv()`].
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TryRecvError {
    /// The channel has overflowed since the last element was seen.  Future recv operations will
    /// succeed, but some messages have been skipped.
    Overflowed(u64),

    /// The channel is empty but not closed.
    Empty,

    /// The channel is empty and closed.
    Closed,
}

impl TryRecvError {
    /// Returns `true` if the channel is empty but not closed.
    pub fn is_empty(&self) -> bool {
        match self {
            TryRecvError::Empty => true,
            TryRecvError::Closed => false,
            TryRecvError::Overflowed(_) => false,
        }
    }

    /// Returns `true` if the channel is empty and closed.
    pub fn is_closed(&self) -> bool {
        match self {
            TryRecvError::Empty => false,
            TryRecvError::Closed => true,
            TryRecvError::Overflowed(_) => false,
        }
    }

    /// Returns `true` if this error indicates the receiver missed messages.
    pub fn is_overflowed(&self) -> bool {
        match self {
            TryRecvError::Empty => false,
            TryRecvError::Closed => false,
            TryRecvError::Overflowed(_) => true,
        }
    }
}

impl error::Error for TryRecvError {}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryRecvError::Empty => write!(f, "receiving from an empty channel"),
            TryRecvError::Closed => write!(f, "receiving from an empty and closed channel"),
            TryRecvError::Overflowed(n) => {
                write!(f, "receiving operation observed {} lost messages", n)
            }
        }
    }
}

easy_wrapper! {
    /// A future returned by [`Sender::broadcast()`].
    #[derive(Debug)]
    #[must_use = "futures do nothing unless .awaited"]
    pub struct Send<'a, T: Clone>(SendInner<'a, T> => Result<Option<T>, SendError<T>>);
    #[cfg(not(target_family = "wasm"))]
    pub(crate) wait();
}

pin_project! {
    #[derive(Debug)]
    struct SendInner<'a, T> {
        sender: &'a Sender<T>,
        listener: Option<EventListener>,
        msg: Option<T>,

        // Keeping this type `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }
}

impl<T: Clone> EventListenerFuture for SendInner<'_, T> {
    type Output = Result<Option<T>, SendError<T>>;

    fn poll_with_strategy<'x, S: event_listener_strategy::Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        context: &mut S::Context,
    ) -> Poll<Self::Output> {
        let this = self.project();

        loop {
            let msg = this.msg.take().unwrap();
            let inner = &this.sender.inner;

            // Attempt to send a message.
            match this.sender.try_broadcast(msg) {
                Ok(msg) => {
                    let inner = inner.lock().unwrap();

                    if inner.queue.len() < inner.capacity {
                        // Not full still, so notify the next awaiting sender.
                        inner.send_ops.notify(1);
                    }

                    return Poll::Ready(Ok(msg));
                }
                Err(TrySendError::Closed(msg)) => return Poll::Ready(Err(SendError(msg))),
                Err(TrySendError::Full(m)) => *this.msg = Some(m),
                Err(TrySendError::Inactive(m)) if inner.lock().unwrap().await_active => {
                    *this.msg = Some(m)
                }
                Err(TrySendError::Inactive(m)) => return Poll::Ready(Err(SendError(m))),
            }

            // Sending failed - now start listening for notifications or wait for one.
            match &this.listener {
                None => {
                    // Start listening and then try sending again.
                    let inner = inner.lock().unwrap();
                    *this.listener = Some(inner.send_ops.listen());
                }
                Some(_) => {
                    // Wait for a notification.
                    ready!(strategy.poll(this.listener, context));
                    *this.listener = None;
                }
            }
        }
    }
}

easy_wrapper! {
    /// A future returned by [`Receiver::recv()`].
    #[derive(Debug)]
    #[must_use = "futures do nothing unless .awaited"]
    pub struct Recv<'a, T: Clone>(RecvInner<'a, T> => Result<T, RecvError>);
    #[cfg(not(target_family = "wasm"))]
    pub(crate) wait();
}

pin_project! {
    #[derive(Debug)]
    struct RecvInner<'a, T> {
        receiver: &'a mut Receiver<T>,
        listener: Option<EventListener>,

        // Keeping this type `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }
}

impl<T: Clone> EventListenerFuture for RecvInner<'_, T> {
    type Output = Result<T, RecvError>;

    fn poll_with_strategy<'x, S: event_listener_strategy::Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        context: &mut S::Context,
    ) -> Poll<Self::Output> {
        let this = self.project();

        loop {
            // Attempt to receive a message.
            match this.receiver.try_recv() {
                Ok(msg) => return Poll::Ready(Ok(msg)),
                Err(TryRecvError::Closed) => return Poll::Ready(Err(RecvError::Closed)),
                Err(TryRecvError::Overflowed(n)) => {
                    return Poll::Ready(Err(RecvError::Overflowed(n)));
                }
                Err(TryRecvError::Empty) => {}
            }

            // Receiving failed - now start listening for notifications or wait for one.
            match &this.listener {
                None => {
                    // Start listening and then try receiving again.
                    *this.listener = {
                        let inner = this.receiver.inner.lock().unwrap();
                        Some(inner.recv_ops.listen())
                    };
                }
                Some(_) => {
                    // Wait for a notification.
                    ready!(strategy.poll(this.listener, context));
                    *this.listener = None;
                }
            }
        }
    }
}

/// An inactive  receiver.
///
/// An inactive receiver is a receiver that is unable to receive messages. It's only useful for
/// keeping a channel open even when no associated active receivers exist.
#[derive(Debug)]
pub struct InactiveReceiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> InactiveReceiver<T> {
    /// Convert to an activate [`Receiver`].
    ///
    /// Consumes `self`. Use [`InactiveReceiver::activate_cloned`] if you want to keep `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError};
    ///
    /// let (s, r) = broadcast(1);
    /// let inactive = r.deactivate();
    /// assert_eq!(s.try_broadcast(10), Err(TrySendError::Inactive(10)));
    ///
    /// let mut r = inactive.activate();
    /// assert_eq!(s.try_broadcast(10), Ok(None));
    /// assert_eq!(r.try_recv(), Ok(10));
    /// ```
    pub fn activate(self) -> Receiver<T> {
        self.activate_cloned()
    }

    /// Create an activate [`Receiver`] for the associated channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::{broadcast, TrySendError};
    ///
    /// let (s, r) = broadcast(1);
    /// let inactive = r.deactivate();
    /// assert_eq!(s.try_broadcast(10), Err(TrySendError::Inactive(10)));
    ///
    /// let mut r = inactive.activate_cloned();
    /// assert_eq!(s.try_broadcast(10), Ok(None));
    /// assert_eq!(r.try_recv(), Ok(10));
    /// ```
    pub fn activate_cloned(&self) -> Receiver<T> {
        let mut inner = self.inner.lock().unwrap();
        inner.receiver_count += 1;

        if inner.receiver_count == 1 {
            // Notify 1 awaiting senders that there is now a receiver. If there is still room in the
            // queue, the notified operation will notify another awaiting sender.
            inner.send_ops.notify(1);
        }

        Receiver {
            inner: self.inner.clone(),
            pos: inner.head_pos + inner.queue.len() as u64,
            listener: None,
        }
    }

    /// Returns the channel capacity.
    ///
    /// See [`Receiver::capacity`] documentation for examples.
    pub fn capacity(&self) -> usize {
        self.inner.lock().unwrap().capacity
    }

    /// Set the channel capacity.
    ///
    /// There are times when you need to change the channel's capacity after creating it. If the
    /// `new_cap` is less than the number of messages in the channel, the oldest messages will be
    /// dropped to shrink the channel.
    ///
    /// See [`Receiver::set_capacity`] documentation for examples.
    pub fn set_capacity(&mut self, new_cap: usize) {
        self.inner.lock().unwrap().set_capacity(new_cap);
    }

    /// If overflow mode is enabled on this channel.
    ///
    /// See [`Receiver::overflow`] documentation for examples.
    pub fn overflow(&self) -> bool {
        self.inner.lock().unwrap().overflow
    }

    /// Set overflow mode on the channel.
    ///
    /// When overflow mode is set, broadcasting to the channel will succeed even if the channel is
    /// full. It achieves that by removing the oldest message from the channel.
    ///
    /// See [`Receiver::set_overflow`] documentation for examples.
    pub fn set_overflow(&mut self, overflow: bool) {
        self.inner.lock().unwrap().overflow = overflow;
    }

    /// If sender will wait for active receivers.
    ///
    /// If set to `false`, [`Send`] will resolve immediately with a [`SendError`]. Defaults to
    /// `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (_, r) = broadcast::<i32>(5);
    /// let r = r.deactivate();
    /// assert!(r.await_active());
    /// ```
    pub fn await_active(&self) -> bool {
        self.inner.lock().unwrap().await_active
    }

    /// Specify if sender will wait for active receivers.
    ///
    /// If set to `false`, [`Send`] will resolve immediately with a [`SendError`]. Defaults to
    /// `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<i32>(2);
    /// s.broadcast(1).await.unwrap();
    ///
    /// let mut r = r.deactivate();
    /// r.set_await_active(false);
    /// assert!(s.broadcast(2).await.is_err());
    /// # });
    /// ```
    pub fn set_await_active(&mut self, await_active: bool) {
        self.inner.lock().unwrap().await_active = await_active;
    }

    /// Closes the channel.
    ///
    /// Returns `true` if this call has closed the channel and it was not closed already.
    ///
    /// The remaining messages can still be received.
    ///
    /// See [`Receiver::close`] documentation for examples.
    pub fn close(&self) -> bool {
        self.inner.lock().unwrap().close()
    }

    /// Returns `true` if the channel is closed.
    ///
    /// See [`Receiver::is_closed`] documentation for examples.
    pub fn is_closed(&self) -> bool {
        self.inner.lock().unwrap().is_closed
    }

    /// Returns `true` if the channel is empty.
    ///
    /// See [`Receiver::is_empty`] documentation for examples.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().queue.is_empty()
    }

    /// Returns `true` if the channel is full.
    ///
    /// See [`Receiver::is_full`] documentation for examples.
    pub fn is_full(&self) -> bool {
        let inner = self.inner.lock().unwrap();

        inner.queue.len() == inner.capacity
    }

    /// Returns the number of messages in the channel.
    ///
    /// See [`Receiver::len`] documentation for examples.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().queue.len()
    }

    /// Returns the number of receivers for the channel.
    ///
    /// This does not include inactive receivers. Use [`InactiveReceiver::inactive_receiver_count`]
    /// if you're interested in that.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.receiver_count(), 1);
    /// let r = r.deactivate();
    /// assert_eq!(s.receiver_count(), 0);
    ///
    /// let r2 = r.activate_cloned();
    /// assert_eq!(r.receiver_count(), 1);
    /// assert_eq!(r.inactive_receiver_count(), 1);
    /// ```
    pub fn receiver_count(&self) -> usize {
        self.inner.lock().unwrap().receiver_count
    }

    /// Returns the number of inactive receivers for the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_broadcast::broadcast;
    ///
    /// let (s, r) = broadcast::<()>(1);
    /// assert_eq!(s.receiver_count(), 1);
    /// let r = r.deactivate();
    /// assert_eq!(s.receiver_count(), 0);
    ///
    /// let r2 = r.activate_cloned();
    /// assert_eq!(r.receiver_count(), 1);
    /// assert_eq!(r.inactive_receiver_count(), 1);
    /// ```
    pub fn inactive_receiver_count(&self) -> usize {
        self.inner.lock().unwrap().inactive_receiver_count
    }

    /// Returns the number of senders for the channel.
    ///
    /// See [`Receiver::sender_count`] documentation for examples.
    pub fn sender_count(&self) -> usize {
        self.inner.lock().unwrap().sender_count
    }
}

impl<T> Clone for InactiveReceiver<T> {
    fn clone(&self) -> Self {
        self.inner.lock().unwrap().inactive_receiver_count += 1;

        InactiveReceiver {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for InactiveReceiver<T> {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();

        inner.inactive_receiver_count -= 1;
        inner.close_channel();
    }
}
