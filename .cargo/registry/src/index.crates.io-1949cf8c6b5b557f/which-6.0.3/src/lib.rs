//! which
//!
//! A Rust equivalent of Unix command `which(1)`.
//! # Example:
//!
//! To find which rustc executable binary is using:
//!
//! ```no_run
//! use which::which;
//! use std::path::PathBuf;
//!
//! let result = which("rustc").unwrap();
//! assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
//!
//! ```

#![forbid(unsafe_code)]

mod checker;
mod error;
mod finder;
#[cfg(windows)]
mod helper;

#[cfg(feature = "regex")]
use std::borrow::Borrow;
use std::env;
use std::fmt;
use std::path;

use std::ffi::{OsStr, OsString};

use crate::checker::{CompositeChecker, ExecutableChecker, ExistedChecker};
pub use crate::error::*;
use crate::finder::Finder;

/// Find an executable binary's path by name.
///
/// If given an absolute path, returns it if the file exists and is executable.
///
/// If given a relative path, returns an absolute path to the file if
/// it exists and is executable.
///
/// If given a string without path separators, looks for a file named
/// `binary_name` at each directory in `$PATH` and if it finds an executable
/// file there, returns it.
///
/// # Example
///
/// ```no_run
/// use which::which;
/// use std::path::PathBuf;
///
/// let result = which::which("rustc").unwrap();
/// assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
///
/// ```
pub fn which<T: AsRef<OsStr>>(binary_name: T) -> Result<path::PathBuf> {
    which_all(binary_name).and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
}

/// Find an executable binary's path by name, ignoring `cwd`.
///
/// If given an absolute path, returns it if the file exists and is executable.
///
/// Does not resolve relative paths.
///
/// If given a string without path separators, looks for a file named
/// `binary_name` at each directory in `$PATH` and if it finds an executable
/// file there, returns it.
///
/// # Example
///
/// ```no_run
/// use which::which;
/// use std::path::PathBuf;
///
/// let result = which::which_global("rustc").unwrap();
/// assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
///
/// ```
pub fn which_global<T: AsRef<OsStr>>(binary_name: T) -> Result<path::PathBuf> {
    which_all_global(binary_name).and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
}

/// Find all binaries with `binary_name` using `cwd` to resolve relative paths.
pub fn which_all<T: AsRef<OsStr>>(binary_name: T) -> Result<impl Iterator<Item = path::PathBuf>> {
    let cwd = env::current_dir().ok();

    let binary_checker = build_binary_checker();

    let finder = Finder::new();

    finder.find(binary_name, env::var_os("PATH"), cwd, binary_checker)
}

/// Find all binaries with `binary_name` ignoring `cwd`.
pub fn which_all_global<T: AsRef<OsStr>>(
    binary_name: T,
) -> Result<impl Iterator<Item = path::PathBuf>> {
    let binary_checker = build_binary_checker();

    let finder = Finder::new();

    finder.find(
        binary_name,
        env::var_os("PATH"),
        Option::<&Path>::None,
        binary_checker,
    )
}

/// Find all binaries matching a regular expression in a the system PATH.
///
/// Only available when feature `regex` is enabled.
///
/// # Arguments
///
/// * `regex` - A regular expression to match binaries with
///
/// # Examples
///
/// Find Python executables:
///
/// ```no_run
/// use regex::Regex;
/// use which::which;
/// use std::path::PathBuf;
///
/// let re = Regex::new(r"python\d$").unwrap();
/// let binaries: Vec<PathBuf> = which::which_re(re).unwrap().collect();
/// let python_paths = vec![PathBuf::from("/usr/bin/python2"), PathBuf::from("/usr/bin/python3")];
/// assert_eq!(binaries, python_paths);
/// ```
///
/// Find all cargo subcommand executables on the path:
///
/// ```
/// use which::which_re;
/// use regex::Regex;
///
/// which_re(Regex::new("^cargo-.*").unwrap()).unwrap()
///     .for_each(|pth| println!("{}", pth.to_string_lossy()));
/// ```
#[cfg(feature = "regex")]
pub fn which_re(regex: impl Borrow<Regex>) -> Result<impl Iterator<Item = path::PathBuf>> {
    which_re_in(regex, env::var_os("PATH"))
}

/// Find `binary_name` in the path list `paths`, using `cwd` to resolve relative paths.
pub fn which_in<T, U, V>(binary_name: T, paths: Option<U>, cwd: V) -> Result<path::PathBuf>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
    V: AsRef<path::Path>,
{
    which_in_all(binary_name, paths, cwd)
        .and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
}

/// Find all binaries matching a regular expression in a list of paths.
///
/// Only available when feature `regex` is enabled.
///
/// # Arguments
///
/// * `regex` - A regular expression to match binaries with
/// * `paths` - A string containing the paths to search
///             (separated in the same way as the PATH environment variable)
///
/// # Examples
///
/// ```no_run
/// use regex::Regex;
/// use which::which;
/// use std::path::PathBuf;
///
/// let re = Regex::new(r"python\d$").unwrap();
/// let paths = Some("/usr/bin:/usr/local/bin");
/// let binaries: Vec<PathBuf> = which::which_re_in(re, paths).unwrap().collect();
/// let python_paths = vec![PathBuf::from("/usr/bin/python2"), PathBuf::from("/usr/bin/python3")];
/// assert_eq!(binaries, python_paths);
/// ```
#[cfg(feature = "regex")]
pub fn which_re_in<T>(
    regex: impl Borrow<Regex>,
    paths: Option<T>,
) -> Result<impl Iterator<Item = path::PathBuf>>
where
    T: AsRef<OsStr>,
{
    let binary_checker = build_binary_checker();

    let finder = Finder::new();

    finder.find_re(regex, paths, binary_checker)
}

/// Find all binaries with `binary_name` in the path list `paths`, using `cwd` to resolve relative paths.
pub fn which_in_all<T, U, V>(
    binary_name: T,
    paths: Option<U>,
    cwd: V,
) -> Result<impl Iterator<Item = path::PathBuf>>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
    V: AsRef<path::Path>,
{
    let binary_checker = build_binary_checker();

    let finder = Finder::new();

    finder.find(binary_name, paths, Some(cwd), binary_checker)
}

/// Find all binaries with `binary_name` in the path list `paths`, ignoring `cwd`.
pub fn which_in_global<T, U>(
    binary_name: T,
    paths: Option<U>,
) -> Result<impl Iterator<Item = path::PathBuf>>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
{
    let binary_checker = build_binary_checker();

    let finder = Finder::new();

    finder.find(binary_name, paths, Option::<&Path>::None, binary_checker)
}

fn build_binary_checker() -> CompositeChecker {
    CompositeChecker::new()
        .add_checker(Box::new(ExistedChecker::new()))
        .add_checker(Box::new(ExecutableChecker::new()))
}

/// A wrapper containing all functionality in this crate.
pub struct WhichConfig {
    cwd: Option<either::Either<bool, path::PathBuf>>,
    custom_path_list: Option<OsString>,
    binary_name: Option<OsString>,
    #[cfg(feature = "regex")]
    regex: Option<Regex>,
}

impl Default for WhichConfig {
    fn default() -> Self {
        Self {
            cwd: Some(either::Either::Left(true)),
            custom_path_list: None,
            binary_name: None,
            #[cfg(feature = "regex")]
            regex: None,
        }
    }
}

#[cfg(feature = "regex")]
type Regex = regex::Regex;

#[cfg(not(feature = "regex"))]
type Regex = ();

impl WhichConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether or not to use the current working directory. `true` by default.
    ///
    /// # Panics
    ///
    /// If regex was set previously, and you've just passed in `use_cwd: true`, this will panic.
    pub fn system_cwd(mut self, use_cwd: bool) -> Self {
        #[cfg(feature = "regex")]
        if self.regex.is_some() && use_cwd {
            panic!("which can't use regex and cwd at the same time!")
        }
        self.cwd = Some(either::Either::Left(use_cwd));
        self
    }

    /// Sets a custom path for resolving relative paths.
    ///
    /// # Panics
    ///
    /// If regex was set previously, this will panic.
    pub fn custom_cwd(mut self, cwd: path::PathBuf) -> Self {
        #[cfg(feature = "regex")]
        if self.regex.is_some() {
            panic!("which can't use regex and cwd at the same time!")
        }
        self.cwd = Some(either::Either::Right(cwd));
        self
    }

    /// Sets the path name regex to search for. You ***MUST*** call this, or [`Self::binary_name`] prior to searching.
    ///
    /// When `Regex` is disabled this function takes the unit type as a stand in. The parameter will change when
    /// `Regex` is enabled.
    ///
    /// # Panics
    ///
    /// If the `regex` feature wasn't turned on for this crate this will always panic. Additionally if a
    /// `cwd` (aka current working directory) or `binary_name` was set previously, this will panic, as those options
    /// are incompatible with `regex`.
    #[allow(unused_variables)]
    #[allow(unused_mut)]
    pub fn regex(mut self, regex: Regex) -> Self {
        #[cfg(not(feature = "regex"))]
        {
            panic!("which's regex feature was not enabled in your Cargo.toml!")
        }
        #[cfg(feature = "regex")]
        {
            if self.cwd != Some(either::Either::Left(false)) && self.cwd.is_some() {
                panic!("which can't use regex and cwd at the same time!")
            }
            if self.binary_name.is_some() {
                panic!("which can't use `binary_name` and `regex` at the same time!");
            }
            self.regex = Some(regex);
            self
        }
    }

    /// Sets the path name to search for. You ***MUST*** call this, or [`Self::regex`] prior to searching.
    ///
    /// # Panics
    ///
    /// If a `regex` was set previously this will panic as this is not compatible with `regex`.
    pub fn binary_name(mut self, name: OsString) -> Self {
        #[cfg(feature = "regex")]
        if self.regex.is_some() {
            panic!("which can't use `binary_name` and `regex` at the same time!");
        }
        self.binary_name = Some(name);
        self
    }

    /// Uses the given string instead of the `PATH` env variable.
    pub fn custom_path_list(mut self, custom_path_list: OsString) -> Self {
        self.custom_path_list = Some(custom_path_list);
        self
    }

    /// Uses the `PATH` env variable. Enabled by default.
    pub fn system_path_list(mut self) -> Self {
        self.custom_path_list = None;
        self
    }

    /// Finishes configuring, runs the query and returns the first result.
    pub fn first_result(self) -> Result<path::PathBuf> {
        self.all_results()
            .and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
    }

    /// Finishes configuring, runs the query and returns all results.
    pub fn all_results(self) -> Result<impl Iterator<Item = path::PathBuf>> {
        let binary_checker = build_binary_checker();

        let finder = Finder::new();

        let paths = self.custom_path_list.or_else(|| env::var_os("PATH"));

        #[cfg(feature = "regex")]
        if let Some(regex) = self.regex {
            return finder
                .find_re(regex, paths, binary_checker)
                .map(|i| Box::new(i) as Box<dyn Iterator<Item = path::PathBuf>>);
        }

        let cwd = match self.cwd {
            Some(either::Either::Left(false)) => None,
            Some(either::Either::Right(custom)) => Some(custom),
            None | Some(either::Either::Left(true)) => env::current_dir().ok(),
        };

        finder
            .find(
                self.binary_name.expect(
                    "binary_name not set! You must set binary_name or regex before searching!",
                ),
                paths,
                cwd,
                binary_checker,
            )
            .map(|i| Box::new(i) as Box<dyn Iterator<Item = path::PathBuf>>)
    }
}

/// An owned, immutable wrapper around a `PathBuf` containing the path of an executable.
///
/// The constructed `PathBuf` is the output of `which` or `which_in`, but `which::Path` has the
/// advantage of being a type distinct from `std::path::Path` and `std::path::PathBuf`.
///
/// It can be beneficial to use `which::Path` instead of `std::path::Path` when you want the type
/// system to enforce the need for a path that exists and points to a binary that is executable.
///
/// Since `which::Path` implements `Deref` for `std::path::Path`, all methods on `&std::path::Path`
/// are also available to `&which::Path` values.
#[derive(Clone, PartialEq, Eq)]
pub struct Path {
    inner: path::PathBuf,
}

impl Path {
    /// Returns the path of an executable binary by name.
    ///
    /// This calls `which` and maps the result into a `Path`.
    pub fn new<T: AsRef<OsStr>>(binary_name: T) -> Result<Path> {
        which(binary_name).map(|inner| Path { inner })
    }

    /// Returns the paths of all executable binaries by a name.
    ///
    /// this calls `which_all` and maps the results into `Path`s.
    pub fn all<T: AsRef<OsStr>>(binary_name: T) -> Result<impl Iterator<Item = Path>> {
        which_all(binary_name).map(|inner| inner.map(|inner| Path { inner }))
    }

    /// Returns the path of an executable binary by name in the path list `paths` and using the
    /// current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in` and maps the result into a `Path`.
    pub fn new_in<T, U, V>(binary_name: T, paths: Option<U>, cwd: V) -> Result<Path>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path>,
    {
        which_in(binary_name, paths, cwd).map(|inner| Path { inner })
    }

    /// Returns all paths of an executable binary by name in the path list `paths` and using the
    /// current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in_all` and maps the results into a `Path`.
    pub fn all_in<T, U, V>(
        binary_name: T,
        paths: Option<U>,
        cwd: V,
    ) -> Result<impl Iterator<Item = Path>>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path>,
    {
        which_in_all(binary_name, paths, cwd).map(|inner| inner.map(|inner| Path { inner }))
    }

    /// Returns a reference to a `std::path::Path`.
    pub fn as_path(&self) -> &path::Path {
        self.inner.as_path()
    }

    /// Consumes the `which::Path`, yielding its underlying `std::path::PathBuf`.
    pub fn into_path_buf(self) -> path::PathBuf {
        self.inner
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::ops::Deref for Path {
    type Target = path::Path;

    fn deref(&self) -> &path::Path {
        self.inner.deref()
    }
}

impl AsRef<path::Path> for Path {
    fn as_ref(&self) -> &path::Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for Path {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl PartialEq<path::PathBuf> for Path {
    fn eq(&self, other: &path::PathBuf) -> bool {
        self.inner == *other
    }
}

impl PartialEq<Path> for path::PathBuf {
    fn eq(&self, other: &Path) -> bool {
        *self == other.inner
    }
}

/// An owned, immutable wrapper around a `PathBuf` containing the _canonical_ path of an
/// executable.
///
/// The constructed `PathBuf` is the result of `which` or `which_in` followed by
/// `Path::canonicalize`, but `CanonicalPath` has the advantage of being a type distinct from
/// `std::path::Path` and `std::path::PathBuf`.
///
/// It can be beneficial to use `CanonicalPath` instead of `std::path::Path` when you want the type
/// system to enforce the need for a path that exists, points to a binary that is executable, is
/// absolute, has all components normalized, and has all symbolic links resolved
///
/// Since `CanonicalPath` implements `Deref` for `std::path::Path`, all methods on
/// `&std::path::Path` are also available to `&CanonicalPath` values.
#[derive(Clone, PartialEq, Eq)]
pub struct CanonicalPath {
    inner: path::PathBuf,
}

impl CanonicalPath {
    /// Returns the canonical path of an executable binary by name.
    ///
    /// This calls `which` and `Path::canonicalize` and maps the result into a `CanonicalPath`.
    pub fn new<T: AsRef<OsStr>>(binary_name: T) -> Result<CanonicalPath> {
        which(binary_name)
            .and_then(|p| p.canonicalize().map_err(|_| Error::CannotCanonicalize))
            .map(|inner| CanonicalPath { inner })
    }

    /// Returns the canonical paths of an executable binary by name.
    ///
    /// This calls `which_all` and `Path::canonicalize` and maps the results into `CanonicalPath`s.
    pub fn all<T: AsRef<OsStr>>(
        binary_name: T,
    ) -> Result<impl Iterator<Item = Result<CanonicalPath>>> {
        which_all(binary_name).map(|inner| {
            inner.map(|inner| {
                inner
                    .canonicalize()
                    .map_err(|_| Error::CannotCanonicalize)
                    .map(|inner| CanonicalPath { inner })
            })
        })
    }

    /// Returns the canonical path of an executable binary by name in the path list `paths` and
    /// using the current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in` and `Path::canonicalize` and maps the result into a `CanonicalPath`.
    pub fn new_in<T, U, V>(binary_name: T, paths: Option<U>, cwd: V) -> Result<CanonicalPath>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path>,
    {
        which_in(binary_name, paths, cwd)
            .and_then(|p| p.canonicalize().map_err(|_| Error::CannotCanonicalize))
            .map(|inner| CanonicalPath { inner })
    }

    /// Returns all of the canonical paths of an executable binary by name in the path list `paths` and
    /// using the current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in_all` and `Path::canonicalize` and maps the result into a `CanonicalPath`.
    pub fn all_in<T, U, V>(
        binary_name: T,
        paths: Option<U>,
        cwd: V,
    ) -> Result<impl Iterator<Item = Result<CanonicalPath>>>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path>,
    {
        which_in_all(binary_name, paths, cwd).map(|inner| {
            inner.map(|inner| {
                inner
                    .canonicalize()
                    .map_err(|_| Error::CannotCanonicalize)
                    .map(|inner| CanonicalPath { inner })
            })
        })
    }

    /// Returns a reference to a `std::path::Path`.
    pub fn as_path(&self) -> &path::Path {
        self.inner.as_path()
    }

    /// Consumes the `which::CanonicalPath`, yielding its underlying `std::path::PathBuf`.
    pub fn into_path_buf(self) -> path::PathBuf {
        self.inner
    }
}

impl fmt::Debug for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::ops::Deref for CanonicalPath {
    type Target = path::Path;

    fn deref(&self) -> &path::Path {
        self.inner.deref()
    }
}

impl AsRef<path::Path> for CanonicalPath {
    fn as_ref(&self) -> &path::Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for CanonicalPath {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl PartialEq<path::PathBuf> for CanonicalPath {
    fn eq(&self, other: &path::PathBuf) -> bool {
        self.inner == *other
    }
}

impl PartialEq<CanonicalPath> for path::PathBuf {
    fn eq(&self, other: &CanonicalPath) -> bool {
        *self == other.inner
    }
}
