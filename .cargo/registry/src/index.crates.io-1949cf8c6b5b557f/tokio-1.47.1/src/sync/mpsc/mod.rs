#![cfg_attr(not(feature = "sync"), allow(dead_code, unreachable_pub))]

//! A multi-producer, single-consumer queue for sending values between
//! asynchronous tasks.
//!
//! This module provides two variants of the channel: bounded and unbounded. The
//! bounded variant has a limit on the number of messages that the channel can
//! store, and if this limit is reached, trying to send another message will
//! wait until a message is received from the channel. An unbounded channel has
//! an infinite capacity, so the `send` method will always complete immediately.
//! This makes the [`UnboundedSender`] usable from both synchronous and
//! asynchronous code.
//!
//! Similar to the `mpsc` channels provided by `std`, the channel constructor
//! functions provide separate send and receive handles, [`Sender`] and
//! [`Receiver`] for the bounded channel, [`UnboundedSender`] and
//! [`UnboundedReceiver`] for the unbounded channel. If there is no message to read,
//! the current task will be notified when a new value is sent. [`Sender`] and
//! [`UnboundedSender`] allow sending values into the channel. If the bounded
//! channel is at capacity, the send is rejected and the task will be notified
//! when additional capacity is available. In other words, the channel provides
//! backpressure.
//!
//! This channel is also suitable for the single-producer single-consumer
//! use-case. (Unless you only need to send one message, in which case you
//! should use the [oneshot] channel.)
//!
//! # Disconnection
//!
//! When all [`Sender`] handles have been dropped, it is no longer
//! possible to send values into the channel. This is considered the termination
//! event of the stream. As such, `Receiver::poll` returns `Ok(Ready(None))`.
//!
//! If the [`Receiver`] handle is dropped, then messages can no longer
//! be read out of the channel. In this case, all further attempts to send will
//! result in an error. Additionally, all unread messages will be drained from the
//! channel and dropped.
//!
//! # Clean Shutdown
//!
//! When the [`Receiver`] is dropped, it is possible for unprocessed messages to
//! remain in the channel. Instead, it is usually desirable to perform a "clean"
//! shutdown. To do this, the receiver first calls `close`, which will prevent
//! any further messages to be sent into the channel. Then, the receiver
//! consumes the channel to completion, at which point the receiver can be
//! dropped.
//!
//! # Communicating between sync and async code
//!
//! When you want to communicate between synchronous and asynchronous code, there
//! are two situations to consider:
//!
//! **Bounded channel**: If you need a bounded channel, you should use a bounded
//! Tokio `mpsc` channel for both directions of communication. Instead of calling
//! the async [`send`][bounded-send] or [`recv`][bounded-recv] methods, in
//! synchronous code you will need to use the [`blocking_send`][blocking-send] or
//! [`blocking_recv`][blocking-recv] methods.
//!
//! **Unbounded channel**: You should use the kind of channel that matches where
//! the receiver is. So for sending a message _from async to sync_, you should
//! use [the standard library unbounded channel][std-unbounded] or
//! [crossbeam][crossbeam-unbounded].  Similarly, for sending a message _from sync
//! to async_, you should use an unbounded Tokio `mpsc` channel.
//!
//! Please be aware that the above remarks were written with the `mpsc` channel
//! in mind, but they can also be generalized to other kinds of channels. In
//! general, any channel method that isn't marked async can be called anywhere,
//! including outside of the runtime. For example, sending a message on a
//! [oneshot] channel from outside the runtime is perfectly fine.
//!
//! # Multiple runtimes
//!
//! The `mpsc` channel is runtime agnostic. You can freely move it between
//! different instances of the Tokio runtime or even use it from non-Tokio
//! runtimes.
//!
//! When used in a Tokio runtime, it participates in
//! [cooperative scheduling](crate::task::coop#cooperative-scheduling) to avoid
//! starvation. This feature does not apply when used from non-Tokio runtimes.
//!
//! As an exception, methods ending in `_timeout` are not runtime agnostic
//! because they require access to the Tokio timer. See the documentation of
//! each `*_timeout` method for more information on its use.
//!
//! # Allocation behavior
//!
//! <div class="warning">The implementation details described in this section may change in future
//! Tokio releases.</div>
//!
//! The mpsc channel stores elements in blocks. Blocks are organized in a linked list. Sending
//! pushes new elements onto the block at the front of the list, and receiving pops them off the
//! one at the back. A block can hold 32 messages on a 64-bit target and 16 messages on a 32-bit
//! target. This number is independent of channel and message size. Each block also stores 4
//! pointer-sized values for bookkeeping (so on a 64-bit machine, each message has 1 byte of
//! overhead).
//!
//! When all values in a block have been received, it becomes empty. It will then be freed, unless
//! the channel's first block (where newly-sent elements are being stored) has no next block. In
//! that case, the empty block is reused as the next block.
//!
//! [`Sender`]: crate::sync::mpsc::Sender
//! [`Receiver`]: crate::sync::mpsc::Receiver
//! [bounded-send]: crate::sync::mpsc::Sender::send()
//! [bounded-recv]: crate::sync::mpsc::Receiver::recv()
//! [blocking-send]: crate::sync::mpsc::Sender::blocking_send()
//! [blocking-recv]: crate::sync::mpsc::Receiver::blocking_recv()
//! [`UnboundedSender`]: crate::sync::mpsc::UnboundedSender
//! [`UnboundedReceiver`]: crate::sync::mpsc::UnboundedReceiver
//! [oneshot]: crate::sync::oneshot
//! [`Handle::block_on`]: crate::runtime::Handle::block_on()
//! [std-unbounded]: std::sync::mpsc::channel
//! [crossbeam-unbounded]: https://docs.rs/crossbeam/*/crossbeam/channel/fn.unbounded.html
//! [`send_timeout`]: crate::sync::mpsc::Sender::send_timeout

pub(super) mod block;

mod bounded;
pub use self::bounded::{
    channel, OwnedPermit, Permit, PermitIterator, Receiver, Sender, WeakSender,
};

mod chan;

pub(super) mod list;

mod unbounded;
pub use self::unbounded::{
    unbounded_channel, UnboundedReceiver, UnboundedSender, WeakUnboundedSender,
};

pub mod error;

/// The number of values a block can contain.
///
/// This value must be a power of 2. It also must be smaller than the number of
/// bits in `usize`.
#[cfg(all(target_pointer_width = "64", not(loom)))]
const BLOCK_CAP: usize = 32;

#[cfg(all(not(target_pointer_width = "64"), not(loom)))]
const BLOCK_CAP: usize = 16;

#[cfg(loom)]
const BLOCK_CAP: usize = 2;
