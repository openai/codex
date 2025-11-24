//! A multi-producer, single-consumer queue for sending values across
//! asynchronous tasks.
//!
//! Similarly to the `std`, channel creation provides [`Receiver`] and
//! [`Sender`] handles. [`Receiver`] implements [`Stream`] and allows a task to
//! read values out of the channel. If there is no message to read from the
//! channel, the current task will be notified when a new value is sent.
//! [`Sender`] implements the `Sink` trait and allows a task to send messages into
//! the channel. If the channel is at capacity, the send will be rejected and
//! the task will be notified when additional capacity is available. In other
//! words, the channel provides backpressure.
//!
//! Unbounded channels are also available using the `unbounded` constructor.
//!
//! # Disconnection
//!
//! When all [`Sender`] handles have been dropped, it is no longer
//! possible to send values into the channel. This is considered the termination
//! event of the stream. As such, [`Receiver::poll_next`]
//! will return `Ok(Ready(None))`.
//!
//! If the [`Receiver`] handle is dropped, then messages can no longer
//! be read out of the channel. In this case, all further attempts to send will
//! result in an error.
//!
//! # Clean Shutdown
//!
//! If the [`Receiver`] is simply dropped, then it is possible for
//! there to be messages still in the channel that will not be processed. As
//! such, it is usually desirable to perform a "clean" shutdown. To do this, the
//! receiver will first call `close`, which will prevent any further messages to
//! be sent into the channel. Then, the receiver consumes the channel to
//! completion, at which point the receiver can be dropped.
//!
//! [`Sender`]: struct.Sender.html
//! [`Receiver`]: struct.Receiver.html
//! [`Stream`]: ../../futures_core/stream/trait.Stream.html
//! [`Receiver::poll_next`]:
//!     ../../futures_core/stream/trait.Stream.html#tymethod.poll_next

// At the core, the channel uses an atomic FIFO queue for message passing. This
// queue is used as the primary coordination primitive. In order to enforce
// capacity limits and handle back pressure, a secondary FIFO queue is used to
// send parked task handles.
//
// The general idea is that the channel is created with a `buffer` size of `n`.
// The channel capacity is `n + num-senders`. Each sender gets one "guaranteed"
// slot to hold a message. This allows `Sender` to know for a fact that a send
// will succeed *before* starting to do the actual work of sending the value.
// Since most of this work is lock-free, once the work starts, it is impossible
// to safely revert.
//
// If the sender is unable to process a send operation, then the current
// task is parked and the handle is sent on the parked task queue.
//
// Note that the implementation guarantees that the channel capacity will never
// exceed the configured limit, however there is no *strict* guarantee that the
// receiver will wake up a parked task *immediately* when a slot becomes
// available. However, it will almost always unpark a task when a slot becomes
// available and it is *guaranteed* that a sender will be unparked when the
// message that caused the sender to become parked is read out of the channel.
//
// The steps for sending a message are roughly:
//
// 1) Increment the channel message count
// 2) If the channel is at capacity, push the task handle onto the wait queue
// 3) Push the message onto the message queue.
//
// The steps for receiving a message are roughly:
//
// 1) Pop a message from the message queue
// 2) Pop a task handle from the wait queue
// 3) Decrement the channel message count.
//
// It's important for the order of operations on lock-free structures to happen
// in reverse order between the sender and receiver. This makes the message
// queue the primary coordination structure and establishes the necessary
// happens-before semantics required for the acquire / release semantics used
// by the queue structure.

use futures_core::stream::{FusedStream, Stream};
use futures_core::task::__internal::AtomicWaker;
use futures_core::task::{Context, Poll, Waker};
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::mpsc::queue::Queue;

mod queue;
#[cfg(feature = "sink")]
mod sink_impl;

struct UnboundedSenderInner<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<UnboundedInner<T>>,
}

struct BoundedSenderInner<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<BoundedInner<T>>,

    // Handle to the task that is blocked on this sender. This handle is sent
    // to the receiver half in order to be notified when the sender becomes
    // unblocked.
    sender_task: Arc<Mutex<SenderTask>>,

    // `true` if the sender might be blocked. This is an optimization to avoid
    // having to lock the mutex most of the time.
    maybe_parked: bool,
}

// We never project Pin<&mut SenderInner> to `Pin<&mut T>`
impl<T> Unpin for UnboundedSenderInner<T> {}
impl<T> Unpin for BoundedSenderInner<T> {}

/// The transmission end of a bounded mpsc channel.
///
/// This value is created by the [`channel`] function.
pub struct Sender<T>(Option<BoundedSenderInner<T>>);

/// The transmission end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`] function.
pub struct UnboundedSender<T>(Option<UnboundedSenderInner<T>>);

#[allow(dead_code)]
trait AssertKinds: Send + Sync + Clone {}
impl AssertKinds for UnboundedSender<u32> {}

/// The receiving end of a bounded mpsc channel.
///
/// This value is created by the [`channel`] function.
pub struct Receiver<T> {
    inner: Option<Arc<BoundedInner<T>>>,
}

/// The receiving end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`] function.
pub struct UnboundedReceiver<T> {
    inner: Option<Arc<UnboundedInner<T>>>,
}

// `Pin<&mut UnboundedReceiver<T>>` is never projected to `Pin<&mut T>`
impl<T> Unpin for UnboundedReceiver<T> {}

/// The error type for [`Sender`s](Sender) used as `Sink`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendError {
    kind: SendErrorKind,
}

/// The error type returned from [`try_send`](Sender::try_send).
#[derive(Clone, PartialEq, Eq)]
pub struct TrySendError<T> {
    err: SendError,
    val: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SendErrorKind {
    Full,
    Disconnected,
}

/// The error type returned from [`try_next`](Receiver::try_next).
pub struct TryRecvError {
    _priv: (),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_full() {
            write!(f, "send failed because channel is full")
        } else {
            write!(f, "send failed because receiver is gone")
        }
    }
}

impl std::error::Error for SendError {}

impl SendError {
    /// Returns `true` if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        match self.kind {
            SendErrorKind::Full => true,
            _ => false,
        }
    }

    /// Returns `true` if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        match self.kind {
            SendErrorKind::Disconnected => true,
            _ => false,
        }
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrySendError").field("kind", &self.err.kind).finish()
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_full() {
            write!(f, "send failed because channel is full")
        } else {
            write!(f, "send failed because receiver is gone")
        }
    }
}

impl<T: core::any::Any> std::error::Error for TrySendError<T> {}

impl<T> TrySendError<T> {
    /// Returns `true` if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        self.err.is_full()
    }

    /// Returns `true` if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        self.err.is_disconnected()
    }

    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.val
    }

    /// Drops the message and converts into a `SendError`.
    pub fn into_send_error(self) -> SendError {
        self.err
    }
}

impl fmt::Debug for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TryRecvError").finish()
    }
}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "receiver channel is empty")
    }
}

impl std::error::Error for TryRecvError {}

struct UnboundedInner<T> {
    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<T>,

    // Number of senders in existence
    num_senders: AtomicUsize,

    // Handle to the receiver's task.
    recv_task: AtomicWaker,
}

struct BoundedInner<T> {
    // Max buffer size of the channel. If `None` then the channel is unbounded.
    buffer: usize,

    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<T>,

    // Atomic, FIFO queue used to send parked task handles to the receiver.
    parked_queue: Queue<Arc<Mutex<SenderTask>>>,

    // Number of senders in existence
    num_senders: AtomicUsize,

    // Handle to the receiver's task.
    recv_task: AtomicWaker,
}

// Struct representation of `Inner::state`.
#[derive(Clone, Copy)]
struct State {
    // `true` when the channel is open
    is_open: bool,

    // Number of messages in the channel
    num_messages: usize,
}

// The `is_open` flag is stored in the left-most bit of `Inner::state`
const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);

// When a new channel is created, it is created in the open state with no
// pending messages.
const INIT_STATE: usize = OPEN_MASK;

// The maximum number of messages that a channel can track is `usize::MAX >> 1`
const MAX_CAPACITY: usize = !(OPEN_MASK);

// The maximum requested buffer size must be less than the maximum capacity of
// a channel. This is because each sender gets a guaranteed slot.
const MAX_BUFFER: usize = MAX_CAPACITY >> 1;

// Sent to the consumer to wake up blocked producers
struct SenderTask {
    task: Option<Waker>,
    is_parked: bool,
}

impl SenderTask {
    fn new() -> Self {
        Self { task: None, is_parked: false }
    }

    fn notify(&mut self) {
        self.is_parked = false;

        if let Some(task) = self.task.take() {
            task.wake();
        }
    }
}

/// Creates a bounded mpsc channel for communicating between asynchronous tasks.
///
/// Being bounded, this channel provides backpressure to ensure that the sender
/// outpaces the receiver by only a limited amount. The channel's capacity is
/// equal to `buffer + num-senders`. In other words, each sender gets a
/// guaranteed slot in the channel capacity, and on top of that there are
/// `buffer` "first come, first serve" slots available to all senders.
///
/// The [`Receiver`] returned implements the [`Stream`] trait, while [`Sender`]
/// implements `Sink`.
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    // Check that the requested buffer size does not exceed the maximum buffer
    // size permitted by the system.
    assert!(buffer < MAX_BUFFER, "requested buffer size too large");

    let inner = Arc::new(BoundedInner {
        buffer,
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        parked_queue: Queue::new(),
        num_senders: AtomicUsize::new(1),
        recv_task: AtomicWaker::new(),
    });

    let tx = BoundedSenderInner {
        inner: inner.clone(),
        sender_task: Arc::new(Mutex::new(SenderTask::new())),
        maybe_parked: false,
    };

    let rx = Receiver { inner: Some(inner) };

    (Sender(Some(tx)), rx)
}

/// Creates an unbounded mpsc channel for communicating between asynchronous
/// tasks.
///
/// A `send` on this channel will always succeed as long as the receive half has
/// not been closed. If the receiver falls behind, messages will be arbitrarily
/// buffered.
///
/// **Note** that the amount of available system memory is an implicit bound to
/// the channel. Using an `unbounded` channel has the ability of causing the
/// process to run out of memory. In this case, the process will be aborted.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let inner = Arc::new(UnboundedInner {
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        num_senders: AtomicUsize::new(1),
        recv_task: AtomicWaker::new(),
    });

    let tx = UnboundedSenderInner { inner: inner.clone() };

    let rx = UnboundedReceiver { inner: Some(inner) };

    (UnboundedSender(Some(tx)), rx)
}

/*
 *
 * ===== impl Sender =====
 *
 */

impl<T> UnboundedSenderInner<T> {
    fn poll_ready_nb(&self) -> Poll<Result<(), SendError>> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if state.is_open {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(SendError { kind: SendErrorKind::Disconnected }))
        }
    }

    // Push message to the queue and signal to the receiver
    fn queue_push_and_signal(&self, msg: T) {
        // Push the message onto the message queue
        self.inner.message_queue.push(msg);

        // Signal to the receiver that a message has been enqueued. If the
        // receiver is parked, this will unpark the task.
        self.inner.recv_task.wake();
    }

    // Increment the number of queued messages. Returns the resulting number.
    fn inc_num_messages(&self) -> Option<usize> {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            // The receiver end closed the channel.
            if !state.is_open {
                return None;
            }

            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(
                state.num_messages < MAX_CAPACITY,
                "buffer space \
                    exhausted; sending this messages would overflow the state"
            );

            state.num_messages += 1;

            let next = encode_state(&state);
            match self.inner.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => return Some(state.num_messages),
                Err(actual) => curr = actual,
            }
        }
    }

    /// Returns whether the senders send to the same receiver.
    fn same_receiver(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Returns whether the sender send to this receiver.
    fn is_connected_to(&self, inner: &Arc<UnboundedInner<T>>) -> bool {
        Arc::ptr_eq(&self.inner, inner)
    }

    /// Returns pointer to the Arc containing sender
    ///
    /// The returned pointer is not referenced and should be only used for hashing!
    fn ptr(&self) -> *const UnboundedInner<T> {
        &*self.inner
    }

    /// Returns whether this channel is closed without needing a context.
    fn is_closed(&self) -> bool {
        !decode_state(self.inner.state.load(SeqCst)).is_open
    }

    /// Closes this channel from the sender side, preventing any new messages.
    fn close_channel(&self) {
        // There's no need to park this sender, its dropping,
        // and we don't want to check for capacity, so skip
        // that stuff from `do_send`.

        self.inner.set_closed();
        self.inner.recv_task.wake();
    }
}

impl<T> BoundedSenderInner<T> {
    /// Attempts to send a message on this `Sender`, returning the message
    /// if there was an error.
    fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        // If the sender is currently blocked, reject the message
        if !self.poll_unparked(None).is_ready() {
            return Err(TrySendError { err: SendError { kind: SendErrorKind::Full }, val: msg });
        }

        // The channel has capacity to accept the message, so send it
        self.do_send_b(msg)
    }

    // Do the send without failing.
    // Can be called only by bounded sender.
    fn do_send_b(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        // Anyone calling do_send *should* make sure there is room first,
        // but assert here for tests as a sanity check.
        debug_assert!(self.poll_unparked(None).is_ready());

        // First, increment the number of messages contained by the channel.
        // This operation will also atomically determine if the sender task
        // should be parked.
        //
        // `None` is returned in the case that the channel has been closed by the
        // receiver. This happens when `Receiver::close` is called or the
        // receiver is dropped.
        let park_self = match self.inc_num_messages() {
            Some(num_messages) => {
                // Block if the current number of pending messages has exceeded
                // the configured buffer size
                num_messages > self.inner.buffer
            }
            None => {
                return Err(TrySendError {
                    err: SendError { kind: SendErrorKind::Disconnected },
                    val: msg,
                })
            }
        };

        // If the channel has reached capacity, then the sender task needs to
        // be parked. This will send the task handle on the parked task queue.
        //
        // However, when `do_send` is called while dropping the `Sender`,
        // `task::current()` can't be called safely. In this case, in order to
        // maintain internal consistency, a blank message is pushed onto the
        // parked task queue.
        if park_self {
            self.park();
        }

        self.queue_push_and_signal(msg);

        Ok(())
    }

    // Push message to the queue and signal to the receiver
    fn queue_push_and_signal(&self, msg: T) {
        // Push the message onto the message queue
        self.inner.message_queue.push(msg);

        // Signal to the receiver that a message has been enqueued. If the
        // receiver is parked, this will unpark the task.
        self.inner.recv_task.wake();
    }

    // Increment the number of queued messages. Returns the resulting number.
    fn inc_num_messages(&self) -> Option<usize> {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            // The receiver end closed the channel.
            if !state.is_open {
                return None;
            }

            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(
                state.num_messages < MAX_CAPACITY,
                "buffer space \
                    exhausted; sending this messages would overflow the state"
            );

            state.num_messages += 1;

            let next = encode_state(&state);
            match self.inner.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => return Some(state.num_messages),
                Err(actual) => curr = actual,
            }
        }
    }

    fn park(&mut self) {
        {
            let mut sender = self.sender_task.lock().unwrap();
            sender.task = None;
            sender.is_parked = true;
        }

        // Send handle over queue
        let t = self.sender_task.clone();
        self.inner.parked_queue.push(t);

        // Check to make sure we weren't closed after we sent our task on the
        // queue
        let state = decode_state(self.inner.state.load(SeqCst));
        self.maybe_parked = state.is_open;
    }

    /// Polls the channel to determine if there is guaranteed capacity to send
    /// at least one item without waiting.
    ///
    /// # Return value
    ///
    /// This method returns:
    ///
    /// - `Poll::Ready(Ok(_))` if there is sufficient capacity;
    /// - `Poll::Pending` if the channel may not have
    ///   capacity, in which case the current task is queued to be notified once
    ///   capacity is available;
    /// - `Poll::Ready(Err(SendError))` if the receiver has been dropped.
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), SendError>> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if !state.is_open {
            return Poll::Ready(Err(SendError { kind: SendErrorKind::Disconnected }));
        }

        self.poll_unparked(Some(cx)).map(Ok)
    }

    /// Returns whether the senders send to the same receiver.
    fn same_receiver(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Returns whether the sender send to this receiver.
    fn is_connected_to(&self, receiver: &Arc<BoundedInner<T>>) -> bool {
        Arc::ptr_eq(&self.inner, receiver)
    }

    /// Returns pointer to the Arc containing sender
    ///
    /// The returned pointer is not referenced and should be only used for hashing!
    fn ptr(&self) -> *const BoundedInner<T> {
        &*self.inner
    }

    /// Returns whether this channel is closed without needing a context.
    fn is_closed(&self) -> bool {
        !decode_state(self.inner.state.load(SeqCst)).is_open
    }

    /// Closes this channel from the sender side, preventing any new messages.
    fn close_channel(&self) {
        // There's no need to park this sender, its dropping,
        // and we don't want to check for capacity, so skip
        // that stuff from `do_send`.

        self.inner.set_closed();
        self.inner.recv_task.wake();
    }

    fn poll_unparked(&mut self, cx: Option<&mut Context<'_>>) -> Poll<()> {
        // First check the `maybe_parked` variable. This avoids acquiring the
        // lock in most cases
        if self.maybe_parked {
            // Get a lock on the task handle
            let mut task = self.sender_task.lock().unwrap();

            if !task.is_parked {
                self.maybe_parked = false;
                return Poll::Ready(());
            }

            // At this point, an unpark request is pending, so there will be an
            // unpark sometime in the future. We just need to make sure that
            // the correct task will be notified.
            //
            // Update the task in case the `Sender` has been moved to another
            // task
            task.task = cx.map(|cx| cx.waker().clone());

            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

impl<T> Sender<T> {
    /// Attempts to send a message on this `Sender`, returning the message
    /// if there was an error.
    pub fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        if let Some(inner) = &mut self.0 {
            inner.try_send(msg)
        } else {
            Err(TrySendError { err: SendError { kind: SendErrorKind::Disconnected }, val: msg })
        }
    }

    /// Send a message on the channel.
    ///
    /// This function should only be called after
    /// [`poll_ready`](Sender::poll_ready) has reported that the channel is
    /// ready to receive a message.
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        self.try_send(msg).map_err(|e| e.err)
    }

    /// Polls the channel to determine if there is guaranteed capacity to send
    /// at least one item without waiting.
    ///
    /// # Return value
    ///
    /// This method returns:
    ///
    /// - `Poll::Ready(Ok(_))` if there is sufficient capacity;
    /// - `Poll::Pending` if the channel may not have
    ///   capacity, in which case the current task is queued to be notified once
    ///   capacity is available;
    /// - `Poll::Ready(Err(SendError))` if the receiver has been dropped.
    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), SendError>> {
        let inner = self.0.as_mut().ok_or(SendError { kind: SendErrorKind::Disconnected })?;
        inner.poll_ready(cx)
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        self.0.as_ref().map(BoundedSenderInner::is_closed).unwrap_or(true)
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&mut self) {
        if let Some(inner) = &mut self.0 {
            inner.close_channel();
        }
    }

    /// Disconnects this sender from the channel, closing it if there are no more senders left.
    pub fn disconnect(&mut self) {
        self.0 = None;
    }

    /// Returns whether the senders send to the same receiver.
    pub fn same_receiver(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(inner), Some(other)) => inner.same_receiver(other),
            _ => false,
        }
    }

    /// Returns whether the sender send to this receiver.
    pub fn is_connected_to(&self, receiver: &Receiver<T>) -> bool {
        match (&self.0, &receiver.inner) {
            (Some(inner), Some(receiver)) => inner.is_connected_to(receiver),
            _ => false,
        }
    }

    /// Hashes the receiver into the provided hasher
    pub fn hash_receiver<H>(&self, hasher: &mut H)
    where
        H: std::hash::Hasher,
    {
        use std::hash::Hash;

        let ptr = self.0.as_ref().map(|inner| inner.ptr());
        ptr.hash(hasher);
    }
}

impl<T> UnboundedSender<T> {
    /// Check if the channel is ready to receive a message.
    pub fn poll_ready(&self, _: &mut Context<'_>) -> Poll<Result<(), SendError>> {
        let inner = self.0.as_ref().ok_or(SendError { kind: SendErrorKind::Disconnected })?;
        inner.poll_ready_nb()
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        self.0.as_ref().map(UnboundedSenderInner::is_closed).unwrap_or(true)
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&self) {
        if let Some(inner) = &self.0 {
            inner.close_channel();
        }
    }

    /// Disconnects this sender from the channel, closing it if there are no more senders left.
    pub fn disconnect(&mut self) {
        self.0 = None;
    }

    // Do the send without parking current task.
    fn do_send_nb(&self, msg: T) -> Result<(), TrySendError<T>> {
        if let Some(inner) = &self.0 {
            if inner.inc_num_messages().is_some() {
                inner.queue_push_and_signal(msg);
                return Ok(());
            }
        }

        Err(TrySendError { err: SendError { kind: SendErrorKind::Disconnected }, val: msg })
    }

    /// Send a message on the channel.
    ///
    /// This method should only be called after `poll_ready` has been used to
    /// verify that the channel is ready to receive a message.
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        self.do_send_nb(msg).map_err(|e| e.err)
    }

    /// Sends a message along this channel.
    ///
    /// This is an unbounded sender, so this function differs from `Sink::send`
    /// by ensuring the return type reflects that the channel is always ready to
    /// receive messages.
    pub fn unbounded_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        self.do_send_nb(msg)
    }

    /// Returns whether the senders send to the same receiver.
    pub fn same_receiver(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(inner), Some(other)) => inner.same_receiver(other),
            _ => false,
        }
    }

    /// Returns whether the sender send to this receiver.
    pub fn is_connected_to(&self, receiver: &UnboundedReceiver<T>) -> bool {
        match (&self.0, &receiver.inner) {
            (Some(inner), Some(receiver)) => inner.is_connected_to(receiver),
            _ => false,
        }
    }

    /// Hashes the receiver into the provided hasher
    pub fn hash_receiver<H>(&self, hasher: &mut H)
    where
        H: std::hash::Hasher,
    {
        use std::hash::Hash;

        let ptr = self.0.as_ref().map(|inner| inner.ptr());
        ptr.hash(hasher);
    }

    /// Return the number of messages in the queue or 0 if channel is disconnected.
    pub fn len(&self) -> usize {
        if let Some(sender) = &self.0 {
            decode_state(sender.inner.state.load(SeqCst)).num_messages
        } else {
            0
        }
    }

    /// Return false is channel has no queued messages, true otherwise.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Clone for UnboundedSenderInner<T> {
    fn clone(&self) -> Self {
        // Since this atomic op isn't actually guarding any memory and we don't
        // care about any orderings besides the ordering on the single atomic
        // variable, a relaxed ordering is acceptable.
        let mut curr = self.inner.num_senders.load(SeqCst);

        loop {
            // If the maximum number of senders has been reached, then fail
            if curr == MAX_BUFFER {
                panic!("cannot clone `Sender` -- too many outstanding senders");
            }

            debug_assert!(curr < MAX_BUFFER);

            let next = curr + 1;
            match self.inner.num_senders.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => {
                    // The ABA problem doesn't matter here. We only care that the
                    // number of senders never exceeds the maximum.
                    return Self { inner: self.inner.clone() };
                }
                Err(actual) => curr = actual,
            }
        }
    }
}

impl<T> Clone for BoundedSenderInner<T> {
    fn clone(&self) -> Self {
        // Since this atomic op isn't actually guarding any memory and we don't
        // care about any orderings besides the ordering on the single atomic
        // variable, a relaxed ordering is acceptable.
        let mut curr = self.inner.num_senders.load(SeqCst);

        loop {
            // If the maximum number of senders has been reached, then fail
            if curr == self.inner.max_senders() {
                panic!("cannot clone `Sender` -- too many outstanding senders");
            }

            debug_assert!(curr < self.inner.max_senders());

            let next = curr + 1;
            match self.inner.num_senders.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => {
                    // The ABA problem doesn't matter here. We only care that the
                    // number of senders never exceeds the maximum.
                    return Self {
                        inner: self.inner.clone(),
                        sender_task: Arc::new(Mutex::new(SenderTask::new())),
                        maybe_parked: false,
                    };
                }
                Err(actual) => curr = actual,
            }
        }
    }
}

impl<T> Drop for UnboundedSenderInner<T> {
    fn drop(&mut self) {
        // Ordering between variables don't matter here
        let prev = self.inner.num_senders.fetch_sub(1, SeqCst);

        if prev == 1 {
            self.close_channel();
        }
    }
}

impl<T> Drop for BoundedSenderInner<T> {
    fn drop(&mut self) {
        // Ordering between variables don't matter here
        let prev = self.inner.num_senders.fetch_sub(1, SeqCst);

        if prev == 1 {
            self.close_channel();
        }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").field("closed", &self.is_closed()).finish()
    }
}

impl<T> fmt::Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnboundedSender").field("closed", &self.is_closed()).finish()
    }
}

/*
 *
 * ===== impl Receiver =====
 *
 */

impl<T> Receiver<T> {
    /// Closes the receiving half of a channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.set_closed();

            // Wake up any threads waiting as they'll see that we've closed the
            // channel and will continue on their merry way.
            while let Some(task) = unsafe { inner.parked_queue.pop_spin() } {
                task.lock().unwrap().notify();
            }
        }
    }

    /// Tries to receive the next message without notifying a context if empty.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only when you've otherwise arranged to be notified when the channel is
    /// no longer empty.
    ///
    /// This function returns:
    /// * `Ok(Some(t))` when message is fetched
    /// * `Ok(None)` when channel is closed and no messages left in the queue
    /// * `Err(e)` when there are no messages available, but channel is not yet closed
    pub fn try_next(&mut self) -> Result<Option<T>, TryRecvError> {
        match self.next_message() {
            Poll::Ready(msg) => Ok(msg),
            Poll::Pending => Err(TryRecvError { _priv: () }),
        }
    }

    fn next_message(&mut self) -> Poll<Option<T>> {
        let inner = match self.inner.as_mut() {
            None => return Poll::Ready(None),
            Some(inner) => inner,
        };
        // Pop off a message
        match unsafe { inner.message_queue.pop_spin() } {
            Some(msg) => {
                // If there are any parked task handles in the parked queue,
                // pop one and unpark it.
                self.unpark_one();

                // Decrement number of messages
                self.dec_num_messages();

                Poll::Ready(Some(msg))
            }
            None => {
                let state = decode_state(inner.state.load(SeqCst));
                if state.is_closed() {
                    // If closed flag is set AND there are no pending messages
                    // it means end of stream
                    self.inner = None;
                    Poll::Ready(None)
                } else {
                    // If queue is open, we need to return Pending
                    // to be woken up when new messages arrive.
                    // If queue is closed but num_messages is non-zero,
                    // it means that senders updated the state,
                    // but didn't put message to queue yet,
                    // so we need to park until sender unparks the task
                    // after queueing the message.
                    Poll::Pending
                }
            }
        }
    }

    // Unpark a single task handle if there is one pending in the parked queue
    fn unpark_one(&mut self) {
        if let Some(inner) = &mut self.inner {
            if let Some(task) = unsafe { inner.parked_queue.pop_spin() } {
                task.lock().unwrap().notify();
            }
        }
    }

    fn dec_num_messages(&self) {
        if let Some(inner) = &self.inner {
            // OPEN_MASK is highest bit, so it's unaffected by subtraction
            // unless there's underflow, and we know there's no underflow
            // because number of messages at this point is always > 0.
            inner.state.fetch_sub(1, SeqCst);
        }
    }
}

// The receiver does not ever take a Pin to the inner T
impl<T> Unpin for Receiver<T> {}

impl<T> FusedStream for Receiver<T> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        // Try to read a message off of the message queue.
        match self.next_message() {
            Poll::Ready(msg) => {
                if msg.is_none() {
                    self.inner = None;
                }
                Poll::Ready(msg)
            }
            Poll::Pending => {
                // There are no messages to read, in this case, park.
                self.inner.as_ref().unwrap().recv_task.register(cx.waker());
                // Check queue again after parking to prevent race condition:
                // a message could be added to the queue after previous `next_message`
                // before `register` call.
                self.next_message()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(inner) = &self.inner {
            decode_state(inner.state.load(SeqCst)).size_hint()
        } else {
            (0, Some(0))
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        if self.inner.is_some() {
            loop {
                match self.next_message() {
                    Poll::Ready(Some(_)) => {}
                    Poll::Ready(None) => break,
                    Poll::Pending => {
                        let state = decode_state(self.inner.as_ref().unwrap().state.load(SeqCst));

                        // If the channel is closed, then there is no need to park.
                        if state.is_closed() {
                            break;
                        }

                        // TODO: Spinning isn't ideal, it might be worth
                        // investigating using a condvar or some other strategy
                        // here. That said, if this case is hit, then another thread
                        // is about to push the value into the queue and this isn't
                        // the only spinlock in the impl right now.
                        thread::yield_now();
                    }
                }
            }
        }
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let closed = if let Some(ref inner) = self.inner {
            decode_state(inner.state.load(SeqCst)).is_closed()
        } else {
            false
        };

        f.debug_struct("Receiver").field("closed", &closed).finish()
    }
}

impl<T> UnboundedReceiver<T> {
    /// Closes the receiving half of a channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.set_closed();
        }
    }

    /// Tries to receive the next message without notifying a context if empty.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only when you've otherwise arranged to be notified when the channel is
    /// no longer empty.
    ///
    /// This function returns:
    /// * `Ok(Some(t))` when message is fetched
    /// * `Ok(None)` when channel is closed and no messages left in the queue
    /// * `Err(e)` when there are no messages available, but channel is not yet closed
    pub fn try_next(&mut self) -> Result<Option<T>, TryRecvError> {
        match self.next_message() {
            Poll::Ready(msg) => Ok(msg),
            Poll::Pending => Err(TryRecvError { _priv: () }),
        }
    }

    fn next_message(&mut self) -> Poll<Option<T>> {
        let inner = match self.inner.as_mut() {
            None => return Poll::Ready(None),
            Some(inner) => inner,
        };
        // Pop off a message
        match unsafe { inner.message_queue.pop_spin() } {
            Some(msg) => {
                // Decrement number of messages
                self.dec_num_messages();

                Poll::Ready(Some(msg))
            }
            None => {
                let state = decode_state(inner.state.load(SeqCst));
                if state.is_closed() {
                    // If closed flag is set AND there are no pending messages
                    // it means end of stream
                    self.inner = None;
                    Poll::Ready(None)
                } else {
                    // If queue is open, we need to return Pending
                    // to be woken up when new messages arrive.
                    // If queue is closed but num_messages is non-zero,
                    // it means that senders updated the state,
                    // but didn't put message to queue yet,
                    // so we need to park until sender unparks the task
                    // after queueing the message.
                    Poll::Pending
                }
            }
        }
    }

    fn dec_num_messages(&self) {
        if let Some(inner) = &self.inner {
            // OPEN_MASK is highest bit, so it's unaffected by subtraction
            // unless there's underflow, and we know there's no underflow
            // because number of messages at this point is always > 0.
            inner.state.fetch_sub(1, SeqCst);
        }
    }
}

impl<T> FusedStream for UnboundedReceiver<T> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        // Try to read a message off of the message queue.
        match self.next_message() {
            Poll::Ready(msg) => {
                if msg.is_none() {
                    self.inner = None;
                }
                Poll::Ready(msg)
            }
            Poll::Pending => {
                // There are no messages to read, in this case, park.
                self.inner.as_ref().unwrap().recv_task.register(cx.waker());
                // Check queue again after parking to prevent race condition:
                // a message could be added to the queue after previous `next_message`
                // before `register` call.
                self.next_message()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(inner) = &self.inner {
            decode_state(inner.state.load(SeqCst)).size_hint()
        } else {
            (0, Some(0))
        }
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        if self.inner.is_some() {
            loop {
                match self.next_message() {
                    Poll::Ready(Some(_)) => {}
                    Poll::Ready(None) => break,
                    Poll::Pending => {
                        let state = decode_state(self.inner.as_ref().unwrap().state.load(SeqCst));

                        // If the channel is closed, then there is no need to park.
                        if state.is_closed() {
                            break;
                        }

                        // TODO: Spinning isn't ideal, it might be worth
                        // investigating using a condvar or some other strategy
                        // here. That said, if this case is hit, then another thread
                        // is about to push the value into the queue and this isn't
                        // the only spinlock in the impl right now.
                        thread::yield_now();
                    }
                }
            }
        }
    }
}

impl<T> fmt::Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let closed = if let Some(ref inner) = self.inner {
            decode_state(inner.state.load(SeqCst)).is_closed()
        } else {
            false
        };

        f.debug_struct("Receiver").field("closed", &closed).finish()
    }
}

/*
 *
 * ===== impl Inner =====
 *
 */

impl<T> UnboundedInner<T> {
    // Clear `open` flag in the state, keep `num_messages` intact.
    fn set_closed(&self) {
        let curr = self.state.load(SeqCst);
        if !decode_state(curr).is_open {
            return;
        }

        self.state.fetch_and(!OPEN_MASK, SeqCst);
    }
}

impl<T> BoundedInner<T> {
    // The return value is such that the total number of messages that can be
    // enqueued into the channel will never exceed MAX_CAPACITY
    fn max_senders(&self) -> usize {
        MAX_CAPACITY - self.buffer
    }

    // Clear `open` flag in the state, keep `num_messages` intact.
    fn set_closed(&self) {
        let curr = self.state.load(SeqCst);
        if !decode_state(curr).is_open {
            return;
        }

        self.state.fetch_and(!OPEN_MASK, SeqCst);
    }
}

unsafe impl<T: Send> Send for UnboundedInner<T> {}
unsafe impl<T: Send> Sync for UnboundedInner<T> {}

unsafe impl<T: Send> Send for BoundedInner<T> {}
unsafe impl<T: Send> Sync for BoundedInner<T> {}

impl State {
    fn is_closed(&self) -> bool {
        !self.is_open && self.num_messages == 0
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_open {
            (self.num_messages, None)
        } else {
            (self.num_messages, Some(self.num_messages))
        }
    }
}

/*
 *
 * ===== Helpers =====
 *
 */

fn decode_state(num: usize) -> State {
    State { is_open: num & OPEN_MASK == OPEN_MASK, num_messages: num & MAX_CAPACITY }
}

fn encode_state(state: &State) -> usize {
    let mut num = state.num_messages;

    if state.is_open {
        num |= OPEN_MASK;
    }

    num
}
