//! **Assert [`Command`]** - Easy command initialization and assertions.
//!
//! `assert_cmd` aims to simplify the process for doing integration testing of CLIs, including:
//! - Finding your crate's binary to test
//! - Assert on the result of your program's run.
//!
//! ## Overview
//!
//! Create a [`Command`]:
//! - `Command::new(path)`
//! - `Command::from_std(...)`
//! - `Command::cargo_bin(name)`
//!
//! Configure a [`Command`]:
//! - `arg` / `args`
//! - `current_dir`
//! - `env` / `envs` / `env_remove` / `env_clear`
//! - `write_stdin` / `pipe_stdin`
//! - `timeout`
//!
//! Validate a [`Command`]:
//! - `ok` / `unwrap` / `unwrap_err`
//! - `assert`
//!   - `success`, see [`Assert`]
//!   - `failure`, see [`Assert`]
//!   - `interrupted`, see [`Assert`]
//!   - `code`, see [`Assert`]
//!   - `stdout`, see [`Assert`]
//!   - `stderr`, see [`Assert`]
//!   - `get_output` for everything else, see [`Assert`]
//!
//! Note: [`Command`] is provided as a convenience. Extension traits for [`std::process::Command`]
//! and `Output` are provided for interoperability:
//! - [`CommandCargoExt`]
//! - [`OutputOkExt`]
//! - [`OutputAssertExt`]
//!
//! ## Examples
//!
//! Here's a trivial example:
//! ```rust,no_run
//! use assert_cmd::Command;
//!
//! let mut cmd = Command::cargo_bin("bin_fixture").unwrap();
//! cmd.assert().success();
//! ```
//!
//! And a little of everything:
//! ```rust,no_run
//! use assert_cmd::Command;
//!
//! let mut cmd = Command::cargo_bin("bin_fixture").unwrap();
//! let assert = cmd
//!     .arg("-A")
//!     .env("stdout", "hello")
//!     .env("exit", "42")
//!     .write_stdin("42")
//!     .assert();
//! assert
//!     .failure()
//!     .code(42)
//!     .stdout("hello\n");
//! ```
//!
//! ## Relevant crates
//!
//! Other crates that might be useful in testing command line programs.
//! * [escargot] for more control over configuring the crate's binary.
//! * [duct] for orchestrating multiple processes.
//!   * or [commandspec] for easier writing of commands
//! * [rexpect][rexpect] for testing interactive programs.
//! * [assert_fs] for filesystem fixtures and assertions.
//!   * or [tempfile] for scratchpad directories.
//! * [dir-diff] for testing file side-effects.
//!
//! ## Migrating from `assert_cli` v0.6
//!
//! `assert_cmd` is the successor to [the original `assert_cli`][assert_cli]:
//! - More flexible, reusable assertions (also used by [assert_fs]).
//! - Can integrate with other process-management crates, like `duct`.
//! - Addresses several architectural problems.
//!
//! Key points in migrating from `assert_cli`:
//! - The command-under-test is run eagerly, with assertions happening immediately.
//! - [`success()`] is not implicit and requires being explicitly called.
//! - `stdout`/`stderr` aren't automatically trimmed before being passed to the `Predicate`.
//!
//! [commandspec]: https://crates.io/crates/commandspec
//! [assert_cli]: https://crates.io/crates/assert_cli/0.6.3
//! [dir-diff]: https://crates.io/crates/dir-diff
//! [tempfile]: https://crates.io/crates/tempfile
//! [escargot]: https://crates.io/crates/escargot
//! [duct]: https://crates.io/crates/duct
//! [assert_fs]: https://crates.io/crates/assert_fs
//! [rexpect]: https://crates.io/crates/rexpect
//! [`Command`]: cmd::Command
//! [`Assert`]: assert::Assert
//! [`success()`]: assert::Assert::success()
//! [`CommandCargoExt`]: cargo::CommandCargoExt
//! [`OutputOkExt`]: output::OutputOkExt
//! [`OutputAssertExt`]: assert::OutputAssertExt

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

mod macros;

pub mod assert;
pub mod cargo;
pub mod cmd;
pub mod output;

/// Extension traits that are useful to have available.
pub mod prelude {
    pub use crate::assert::OutputAssertExt;
    pub use crate::cargo::CommandCargoExt;
    pub use crate::output::OutputOkExt;
}

pub use crate::cmd::Command;

mod color;
use color::Palette;

doc_comment::doctest!("../README.md");
