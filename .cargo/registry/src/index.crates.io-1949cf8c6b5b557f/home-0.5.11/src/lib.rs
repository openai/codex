//! Canonical definitions of `home_dir`, `cargo_home`, and `rustup_home`.
//!
//! The definition of `home_dir` provided by the standard library is
//! incorrect because it considers the `HOME` environment variable on
//! Windows. This causes surprising situations where a Rust program
//! will behave differently depending on whether it is run under a
//! Unix emulation environment like Cygwin or MinGW. Neither Cargo nor
//! rustup use the standard libraries definition - they use the
//! definition here.
//!
//! This crate provides two additional functions, `cargo_home` and
//! `rustup_home`, which are the canonical way to determine the
//! location that Cargo and rustup use to store their data.
//! The `env` module contains utilities for mocking the process environment
//! by Cargo and rustup.
//!
//! See also this [discussion].
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo and Rustup
//! > and not intended for external use. This
//! > crate may make major changes to its APIs or be deprecated without warning.
//!
//! [discussion]: https://github.com/rust-lang/rust/pull/46799#issuecomment-361156935

#![allow(clippy::disallowed_methods)]

pub mod env;

#[cfg(target_os = "windows")]
mod windows;

use std::io;
use std::path::{Path, PathBuf};

/// Returns the path of the current user's home directory using environment
/// variables or OS-specific APIs.
///
/// # Unix
///
/// Returns the value of the `HOME` environment variable if it is set
/// **even** if it is an empty string. Otherwise, it tries to determine the
/// home directory by invoking the [`getpwuid_r`][getpwuid] function with
/// the UID of the current user.
///
/// [getpwuid]: https://linux.die.net/man/3/getpwuid_r
///
/// # Windows
///
/// Returns the value of the `USERPROFILE` environment variable if it is set
/// **and** it is not an empty string. Otherwise, it tries to determine the
/// home directory by invoking the [`SHGetKnownFolderPath`][shgkfp] function with
/// [`FOLDERID_Profile`][knownfolderid].
///
/// [shgkfp]: https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath
/// [knownfolderid]: https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid
///
/// # Examples
///
/// ```
/// match home::home_dir() {
///     Some(path) if !path.as_os_str().is_empty() => println!("{}", path.display()),
///     _ => println!("Unable to get your home dir!"),
/// }
/// ```
pub fn home_dir() -> Option<PathBuf> {
    env::home_dir_with_env(&env::OS_ENV)
}

#[cfg(windows)]
use windows::home_dir_inner;

#[cfg(any(unix, target_os = "redox"))]
fn home_dir_inner() -> Option<PathBuf> {
    #[allow(deprecated)]
    std::env::home_dir()
}

/// Returns the storage directory used by Cargo, often knowns as
/// `.cargo` or `CARGO_HOME`.
///
/// It returns one of the following values, in this order of
/// preference:
///
/// - The value of the `CARGO_HOME` environment variable, if it is
///   an absolute path.
/// - The value of the current working directory joined with the value
///   of the `CARGO_HOME` environment variable, if `CARGO_HOME` is a
///   relative directory.
/// - The `.cargo` directory in the user's home directory, as reported
///   by the `home_dir` function.
///
/// # Errors
///
/// This function fails if it fails to retrieve the current directory,
/// or if the home directory cannot be determined.
///
/// # Examples
///
/// ```
/// match home::cargo_home() {
///     Ok(path) => println!("{}", path.display()),
///     Err(err) => eprintln!("Cannot get your cargo home dir: {:?}", err),
/// }
/// ```
pub fn cargo_home() -> io::Result<PathBuf> {
    env::cargo_home_with_env(&env::OS_ENV)
}

/// Returns the storage directory used by Cargo within `cwd`.
/// For more details, see [`cargo_home`](fn.cargo_home.html).
pub fn cargo_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    env::cargo_home_with_cwd_env(&env::OS_ENV, cwd)
}

/// Returns the storage directory used by rustup, often knowns as
/// `.rustup` or `RUSTUP_HOME`.
///
/// It returns one of the following values, in this order of
/// preference:
///
/// - The value of the `RUSTUP_HOME` environment variable, if it is
///   an absolute path.
/// - The value of the current working directory joined with the value
///   of the `RUSTUP_HOME` environment variable, if `RUSTUP_HOME` is a
///   relative directory.
/// - The `.rustup` directory in the user's home directory, as reported
///   by the `home_dir` function.
///
/// # Errors
///
/// This function fails if it fails to retrieve the current directory,
/// or if the home directory cannot be determined.
///
/// # Examples
///
/// ```
/// match home::rustup_home() {
///     Ok(path) => println!("{}", path.display()),
///     Err(err) => eprintln!("Cannot get your rustup home dir: {:?}", err),
/// }
/// ```
pub fn rustup_home() -> io::Result<PathBuf> {
    env::rustup_home_with_env(&env::OS_ENV)
}

/// Returns the storage directory used by rustup within `cwd`.
/// For more details, see [`rustup_home`](fn.rustup_home.html).
pub fn rustup_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    env::rustup_home_with_cwd_env(&env::OS_ENV, cwd)
}
