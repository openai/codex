#![doc(test(attr(deny(warnings))))]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! A crate offering integration with the MIO runtime.
//!
//! There are different sub modules for supporting different MIO
//! versions. The support for a version must be activated by a
//! feature flag:
//!
//! * `support-v0_6` for sub module [`v0_6`]
//! * `support-v0_7` for sub module [`v0_7`]
//! * `support-v0_8` for sub module [`v0_8`]
//! * `support-v0_8` for sub module [`v1_0`]
//!
//! See the specific sub modules for usage examples.

#[cfg(any(
    feature = "support-v0_6",
    feature = "support-v0_7",
    feature = "support-v0_8",
    feature = "support-v1_0"
))]
macro_rules! implement_signals_with_pipe {
    ($pipe:path) => {
        use std::borrow::Borrow;
        use std::io::Error;

        use signal_hook::iterator::backend::{self, SignalDelivery};
        use signal_hook::iterator::exfiltrator::{Exfiltrator, SignalOnly};

        use $pipe as Pipe;

        use libc::c_int;

        /// A struct which mimics [`signal_hook::iterator::SignalsInfo`]
        /// but also allows usage together with MIO runtime.
        pub struct SignalsInfo<E: Exfiltrator = SignalOnly>(SignalDelivery<Pipe, E>);

        pub use backend::Pending;

        impl<E: Exfiltrator> SignalsInfo<E> {
            /// Create a `Signals` instance.
            ///
            /// This registers all the signals listed. The same restrictions (panics, errors) apply
            /// as with [`Handle::add_signal`][backend::Handle::add_signal].
            pub fn new<I, S>(signals: I) -> Result<Self, Error>
            where
                I: IntoIterator<Item = S>,
                S: Borrow<c_int>,
                E: Default,
            {
                Self::with_exfiltrator(signals, E::default())
            }

            /// A constructor with specifying an exfiltrator to pass information out of the signal
            /// handlers.
            pub fn with_exfiltrator<I, S>(signals: I, exfiltrator: E) -> Result<Self, Error>
            where
                I: IntoIterator<Item = S>,
                S: Borrow<c_int>,
            {
                let (read, write) = Pipe::pair()?;
                let delivery = SignalDelivery::with_pipe(read, write, exfiltrator, signals)?;
                Ok(Self(delivery))
            }

            /// Registers another signal to the set watched by this [`Signals`] instance.
            ///
            /// The same restrictions (panics, errors) apply as with
            /// [`Handle::add_signal`][backend::Handle::add_signal].
            pub fn add_signal(&self, signal: c_int) -> Result<(), Error> {
                self.0.handle().add_signal(signal)
            }

            /// Returns an iterator of already received signals.
            ///
            /// This returns an iterator over all the signal numbers of the signals received since last
            /// time they were read (out of the set registered by this `Signals` instance). Note that they
            /// are returned in arbitrary order and a signal number is returned only once even if it was
            /// received multiple times.
            ///
            /// This method returns immediately (does not block) and may produce an empty iterator if there
            /// are no signals ready. So you should register an instance of this struct at an instance of
            /// [`mio::Poll`] to query for readability of the underlying self pipe.
            pub fn pending(&mut self) -> Pending<E> {
                self.0.pending()
            }
        }

        /// A simplified signal iterator.
        ///
        /// This is the [`SignalsInfo`], but returning only the signal numbers. This is likely the
        /// one you want to use.
        pub type Signals = SignalsInfo<SignalOnly>;
    };
}

/// A module for integrating signal handling with the MIO 1.0 runtime.
///
/// This provides the [`Signals`][v1_0::Signals] struct as an abstraction
/// which can be used with [`mio::Poll`][mio_1_0::Poll].
///
/// # Examples
///
/// ```rust
/// # use mio_1_0 as mio;
/// use std::io::{Error, ErrorKind};
///
/// use signal_hook::consts::signal::*;
/// use signal_hook_mio::v1_0::Signals;
///
/// use mio::{Events, Poll, Interest, Token};
///
/// fn main() -> Result<(), Error> {
///     let mut poll = Poll::new()?;
///
///     let mut signals = Signals::new(&[
///         SIGTERM,
/// #       SIGUSR1,
///     ])?;
///
///     let signal_token = Token(0);
///
///     poll.registry().register(&mut signals, signal_token, Interest::READABLE)?;
/// #   signal_hook::low_level::raise(SIGUSR1).unwrap(); // Just for terminating the example
///
///     let mut events = Events::with_capacity(10);
///     'outer: loop {
///         poll.poll(&mut events, None)
///             .or_else(|e| if e.kind() == ErrorKind::Interrupted {
///                 // We get interrupt when a signal happens inside poll. That's non-fatal, just
///                 // retry.
///                 events.clear();
///                 Ok(())
///             } else {
///                 Err(e)
///             })?;
///         for event in events.iter() {
///             match event.token() {
///                 Token(0) => {
///                     for signal in signals.pending() {
///                         match signal {
///                             SIGTERM => break 'outer,
/// #                           SIGUSR1 => return Ok(()),
///                             _ => unreachable!(),
///                         }
///                     }
///                 },
///                 _ => unreachable!("Register other sources and match for their tokens here"),
///             }
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "support-v1_0")]
pub mod v1_0 {
    use mio::event::Source;
    use mio::{Interest, Registry, Token};
    use mio_1_0 as mio;

    implement_signals_with_pipe!(mio::net::UnixStream);

    impl Source for Signals {
        fn register(
            &mut self,
            registry: &Registry,
            token: Token,
            interest: Interest,
        ) -> Result<(), Error> {
            self.0.get_read_mut().register(registry, token, interest)
        }

        fn reregister(
            &mut self,
            registry: &Registry,
            token: Token,
            interest: Interest,
        ) -> Result<(), Error> {
            self.0.get_read_mut().reregister(registry, token, interest)
        }

        fn deregister(&mut self, registry: &Registry) -> Result<(), Error> {
            self.0.get_read_mut().deregister(registry)
        }
    }
}

/// A module for integrating signal handling with the MIO 0.8 runtime.
///
/// This provides the [`Signals`][v0_8::Signals] struct as an abstraction
/// which can be used with [`mio::Poll`][mio_0_8::Poll].
///
/// # Examples
///
/// ```rust
/// # use mio_0_8 as mio;
/// use std::io::{Error, ErrorKind};
///
/// use signal_hook::consts::signal::*;
/// use signal_hook_mio::v0_8::Signals;
///
/// use mio::{Events, Poll, Interest, Token};
///
/// fn main() -> Result<(), Error> {
///     let mut poll = Poll::new()?;
///
///     let mut signals = Signals::new(&[
///         SIGTERM,
/// #       SIGUSR1,
///     ])?;
///
///     let signal_token = Token(0);
///
///     poll.registry().register(&mut signals, signal_token, Interest::READABLE)?;
/// #   signal_hook::low_level::raise(SIGUSR1).unwrap(); // Just for terminating the example
///
///     let mut events = Events::with_capacity(10);
///     'outer: loop {
///         poll.poll(&mut events, None)
///             .or_else(|e| if e.kind() == ErrorKind::Interrupted {
///                 // We get interrupt when a signal happens inside poll. That's non-fatal, just
///                 // retry.
///                 events.clear();
///                 Ok(())
///             } else {
///                 Err(e)
///             })?;
///         for event in events.iter() {
///             match event.token() {
///                 Token(0) => {
///                     for signal in signals.pending() {
///                         match signal {
///                             SIGTERM => break 'outer,
/// #                           SIGUSR1 => return Ok(()),
///                             _ => unreachable!(),
///                         }
///                     }
///                 },
///                 _ => unreachable!("Register other sources and match for their tokens here"),
///             }
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "support-v0_8")]
pub mod v0_8 {
    use mio::event::Source;
    use mio::{Interest, Registry, Token};
    use mio_0_8 as mio;

    implement_signals_with_pipe!(mio::net::UnixStream);

    impl Source for Signals {
        fn register(
            &mut self,
            registry: &Registry,
            token: Token,
            interest: Interest,
        ) -> Result<(), Error> {
            self.0.get_read_mut().register(registry, token, interest)
        }

        fn reregister(
            &mut self,
            registry: &Registry,
            token: Token,
            interest: Interest,
        ) -> Result<(), Error> {
            self.0.get_read_mut().reregister(registry, token, interest)
        }

        fn deregister(&mut self, registry: &Registry) -> Result<(), Error> {
            self.0.get_read_mut().deregister(registry)
        }
    }
}

/// A module for integrating signal handling with the MIO 0.7 runtime.
///
/// This provides the [`Signals`][v0_7::Signals] struct as an abstraction
/// which can be used with [`mio::Poll`][mio_0_7::Poll].
///
/// # Examples
///
/// ```rust
/// # use mio_0_7 as mio;
/// use std::io::{Error, ErrorKind};
///
/// use signal_hook::consts::signal::*;
/// use signal_hook_mio::v0_7::Signals;
///
/// use mio::{Events, Poll, Interest, Token};
///
/// fn main() -> Result<(), Error> {
///     let mut poll = Poll::new()?;
///
///     let mut signals = Signals::new(&[
///         SIGTERM,
/// #       SIGUSR1,
///     ])?;
///
///     let signal_token = Token(0);
///
///     poll.registry().register(&mut signals, signal_token, Interest::READABLE)?;
/// #   signal_hook::low_level::raise(SIGUSR1).unwrap(); // Just for terminating the example
///
///     let mut events = Events::with_capacity(10);
///     'outer: loop {
///         poll.poll(&mut events, None)
///             .or_else(|e| if e.kind() == ErrorKind::Interrupted {
///                 // We get interrupt when a signal happens inside poll. That's non-fatal, just
///                 // retry.
///                 events.clear();
///                 Ok(())
///             } else {
///                 Err(e)
///             })?;
///         for event in events.iter() {
///             match event.token() {
///                 Token(0) => {
///                     for signal in signals.pending() {
///                         match signal {
///                             SIGTERM => break 'outer,
/// #                           SIGUSR1 => return Ok(()),
///                             _ => unreachable!(),
///                         }
///                     }
///                 },
///                 _ => unreachable!("Register other sources and match for their tokens here"),
///             }
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "support-v0_7")]
pub mod v0_7 {
    use mio::event::Source;
    use mio::{Interest, Registry, Token};
    use mio_0_7 as mio;

    implement_signals_with_pipe!(mio::net::UnixStream);

    impl Source for Signals {
        fn register(
            &mut self,
            registry: &Registry,
            token: Token,
            interest: Interest,
        ) -> Result<(), Error> {
            self.0.get_read_mut().register(registry, token, interest)
        }

        fn reregister(
            &mut self,
            registry: &Registry,
            token: Token,
            interest: Interest,
        ) -> Result<(), Error> {
            self.0.get_read_mut().reregister(registry, token, interest)
        }

        fn deregister(&mut self, registry: &Registry) -> Result<(), Error> {
            self.0.get_read_mut().deregister(registry)
        }
    }
}

/// A module for integrating signal handling with the MIO 0.6 runtime.
///
/// This provides the [`Signals`][v0_6::Signals] struct as an abstraction
/// which can be used with [`mio::Poll`][mio_0_6::Poll].
///
/// # Examples
///
/// ```rust
/// # use mio_0_6 as mio;
/// use std::io::{Error, ErrorKind};
///
/// use signal_hook::consts::signal::*;
/// use signal_hook_mio::v0_6::Signals;
///
/// use mio::{Events, Poll, PollOpt, Ready, Token};
///
/// fn main() -> Result<(), Error> {
///     let poll = Poll::new()?;
///
///     let mut signals = Signals::new(&[
///         SIGTERM,
/// #       SIGUSR1,
///     ])?;
///
///     let signal_token = Token(0);
///
///     poll.register(&mut signals, signal_token, Ready::readable(), PollOpt::level())?;
/// #   signal_hook::low_level::raise(SIGUSR1).unwrap(); // Just for terminating the example
///
///     let mut events = Events::with_capacity(10);
///     'outer: loop {
///         poll.poll(&mut events, None)
///             .or_else(|e| if e.kind() == ErrorKind::Interrupted {
///                 // We get interrupt when a signal happens inside poll. That's non-fatal, just
///                 // retry.
///                 events.clear();
///                 Ok(0)
///             } else {
///                 Err(e)
///             })?;
///         for event in events.iter() {
///             match event.token() {
///                 Token(0) => {
///                     for signal in signals.pending() {
///                         match signal {
///                             SIGTERM => break 'outer,
/// #                           SIGUSR1 => return Ok(()),
///                             _ => unreachable!(),
///                         }
///                     }
///                 },
///                 _ => unreachable!("Register other sources and match for their tokens here"),
///             }
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "support-v0_6")]
pub mod v0_6 {

    use mio::event::Evented;
    use mio::{Poll, PollOpt, Ready, Token};
    use mio_0_6 as mio;

    implement_signals_with_pipe!(mio_uds::UnixStream);

    impl Evented for Signals {
        fn register(
            &self,
            poll: &Poll,
            token: Token,
            interest: Ready,
            opts: PollOpt,
        ) -> Result<(), Error> {
            self.0.get_read().register(poll, token, interest, opts)
        }

        fn reregister(
            &self,
            poll: &Poll,
            token: Token,
            interest: Ready,
            opts: PollOpt,
        ) -> Result<(), Error> {
            self.0.get_read().reregister(poll, token, interest, opts)
        }

        fn deregister(&self, poll: &Poll) -> Result<(), Error> {
            self.0.get_read().deregister(poll)
        }
    }
}
