//! Styling legacy Windows terminals
//!
//! See [`WinconStream`]
//!
//! This fills a similar role as [`winapi-util`](https://crates.io/crates/winapi-util) does for
//! [`termcolor`](https://crates.io/crates/termcolor) with the differences
//! - Uses `windows-sys` rather than `winapi`
//! - Uses [`anstyle`](https://crates.io/crates/termcolor) rather than defining its own types
//! - More focused, smaller

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod ansi;
mod stream;
#[cfg(windows)]
pub mod windows;

pub use stream::WinconStream;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
