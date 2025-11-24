//! The channel interface.

use std::fmt;
use std::iter::FusedIterator;
use std::mem;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::context::Context;
use crate::counter;
use crate::err::{
    RecvError, RecvTimeoutError, SendError, SendTimeoutError, TryRecvError, TrySendError,
};
use crate::flavors;
use crate::select::{Operation, SelectHandle, Token};

/// Creates a channel of unbounded capacity.
///
/// This channel has a growable buffer that can hold any number of messages at a time.
///
/// # Examples
///
/// ```
/// use std::thread;
/// use crossbeam_channel::unbounded;
///
/// let (s, r) = unbounded();
///
/// // Computes the n-th Fibonacci number.
/// fn fib(n: i32) -> i32 {
///     if n <= 1 {
///         n
///     } else {
///         fib(n - 1) + fib(n - 2)
///     }
/// }
///
/// // Spawn an asynchronous computation.
/// thread::spawn(move || s.send(fib(20)).unwrap());
///
/// // Print the result of the computation.
/// println!("{}", r.recv().unwrap());
/// ```
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let (s, r) = counter::new(flavors::list::Channel::new());
    let s = Sender {
        flavor: SenderFlavor::List(s),
    };
    let r = Receiver {
        flavor: ReceiverFlavor::List(r),
    };
    (s, r)
}

/// Creates a channel of bounded capacity.
///
/// This channel has a buffer that can hold at most `cap` messages at a time.
///
/// A special case is zero-capacity channel, which cannot hold any messages. Instead, send and
/// receive operations must appear at the same time in order to pair up and pass the message over.
///
/// # Examples
///
/// A channel of capacity 1:
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use crossbeam_channel::bounded;
///
/// let (s, r) = bounded(1);
///
/// // This call returns immediately because there is enough space in the channel.
/// s.send(1).unwrap();
///
/// thread::spawn(move || {
///     // This call blocks the current thread because the channel is full.
///     // It will be able to complete only after the first message is received.
///     s.send(2).unwrap();
/// });
///
/// thread::sleep(Duration::from_secs(1));
/// assert_eq!(r.recv(), Ok(1));
/// assert_eq!(r.recv(), Ok(2));
/// ```
///
/// A zero-capacity channel:
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use crossbeam_channel::bounded;
///
/// let (s, r) = bounded(0);
///
/// thread::spawn(move || {
///     // This call blocks the current thread until a receive operation appears
///     // on the other side of the channel.
///     s.send(1).unwrap();
/// });
///
/// thread::sleep(Duration::from_secs(1));
/// assert_eq!(r.recv(), Ok(1));
/// ```
pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    if cap == 0 {
        let (s, r) = counter::new(flavors::zero::Channel::new());
        let s = Sender {
            flavor: SenderFlavor::Zero(s),
        };
        let r = Receiver {
            flavor: ReceiverFlavor::Zero(r),
        };
        (s, r)
    } else {
        let (s, r) = counter::new(flavors::array::Channel::with_capacity(cap));
        let s = Sender {
            flavor: SenderFlavor::Array(s),
        };
        let r = Receiver {
            flavor: ReceiverFlavor::Array(r),
        };
        (s, r)
    }
}

/// Creates a receiver that delivers a message after a certain duration of time.
///
/// The channel is bounded with capacity of 1 and never gets disconnected. Exactly one message will
/// be sent into the channel after `duration` elapses. The message is the instant at which it is
/// sent.
///
/// # Examples
///
/// Using an `after` channel for timeouts:
///
/// ```
/// use std::time::Duration;
/// use crossbeam_channel::{after, select, unbounded};
///
/// let (s, r) = unbounded::<i32>();
/// let timeout = Duration::from_millis(100);
///
/// select! {
///     recv(r) -> msg => println!("received {:?}", msg),
///     recv(after(timeout)) -> _ => println!("timed out"),
/// }
/// ```
///
/// When the message gets sent:
///
/// ```
/// use std::thread;
/// use std::time::{Duration, Instant};
/// use crossbeam_channel::after;
///
/// // Converts a number of milliseconds into a `Duration`.
/// let ms = |ms| Duration::from_millis(ms);
///
/// // Returns `true` if `a` and `b` are very close `Instant`s.
/// let eq = |a, b| a + ms(60) > b && b + ms(60) > a;
///
/// let start = Instant::now();
/// let r = after(ms(100));
///
/// thread::sleep(ms(500));
///
/// // This message was sent 100 ms from the start and received 500 ms from the start.
/// assert!(eq(r.recv().unwrap(), start + ms(100)));
/// assert!(eq(Instant::now(), start + ms(500)));
/// ```
pub fn after(duration: Duration) -> Receiver<Instant> {
    match Instant::now().checked_add(duration) {
        Some(deadline) => Receiver {
            flavor: ReceiverFlavor::At(Arc::new(flavors::at::Channel::new_deadline(deadline))),
        },
        None => never(),
    }
}

/// Creates a receiver that delivers a message at a certain instant in time.
///
/// The channel is bounded with capacity of 1 and never gets disconnected. Exactly one message will
/// be sent into the channel at the moment in time `when`. The message is the instant at which it
/// is sent, which is the same as `when`. If `when` is in the past, the message will be delivered
/// instantly to the receiver.
///
/// # Examples
///
/// Using an `at` channel for timeouts:
///
/// ```
/// use std::time::{Instant, Duration};
/// use crossbeam_channel::{at, select, unbounded};
///
/// let (s, r) = unbounded::<i32>();
/// let deadline = Instant::now() + Duration::from_millis(500);
///
/// select! {
///     recv(r) -> msg => println!("received {:?}", msg),
///     recv(at(deadline)) -> _ => println!("timed out"),
/// }
/// ```
///
/// When the message gets sent:
///
/// ```
/// use std::time::{Duration, Instant};
/// use crossbeam_channel::at;
///
/// // Converts a number of milliseconds into a `Duration`.
/// let ms = |ms| Duration::from_millis(ms);
///
/// let start = Instant::now();
/// let end = start + ms(100);
///
/// let r = at(end);
///
/// // This message was sent 100 ms from the start
/// assert_eq!(r.recv().unwrap(), end);
/// assert!(Instant::now() > start + ms(100));
/// ```
pub fn at(when: Instant) -> Receiver<Instant> {
    Receiver {
        flavor: ReceiverFlavor::At(Arc::new(flavors::at::Channel::new_deadline(when))),
    }
}

/// Creates a receiver that never delivers messages.
///
/// The channel is bounded with capacity of 0 and never gets disconnected.
///
/// # Examples
///
/// Using a `never` channel to optionally add a timeout to [`select!`]:
///
/// [`select!`]: crate::select!
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use crossbeam_channel::{after, select, never, unbounded};
///
/// let (s, r) = unbounded();
///
/// thread::spawn(move || {
///     thread::sleep(Duration::from_secs(1));
///     s.send(1).unwrap();
/// });
///
/// // Suppose this duration can be a `Some` or a `None`.
/// let duration = Some(Duration::from_millis(100));
///
/// // Create a channel that times out after the specified duration.
/// let timeout = duration
///     .map(|d| after(d))
///     .unwrap_or(never());
///
/// select! {
///     recv(r) -> msg => assert_eq!(msg, Ok(1)),
///     recv(timeout) -> _ => println!("timed out"),
/// }
/// ```
pub fn never<T>() -> Receiver<T> {
    Receiver {
        flavor: ReceiverFlavor::Never(flavors::never::Channel::new()),
    }
}

/// Creates a receiver that delivers messages periodically.
///
/// The channel is bounded with capacity of 1 and never gets disconnected. Messages will be
/// sent into the channel in intervals of `duration`. Each message is the instant at which it is
/// sent.
///
/// # Examples
///
/// Using a `tick` channel to periodically print elapsed time:
///
/// ```
/// use std::time::{Duration, Instant};
/// use crossbeam_channel::tick;
///
/// let start = Instant::now();
/// let ticker = tick(Duration::from_millis(100));
///
/// for _ in 0..5 {
///     ticker.recv().unwrap();
///     println!("elapsed: {:?}", start.elapsed());
/// }
/// ```
///
/// When messages get sent:
///
/// ```
/// use std::thread;
/// use std::time::{Duration, Instant};
/// use crossbeam_channel::tick;
///
/// // Converts a number of milliseconds into a `Duration`.
/// let ms = |ms| Duration::from_millis(ms);
///
/// // Returns `true` if `a` and `b` are very close `Instant`s.
/// let eq = |a, b| a + ms(65) > b && b + ms(65) > a;
///
/// let start = Instant::now();
/// let r = tick(ms(100));
///
/// // This message was sent 100 ms from the start and received 100 ms from the start.
/// assert!(eq(r.recv().unwrap(), start + ms(100)));
/// assert!(eq(Instant::now(), start + ms(100)));
///
/// thread::sleep(ms(500));
///
/// // This message was sent 200 ms from the start and received 600 ms from the start.
/// assert!(eq(r.recv().unwrap(), start + ms(200)));
/// assert!(eq(Instant::now(), start + ms(600)));
///
/// // This message was sent 700 ms from the start and received 700 ms from the start.
/// assert!(eq(r.recv().unwrap(), start + ms(700)));
/// assert!(eq(Instant::now(), start + ms(700)));
/// ```
pub fn tick(duration: Duration) -> Receiver<Instant> {
    match Instant::now().checked_add(duration) {
        Some(delivery_time) => Receiver {
            flavor: ReceiverFlavor::Tick(Arc::new(flavors::tick::Channel::new(
                delivery_time,
                duration,
            ))),
        },
        None => never(),
    }
}

/// The sending side of a channel.
///
/// # Examples
///
/// ```
/// use std::thread;
/// use crossbeam_channel::unbounded;
///
/// let (s1, r) = unbounded();
/// let s2 = s1.clone();
///
/// thread::spawn(move || s1.send(1).unwrap());
/// thread::spawn(move || s2.send(2).unwrap());
///
/// let msg1 = r.recv().unwrap();
/// let msg2 = r.recv().unwrap();
///
/// assert_eq!(msg1 + msg2, 3);
/// ```
pub struct Sender<T> {
    flavor: SenderFlavor<T>,
}

/// Sender flavors.
enum SenderFlavor<T> {
    /// Bounded channel based on a preallocated array.
    Array(counter::Sender<flavors::array::Channel<T>>),

    /// Unbounded channel implemented as a linked list.
    List(counter::Sender<flavors::list::Channel<T>>),

    /// Zero-capacity channel.
    Zero(counter::Sender<flavors::zero::Channel<T>>),
}

unsafe impl<T: Send> Send for Sender<T> {}
unsafe impl<T: Send> Sync for Sender<T> {}

impl<T> UnwindSafe for Sender<T> {}
impl<T> RefUnwindSafe for Sender<T> {}

impl<T> Sender<T> {
    /// Attempts to send a message into the channel without blocking.
    ///
    /// This method will either send a message into the channel immediately or return an error if
    /// the channel is full or disconnected. The returned error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will send the message only if there
    /// happens to be a receive operation on the other side of the channel at the same time.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{bounded, TrySendError};
    ///
    /// let (s, r) = bounded(1);
    ///
    /// assert_eq!(s.try_send(1), Ok(()));
    /// assert_eq!(s.try_send(2), Err(TrySendError::Full(2)));
    ///
    /// drop(r);
    /// assert_eq!(s.try_send(3), Err(TrySendError::Disconnected(3)));
    /// ```
    pub fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.try_send(msg),
            SenderFlavor::List(chan) => chan.try_send(msg),
            SenderFlavor::Zero(chan) => chan.try_send(msg),
        }
    }

    /// Blocks the current thread until a message is sent or the channel is disconnected.
    ///
    /// If the channel is full and not disconnected, this call will block until the send operation
    /// can proceed. If the channel becomes disconnected, this call will wake up and return an
    /// error. The returned error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will wait for a receive operation to
    /// appear on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{bounded, SendError};
    ///
    /// let (s, r) = bounded(1);
    /// assert_eq!(s.send(1), Ok(()));
    ///
    /// thread::spawn(move || {
    ///     assert_eq!(r.recv(), Ok(1));
    ///     thread::sleep(Duration::from_secs(1));
    ///     drop(r);
    /// });
    ///
    /// assert_eq!(s.send(2), Ok(()));
    /// assert_eq!(s.send(3), Err(SendError(3)));
    /// ```
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.send(msg, None),
            SenderFlavor::List(chan) => chan.send(msg, None),
            SenderFlavor::Zero(chan) => chan.send(msg, None),
        }
        .map_err(|err| match err {
            SendTimeoutError::Disconnected(msg) => SendError(msg),
            SendTimeoutError::Timeout(_) => unreachable!(),
        })
    }

    /// Waits for a message to be sent into the channel, but only for a limited time.
    ///
    /// If the channel is full and not disconnected, this call will block until the send operation
    /// can proceed or the operation times out. If the channel becomes disconnected, this call will
    /// wake up and return an error. The returned error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will wait for a receive operation to
    /// appear on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{bounded, SendTimeoutError};
    ///
    /// let (s, r) = bounded(0);
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     assert_eq!(r.recv(), Ok(2));
    ///     drop(r);
    /// });
    ///
    /// assert_eq!(
    ///     s.send_timeout(1, Duration::from_millis(500)),
    ///     Err(SendTimeoutError::Timeout(1)),
    /// );
    /// assert_eq!(
    ///     s.send_timeout(2, Duration::from_secs(1)),
    ///     Ok(()),
    /// );
    /// assert_eq!(
    ///     s.send_timeout(3, Duration::from_millis(500)),
    ///     Err(SendTimeoutError::Disconnected(3)),
    /// );
    /// ```
    pub fn send_timeout(&self, msg: T, timeout: Duration) -> Result<(), SendTimeoutError<T>> {
        match Instant::now().checked_add(timeout) {
            Some(deadline) => self.send_deadline(msg, deadline),
            None => self.send(msg).map_err(SendTimeoutError::from),
        }
    }

    /// Waits for a message to be sent into the channel, but only until a given deadline.
    ///
    /// If the channel is full and not disconnected, this call will block until the send operation
    /// can proceed or the operation times out. If the channel becomes disconnected, this call will
    /// wake up and return an error. The returned error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will wait for a receive operation to
    /// appear on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    /// use crossbeam_channel::{bounded, SendTimeoutError};
    ///
    /// let (s, r) = bounded(0);
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     assert_eq!(r.recv(), Ok(2));
    ///     drop(r);
    /// });
    ///
    /// let now = Instant::now();
    ///
    /// assert_eq!(
    ///     s.send_deadline(1, now + Duration::from_millis(500)),
    ///     Err(SendTimeoutError::Timeout(1)),
    /// );
    /// assert_eq!(
    ///     s.send_deadline(2, now + Duration::from_millis(1500)),
    ///     Ok(()),
    /// );
    /// assert_eq!(
    ///     s.send_deadline(3, now + Duration::from_millis(2000)),
    ///     Err(SendTimeoutError::Disconnected(3)),
    /// );
    /// ```
    pub fn send_deadline(&self, msg: T, deadline: Instant) -> Result<(), SendTimeoutError<T>> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.send(msg, Some(deadline)),
            SenderFlavor::List(chan) => chan.send(msg, Some(deadline)),
            SenderFlavor::Zero(chan) => chan.send(msg, Some(deadline)),
        }
    }

    /// Returns `true` if the channel is empty.
    ///
    /// Note: Zero-capacity channels are always empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    /// assert!(s.is_empty());
    ///
    /// s.send(0).unwrap();
    /// assert!(!s.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.is_empty(),
            SenderFlavor::List(chan) => chan.is_empty(),
            SenderFlavor::Zero(chan) => chan.is_empty(),
        }
    }

    /// Returns `true` if the channel is full.
    ///
    /// Note: Zero-capacity channels are always full.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::bounded;
    ///
    /// let (s, r) = bounded(1);
    ///
    /// assert!(!s.is_full());
    /// s.send(0).unwrap();
    /// assert!(s.is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.is_full(),
            SenderFlavor::List(chan) => chan.is_full(),
            SenderFlavor::Zero(chan) => chan.is_full(),
        }
    }

    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    /// assert_eq!(s.len(), 0);
    ///
    /// s.send(1).unwrap();
    /// s.send(2).unwrap();
    /// assert_eq!(s.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.len(),
            SenderFlavor::List(chan) => chan.len(),
            SenderFlavor::Zero(chan) => chan.len(),
        }
    }

    /// If the channel is bounded, returns its capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{bounded, unbounded};
    ///
    /// let (s, _) = unbounded::<i32>();
    /// assert_eq!(s.capacity(), None);
    ///
    /// let (s, _) = bounded::<i32>(5);
    /// assert_eq!(s.capacity(), Some(5));
    ///
    /// let (s, _) = bounded::<i32>(0);
    /// assert_eq!(s.capacity(), Some(0));
    /// ```
    pub fn capacity(&self) -> Option<usize> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.capacity(),
            SenderFlavor::List(chan) => chan.capacity(),
            SenderFlavor::Zero(chan) => chan.capacity(),
        }
    }

    /// Returns `true` if senders belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, _) = unbounded::<usize>();
    ///
    /// let s2 = s.clone();
    /// assert!(s.same_channel(&s2));
    ///
    /// let (s3, _) = unbounded();
    /// assert!(!s.same_channel(&s3));
    /// ```
    pub fn same_channel(&self, other: &Sender<T>) -> bool {
        match (&self.flavor, &other.flavor) {
            (SenderFlavor::Array(ref a), SenderFlavor::Array(ref b)) => a == b,
            (SenderFlavor::List(ref a), SenderFlavor::List(ref b)) => a == b,
            (SenderFlavor::Zero(ref a), SenderFlavor::Zero(ref b)) => a == b,
            _ => false,
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        unsafe {
            match &self.flavor {
                SenderFlavor::Array(chan) => chan.release(|c| c.disconnect()),
                SenderFlavor::List(chan) => chan.release(|c| c.disconnect_senders()),
                SenderFlavor::Zero(chan) => chan.release(|c| c.disconnect()),
            }
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let flavor = match &self.flavor {
            SenderFlavor::Array(chan) => SenderFlavor::Array(chan.acquire()),
            SenderFlavor::List(chan) => SenderFlavor::List(chan.acquire()),
            SenderFlavor::Zero(chan) => SenderFlavor::Zero(chan.acquire()),
        };

        Sender { flavor }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Sender { .. }")
    }
}

/// The receiving side of a channel.
///
/// # Examples
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use crossbeam_channel::unbounded;
///
/// let (s, r) = unbounded();
///
/// thread::spawn(move || {
///     let _ = s.send(1);
///     thread::sleep(Duration::from_secs(1));
///     let _ = s.send(2);
/// });
///
/// assert_eq!(r.recv(), Ok(1)); // Received immediately.
/// assert_eq!(r.recv(), Ok(2)); // Received after 1 second.
/// ```
pub struct Receiver<T> {
    flavor: ReceiverFlavor<T>,
}

/// Receiver flavors.
enum ReceiverFlavor<T> {
    /// Bounded channel based on a preallocated array.
    Array(counter::Receiver<flavors::array::Channel<T>>),

    /// Unbounded channel implemented as a linked list.
    List(counter::Receiver<flavors::list::Channel<T>>),

    /// Zero-capacity channel.
    Zero(counter::Receiver<flavors::zero::Channel<T>>),

    /// The after flavor.
    At(Arc<flavors::at::Channel>),

    /// The tick flavor.
    Tick(Arc<flavors::tick::Channel>),

    /// The never flavor.
    Never(flavors::never::Channel<T>),
}

unsafe impl<T: Send> Send for Receiver<T> {}
unsafe impl<T: Send> Sync for Receiver<T> {}

impl<T> UnwindSafe for Receiver<T> {}
impl<T> RefUnwindSafe for Receiver<T> {}

impl<T> Receiver<T> {
    /// Attempts to receive a message from the channel without blocking.
    ///
    /// This method will either receive a message from the channel immediately or return an error
    /// if the channel is empty.
    ///
    /// If called on a zero-capacity channel, this method will receive a message only if there
    /// happens to be a send operation on the other side of the channel at the same time.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{unbounded, TryRecvError};
    ///
    /// let (s, r) = unbounded();
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
    ///
    /// s.send(5).unwrap();
    /// drop(s);
    ///
    /// assert_eq!(r.try_recv(), Ok(5));
    /// assert_eq!(r.try_recv(), Err(TryRecvError::Disconnected));
    /// ```
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.try_recv(),
            ReceiverFlavor::List(chan) => chan.try_recv(),
            ReceiverFlavor::Zero(chan) => chan.try_recv(),
            ReceiverFlavor::At(chan) => {
                let msg = chan.try_recv();
                unsafe {
                    mem::transmute_copy::<Result<Instant, TryRecvError>, Result<T, TryRecvError>>(
                        &msg,
                    )
                }
            }
            ReceiverFlavor::Tick(chan) => {
                let msg = chan.try_recv();
                unsafe {
                    mem::transmute_copy::<Result<Instant, TryRecvError>, Result<T, TryRecvError>>(
                        &msg,
                    )
                }
            }
            ReceiverFlavor::Never(chan) => chan.try_recv(),
        }
    }

    /// Blocks the current thread until a message is received or the channel is empty and
    /// disconnected.
    ///
    /// If the channel is empty and not disconnected, this call will block until the receive
    /// operation can proceed. If the channel is empty and becomes disconnected, this call will
    /// wake up and return an error.
    ///
    /// If called on a zero-capacity channel, this method will wait for a send operation to appear
    /// on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{unbounded, RecvError};
    ///
    /// let (s, r) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s.send(5).unwrap();
    ///     drop(s);
    /// });
    ///
    /// assert_eq!(r.recv(), Ok(5));
    /// assert_eq!(r.recv(), Err(RecvError));
    /// ```
    pub fn recv(&self) -> Result<T, RecvError> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.recv(None),
            ReceiverFlavor::List(chan) => chan.recv(None),
            ReceiverFlavor::Zero(chan) => chan.recv(None),
            ReceiverFlavor::At(chan) => {
                let msg = chan.recv(None);
                unsafe {
                    mem::transmute_copy::<
                        Result<Instant, RecvTimeoutError>,
                        Result<T, RecvTimeoutError>,
                    >(&msg)
                }
            }
            ReceiverFlavor::Tick(chan) => {
                let msg = chan.recv(None);
                unsafe {
                    mem::transmute_copy::<
                        Result<Instant, RecvTimeoutError>,
                        Result<T, RecvTimeoutError>,
                    >(&msg)
                }
            }
            ReceiverFlavor::Never(chan) => chan.recv(None),
        }
        .map_err(|_| RecvError)
    }

    /// Waits for a message to be received from the channel, but only for a limited time.
    ///
    /// If the channel is empty and not disconnected, this call will block until the receive
    /// operation can proceed or the operation times out. If the channel is empty and becomes
    /// disconnected, this call will wake up and return an error.
    ///
    /// If called on a zero-capacity channel, this method will wait for a send operation to appear
    /// on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::{unbounded, RecvTimeoutError};
    ///
    /// let (s, r) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s.send(5).unwrap();
    ///     drop(s);
    /// });
    ///
    /// assert_eq!(
    ///     r.recv_timeout(Duration::from_millis(500)),
    ///     Err(RecvTimeoutError::Timeout),
    /// );
    /// assert_eq!(
    ///     r.recv_timeout(Duration::from_secs(1)),
    ///     Ok(5),
    /// );
    /// assert_eq!(
    ///     r.recv_timeout(Duration::from_secs(1)),
    ///     Err(RecvTimeoutError::Disconnected),
    /// );
    /// ```
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        match Instant::now().checked_add(timeout) {
            Some(deadline) => self.recv_deadline(deadline),
            None => self.recv().map_err(RecvTimeoutError::from),
        }
    }

    /// Waits for a message to be received from the channel, but only before a given deadline.
    ///
    /// If the channel is empty and not disconnected, this call will block until the receive
    /// operation can proceed or the operation times out. If the channel is empty and becomes
    /// disconnected, this call will wake up and return an error.
    ///
    /// If called on a zero-capacity channel, this method will wait for a send operation to appear
    /// on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::{Instant, Duration};
    /// use crossbeam_channel::{unbounded, RecvTimeoutError};
    ///
    /// let (s, r) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     s.send(5).unwrap();
    ///     drop(s);
    /// });
    ///
    /// let now = Instant::now();
    ///
    /// assert_eq!(
    ///     r.recv_deadline(now + Duration::from_millis(500)),
    ///     Err(RecvTimeoutError::Timeout),
    /// );
    /// assert_eq!(
    ///     r.recv_deadline(now + Duration::from_millis(1500)),
    ///     Ok(5),
    /// );
    /// assert_eq!(
    ///     r.recv_deadline(now + Duration::from_secs(5)),
    ///     Err(RecvTimeoutError::Disconnected),
    /// );
    /// ```
    pub fn recv_deadline(&self, deadline: Instant) -> Result<T, RecvTimeoutError> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.recv(Some(deadline)),
            ReceiverFlavor::List(chan) => chan.recv(Some(deadline)),
            ReceiverFlavor::Zero(chan) => chan.recv(Some(deadline)),
            ReceiverFlavor::At(chan) => {
                let msg = chan.recv(Some(deadline));
                unsafe {
                    mem::transmute_copy::<
                        Result<Instant, RecvTimeoutError>,
                        Result<T, RecvTimeoutError>,
                    >(&msg)
                }
            }
            ReceiverFlavor::Tick(chan) => {
                let msg = chan.recv(Some(deadline));
                unsafe {
                    mem::transmute_copy::<
                        Result<Instant, RecvTimeoutError>,
                        Result<T, RecvTimeoutError>,
                    >(&msg)
                }
            }
            ReceiverFlavor::Never(chan) => chan.recv(Some(deadline)),
        }
    }

    /// Returns `true` if the channel is empty.
    ///
    /// Note: Zero-capacity channels are always empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    ///
    /// assert!(r.is_empty());
    /// s.send(0).unwrap();
    /// assert!(!r.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.is_empty(),
            ReceiverFlavor::List(chan) => chan.is_empty(),
            ReceiverFlavor::Zero(chan) => chan.is_empty(),
            ReceiverFlavor::At(chan) => chan.is_empty(),
            ReceiverFlavor::Tick(chan) => chan.is_empty(),
            ReceiverFlavor::Never(chan) => chan.is_empty(),
        }
    }

    /// Returns `true` if the channel is full.
    ///
    /// Note: Zero-capacity channels are always full.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::bounded;
    ///
    /// let (s, r) = bounded(1);
    ///
    /// assert!(!r.is_full());
    /// s.send(0).unwrap();
    /// assert!(r.is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.is_full(),
            ReceiverFlavor::List(chan) => chan.is_full(),
            ReceiverFlavor::Zero(chan) => chan.is_full(),
            ReceiverFlavor::At(chan) => chan.is_full(),
            ReceiverFlavor::Tick(chan) => chan.is_full(),
            ReceiverFlavor::Never(chan) => chan.is_full(),
        }
    }

    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    /// assert_eq!(r.len(), 0);
    ///
    /// s.send(1).unwrap();
    /// s.send(2).unwrap();
    /// assert_eq!(r.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.len(),
            ReceiverFlavor::List(chan) => chan.len(),
            ReceiverFlavor::Zero(chan) => chan.len(),
            ReceiverFlavor::At(chan) => chan.len(),
            ReceiverFlavor::Tick(chan) => chan.len(),
            ReceiverFlavor::Never(chan) => chan.len(),
        }
    }

    /// If the channel is bounded, returns its capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_channel::{bounded, unbounded};
    ///
    /// let (_, r) = unbounded::<i32>();
    /// assert_eq!(r.capacity(), None);
    ///
    /// let (_, r) = bounded::<i32>(5);
    /// assert_eq!(r.capacity(), Some(5));
    ///
    /// let (_, r) = bounded::<i32>(0);
    /// assert_eq!(r.capacity(), Some(0));
    /// ```
    pub fn capacity(&self) -> Option<usize> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.capacity(),
            ReceiverFlavor::List(chan) => chan.capacity(),
            ReceiverFlavor::Zero(chan) => chan.capacity(),
            ReceiverFlavor::At(chan) => chan.capacity(),
            ReceiverFlavor::Tick(chan) => chan.capacity(),
            ReceiverFlavor::Never(chan) => chan.capacity(),
        }
    }

    /// A blocking iterator over messages in the channel.
    ///
    /// Each call to [`next`] blocks waiting for the next message and then returns it. However, if
    /// the channel becomes empty and disconnected, it returns [`None`] without blocking.
    ///
    /// [`next`]: Iterator::next
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded();
    ///
    /// thread::spawn(move || {
    ///     s.send(1).unwrap();
    ///     s.send(2).unwrap();
    ///     s.send(3).unwrap();
    ///     drop(s); // Disconnect the channel.
    /// });
    ///
    /// // Collect all messages from the channel.
    /// // Note that the call to `collect` blocks until the sender is dropped.
    /// let v: Vec<_> = r.iter().collect();
    ///
    /// assert_eq!(v, [1, 2, 3]);
    /// ```
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { receiver: self }
    }

    /// A non-blocking iterator over messages in the channel.
    ///
    /// Each call to [`next`] returns a message if there is one ready to be received. The iterator
    /// never blocks waiting for the next message.
    ///
    /// [`next`]: Iterator::next
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_channel::unbounded;
    ///
    /// let (s, r) = unbounded::<i32>();
    ///
    /// thread::spawn(move || {
    ///     s.send(1).unwrap();
    ///     thread::sleep(Duration::from_secs(1));
    ///     s.send(2).unwrap();
    ///     thread::sleep(Duration::from_secs(2));
    ///     s.send(3).unwrap();
    /// });
    ///
    /// thread::sleep(Duration::from_secs(2));
    ///
    /// // Collect all messages from the channel without blocking.
    /// // The third message hasn't been sent yet so we'll collect only the first two.
    /// let v: Vec<_> = r.try_iter().collect();
    ///
    /// assert_eq!(v, [1, 2]);
    /// ```
    pub fn try_iter(&self) -> TryIter<'_, T> {
        TryIter { receiver: self }
    }

    /// Returns `true` if receivers belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crossbeam_channel::unbounded;
    ///
    /// let (_, r) = unbounded::<usize>();
    ///
    /// let r2 = r.clone();
    /// assert!(r.same_channel(&r2));
    ///
    /// let (_, r3) = unbounded();
    /// assert!(!r.same_channel(&r3));
    /// ```
    pub fn same_channel(&self, other: &Receiver<T>) -> bool {
        match (&self.flavor, &other.flavor) {
            (ReceiverFlavor::Array(a), ReceiverFlavor::Array(b)) => a == b,
            (ReceiverFlavor::List(a), ReceiverFlavor::List(b)) => a == b,
            (ReceiverFlavor::Zero(a), ReceiverFlavor::Zero(b)) => a == b,
            (ReceiverFlavor::At(a), ReceiverFlavor::At(b)) => Arc::ptr_eq(a, b),
            (ReceiverFlavor::Tick(a), ReceiverFlavor::Tick(b)) => Arc::ptr_eq(a, b),
            (ReceiverFlavor::Never(_), ReceiverFlavor::Never(_)) => true,
            _ => false,
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        unsafe {
            match &self.flavor {
                ReceiverFlavor::Array(chan) => chan.release(|c| c.disconnect()),
                ReceiverFlavor::List(chan) => chan.release(|c| c.disconnect_receivers()),
                ReceiverFlavor::Zero(chan) => chan.release(|c| c.disconnect()),
                ReceiverFlavor::At(_) => {}
                ReceiverFlavor::Tick(_) => {}
                ReceiverFlavor::Never(_) => {}
            }
        }
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        let flavor = match &self.flavor {
            ReceiverFlavor::Array(chan) => ReceiverFlavor::Array(chan.acquire()),
            ReceiverFlavor::List(chan) => ReceiverFlavor::List(chan.acquire()),
            ReceiverFlavor::Zero(chan) => ReceiverFlavor::Zero(chan.acquire()),
            ReceiverFlavor::At(chan) => ReceiverFlavor::At(chan.clone()),
            ReceiverFlavor::Tick(chan) => ReceiverFlavor::Tick(chan.clone()),
            ReceiverFlavor::Never(_) => ReceiverFlavor::Never(flavors::never::Channel::new()),
        };

        Receiver { flavor }
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Receiver { .. }")
    }
}

impl<'a, T> IntoIterator for &'a Receiver<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { receiver: self }
    }
}

/// A blocking iterator over messages in a channel.
///
/// Each call to [`next`] blocks waiting for the next message and then returns it. However, if the
/// channel becomes empty and disconnected, it returns [`None`] without blocking.
///
/// [`next`]: Iterator::next
///
/// # Examples
///
/// ```
/// use std::thread;
/// use crossbeam_channel::unbounded;
///
/// let (s, r) = unbounded();
///
/// thread::spawn(move || {
///     s.send(1).unwrap();
///     s.send(2).unwrap();
///     s.send(3).unwrap();
///     drop(s); // Disconnect the channel.
/// });
///
/// // Collect all messages from the channel.
/// // Note that the call to `collect` blocks until the sender is dropped.
/// let v: Vec<_> = r.iter().collect();
///
/// assert_eq!(v, [1, 2, 3]);
/// ```
pub struct Iter<'a, T> {
    receiver: &'a Receiver<T>,
}

impl<T> FusedIterator for Iter<'_, T> {}

impl<T> Iterator for Iter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

impl<T> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Iter { .. }")
    }
}

/// A non-blocking iterator over messages in a channel.
///
/// Each call to [`next`] returns a message if there is one ready to be received. The iterator
/// never blocks waiting for the next message.
///
/// [`next`]: Iterator::next
///
/// # Examples
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use crossbeam_channel::unbounded;
///
/// let (s, r) = unbounded::<i32>();
///
/// thread::spawn(move || {
///     s.send(1).unwrap();
///     thread::sleep(Duration::from_secs(1));
///     s.send(2).unwrap();
///     thread::sleep(Duration::from_secs(2));
///     s.send(3).unwrap();
/// });
///
/// thread::sleep(Duration::from_secs(2));
///
/// // Collect all messages from the channel without blocking.
/// // The third message hasn't been sent yet so we'll collect only the first two.
/// let v: Vec<_> = r.try_iter().collect();
///
/// assert_eq!(v, [1, 2]);
/// ```
pub struct TryIter<'a, T> {
    receiver: &'a Receiver<T>,
}

impl<T> Iterator for TryIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.try_recv().ok()
    }
}

impl<T> fmt::Debug for TryIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("TryIter { .. }")
    }
}

/// A blocking iterator over messages in a channel.
///
/// Each call to [`next`] blocks waiting for the next message and then returns it. However, if the
/// channel becomes empty and disconnected, it returns [`None`] without blocking.
///
/// [`next`]: Iterator::next
///
/// # Examples
///
/// ```
/// use std::thread;
/// use crossbeam_channel::unbounded;
///
/// let (s, r) = unbounded();
///
/// thread::spawn(move || {
///     s.send(1).unwrap();
///     s.send(2).unwrap();
///     s.send(3).unwrap();
///     drop(s); // Disconnect the channel.
/// });
///
/// // Collect all messages from the channel.
/// // Note that the call to `collect` blocks until the sender is dropped.
/// let v: Vec<_> = r.into_iter().collect();
///
/// assert_eq!(v, [1, 2, 3]);
/// ```
pub struct IntoIter<T> {
    receiver: Receiver<T>,
}

impl<T> FusedIterator for IntoIter<T> {}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

impl<T> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("IntoIter { .. }")
    }
}

impl<T> SelectHandle for Sender<T> {
    fn try_select(&self, token: &mut Token) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().try_select(token),
            SenderFlavor::List(chan) => chan.sender().try_select(token),
            SenderFlavor::Zero(chan) => chan.sender().try_select(token),
        }
    }

    fn deadline(&self) -> Option<Instant> {
        None
    }

    fn register(&self, oper: Operation, cx: &Context) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().register(oper, cx),
            SenderFlavor::List(chan) => chan.sender().register(oper, cx),
            SenderFlavor::Zero(chan) => chan.sender().register(oper, cx),
        }
    }

    fn unregister(&self, oper: Operation) {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().unregister(oper),
            SenderFlavor::List(chan) => chan.sender().unregister(oper),
            SenderFlavor::Zero(chan) => chan.sender().unregister(oper),
        }
    }

    fn accept(&self, token: &mut Token, cx: &Context) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().accept(token, cx),
            SenderFlavor::List(chan) => chan.sender().accept(token, cx),
            SenderFlavor::Zero(chan) => chan.sender().accept(token, cx),
        }
    }

    fn is_ready(&self) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().is_ready(),
            SenderFlavor::List(chan) => chan.sender().is_ready(),
            SenderFlavor::Zero(chan) => chan.sender().is_ready(),
        }
    }

    fn watch(&self, oper: Operation, cx: &Context) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().watch(oper, cx),
            SenderFlavor::List(chan) => chan.sender().watch(oper, cx),
            SenderFlavor::Zero(chan) => chan.sender().watch(oper, cx),
        }
    }

    fn unwatch(&self, oper: Operation) {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.sender().unwatch(oper),
            SenderFlavor::List(chan) => chan.sender().unwatch(oper),
            SenderFlavor::Zero(chan) => chan.sender().unwatch(oper),
        }
    }
}

impl<T> SelectHandle for Receiver<T> {
    fn try_select(&self, token: &mut Token) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().try_select(token),
            ReceiverFlavor::List(chan) => chan.receiver().try_select(token),
            ReceiverFlavor::Zero(chan) => chan.receiver().try_select(token),
            ReceiverFlavor::At(chan) => chan.try_select(token),
            ReceiverFlavor::Tick(chan) => chan.try_select(token),
            ReceiverFlavor::Never(chan) => chan.try_select(token),
        }
    }

    fn deadline(&self) -> Option<Instant> {
        match &self.flavor {
            ReceiverFlavor::Array(_) => None,
            ReceiverFlavor::List(_) => None,
            ReceiverFlavor::Zero(_) => None,
            ReceiverFlavor::At(chan) => chan.deadline(),
            ReceiverFlavor::Tick(chan) => chan.deadline(),
            ReceiverFlavor::Never(chan) => chan.deadline(),
        }
    }

    fn register(&self, oper: Operation, cx: &Context) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().register(oper, cx),
            ReceiverFlavor::List(chan) => chan.receiver().register(oper, cx),
            ReceiverFlavor::Zero(chan) => chan.receiver().register(oper, cx),
            ReceiverFlavor::At(chan) => chan.register(oper, cx),
            ReceiverFlavor::Tick(chan) => chan.register(oper, cx),
            ReceiverFlavor::Never(chan) => chan.register(oper, cx),
        }
    }

    fn unregister(&self, oper: Operation) {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().unregister(oper),
            ReceiverFlavor::List(chan) => chan.receiver().unregister(oper),
            ReceiverFlavor::Zero(chan) => chan.receiver().unregister(oper),
            ReceiverFlavor::At(chan) => chan.unregister(oper),
            ReceiverFlavor::Tick(chan) => chan.unregister(oper),
            ReceiverFlavor::Never(chan) => chan.unregister(oper),
        }
    }

    fn accept(&self, token: &mut Token, cx: &Context) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().accept(token, cx),
            ReceiverFlavor::List(chan) => chan.receiver().accept(token, cx),
            ReceiverFlavor::Zero(chan) => chan.receiver().accept(token, cx),
            ReceiverFlavor::At(chan) => chan.accept(token, cx),
            ReceiverFlavor::Tick(chan) => chan.accept(token, cx),
            ReceiverFlavor::Never(chan) => chan.accept(token, cx),
        }
    }

    fn is_ready(&self) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().is_ready(),
            ReceiverFlavor::List(chan) => chan.receiver().is_ready(),
            ReceiverFlavor::Zero(chan) => chan.receiver().is_ready(),
            ReceiverFlavor::At(chan) => chan.is_ready(),
            ReceiverFlavor::Tick(chan) => chan.is_ready(),
            ReceiverFlavor::Never(chan) => chan.is_ready(),
        }
    }

    fn watch(&self, oper: Operation, cx: &Context) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().watch(oper, cx),
            ReceiverFlavor::List(chan) => chan.receiver().watch(oper, cx),
            ReceiverFlavor::Zero(chan) => chan.receiver().watch(oper, cx),
            ReceiverFlavor::At(chan) => chan.watch(oper, cx),
            ReceiverFlavor::Tick(chan) => chan.watch(oper, cx),
            ReceiverFlavor::Never(chan) => chan.watch(oper, cx),
        }
    }

    fn unwatch(&self, oper: Operation) {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.receiver().unwatch(oper),
            ReceiverFlavor::List(chan) => chan.receiver().unwatch(oper),
            ReceiverFlavor::Zero(chan) => chan.receiver().unwatch(oper),
            ReceiverFlavor::At(chan) => chan.unwatch(oper),
            ReceiverFlavor::Tick(chan) => chan.unwatch(oper),
            ReceiverFlavor::Never(chan) => chan.unwatch(oper),
        }
    }
}

/// Writes a message into the channel.
pub(crate) unsafe fn write<T>(s: &Sender<T>, token: &mut Token, msg: T) -> Result<(), T> {
    match &s.flavor {
        SenderFlavor::Array(chan) => chan.write(token, msg),
        SenderFlavor::List(chan) => chan.write(token, msg),
        SenderFlavor::Zero(chan) => chan.write(token, msg),
    }
}

/// Reads a message from the channel.
pub(crate) unsafe fn read<T>(r: &Receiver<T>, token: &mut Token) -> Result<T, ()> {
    match &r.flavor {
        ReceiverFlavor::Array(chan) => chan.read(token),
        ReceiverFlavor::List(chan) => chan.read(token),
        ReceiverFlavor::Zero(chan) => chan.read(token),
        ReceiverFlavor::At(chan) => {
            mem::transmute_copy::<Result<Instant, ()>, Result<T, ()>>(&chan.read(token))
        }
        ReceiverFlavor::Tick(chan) => {
            mem::transmute_copy::<Result<Instant, ()>, Result<T, ()>>(&chan.read(token))
        }
        ReceiverFlavor::Never(chan) => chan.read(token),
    }
}
