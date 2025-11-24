//! Asynchronous channels.
//!
//! Like threads, concurrent tasks sometimes need to communicate with each
//! other. This module contains two basic abstractions for doing so:
//!
//! - [oneshot], a way of sending a single value from one task to another.
//! - [mpsc], a multi-producer, single-consumer channel for sending values
//!   between tasks, analogous to the similarly-named structure in the standard
//!   library.
//!
//! All items are only available when the `std` or `alloc` feature of this
//! library is activated, and it is activated by default.

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(missing_docs, unsafe_op_in_unsafe_fn)]

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod lock;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "std")]
pub mod mpsc;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub mod oneshot;
