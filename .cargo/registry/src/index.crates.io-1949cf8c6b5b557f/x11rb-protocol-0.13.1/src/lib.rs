//! X11 rust bindings.
//!
//! This crate provides a representation of the X11 protocol in Rust. With this protocol, raw X11
//! bytes can be parsed into a structured representation or raw bytes can be produces.
//!
//! This protocol does not do any I/O. If you need an X11 client library, look at
//! <https://docs.rs/x11rb/latest/x11rb/>.
//!
//! # Feature flags
//!
//! This crate uses [feature
//! flags](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) to reduce
//! the amount of compiled code. There are two kinds of feature flags available:
//!
//! * Feature flags for specific X11 extensions
//! * Feature flags for additional functionality
//!
//! ## Feature flags for specific X11 extensions
//!
//! By default, only the core X11 protocol and some small, commonly needed X11 extensions are
//! enabled. These are the `bigreq`, `ge` and `xc_misc` extensions. Further extensions need to be
//! explicitly enabled via their feature flag:
//!
//! `composite`, `damage`, `dpms`, `dri2`, `dri3`, `glx`, `present`, `randr`, `record`, `render`,
//! `res`, `screensaver`, `shape`, `shm`, `sync`, `xevie`, `xf86dri`, `xf86vidmode`, `xfixes`,
//! `xinerama`, `xinput`, `xkb`, `xprint`, `xselinux`, `xtest`, `xv`, `xvmc`.
//!
//! If you want to take the "I do not want to think about this"-approach, you can enable the
//! `all-extensions` feature to just enable, well, all extensions.
//!
//! ## Feature flags for additional functionality
//!
//! Additionally, the following flags exist:
//! * `std` (enabled by default): Enable functionality needing the std library, e.g. environment
//!   variables or [`std::os::unix::io::OwnedFd`].
//! * `resource_manager`: Enable the code in [resource_manager] for loading and querying the
//!   X11 resource database.
//! * `serde`: Implement [`serde::Serialize`] and [`serde::Deserialize`] for all objects.
//! * `request-parsing`: Add the ability to parse X11 requests. Not normally needed.
//! * `extra-traits`: Implement extra traits for types. This improves the output of the `Debug`
//!   impl and adds `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash` where possible.

// A list of lints that are only #![deny] and not the stronger #![forbid]. Each one has a comment
// explaining why it gets the weaker treatment.
#![deny(
    // Contains unreachable_code and "?" generates an #[allow] for this
    unused,
    // #[derive] generates an #[allow] for this; not part of "unused"
    unused_qualifications,
    // serde's Deserialize/Serialize impls add allows for this
    rust_2018_idioms,
    // Not everything in x11rb_protocol::protocol has doc comments
    missing_docs,
)]
#![forbid(
    missing_copy_implementations,
    missing_debug_implementations,
    rustdoc::private_doc_tests,
    //single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unused_must_use,
    unused_results,
    clippy::cast_lossless,
    clippy::needless_pass_by_value,
)]
#![no_std]

// std crate imports
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::vec::Vec;

pub mod connect;
pub mod connection;
#[macro_use]
pub mod x11_utils;
pub mod errors;
pub mod id_allocator;
pub mod packet_reader;
pub mod parse_display;
#[rustfmt::skip]
#[allow(missing_docs)]
pub mod protocol;
#[cfg(feature = "resource_manager")]
pub mod resource_manager;
#[cfg(test)]
mod test;
mod utils;
pub mod wrapper;
pub mod xauth;

pub use utils::RawFdContainer;

// Used to avoid too-complex types.
/// A combination of a buffer and a list of file descriptors.
pub type BufWithFds<B> = (B, Vec<RawFdContainer>);

/// Number type used for referring to things that were sent to the server in responses from the
/// server.
///
/// Each request sent to the X11 server is implicitly assigned a monotonically increasing sequence
/// number. Replies, events, and errors contain the sequence number of the last request that the
/// server received. This allows to map replies to their requests and to figure out which request
/// caused an error.
pub type SequenceNumber = u64;

/// The raw bytes of an event and its sequence number.
pub type RawEventAndSeqNumber<B> = (B, SequenceNumber);

/// Variants describing which responses to a request should be discarded.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DiscardMode {
    /// Only discard the actual reply. Errors go to the main loop.
    DiscardReply,
    /// Ignore any kind of response that this request generates.
    DiscardReplyAndError,
}
