// Copyright ⓒ 2015-2018 Kevin B. Knapp
//
// `clap_complete` is distributed under the terms of both the MIT license and the Apache License
// (Version 2.0).
// See the [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) files in this repository
// for more information.

//! ## Quick Start
//!
//! - For generating at compile-time, see [`generate_to`]
//! - For generating at runtime, see [`generate`]
//!
//! [`Shell`] is a convenience `enum` for an argument value type that implements `Generator`
//! for each natively-supported shell type.
//!
//! ## Example
//!
//! ```rust,no_run
//! use clap::{Command, Arg, ValueHint, value_parser, ArgAction};
//! use clap_complete::aot::{generate, Generator, Shell};
//! use std::io;
//!
//! fn build_cli() -> Command {
//!     Command::new("example")
//!         .arg(Arg::new("file")
//!             .help("some input file")
//!             .value_hint(ValueHint::AnyPath))
//!         .arg(Arg::new("generator")
//!             .long("generate")
//!             .action(ArgAction::Set)
//!             .value_parser(value_parser!(Shell)))
//! }
//!
//! fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
//!     generate(generator, cmd, cmd.get_name().to_string(), &mut io::stdout());
//! }
//!
//! fn main() {
//!     let matches = build_cli().get_matches();
//!
//!     if let Some(generator) = matches.get_one::<Shell>("generator").copied() {
//!         let mut cmd = build_cli();
//!         eprintln!("Generating completion file for {generator}...");
//!         print_completions(generator, &mut cmd);
//!     }
//! }
//! ```

#![doc(html_logo_url = "https://raw.githubusercontent.com/clap-rs/clap/master/assets/clap.png")]
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::needless_doctest_main)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

const INTERNAL_ERROR_MSG: &str = "Fatal internal error. Please consider filing a bug \
                                  report at https://github.com/clap-rs/clap/issues";

#[macro_use]
#[allow(missing_docs)]
mod macros;

pub mod aot;
#[cfg(feature = "unstable-dynamic")]
pub mod engine;
#[cfg(feature = "unstable-dynamic")]
pub mod env;

pub use clap::ValueHint;
#[doc(inline)]
#[cfg(feature = "unstable-dynamic")]
pub use engine::ArgValueCandidates;
#[cfg(feature = "unstable-dynamic")]
pub use engine::ArgValueCompleter;
#[doc(inline)]
#[cfg(feature = "unstable-dynamic")]
pub use engine::CompletionCandidate;
#[cfg(feature = "unstable-dynamic")]
pub use engine::PathCompleter;
#[cfg(feature = "unstable-dynamic")]
pub use env::CompleteEnv;

/// Deprecated, see [`aot`]
pub mod generator {
    pub use crate::aot::generate;
    pub use crate::aot::generate_to;
    pub use crate::aot::utils;
    pub use crate::aot::Generator;
}
/// Deprecated, see [`aot`]
pub mod shells {
    pub use crate::aot::Bash;
    pub use crate::aot::Elvish;
    pub use crate::aot::Fish;
    pub use crate::aot::PowerShell;
    pub use crate::aot::Shell;
    pub use crate::aot::Zsh;
}
/// Deprecated, see [`aot::generate`]
pub use aot::generate;
/// Deprecated, see [`aot::generate_to`]
pub use aot::generate_to;
/// Deprecated, see [`aot::Generator`]
pub use aot::Generator;
/// Deprecated, see [`aot::Shell`]
pub use aot::Shell;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
