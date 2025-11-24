use crate::{sys, Registry, Token};

use std::io;

/// Waker allows cross-thread waking of [`Poll`].
///
/// When created it will cause events with [`readable`] readiness and the
/// provided `token` if [`wake`] is called, possibly from another thread.
///
/// [`Poll`]: struct.Poll.html
/// [`readable`]: ./event/struct.Event.html#method.is_readable
/// [`wake`]: struct.Waker.html#method.wake
///
/// # Notes
///
/// `Waker` events are only guaranteed to be delivered while the `Waker` value
/// is alive.
///
/// Only a single `Waker` can be active per [`Poll`], if multiple threads need
/// access to the `Waker` it can be shared via for example an `Arc`. What
/// happens if multiple `Waker`s are registered with the same `Poll` is
/// unspecified.
///
/// # Implementation notes
///
/// On platforms that support kqueue this will use the `EVFILT_USER` event
/// filter, see [implementation notes of `Poll`] to see what platforms support
/// kqueue. On Linux it uses [eventfd].
///
/// [implementation notes of `Poll`]: struct.Poll.html#implementation-notes
/// [eventfd]: https://man7.org/linux/man-pages/man2/eventfd.2.html
///
/// # Examples
///
/// Wake a [`Poll`] instance from another thread.
///
#[cfg_attr(feature = "os-poll", doc = "```")]
#[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::thread;
/// use std::time::Duration;
/// use std::sync::Arc;
///
/// use mio::{Events, Token, Poll, Waker};
///
/// const WAKE_TOKEN: Token = Token(10);
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(2);
///
/// let waker = Arc::new(Waker::new(poll.registry(), WAKE_TOKEN)?);
///
/// // We need to keep the Waker alive, so we'll create a clone for the
/// // thread we create below.
/// let waker1 = waker.clone();
/// let handle = thread::spawn(move || {
///     // Working hard, or hardly working?
///     thread::sleep(Duration::from_millis(500));
///
///     // Now we'll wake the queue on the other thread.
///     waker1.wake().expect("unable to wake");
/// });
///
/// // On our current thread we'll poll for events, without a timeout.
/// poll.poll(&mut events, None)?;
///
/// // After about 500 milliseconds we should be awoken by the other thread and
/// // get a single event.
/// assert!(!events.is_empty());
/// let waker_event = events.iter().next().unwrap();
/// assert!(waker_event.is_readable());
/// assert_eq!(waker_event.token(), WAKE_TOKEN);
/// # handle.join().unwrap();
/// #     Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Waker {
    inner: sys::Waker,
}

impl Waker {
    /// Create a new `Waker`.
    pub fn new(registry: &Registry, token: Token) -> io::Result<Waker> {
        #[cfg(debug_assertions)]
        registry.register_waker();
        sys::Waker::new(registry.selector(), token).map(|inner| Waker { inner })
    }

    /// Wake up the [`Poll`] associated with this `Waker`.
    ///
    /// [`Poll`]: struct.Poll.html
    pub fn wake(&self) -> io::Result<()> {
        self.inner.wake()
    }
}
