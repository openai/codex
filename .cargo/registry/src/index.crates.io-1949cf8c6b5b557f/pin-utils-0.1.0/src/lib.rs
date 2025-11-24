//! Utilities for pinning

#![no_std]
#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]
#![allow(unknown_lints)]
#![doc(html_root_url = "https://docs.rs/pin-utils/0.1.0")]

#[doc(hidden)]
pub mod core_reexport {
    pub use core::*;
}

#[macro_use]
mod stack_pin;
#[macro_use]
mod projection;
