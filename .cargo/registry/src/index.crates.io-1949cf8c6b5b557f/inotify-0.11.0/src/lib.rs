//! Idiomatic inotify wrapper for the Rust programming language
//!
//! # About
//!
//! [inotify-rs] is an idiomatic wrapper around the Linux kernel's [inotify] API
//! for the Rust programming language. It can be used for monitoring changes to
//! files or directories.
//!
//! The [`Inotify`] struct is the main entry point into the API.
//!
//! # Example
//!
//! ```
//! use inotify::{
//!     Inotify,
//!     WatchMask,
//! };
//!
//! let mut inotify = Inotify::init()
//!     .expect("Error while initializing inotify instance");
//!
//! # // Create a temporary file, so `Watches::add` won't return an error.
//! # use std::fs::File;
//! # let mut test_file = File::create("/tmp/inotify-rs-test-file")
//! #     .expect("Failed to create test file");
//! #
//! // Watch for modify and close events.
//! inotify
//!     .watches()
//!     .add(
//!         "/tmp/inotify-rs-test-file",
//!         WatchMask::MODIFY | WatchMask::CLOSE,
//!     )
//!     .expect("Failed to add file watch");
//!
//! # // Modify file, so the following `read_events_blocking` won't block.
//! # use std::io::Write;
//! # write!(&mut test_file, "something\n")
//! #     .expect("Failed to write something to test file");
//! #
//! // Read events that were added with `Watches::add` above.
//! let mut buffer = [0; 1024];
//! let events = inotify.read_events_blocking(&mut buffer)
//!     .expect("Error while reading events");
//!
//! for event in events {
//!     // Handle event
//! }
//! ```
//!
//! # Attention: inotify gotchas
//!
//! inotify (as in, the Linux API, not this wrapper) has many edge cases, making
//! it hard to use correctly. This can lead to weird and hard to find bugs in
//! applications that are based on it. inotify-rs does its best to fix these
//! issues, but sometimes this would require an amount of runtime overhead that
//! is just unacceptable for a low-level wrapper such as this.
//!
//! We've documented any issues that inotify-rs has inherited from inotify, as
//! far as we are aware of them. Please watch out for any further warnings
//! throughout this documentation. If you want to be on the safe side, in case
//! we have missed something, please read the [inotify man pages] carefully.
//!
//! [inotify-rs]: https://crates.io/crates/inotify
//! [inotify]: https://en.wikipedia.org/wiki/Inotify
//! [inotify man pages]: http://man7.org/linux/man-pages/man7/inotify.7.html


#![deny(missing_docs)]
#![deny(warnings)]
#![deny(missing_debug_implementations)]


#[macro_use]
extern crate bitflags;

mod events;
mod fd_guard;
mod inotify;
mod util;
mod watches;

#[cfg(feature = "stream")]
mod stream;


pub use crate::events::{
    Event,
    EventMask,
    EventOwned,
    Events,
};
pub use crate::inotify::Inotify;
pub use crate::util::{
    get_buffer_size,
    get_absolute_path_buffer_size,
};
pub use crate::watches::{
    Watches,
    WatchDescriptor,
    WatchMask,
};

#[cfg(feature = "stream")]
pub use self::stream::EventStream;
