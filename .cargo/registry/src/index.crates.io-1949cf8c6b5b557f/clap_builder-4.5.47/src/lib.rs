// Copyright ⓒ 2015-2016 Kevin B. Knapp and [`clap-rs` contributors](https://github.com/clap-rs/clap/graphs/contributors).
// Licensed under the MIT license
// (see LICENSE or <http://opensource.org/licenses/MIT>) All files in the project carrying such
// notice may not be copied, modified, or distributed except according to those terms.

#![doc = include_str!("../README.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/clap-rs/clap/master/assets/clap.png")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[cfg(not(feature = "std"))]
compile_error!("`std` feature is currently required to build `clap`");

pub use crate::builder::ArgAction;
pub use crate::builder::Command;
pub use crate::builder::ValueHint;
pub use crate::builder::{Arg, ArgGroup};
pub use crate::parser::ArgMatches;
pub use crate::util::color::ColorChoice;
pub use crate::util::Id;

/// Command Line Argument Parser Error
///
/// See [`Command::error`] to create an error.
///
/// [`Command::error`]: crate::Command::error
pub type Error = error::Error<error::DefaultFormatter>;

pub use crate::derive::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

#[macro_use]
#[allow(missing_docs)]
mod macros;

mod derive;

pub mod builder;
pub mod error;
pub mod parser;

mod mkeymap;
mod output;
mod util;

const INTERNAL_ERROR_MSG: &str = "Fatal internal error. Please consider filing a bug \
                                  report at https://github.com/clap-rs/clap/issues";

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
