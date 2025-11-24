#![deny(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unused_imports,
    dead_code
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Disallow warnings when running tests.
#![cfg_attr(test, deny(warnings))]
// Disallow warnings in examples.
#![doc(test(attr(deny(warnings))))]

//! Mio is a fast, low-level I/O library for Rust focusing on non-blocking APIs
//! and event notification for building high performance I/O apps with as little
//! overhead as possible over the OS abstractions.
//!
//! # Usage
//!
//! Using Mio starts by creating a [`Poll`], which reads events from the OS and
//! puts them into [`Events`]. You can handle I/O events from the OS with it.
//!
//! For more detail, see [`Poll`].
//!
//! [`Poll`]: ../mio/struct.Poll.html
//! [`Events`]: ../mio/event/struct.Events.html
//!
//! ## Examples
//!
//! Examples can found in the `examples` directory of the source code, or [on
//! GitHub].
//!
//! [on GitHub]: https://github.com/tokio-rs/mio/tree/master/examples
//!
//! ## Guide
//!
//! A getting started guide is available in the [`guide`] module.
//!
//! ## Available features
//!
//! The available features are described in the [`features`] module.

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
compile_error!("This wasm target is unsupported by mio. If using Tokio, disable the net feature.");

// macros used internally
#[macro_use]
mod macros;

mod interest;
mod poll;
mod sys;
mod token;
#[cfg(not(target_os = "wasi"))]
mod waker;

pub mod event;

cfg_io_source! {
    mod io_source;
}

cfg_net! {
    pub mod net;
}

#[doc(no_inline)]
pub use event::Events;
pub use interest::Interest;
pub use poll::{Poll, Registry};
pub use token::Token;
#[cfg(not(target_os = "wasi"))]
pub use waker::Waker;

#[cfg(all(unix, feature = "os-ext"))]
#[cfg_attr(docsrs, doc(cfg(all(unix, feature = "os-ext"))))]
pub mod unix {
    //! Unix only extensions.

    pub mod pipe {
        //! Unix pipe.
        //!
        //! See the [`new`] function for documentation.

        pub use crate::sys::pipe::{new, Receiver, Sender};
    }

    pub use crate::sys::SourceFd;
}

#[cfg(all(target_os = "hermit", feature = "os-ext"))]
#[cfg_attr(docsrs, doc(cfg(all(target_os = "hermit", feature = "os-ext"))))]
pub mod hermit {
    //! Hermit only extensions.

    pub use crate::sys::SourceFd;
}

#[cfg(all(windows, feature = "os-ext"))]
#[cfg_attr(docsrs, doc(cfg(all(windows, feature = "os-ext"))))]
pub mod windows {
    //! Windows only extensions.

    pub use crate::sys::named_pipe::NamedPipe;
}

pub mod features {
    //! # Mio's optional features.
    //!
    //! This document describes the available features in Mio.
    //!
    #![cfg_attr(feature = "os-poll", doc = "## `os-poll` (enabled)")]
    #![cfg_attr(not(feature = "os-poll"), doc = "## `os-poll` (disabled)")]
    //!
    //! Mio by default provides only a shell implementation that `panic!`s the
    //! moment it is actually run. To run it requires OS support, this is
    //! enabled by activating the `os-poll` feature.
    //!
    //! This makes `Poll`, `Registry` and `Waker` functional.
    //!
    #![cfg_attr(feature = "os-ext", doc = "## `os-ext` (enabled)")]
    #![cfg_attr(not(feature = "os-ext"), doc = "## `os-ext` (disabled)")]
    //!
    //! `os-ext` enables additional OS specific facilities. These facilities can
    //! be found in the `unix` and `windows` module.
    //!
    #![cfg_attr(feature = "net", doc = "## Network types (enabled)")]
    #![cfg_attr(not(feature = "net"), doc = "## Network types (disabled)")]
    //!
    //! The `net` feature enables networking primitives in the `net` module.
}

pub mod guide {
    //! # Getting started guide.
    //!
    //! In this guide we'll do the following:
    //!
    //! 1. Create a [`Poll`] instance (and learn what it is).
    //! 2. Register an [event source].
    //! 3. Create an event loop.
    //!
    //! At the end you'll have a very small (but quick) TCP server that accepts
    //! connections and then drops (disconnects) them.
    //!
    //! ## 1. Creating a `Poll` instance
    //!
    //! Using Mio starts by creating a [`Poll`] instance, which monitors events
    //! from the OS and puts them into [`Events`]. This allows us to execute I/O
    //! operations based on what operations are ready.
    //!
    //! [`Poll`]: ../struct.Poll.html
    //! [`Events`]: ../event/struct.Events.html
    //!
    #![cfg_attr(feature = "os-poll", doc = "```")]
    #![cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    //! # use mio::{Poll, Events};
    //! # fn main() -> std::io::Result<()> {
    //! // `Poll` allows for polling of readiness events.
    //! let poll = Poll::new()?;
    //! // `Events` is collection of readiness `Event`s and can be filled by
    //! // calling `Poll::poll`.
    //! let events = Events::with_capacity(128);
    //! # drop((poll, events));
    //! # Ok(())
    //! # }
    //! ```
    //!
    //! For example if we're using a [`TcpListener`],  we'll only want to
    //! attempt to accept an incoming connection *iff* any connections are
    //! queued and ready to be accepted. We don't want to waste our time if no
    //! connections are ready.
    //!
    //! [`TcpListener`]: ../net/struct.TcpListener.html
    //!
    //! ## 2. Registering event source
    //!
    //! After we've created a [`Poll`] instance that monitors events from the OS
    //! for us, we need to provide it with a source of events. This is done by
    //! registering an [event source]. As the name “event source” suggests it is
    //! a source of events which can be polled using a `Poll` instance. On Unix
    //! systems this is usually a file descriptor, or a socket/handle on
    //! Windows.
    //!
    //! In the example below we'll use a [`TcpListener`] for which we'll receive
    //! an event (from [`Poll`]) once a connection is ready to be accepted.
    //!
    //! [event source]: ../event/trait.Source.html
    //!
    #![cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
    #![cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
    //! # use mio::net::TcpListener;
    //! # use mio::{Poll, Token, Interest};
    //! # fn main() -> std::io::Result<()> {
    //! # let poll = Poll::new()?;
    //! # let address = "127.0.0.1:0".parse().unwrap();
    //! // Create a `TcpListener`, binding it to `address`.
    //! let mut listener = TcpListener::bind(address)?;
    //!
    //! // Next we register it with `Poll` to receive events for it. The `SERVER`
    //! // `Token` is used to determine that we received an event for the listener
    //! // later on.
    //! const SERVER: Token = Token(0);
    //! poll.registry().register(&mut listener, SERVER, Interest::READABLE)?;
    //! # Ok(())
    //! # }
    //! ```
    //!
    //! Multiple event sources can be [registered] (concurrently), so we can
    //! monitor multiple sources at a time.
    //!
    //! [registered]: ../struct.Registry.html#method.register
    //!
    //! ## 3. Creating the event loop
    //!
    //! After we've created a [`Poll`] instance and registered one or more
    //! [event sources] with it, we can [poll] it for events. Polling for events
    //! is simple, we need a container to store the events: [`Events`] and need
    //! to do something based on the polled events (this part is up to you, we
    //! can't do it all!). If we do this in a loop we've got ourselves an event
    //! loop.
    //!
    //! The example below shows the event loop in action, completing our small
    //! TCP server.
    //!
    //! [poll]: ../struct.Poll.html#method.poll
    //! [event sources]: ../event/trait.Source.html
    //!
    #![cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
    #![cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
    //! # use std::io;
    //! # use std::time::Duration;
    //! # use mio::net::TcpListener;
    //! # use mio::{Poll, Token, Interest, Events};
    //! # fn main() -> io::Result<()> {
    //! # let mut poll = Poll::new()?;
    //! # let mut events = Events::with_capacity(128);
    //! # let address = "127.0.0.1:0".parse().unwrap();
    //! # let mut listener = TcpListener::bind(address)?;
    //! # const SERVER: Token = Token(0);
    //! # poll.registry().register(&mut listener, SERVER, Interest::READABLE)?;
    //! // Start our event loop.
    //! loop {
    //!     // Poll the OS for events, waiting at most 100 milliseconds.
    //!     poll.poll(&mut events, Some(Duration::from_millis(100)))?;
    //!
    //!     // Process each event.
    //!     for event in events.iter() {
    //!         // We can use the token we previously provided to `register` to
    //!         // determine for which type the event is.
    //!         match event.token() {
    //!             SERVER => loop {
    //!                 // One or more connections are ready, so we'll attempt to
    //!                 // accept them (in a loop).
    //!                 match listener.accept() {
    //!                     Ok((connection, address)) => {
    //!                         println!("Got a connection from: {}", address);
    //! #                       drop(connection);
    //!                     },
    //!                     // A "would block error" is returned if the operation
    //!                     // is not ready, so we'll stop trying to accept
    //!                     // connections.
    //!                     Err(ref err) if would_block(err) => break,
    //!                     Err(err) => return Err(err),
    //!                 }
    //!             }
    //! #           _ => unreachable!(),
    //!         }
    //!     }
    //! #   return Ok(());
    //! }
    //!
    //! fn would_block(err: &io::Error) -> bool {
    //!     err.kind() == io::ErrorKind::WouldBlock
    //! }
    //! # }
    //! ```
}
