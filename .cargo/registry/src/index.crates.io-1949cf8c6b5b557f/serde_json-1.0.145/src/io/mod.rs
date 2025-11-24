//! A tiny, `no_std`-friendly facade around `std::io`.
//! Reexports types from `std` when available; otherwise reimplements and
//! provides some of the core logic.
//!
//! The main reason that `std::io` hasn't found itself reexported as part of
//! the `core` crate is the `std::io::{Read, Write}` traits' reliance on
//! `std::io::Error`, which may contain internally a heap-allocated `Box<Error>`
//! and/or now relying on OS-specific `std::backtrace::Backtrace`.

pub use self::imp::{Error, ErrorKind, Result, Write};

#[cfg(not(feature = "std"))]
#[path = "core.rs"]
mod imp;

#[cfg(feature = "std")]
use std::io as imp;

#[cfg(feature = "std")]
pub use std::io::{Bytes, Read};
