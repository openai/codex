//! An iterator over incoming signals.
//!
//! This provides a higher abstraction over the signals, providing
//! the [`SignalsInfo`] structure which is able to iterate over the
//! incoming signals. The structure is parametrized by an
//! [`Exfiltrator`][self::exfiltrator::Exfiltrator], which specifies what information is returned
//! for each delivered signal. Note that some exfiltrators are behind a feature flag.
//!
//! The [`Signals`] is a type alias for the common case when it is enough to get the signal number.
//!
//! This module (and everything in it) is turned by the `iterator` feature. It is **on** by
//! default, the possibility to turn off is mostly possible for very special purposes (compiling on
//! `<rustc-1.36`, minimizing the amount of code compiled, …). In a sense, this is the highest
//! level abstraction of the crate and the API expected to be used by most of the people.
//!
//! # Examples
//!
//! ```rust
//! extern crate libc;
//! extern crate signal_hook;
//!
//! use std::io::Error;
//!
//! use signal_hook::consts::signal::*;
//! use signal_hook::iterator::Signals;
//!
//! fn main() -> Result<(), Error> {
//!     let mut signals = Signals::new(&[
//!         SIGHUP,
//!         SIGTERM,
//!         SIGINT,
//!         SIGQUIT,
//! #       SIGUSR1,
//!     ])?;
//! #   // A trick to terminate the example when run as doc-test. Not part of the real code.
//! #   signal_hook::low_level::raise(SIGUSR1).unwrap();
//!     'outer: loop {
//!         // Pick up signals that arrived since last time
//!         for signal in signals.pending() {
//!             match signal as libc::c_int {
//!                 SIGHUP => {
//!                     // Reload configuration
//!                     // Reopen the log file
//!                 }
//!                 SIGTERM | SIGINT | SIGQUIT => {
//!                     break 'outer;
//!                 },
//! #               SIGUSR1 => return Ok(()),
//!                 _ => unreachable!(),
//!             }
//!         }
//!         // Do some bit of work ‒ something with upper limit on waiting, so we don't block
//!         // forever with a SIGTERM already waiting.
//!     }
//!     println!("Terminating. Bye bye");
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod exfiltrator;

use std::borrow::Borrow;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::{Error, ErrorKind, Read};
use std::os::unix::net::UnixStream;

use libc::{self, c_int};

pub use self::backend::{Handle, Pending};
use self::backend::{PollResult, RefSignalIterator, SignalDelivery};
use self::exfiltrator::{Exfiltrator, SignalOnly};

/// The main structure of the module, representing interest in some signals.
///
/// Unlike the helpers in other modules, this registers the signals when created and unregisters
/// them on drop. It provides the pending signals during its lifetime, either in batches or as an
/// infinite iterator.
///
/// Most users will want to use it through the [`Signals`] type alias for simplicity.
///
/// # Multiple threads
///
/// Instances of this struct can be [sent][std::marker::Send] to other threads. In a multithreaded
/// application this can be used to dedicate a separate thread for signal handling. In this case
/// you should get a [`Handle`] using the [`handle`][Signals::handle] method before sending the
/// `Signals` instance to a background thread. With the handle you will be able to shut down the
/// background thread later, or to operatively add more signals.
///
/// The controller handle can be shared between as many threads as you like using its
/// [`clone`][Handle::clone] method.
///
/// # Exfiltrators
///
/// The [`SignalOnly`] provides only the signal number. There are further exfiltrators available in
/// the [`exfiltrator`] module. Note that some of them are behind feature flags that need to be
/// enabled.
///
/// # Examples
///
/// ```rust
/// # extern crate signal_hook;
/// #
/// # use std::io::Error;
/// # use std::thread;
/// use signal_hook::consts::signal::*;
/// use signal_hook::iterator::Signals;
///
/// #
/// # fn main() -> Result<(), Error> {
/// let mut signals = Signals::new(&[SIGUSR1, SIGUSR2])?;
/// let handle = signals.handle();
/// let thread = thread::spawn(move || {
///     for signal in &mut signals {
///         match signal {
///             SIGUSR1 => {},
///             SIGUSR2 => {},
///             _ => unreachable!(),
///         }
///     }
/// });
///
/// // Some time later...
/// handle.close();
/// thread.join().unwrap();
/// # Ok(())
/// # }
/// ```
pub struct SignalsInfo<E: Exfiltrator = SignalOnly>(SignalDelivery<UnixStream, E>);

impl<E: Exfiltrator> SignalsInfo<E> {
    /// Creates the `Signals` structure.
    ///
    /// This registers all the signals listed. The same restrictions (panics, errors) apply as
    /// for the [`Handle::add_signal`] method.
    pub fn new<I, S>(signals: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: Borrow<c_int>,
        E: Default,
    {
        Self::with_exfiltrator(signals, E::default())
    }

    /// An advanced constructor with explicit [`Exfiltrator`].
    pub fn with_exfiltrator<I, S>(signals: I, exfiltrator: E) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: Borrow<c_int>,
    {
        let (read, write) = UnixStream::pair()?;
        Ok(SignalsInfo(SignalDelivery::with_pipe(
            read,
            write,
            exfiltrator,
            signals,
        )?))
    }

    /// Registers another signal to the set watched by this [`Signals`] instance.
    ///
    /// The same restrictions (panics, errors) apply as for the [`Handle::add_signal`]
    /// method.
    pub fn add_signal(&self, signal: c_int) -> Result<(), Error> {
        self.handle().add_signal(signal)
    }

    /// Returns an iterator of already received signals.
    ///
    /// This returns an iterator over all the signal numbers of the signals received since last
    /// time they were read (out of the set registered by this `Signals` instance). Note that they
    /// are returned in arbitrary order and a signal instance may returned only once even if it was
    /// received multiple times.
    ///
    /// This method returns immediately (does not block) and may produce an empty iterator if there
    /// are no signals ready.
    pub fn pending(&mut self) -> Pending<E> {
        self.0.pending()
    }

    /// Block until the stream contains some bytes.
    ///
    /// Returns true if it was possible to read a byte and false otherwise.
    fn has_signals(read: &mut UnixStream) -> Result<bool, Error> {
        loop {
            match read.read(&mut [0u8]) {
                Ok(num_read) => break Ok(num_read > 0),
                // If we get an EINTR error it is fine to retry reading from the stream.
                // Otherwise we should pass on the error to the caller.
                Err(error) => {
                    if error.kind() != ErrorKind::Interrupted {
                        break Err(error);
                    }
                }
            }
        }
    }

    /// Waits for some signals to be available and returns an iterator.
    ///
    /// This is similar to [`pending`][SignalsInfo::pending]. If there are no signals available, it
    /// tries to wait for some to arrive. However, due to implementation details, this still can
    /// produce an empty iterator.
    ///
    /// This can block for arbitrary long time. If the [`Handle::close`] method is used in
    /// another thread this method will return immediately.
    ///
    /// Note that the blocking is done in this method, not in the iterator.
    pub fn wait(&mut self) -> Pending<E> {
        match self.0.poll_pending(&mut Self::has_signals) {
            Ok(Some(pending)) => pending,
            // Because of the blocking has_signals method the poll_pending method
            // only returns None if the instance is closed. But we want to return
            // a possibly empty pending object anyway.
            Ok(None) => self.pending(),
            // Users can't manipulate the internal file descriptors and the way we use them
            // shouldn't produce any errors. So it is OK to panic.
            Err(error) => panic!("Unexpected error: {}", error),
        }
    }

    /// Is it closed?
    ///
    /// See [`close`][Handle::close].
    pub fn is_closed(&self) -> bool {
        self.handle().is_closed()
    }

    /// Get an infinite iterator over arriving signals.
    ///
    /// The iterator's `next()` blocks as necessary to wait for signals to arrive. This is adequate
    /// if you want to designate a thread solely to handling signals. If multiple signals come at
    /// the same time (between two values produced by the iterator), they will be returned in
    /// arbitrary order. Multiple instances of the same signal may be collated.
    ///
    /// This is also the iterator returned by `IntoIterator` implementation on `&mut Signals`.
    ///
    /// This iterator terminates only if explicitly [closed][Handle::close].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate libc;
    /// # extern crate signal_hook;
    /// #
    /// # use std::io::Error;
    /// # use std::thread;
    /// #
    /// use signal_hook::consts::signal::*;
    /// use signal_hook::iterator::Signals;
    ///
    /// # fn main() -> Result<(), Error> {
    /// let mut signals = Signals::new(&[SIGUSR1, SIGUSR2])?;
    /// let handle = signals.handle();
    /// thread::spawn(move || {
    ///     for signal in signals.forever() {
    ///         match signal {
    ///             SIGUSR1 => {},
    ///             SIGUSR2 => {},
    ///             _ => unreachable!(),
    ///         }
    ///     }
    /// });
    /// handle.close();
    /// # Ok(())
    /// # }
    /// ```
    pub fn forever(&mut self) -> Forever<E> {
        Forever(RefSignalIterator::new(&mut self.0))
    }

    /// Get a shareable handle to a [`Handle`] for this instance.
    ///
    /// This can be used to add further signals or close the [`Signals`] instance.
    pub fn handle(&self) -> Handle {
        self.0.handle()
    }
}

impl<E> Debug for SignalsInfo<E>
where
    E: Debug + Exfiltrator,
    E::Storage: Debug,
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        fmt.debug_tuple("Signals").field(&self.0).finish()
    }
}

impl<'a, E: Exfiltrator> IntoIterator for &'a mut SignalsInfo<E> {
    type Item = E::Output;
    type IntoIter = Forever<'a, E>;
    fn into_iter(self) -> Self::IntoIter {
        self.forever()
    }
}

/// An infinite iterator of arriving signals.
pub struct Forever<'a, E: Exfiltrator>(RefSignalIterator<'a, UnixStream, E>);

impl<'a, E: Exfiltrator> Iterator for Forever<'a, E> {
    type Item = E::Output;

    fn next(&mut self) -> Option<E::Output> {
        loop {
            match self.0.poll_signal(&mut SignalsInfo::<E>::has_signals) {
                PollResult::Signal(result) => break Some(result),
                PollResult::Closed => break None,
                // In theory, the poll_signal should not return PollResult::Pending. Nevertheless,
                // there's a race condition - if the other side closes the pipe/socket after
                // checking for it being closed, then the `read` there returns 0 as EOF. That
                // appears as pending here. Next time we should get Closed.
                PollResult::Pending => continue,
                // Users can't manipulate the internal file descriptors and the way we use them
                // shouldn't produce any errors. So it is OK to panic.
                PollResult::Err(error) => panic!("Unexpected error: {}", error),
            }
        }
    }
}

/// A type alias for an iterator returning just the signal numbers.
///
/// This is the simplified version for most of the use cases. For advanced usages, the
/// [`SignalsInfo`] with explicit [`Exfiltrator`] type can be used.
pub type Signals = SignalsInfo<SignalOnly>;
