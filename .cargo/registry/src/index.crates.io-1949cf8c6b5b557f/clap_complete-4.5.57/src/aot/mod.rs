//! Prebuilt completions
//!
//! ## Quick Start
//!
//! - For generating at compile-time, see [`generate_to`]
//! - For generating at runtime, see [`generate`]
//!
//! [`Shell`] is a convenience `enum` for an argument value type that implements `Generator`
//! for each natively-supported shell type.
//!
//! To customize completions, see
//! - [`ValueHint`]
//! - [`ValueEnum`][clap::ValueEnum]
//!
//! ## Example
//!
//! ```rust,no_run
//! use clap::{Command, Arg, ValueHint, value_parser, ArgAction};
//! use clap_complete::{generate, Generator, Shell};
//! use std::io;
//!
//! fn build_cli() -> Command {
//!     Command::new("example")
//!          .arg(Arg::new("file")
//!              .help("some input file")
//!                 .value_hint(ValueHint::AnyPath),
//!         )
//!        .arg(
//!            Arg::new("generator")
//!                .long("generate")
//!                .action(ArgAction::Set)
//!                .value_parser(value_parser!(Shell)),
//!        )
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

mod generator;
mod shells;

pub use clap::ValueHint;
pub use generator::*;
pub use shells::*;
