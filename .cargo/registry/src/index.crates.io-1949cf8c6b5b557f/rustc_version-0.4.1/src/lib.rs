// Copyright 2016 rustc-version-rs developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![warn(missing_docs)]

//! Simple library for getting the version information of a `rustc`
//! compiler.
//!
//! This can be used by build scripts or other tools dealing with Rust sources
//! to make decisions based on the version of the compiler.
//!
//! It calls `$RUSTC --version -v` and parses the output, falling
//! back to `rustc` if `$RUSTC` is not set.
//!
//! # Example
//!
//! ```rust
//! // This could be a cargo build script
//!
//! use rustc_version::{version, version_meta, Channel, Version};
//!
//! // Assert we haven't travelled back in time
//! assert!(version().unwrap().major >= 1);
//!
//! // Set cfg flags depending on release channel
//! match version_meta().unwrap().channel {
//!     Channel::Stable => {
//!         println!("cargo:rustc-cfg=RUSTC_IS_STABLE");
//!     }
//!     Channel::Beta => {
//!         println!("cargo:rustc-cfg=RUSTC_IS_BETA");
//!     }
//!     Channel::Nightly => {
//!         println!("cargo:rustc-cfg=RUSTC_IS_NIGHTLY");
//!     }
//!     Channel::Dev => {
//!         println!("cargo:rustc-cfg=RUSTC_IS_DEV");
//!     }
//! }
//!
//! // Check for a minimum version
//! if version().unwrap() >= Version::parse("1.4.0").unwrap() {
//!     println!("cargo:rustc-cfg=compiler_has_important_bugfix");
//! }
//! ```

#[cfg(test)]
#[macro_use]
extern crate doc_comment;

#[cfg(test)]
doctest!("../README.md");

use std::collections::HashMap;
use std::process::Command;
use std::{env, error, fmt, io, num, str};
use std::{ffi::OsString, str::FromStr};

// Convenience re-export to allow version comparison without needing to add
// semver crate.
pub use semver::Version;

use Error::*;

/// Release channel of the compiler.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Channel {
    /// Development release channel
    Dev,
    /// Nightly release channel
    Nightly,
    /// Beta release channel
    Beta,
    /// Stable release channel
    Stable,
}

/// LLVM version
///
/// LLVM's version numbering scheme is not semver compatible until version 4.0
///
/// rustc [just prints the major and minor versions], so other parts of the version are not included.
///
/// [just prints the major and minor versions]: https://github.com/rust-lang/rust/blob/b5c9e2448c9ace53ad5c11585803894651b18b0a/compiler/rustc_codegen_llvm/src/llvm_util.rs#L173-L178
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LlvmVersion {
    // fields must be ordered major, minor for comparison to be correct
    /// Major version
    pub major: u64,
    /// Minor version
    pub minor: u64,
    // TODO: expose micro version here
}

impl fmt::Display for LlvmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for LlvmVersion {
    type Err = LlvmVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s
            .split('.')
            .map(|part| -> Result<u64, LlvmVersionParseError> {
                if part == "0" {
                    Ok(0)
                } else if part.starts_with('0') {
                    Err(LlvmVersionParseError::ComponentMustNotHaveLeadingZeros)
                } else if part.starts_with('-') || part.starts_with('+') {
                    Err(LlvmVersionParseError::ComponentMustNotHaveSign)
                } else {
                    Ok(part.parse()?)
                }
            });

        let major = parts.next().unwrap()?;
        let mut minor = 0;

        if let Some(part) = parts.next() {
            minor = part?;
        } else if major < 4 {
            // LLVM versions earlier than 4.0 have significant minor versions, so require the minor version in this case.
            return Err(LlvmVersionParseError::MinorVersionRequiredBefore4);
        }

        if let Some(Err(e)) = parts.next() {
            return Err(e);
        }

        if parts.next().is_some() {
            return Err(LlvmVersionParseError::TooManyComponents);
        }

        Ok(Self { major, minor })
    }
}

/// Rustc version plus metadata like git short hash and build date.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VersionMeta {
    /// Version of the compiler
    pub semver: Version,

    /// Git short hash of the build of the compiler
    pub commit_hash: Option<String>,

    /// Commit date of the compiler
    pub commit_date: Option<String>,

    /// Build date of the compiler; this was removed between Rust 1.0.0 and 1.1.0.
    pub build_date: Option<String>,

    /// Release channel of the compiler
    pub channel: Channel,

    /// Host target triple of the compiler
    pub host: String,

    /// Short version string of the compiler
    pub short_version_string: String,

    /// Version of LLVM used by the compiler
    pub llvm_version: Option<LlvmVersion>,
}

impl VersionMeta {
    /// Returns the version metadata for `cmd`, which should be a `rustc` command.
    pub fn for_command(mut cmd: Command) -> Result<VersionMeta> {
        let out = cmd
            .arg("-vV")
            .output()
            .map_err(Error::CouldNotExecuteCommand)?;

        if !out.status.success() {
            return Err(Error::CommandError {
                stdout: String::from_utf8_lossy(&out.stdout).into(),
                stderr: String::from_utf8_lossy(&out.stderr).into(),
            });
        }

        version_meta_for(str::from_utf8(&out.stdout)?)
    }
}

/// Returns the `rustc` SemVer version.
pub fn version() -> Result<Version> {
    Ok(version_meta()?.semver)
}

/// Returns the `rustc` SemVer version and additional metadata
/// like the git short hash and build date.
pub fn version_meta() -> Result<VersionMeta> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let cmd = if let Some(wrapper) = env::var_os("RUSTC_WRAPPER").filter(|w| !w.is_empty()) {
        let mut cmd = Command::new(wrapper);
        cmd.arg(rustc);
        cmd
    } else {
        Command::new(rustc)
    };

    VersionMeta::for_command(cmd)
}

/// Parses a "rustc -vV" output string and returns
/// the SemVer version and additional metadata
/// like the git short hash and build date.
pub fn version_meta_for(verbose_version_string: &str) -> Result<VersionMeta> {
    let mut map = HashMap::new();
    for (i, line) in verbose_version_string.lines().enumerate() {
        if i == 0 {
            map.insert("short", line);
            continue;
        }

        let mut parts = line.splitn(2, ": ");
        let key = match parts.next() {
            Some(key) => key,
            None => continue,
        };

        if let Some(value) = parts.next() {
            map.insert(key, value);
        }
    }

    let short_version_string = expect_key("short", &map)?;
    let host = expect_key("host", &map)?;
    let release = expect_key("release", &map)?;
    let semver: Version = release.parse()?;

    let channel = match semver.pre.split('.').next().unwrap() {
        "" => Channel::Stable,
        "dev" => Channel::Dev,
        "beta" => Channel::Beta,
        "nightly" => Channel::Nightly,
        x => return Err(Error::UnknownPreReleaseTag(x.to_owned())),
    };

    let commit_hash = expect_key_or_unknown("commit-hash", &map)?;
    let commit_date = expect_key_or_unknown("commit-date", &map)?;
    let build_date = map
        .get("build-date")
        .filter(|&v| *v != "unknown")
        .map(|&v| String::from(v));
    let llvm_version = match map.get("LLVM version") {
        Some(&v) => Some(v.parse()?),
        None => None,
    };

    Ok(VersionMeta {
        semver,
        commit_hash,
        commit_date,
        build_date,
        channel,
        host,
        short_version_string,
        llvm_version,
    })
}

fn expect_key_or_unknown(key: &str, map: &HashMap<&str, &str>) -> Result<Option<String>, Error> {
    match map.get(key) {
        Some(&"unknown") => Ok(None),
        Some(&v) => Ok(Some(String::from(v))),
        None => Err(Error::UnexpectedVersionFormat),
    }
}

fn expect_key(key: &str, map: &HashMap<&str, &str>) -> Result<String, Error> {
    map.get(key)
        .map(|&v| String::from(v))
        .ok_or(Error::UnexpectedVersionFormat)
}

/// LLVM Version Parse Error
#[derive(Debug)]
pub enum LlvmVersionParseError {
    /// An error occurred in parsing a version component as an integer
    ParseIntError(num::ParseIntError),
    /// A version component must not have leading zeros
    ComponentMustNotHaveLeadingZeros,
    /// A version component has a sign
    ComponentMustNotHaveSign,
    /// Minor version component must be zero on LLVM versions later than 4.0
    MinorVersionMustBeZeroAfter4,
    /// Minor version component is required on LLVM versions earlier than 4.0
    MinorVersionRequiredBefore4,
    /// Too many components
    TooManyComponents,
}

impl From<num::ParseIntError> for LlvmVersionParseError {
    fn from(e: num::ParseIntError) -> Self {
        LlvmVersionParseError::ParseIntError(e)
    }
}

impl fmt::Display for LlvmVersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlvmVersionParseError::ParseIntError(e) => {
                write!(f, "error parsing LLVM version component: {}", e)
            }
            LlvmVersionParseError::ComponentMustNotHaveLeadingZeros => {
                write!(f, "a version component must not have leading zeros")
            }
            LlvmVersionParseError::ComponentMustNotHaveSign => {
                write!(f, "a version component must not have a sign")
            }
            LlvmVersionParseError::MinorVersionMustBeZeroAfter4 => write!(
                f,
                "LLVM's minor version component must be 0 for versions greater than 4.0"
            ),
            LlvmVersionParseError::MinorVersionRequiredBefore4 => write!(
                f,
                "LLVM's minor version component is required for versions less than 4.0"
            ),
            LlvmVersionParseError::TooManyComponents => write!(f, "too many version components"),
        }
    }
}

impl error::Error for LlvmVersionParseError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            LlvmVersionParseError::ParseIntError(e) => Some(e),
            LlvmVersionParseError::ComponentMustNotHaveLeadingZeros
            | LlvmVersionParseError::ComponentMustNotHaveSign
            | LlvmVersionParseError::MinorVersionMustBeZeroAfter4
            | LlvmVersionParseError::MinorVersionRequiredBefore4
            | LlvmVersionParseError::TooManyComponents => None,
        }
    }
}

/// The error type for this crate.
#[derive(Debug)]
pub enum Error {
    /// An error occurred while trying to find the `rustc` to run.
    CouldNotExecuteCommand(io::Error),
    /// Error output from the command that was run.
    CommandError {
        /// stdout output from the command
        stdout: String,
        /// stderr output from the command
        stderr: String,
    },
    /// The output of `rustc -vV` was not valid utf-8.
    Utf8Error(str::Utf8Error),
    /// The output of `rustc -vV` was not in the expected format.
    UnexpectedVersionFormat,
    /// An error occurred in parsing the semver.
    SemVerError(semver::Error),
    /// The pre-release tag is unknown.
    UnknownPreReleaseTag(String),
    /// An error occurred in parsing a `LlvmVersion`.
    LlvmVersionError(LlvmVersionParseError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CouldNotExecuteCommand(ref e) => write!(f, "could not execute command: {}", e),
            CommandError {
                ref stdout,
                ref stderr,
            } => write!(
                f,
                "error from command -- stderr:\n\n{}\n\nstderr:\n\n{}",
                stderr, stdout,
            ),
            Utf8Error(_) => write!(f, "invalid UTF-8 output from `rustc -vV`"),
            UnexpectedVersionFormat => write!(f, "unexpected `rustc -vV` format"),
            SemVerError(ref e) => write!(f, "error parsing version: {}", e),
            UnknownPreReleaseTag(ref i) => write!(f, "unknown pre-release tag: {}", i),
            LlvmVersionError(ref e) => write!(f, "error parsing LLVM's version: {}", e),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            CouldNotExecuteCommand(ref e) => Some(e),
            CommandError { .. } => None,
            Utf8Error(ref e) => Some(e),
            UnexpectedVersionFormat => None,
            SemVerError(ref e) => Some(e),
            UnknownPreReleaseTag(_) => None,
            LlvmVersionError(ref e) => Some(e),
        }
    }
}

macro_rules! impl_from {
    ($($err_ty:ty => $variant:ident),* $(,)*) => {
        $(
            impl From<$err_ty> for Error {
                fn from(e: $err_ty) -> Error {
                    Error::$variant(e)
                }
            }
        )*
    }
}

impl_from! {
    str::Utf8Error => Utf8Error,
    semver::Error => SemVerError,
    LlvmVersionParseError => LlvmVersionError,
}

/// The result type for this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
