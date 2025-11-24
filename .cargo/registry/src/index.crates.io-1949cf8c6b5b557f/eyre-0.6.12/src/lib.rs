//! This library provides [`eyre::Report`][Report], a trait object based
//! error handling type for easy idiomatic error handling and reporting in Rust
//! applications.
//!
//! This crate is a fork of [`anyhow`]  with support for customized
//! error reports. For more details on customization, check out the docs on
//! [`eyre::EyreHandler`].
//!
//! ## Custom Report Handlers
//!
//! The heart of this crate is its ability to swap out the Handler type to change
//! what information is carried alongside errors and how the end report is
//! formatted. This crate is meant to be used alongside companion crates that
//! customize its behavior. Below is a list of known crates that export report
//! handlers for eyre and short summaries of what features they provide.
//!
//! - [`stable-eyre`]: Switches the backtrace type from `std`'s to `backtrace-rs`'s
//!   so that it can be captured on stable. The report format is identical to
//!   `DefaultHandler`'s report format.
//! - [`color-eyre`]: Captures a `backtrace::Backtrace` and a
//!   `tracing_error::SpanTrace`. Provides a `Section` trait for attaching warnings
//!   and suggestions to error reports. The end report is then pretty printed with
//!   the help of [`color-backtrace`], [`color-spantrace`], and `ansi_term`. Check
//!   out the README on [`color-eyre`] for details on the report format.
//! - [`simple-eyre`]: A minimal `EyreHandler` that captures no additional
//!   information, for when you do not wish to capture `Backtrace`s with errors.
//! - [`jane-eyre`]: A report handler crate that exists purely for the pun.
//!   Currently just re-exports `color-eyre`.
//!
//! ## Usage Recommendations and Stability Considerations
//!
//! **We recommend users do not re-export types from this library as part their
//! own public API for libraries with external users.** The main reason for this
//! is that it will make your library API break if we ever bump the major version
//! number on eyre and your users upgrade the eyre version they use in their
//! application code before you upgrade your own eyre dep version[^1].
//!
//! However, even beyond this API stability hazard, there are other good reasons
//! to avoid using `eyre::Report` as your public error type.
//!
//! - You export an undocumented error interface that is otherwise still
//! accessible via downcast, making it hard for users to react to specific
//! errors while not preventing them from depending on details you didn't mean
//! to make part of your public API.
//!   - This in turn makes the error types of all libraries you use a part of
//!   your public API as well, and makes changing any of those libraries into
//!   undetectable runtime breakage.
//! - If many of your errors are constructed from strings, you encourage your
//! users to use string comparison for reacting to specific errors, which is
//! brittle and turns updating error messages into potentially undetectable
//! runtime breakage.
//!
//! ## Details
//!
//! - Use `Result<T, eyre::Report>`, or equivalently `eyre::Result<T>`, as the
//!   return type of any fallible function.
//!
//!   Within the function, use `?` to easily propagate any error that implements the
//!   `std::error::Error` trait.
//!
//!   ```rust
//!   # pub trait Deserialize {}
//!   #
//!   # mod serde_json {
//!   #     use super::Deserialize;
//!   #     use std::io;
//!   #
//!   #     pub fn from_str<T: Deserialize>(json: &str) -> io::Result<T> {
//!   #         unimplemented!()
//!   #     }
//!   # }
//!   #
//!   # struct ClusterMap;
//!   #
//!   # impl Deserialize for ClusterMap {}
//!   #
//!   use eyre::Result;
//!
//!   fn get_cluster_info() -> Result<ClusterMap> {
//!       let config = std::fs::read_to_string("cluster.json")?;
//!       let map: ClusterMap = serde_json::from_str(&config)?;
//!       Ok(map)
//!   }
//!   #
//!   # fn main() {}
//!   ```
//!
//! - Wrap a lower level error with a new error created from a message to help the
//!   person troubleshooting understand the chain of failures that occurred. A
//!   low-level error like "No such file or directory" can be annoying to debug
//!   without more information about what higher level step the application was in
//!   the middle of.
//!
//!   ```rust
//!   # struct It;
//!   #
//!   # impl It {
//!   #     fn detach(&self) -> Result<()> {
//!   #         unimplemented!()
//!   #     }
//!   # }
//!   #
//!   use eyre::{WrapErr, Result};
//!
//!   fn main() -> Result<()> {
//!       # return Ok(());
//!       #
//!       # const _: &str = stringify! {
//!       ...
//!       # };
//!       #
//!       # let it = It;
//!       # let path = "./path/to/instrs.json";
//!       #
//!       it.detach().wrap_err("Failed to detach the important thing")?;
//!
//!       let content = std::fs::read(path)
//!           .wrap_err_with(|| format!("Failed to read instrs from {}", path))?;
//!       #
//!       # const _: &str = stringify! {
//!       ...
//!       # };
//!       #
//!       # Ok(())
//!   }
//!   ```
//!
//!   ```console
//!   Error: Failed to read instrs from ./path/to/instrs.json
//!
//!   Caused by:
//!       No such file or directory (os error 2)
//!   ```
//!
//! - Downcasting is supported and can be done by value, by shared reference, or by
//!   mutable reference as needed.
//!
//!   ```rust
//!   # use eyre::{Report, eyre};
//!   # use std::fmt::{self, Display};
//!   # use std::task::Poll;
//!   #
//!   # #[derive(Debug)]
//!   # enum DataStoreError {
//!   #     Censored(()),
//!   # }
//!   #
//!   # impl Display for DataStoreError {
//!   #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//!   #         unimplemented!()
//!   #     }
//!   # }
//!   #
//!   # impl std::error::Error for DataStoreError {}
//!   #
//!   # const REDACTED_CONTENT: () = ();
//!   #
//!   # #[cfg(not(feature = "auto-install"))]
//!   # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
//!   #
//!   # let error: Report = eyre!("...");
//!   # let root_cause = &error;
//!   #
//!   # let ret =
//!   // If the error was caused by redaction, then return a
//!   // tombstone instead of the content.
//!   match root_cause.downcast_ref::<DataStoreError>() {
//!       Some(DataStoreError::Censored(_)) => Ok(Poll::Ready(REDACTED_CONTENT)),
//!       None => Err(error),
//!   }
//!   # ;
//!   ```
//!
//! - If using the nightly channel, a backtrace is captured and printed with the
//!   error if the underlying error type does not already provide its own. In order
//!   to see backtraces, they must be enabled through the environment variables
//!   described in [`std::backtrace`]:
//!
//!   - If you want panics and errors to both have backtraces, set
//!     `RUST_BACKTRACE=1`;
//!   - If you want only errors to have backtraces, set `RUST_LIB_BACKTRACE=1`;
//!   - If you want only panics to have backtraces, set `RUST_BACKTRACE=1` and
//!     `RUST_LIB_BACKTRACE=0`.
//!
//!   The tracking issue for this feature is [rust-lang/rust#53487].
//!
//!   [`std::backtrace`]: https://doc.rust-lang.org/std/backtrace/index.html#environment-variables
//!   [rust-lang/rust#53487]: https://github.com/rust-lang/rust/issues/53487
//!
//! - Eyre works with any error type that has an impl of `std::error::Error`,
//!   including ones defined in your crate. We do not bundle a `derive(Error)` macro
//!   but you can write the impls yourself or use a standalone macro like
//!   [thiserror].
//!
//!   ```rust
//!   use thiserror::Error;
//!
//!   #[derive(Error, Debug)]
//!   pub enum FormatError {
//!       #[error("Invalid header (expected {expected:?}, got {found:?})")]
//!       InvalidHeader {
//!           expected: String,
//!           found: String,
//!       },
//!       #[error("Missing attribute: {0}")]
//!       MissingAttribute(String),
//!   }
//!   ```
//!
//! - One-off error messages can be constructed using the `eyre!` macro, which
//!   supports string interpolation and produces an `eyre::Report`.
//!
//!   ```rust
//!   # use eyre::{eyre, Result};
//!   #
//!   # fn demo() -> Result<()> {
//!   #     let missing = "...";
//!   return Err(eyre!("Missing attribute: {}", missing));
//!   #     Ok(())
//!   # }
//!   ```
//!
//! - On newer versions of the compiler (i.e. 1.58 and later) this macro also
//!   supports format args captures.
//!
//!   ```rust
//!   # use eyre::{eyre, Result};
//!   #
//!   # fn demo() -> Result<()> {
//!   #     let missing = "...";
//!   # #[cfg(not(eyre_no_fmt_args_capture))]
//!   return Err(eyre!("Missing attribute: {missing}"));
//!   #     Ok(())
//!   # }
//!   ```
//!
//! ## No-std support
//!
//! No-std support was removed in 2020 in [commit 608a16a] due to unaddressed upstream breakages.
//!
//! [commit 608a16a]: https://github.com/eyre-rs/eyre/pull/29/commits/608a16aa2c2c27eca6c88001cc94c6973c18f1d5
//!
//! ## Comparison to failure
//!
//! The `eyre::Report` type works something like `failure::Error`, but unlike
//! failure ours is built around the standard library's `std::error::Error` trait
//! rather than a separate trait `failure::Fail`. The standard library has adopted
//! the necessary improvements for this to be possible as part of [RFC 2504].
//!
//! [RFC 2504]: https://github.com/rust-lang/rfcs/blob/master/text/2504-fix-error.md
//!
//! ## Comparison to thiserror
//!
//! Use `eyre` if you don't think you'll do anything with an error other than
//! report it. This is common in application code. Use `thiserror` if you think
//! you need an error type that can be handled via match or reported. This is
//! common in library crates where you don't know how your users will handle
//! your errors.
//!
//! [thiserror]: https://github.com/dtolnay/thiserror
//!
//! ## Compatibility with `anyhow`
//!
//! This crate does its best to be usable as a drop in replacement of `anyhow` and
//! vice-versa by re-exporting all of the renamed APIs with the names used in
//! `anyhow`, though there are some differences still.
//!
//! #### `Context` and `Option`
//!
//! As part of renaming `Context` to `WrapErr` we also intentionally do not
//! implement `WrapErr` for `Option`. This decision was made because `wrap_err`
//! implies that you're creating a new error that saves the old error as its
//! `source`. With `Option` there is no source error to wrap, so `wrap_err` ends up
//! being somewhat meaningless.
//!
//! Instead `eyre` offers [`OptionExt::ok_or_eyre`] to yield _static_ errors from `None`,
//! and intends for users to use the combinator functions provided by
//! `std`, converting `Option`s to `Result`s, for _dynamic_ errors.
//! So where you would write this with
//! anyhow:
//!
//! ```rust
//! use anyhow::Context;
//!
//! let opt: Option<()> = None;
//! let result_static = opt.context("static error message");
//! let result_dynamic = opt.with_context(|| format!("{} error message", "dynamic"));
//! ```
//!
//! With `eyre` we want users to write:
//!
//! ```rust
//! use eyre::{eyre, OptionExt, Result};
//!
//! # #[cfg(not(feature = "auto-install"))]
//! # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
//! #
//! let opt: Option<()> = None;
//! let result_static: Result<()> = opt.ok_or_eyre("static error message");
//! let result_dynamic: Result<()> = opt.ok_or_else(|| eyre!("{} error message", "dynamic"));
//! ```
//!
//! **NOTE**: However, to help with porting we do provide a `ContextCompat` trait which
//! implements `context` for options which you can import to make existing
//! `.context` calls compile.
//!
//! [^1]: example and explanation of breakage <https://github.com/eyre-rs/eyre/issues/30#issuecomment-647650361>
//!
//! [Report]: https://docs.rs/eyre/*/eyre/struct.Report.html
//! [`eyre::EyreHandler`]: https://docs.rs/eyre/*/eyre/trait.EyreHandler.html
//! [`eyre::WrapErr`]: https://docs.rs/eyre/*/eyre/trait.WrapErr.html
//! [`anyhow::Context`]: https://docs.rs/anyhow/*/anyhow/trait.Context.html
//! [`anyhow`]: https://github.com/dtolnay/anyhow
//! [`tracing_error::SpanTrace`]: https://docs.rs/tracing-error/*/tracing_error/struct.SpanTrace.html
//! [`stable-eyre`]: https://github.com/eyre-rs/stable-eyre
//! [`color-eyre`]: https://github.com/eyre-rs/color-eyre
//! [`jane-eyre`]: https://github.com/yaahc/jane-eyre
//! [`simple-eyre`]: https://github.com/eyre-rs/simple-eyre
//! [`color-spantrace`]: https://github.com/eyre-rs/color-spantrace
//! [`color-backtrace`]: https://github.com/athre0z/color-backtrace
#![doc(html_root_url = "https://docs.rs/eyre/0.6.12")]
#![cfg_attr(
    nightly,
    feature(rustdoc_missing_doc_code_examples),
    warn(rustdoc::missing_doc_code_examples)
)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_op_in_unsafe_fn,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    unconditional_recursion,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
#![cfg_attr(backtrace, feature(backtrace))]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![allow(
    clippy::needless_doctest_main,
    clippy::new_ret_no_self,
    clippy::wrong_self_convention
)]

extern crate alloc;

#[macro_use]
mod backtrace;
mod chain;
mod context;
mod error;
mod fmt;
mod kind;
mod macros;
mod option;
mod ptr;
mod wrapper;

use crate::backtrace::Backtrace;
use crate::error::ErrorImpl;
use core::fmt::{Debug, Display};

use std::error::Error as StdError;

pub use eyre as format_err;
/// Compatibility re-export of `eyre` for interop with `anyhow`
pub use eyre as anyhow;
use once_cell::sync::OnceCell;
use ptr::OwnedPtr;
#[doc(hidden)]
pub use DefaultHandler as DefaultContext;
#[doc(hidden)]
pub use EyreHandler as EyreContext;
#[doc(hidden)]
pub use Report as ErrReport;
/// Compatibility re-export of `Report` for interop with `anyhow`
pub use Report as Error;
/// Compatibility re-export of `WrapErr` for interop with `anyhow`
pub use WrapErr as Context;

/// The core error reporting type of the library, a wrapper around a dynamic error reporting type.
///
/// `Report` works a lot like `Box<dyn std::error::Error>`, but with these
/// differences:
///
/// - `Report` requires that the error is `Send`, `Sync`, and `'static`.
/// - `Report` guarantees that a backtrace is available, even if the underlying
///   error type does not provide one.
/// - `Report` is represented as a narrow pointer &mdash; exactly one word in
///   size instead of two.
///
/// # Display representations
///
/// When you print an error object using "{}" or to_string(), only the outermost underlying error
/// is printed, not any of the lower level causes. This is exactly as if you had called the Display
/// impl of the error from which you constructed your eyre::Report.
///
/// ```console
/// Failed to read instrs from ./path/to/instrs.json
/// ```
///
/// To print causes as well using eyre's default formatting of causes, use the
/// alternate selector "{:#}".
///
/// ```console
/// Failed to read instrs from ./path/to/instrs.json: No such file or directory (os error 2)
/// ```
///
/// The Debug format "{:?}" includes your backtrace if one was captured. Note
/// that this is the representation you get by default if you return an error
/// from `fn main` instead of printing it explicitly yourself.
///
/// ```console
/// Error: Failed to read instrs from ./path/to/instrs.json
///
/// Caused by:
///     No such file or directory (os error 2)
///
/// Stack backtrace:
///    0: <E as eyre::context::ext::StdError>::ext_report
///              at /git/eyre/src/backtrace.rs:26
///    1: core::result::Result<T,E>::map_err
///              at /git/rustc/src/libcore/result.rs:596
///    2: eyre::context::<impl eyre::WrapErr<T,E,H> for core::result::Result<T,E>>::wrap_err_with
///              at /git/eyre/src/context.rs:58
///    3: testing::main
///              at src/main.rs:5
///    4: std::rt::lang_start
///              at /git/rustc/src/libstd/rt.rs:61
///    5: main
///    6: __libc_start_main
///    7: _start
/// ```
///
/// To see a conventional struct-style Debug representation, use "{:#?}".
///
/// ```console
/// Error {
///     msg: "Failed to read instrs from ./path/to/instrs.json",
///     source: Os {
///         code: 2,
///         kind: NotFound,
///         message: "No such file or directory",
///     },
/// }
/// ```
///
/// If none of the built-in representations are appropriate and you would prefer
/// to render the error and its cause chain yourself, it can be done by defining
/// your own [`EyreHandler`] and [`hook`] to use it.
///
/// [`EyreHandler`]: trait.EyreHandler.html
/// [`hook`]: fn.set_hook.html
#[must_use]
pub struct Report {
    inner: OwnedPtr<ErrorImpl<()>>,
}

type ErrorHook =
    Box<dyn Fn(&(dyn StdError + 'static)) -> Box<dyn EyreHandler> + Sync + Send + 'static>;

static HOOK: OnceCell<ErrorHook> = OnceCell::new();

/// Error indicating that `set_hook` was unable to install the provided ErrorHook
#[derive(Debug, Clone, Copy)]
pub struct InstallError;

impl core::fmt::Display for InstallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("cannot install provided ErrorHook, a hook has already been installed")
    }
}

impl StdError for InstallError {}

/// Install the provided error hook for constructing EyreHandlers when converting
/// Errors to Reports
///
/// # Details
///
/// To customize the format and content of error reports from `eyre` you must
/// first define a new `EyreHandler` type to capture and store the extra context
/// and to define the format of how to display the chain of errors and this
/// stored context. Once this type has been defined you must also define a global
/// hook used to construct these handlers whenever `Report`s are constructed.
///
/// # Examples
///
/// ```rust,should_panic
/// use backtrace::Backtrace;
/// use eyre::EyreHandler;
/// use std::error::Error;
/// use std::{fmt, iter};
///
/// fn main() -> eyre::Result<()> {
///     // Install our custom eyre report hook for constructing our custom Handlers
///     install().unwrap();
///
///     // construct a report with, hopefully, our custom handler!
///     let mut report = eyre::eyre!("hello from custom error town!");
///
///     // manually set the custom msg for this report after it has been constructed
///     if let Some(handler) = report.handler_mut().downcast_mut::<Handler>() {
///         handler.custom_msg = Some("you're the best users, you know that right???");
///     }
///
///     // print that shit!!
///     Err(report)
/// }
///
/// // define a handler that captures backtraces unless told not to
/// fn install() -> Result<(), impl Error> {
///     let capture_backtrace = std::env::var("RUST_BACKWARDS_TRACE")
///         .map(|val| val != "0")
///         .unwrap_or(true);
///
///     let hook = Hook { capture_backtrace };
///
///     eyre::set_hook(Box::new(move |e| Box::new(hook.make_handler(e))))
/// }
///
/// struct Hook {
///     capture_backtrace: bool,
/// }
///
/// impl Hook {
///     fn make_handler(&self, _error: &(dyn Error + 'static)) -> Handler {
///         let backtrace = if self.capture_backtrace {
///             Some(Backtrace::new())
///         } else {
///             None
///         };
///
///         Handler {
///             backtrace,
///             custom_msg: None,
///         }
///     }
/// }
///
/// struct Handler {
///     // custom configured backtrace capture
///     backtrace: Option<Backtrace>,
///     // customizable message payload associated with reports
///     custom_msg: Option<&'static str>,
/// }
///
/// impl EyreHandler for Handler {
///     fn debug(&self, error: &(dyn Error + 'static), f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         if f.alternate() {
///             return fmt::Debug::fmt(error, f);
///         }
///
///         let errors = iter::successors(Some(error), |error| (*error).source());
///
///         for (ind, error) in errors.enumerate() {
///             write!(f, "\n{:>4}: {}", ind, error)?;
///         }
///
///         if let Some(backtrace) = self.backtrace.as_ref() {
///             writeln!(f, "\n\nBacktrace:\n{:?}", backtrace)?;
///         }
///
///         if let Some(msg) = self.custom_msg.as_ref() {
///             writeln!(f, "\n\n{}", msg)?;
///         }
///
///         Ok(())
///     }
/// }
/// ```
pub fn set_hook(hook: ErrorHook) -> Result<(), InstallError> {
    HOOK.set(hook).map_err(|_| InstallError)
}

#[cfg_attr(track_caller, track_caller)]
#[cfg_attr(not(track_caller), allow(unused_mut))]
fn capture_handler(error: &(dyn StdError + 'static)) -> Box<dyn EyreHandler> {
    #[cfg(not(feature = "auto-install"))]
    let hook = HOOK
        .get()
        .expect("a handler must always be installed if the `auto-install` feature is disabled")
        .as_ref();

    #[cfg(feature = "auto-install")]
    let hook = HOOK
        .get_or_init(|| Box::new(DefaultHandler::default_with))
        .as_ref();

    let mut handler = hook(error);

    #[cfg(track_caller)]
    {
        handler.track_caller(std::panic::Location::caller())
    }

    handler
}

impl dyn EyreHandler {
    ///
    pub fn is<T: EyreHandler>(&self) -> bool {
        // Get `TypeId` of the type this function is instantiated with.
        let t = core::any::TypeId::of::<T>();

        // Get `TypeId` of the type in the trait object (`self`).
        let concrete = self.type_id();

        // Compare both `TypeId`s on equality.
        t == concrete
    }

    ///
    pub fn downcast_ref<T: EyreHandler>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const dyn EyreHandler as *const T)) }
        } else {
            None
        }
    }

    ///
    pub fn downcast_mut<T: EyreHandler>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut dyn EyreHandler as *mut T)) }
        } else {
            None
        }
    }
}

/// Error Report Handler trait for customizing `eyre::Report`
pub trait EyreHandler: core::any::Any + Send + Sync {
    /// Define the report format
    ///
    /// Used to override the report format of `eyre::Report`
    ///
    /// # Example
    ///
    /// ```rust
    /// use backtrace::Backtrace;
    /// use eyre::EyreHandler;
    /// use eyre::Chain;
    /// use std::error::Error;
    /// use indenter::indented;
    ///
    /// pub struct Handler {
    ///     backtrace: Backtrace,
    /// }
    ///
    /// impl EyreHandler for Handler {
    ///     fn debug(
    ///         &self,
    ///         error: &(dyn Error + 'static),
    ///         f: &mut core::fmt::Formatter<'_>,
    ///     ) -> core::fmt::Result {
    ///         use core::fmt::Write as _;
    ///
    ///         if f.alternate() {
    ///             return core::fmt::Debug::fmt(error, f);
    ///         }
    ///
    ///         write!(f, "{}", error)?;
    ///
    ///         if let Some(cause) = error.source() {
    ///             write!(f, "\n\nCaused by:")?;
    ///             let multiple = cause.source().is_some();
    ///
    ///             for (n, error) in Chain::new(cause).enumerate() {
    ///                 writeln!(f)?;
    ///                 if multiple {
    ///                     write!(indented(f).ind(n), "{}", error)?;
    ///                 } else {
    ///                     write!(indented(f), "{}", error)?;
    ///                 }
    ///             }
    ///         }
    ///
    ///         let backtrace = &self.backtrace;
    ///         write!(f, "\n\nStack backtrace:\n{:?}", backtrace)?;
    ///
    ///         Ok(())
    ///     }
    /// }
    /// ```
    fn debug(
        &self,
        error: &(dyn StdError + 'static),
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result;

    /// Override for the `Display` format
    fn display(
        &self,
        error: &(dyn StdError + 'static),
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(f, "{}", error)?;

        if f.alternate() {
            for cause in crate::chain::Chain::new(error).skip(1) {
                write!(f, ": {}", cause)?;
            }
        }

        Result::Ok(())
    }

    /// Store the location of the caller who constructed this error report
    #[allow(unused_variables)]
    fn track_caller(&mut self, location: &'static std::panic::Location<'static>) {}
}

/// The default provided error report handler for `eyre::Report`.
///
/// On nightly this supports conditionally capturing a `std::backtrace::Backtrace` if the source
/// error did not already capture one.
#[allow(dead_code)]
pub struct DefaultHandler {
    backtrace: Option<Backtrace>,
    #[cfg(track_caller)]
    location: Option<&'static std::panic::Location<'static>>,
}

impl DefaultHandler {
    /// Manual hook which constructs `DefaultHandler`s.
    ///
    /// # Details
    ///
    /// When supplied to the `set_hook` function, `default_with` will cause `eyre::Report` to use
    /// `DefaultHandler` as the error report handler.
    ///
    /// If the `auto-install` feature is enabled, and a user-provided hook for constructing
    /// `EyreHandlers` was not installed using `set_hook`, `DefaultHandler::default_with`
    /// is automatically installed as the hook.
    ///
    /// # Example
    ///
    /// ```rust,should_panic
    /// use eyre::{DefaultHandler, eyre, InstallError, Result, set_hook};
    ///
    /// fn main() -> Result<()> {
    ///     install_default().expect("default handler inexplicably already installed");
    ///     Err(eyre!("hello from default error city!"))
    /// }
    ///
    /// fn install_default() -> Result<(), InstallError> {
    ///     set_hook(Box::new(DefaultHandler::default_with))
    /// }
    ///
    /// ```
    #[allow(unused_variables)]
    #[cfg_attr(not(feature = "auto-install"), allow(dead_code))]
    pub fn default_with(error: &(dyn StdError + 'static)) -> Box<dyn EyreHandler> {
        let backtrace = backtrace_if_absent!(error);

        Box::new(Self {
            backtrace,
            #[cfg(track_caller)]
            location: None,
        })
    }
}

impl core::fmt::Debug for DefaultHandler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DefaultHandler")
            .field(
                "backtrace",
                match &self.backtrace {
                    Some(_) => &"Some(Backtrace { ... })",
                    None => &"None",
                },
            )
            .finish()
    }
}

impl EyreHandler for DefaultHandler {
    fn debug(
        &self,
        error: &(dyn StdError + 'static),
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        use core::fmt::Write as _;

        if f.alternate() {
            return core::fmt::Debug::fmt(error, f);
        }

        write!(f, "{}", error)?;

        if let Some(cause) = error.source() {
            write!(f, "\n\nCaused by:")?;
            let multiple = cause.source().is_some();
            for (n, error) in crate::chain::Chain::new(cause).enumerate() {
                writeln!(f)?;
                if multiple {
                    write!(indenter::indented(f).ind(n), "{}", error)?;
                } else {
                    write!(indenter::indented(f), "{}", error)?;
                }
            }
        }

        #[cfg(all(track_caller, feature = "track-caller"))]
        {
            if let Some(location) = self.location {
                write!(f, "\n\nLocation:\n")?;
                write!(indenter::indented(f), "{}", location)?;
            }
        }

        #[cfg(backtrace)]
        {
            use std::backtrace::BacktraceStatus;

            let backtrace = self
                .backtrace
                .as_ref()
                .or_else(|| error.backtrace())
                .expect("backtrace capture failed");
            if let BacktraceStatus::Captured = backtrace.status() {
                write!(f, "\n\nStack backtrace:\n{}", backtrace)?;
            }
        }

        Result::Ok(())
    }

    #[cfg(track_caller)]
    fn track_caller(&mut self, location: &'static std::panic::Location<'static>) {
        self.location = Some(location);
    }
}

/// Iterator of a chain of source errors.
///
/// This type is the iterator returned by [`Report::chain`].
///
/// # Example
///
/// ```
/// use eyre::Report;
/// use std::io;
///
/// pub fn underlying_io_error_kind(error: &Report) -> Option<io::ErrorKind> {
///     for cause in error.chain() {
///         if let Some(io_error) = cause.downcast_ref::<io::Error>() {
///             return Some(io_error.kind());
///         }
///     }
///     None
/// }
/// ```
#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct Chain<'a> {
    state: crate::chain::ChainState<'a>,
}

/// type alias for `Result<T, Report>`
///
/// This is a reasonable return type to use throughout your application but also for `fn main`; if
/// you do, failures will be printed along with a backtrace if one was captured.
///
/// `eyre::Result` may be used with one *or* two type parameters.
///
/// ```rust
/// use eyre::Result;
///
/// # const IGNORE: &str = stringify! {
/// fn demo1() -> Result<T> {...}
///            // ^ equivalent to std::result::Result<T, eyre::Report>
///
/// fn demo2() -> Result<T, OtherError> {...}
///            // ^ equivalent to std::result::Result<T, OtherError>
/// # };
/// ```
///
/// # Example
///
/// ```
/// # pub trait Deserialize {}
/// #
/// # mod serde_json {
/// #     use super::Deserialize;
/// #     use std::io;
/// #
/// #     pub fn from_str<T: Deserialize>(json: &str) -> io::Result<T> {
/// #         unimplemented!()
/// #     }
/// # }
/// #
/// # #[derive(Debug)]
/// # struct ClusterMap;
/// #
/// # impl Deserialize for ClusterMap {}
/// #
/// use eyre::Result;
///
/// fn main() -> Result<()> {
///     # return Ok(());
///     let config = std::fs::read_to_string("cluster.json")?;
///     let map: ClusterMap = serde_json::from_str(&config)?;
///     println!("cluster info: {:#?}", map);
///     Ok(())
/// }
/// ```
pub type Result<T, E = Report> = core::result::Result<T, E>;

/// Provides the `wrap_err` method for `Result`.
///
/// This trait is sealed and cannot be implemented for types outside of
/// `eyre`.
///
/// # Example
///
/// ```
/// use eyre::{WrapErr, Result};
/// use std::fs;
/// use std::path::PathBuf;
///
/// pub struct ImportantThing {
///     path: PathBuf,
/// }
///
/// impl ImportantThing {
///     # const IGNORE: &'static str = stringify! {
///     pub fn detach(&mut self) -> Result<()> {...}
///     # };
///     # fn detach(&mut self) -> Result<()> {
///     #     unimplemented!()
///     # }
/// }
///
/// pub fn do_it(mut it: ImportantThing) -> Result<Vec<u8>> {
///     it.detach().wrap_err("Failed to detach the important thing")?;
///
///     let path = &it.path;
///     let content = fs::read(path)
///         .wrap_err_with(|| format!("Failed to read instrs from {}", path.display()))?;
///
///     Ok(content)
/// }
/// ```
///
/// When printed, the outermost error would be printed first and the lower
/// level underlying causes would be enumerated below.
///
/// ```console
/// Error: Failed to read instrs from ./path/to/instrs.json
///
/// Caused by:
///     No such file or directory (os error 2)
/// ```
///
/// # Wrapping Types That Don't impl `Error` (e.g. `&str` and `Box<dyn Error>`)
///
/// Due to restrictions for coherence `Report` cannot impl `From` for types that don't impl
/// `Error`. Attempts to do so will give "this type might implement Error in the future" as an
/// error. As such, `wrap_err`, which uses `From` under the hood, cannot be used to wrap these
/// types. Instead we encourage you to use the combinators provided for `Result` in `std`/`core`.
///
/// For example, instead of this:
///
/// ```rust,compile_fail
/// use std::error::Error;
/// use eyre::{WrapErr, Report};
///
/// fn wrap_example(err: Result<(), Box<dyn Error + Send + Sync + 'static>>) -> Result<(), Report> {
///     err.wrap_err("saw a downstream error")
/// }
/// ```
///
/// We encourage you to write this:
///
/// ```rust
/// use std::error::Error;
/// use eyre::{WrapErr, Report, eyre};
///
/// fn wrap_example(err: Result<(), Box<dyn Error + Send + Sync + 'static>>) -> Result<(), Report> {
///     err.map_err(|e| eyre!(e)).wrap_err("saw a downstream error")
/// }
/// ```
///
/// # Effect on downcasting
///
/// After attaching a message of type `D` onto an error of type `E`, the resulting
/// `eyre::Report` may be downcast to `D` **or** to `E`.
///
/// That is, in codebases that rely on downcasting, Eyre's wrap_err supports
/// both of the following use cases:
///
///   - **Attaching messages whose type is insignificant onto errors whose type
///     is used in downcasts.**
///
///     In other error libraries whose wrap_err is not designed this way, it can
///     be risky to introduce messages to existing code because new message might
///     break existing working downcasts. In Eyre, any downcast that worked
///     before adding the message will continue to work after you add a message, so
///     you should freely wrap errors wherever it would be helpful.
///
///     ```
///     # use eyre::bail;
///     # use thiserror::Error;
///     #
///     # #[derive(Error, Debug)]
///     # #[error("???")]
///     # struct SuspiciousError;
///     #
///     # fn helper() -> Result<()> {
///     #     bail!(SuspiciousError);
///     # }
///     #
///     use eyre::{WrapErr, Result};
///
///     fn do_it() -> Result<()> {
///         helper().wrap_err("Failed to complete the work")?;
///         # const IGNORE: &str = stringify! {
///         ...
///         # };
///         # unreachable!()
///     }
///
///     fn main() {
///         # #[cfg(not(feature = "auto-install"))]
///         # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
///         let err = do_it().unwrap_err();
///         if let Some(e) = err.downcast_ref::<SuspiciousError>() {
///             // If helper() returned SuspiciousError, this downcast will
///             // correctly succeed even with the message in between.
///             # return;
///         }
///         # panic!("expected downcast to succeed");
///     }
///     ```
///
///   - **Attaching message whose type is used in downcasts onto errors whose
///     type is insignificant.**
///
///     Some codebases prefer to use machine-readable messages to categorize
///     lower level errors in a way that will be actionable to higher levels of
///     the application.
///
///     ```
///     # use eyre::bail;
///     # use thiserror::Error;
///     #
///     # #[derive(Error, Debug)]
///     # #[error("???")]
///     # struct HelperFailed;
///     #
///     # fn helper() -> Result<()> {
///     #     bail!("no such file or directory");
///     # }
///     #
///     use eyre::{WrapErr, Result};
///
///     fn do_it() -> Result<()> {
///         helper().wrap_err(HelperFailed)?;
///         # const IGNORE: &str = stringify! {
///         ...
///         # };
///         # unreachable!()
///     }
///
///     fn main() {
///         # #[cfg(not(feature = "auto-install"))]
///         # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
///         let err = do_it().unwrap_err();
///         if let Some(e) = err.downcast_ref::<HelperFailed>() {
///             // If helper failed, this downcast will succeed because
///             // HelperFailed is the message that has been attached to
///             // that error.
///             # return;
///         }
///         # panic!("expected downcast to succeed");
///     }
///     ```
///
/// # `wrap_err` vs `wrap_err_with`
///
/// `wrap_err` incurs a runtime cost even in the non-error case because it requires eagerly
/// constructing the error object. `wrap_err_with` avoids this cost through lazy evaluation. This
/// cost is proportional to the cost of the currently installed [`EyreHandler`]'s creation step.
/// `wrap_err` is useful in cases where an constructed error object already exists.
pub trait WrapErr<T, E>: context::private::Sealed {
    /// Wrap the error value with a new adhoc error
    #[cfg_attr(track_caller, track_caller)]
    fn wrap_err<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Wrap the error value with a new adhoc error that is evaluated lazily
    /// only once an error does occur.
    #[cfg_attr(track_caller, track_caller)]
    fn wrap_err_with<D, F>(self, f: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Compatibility re-export of wrap_err for interop with `anyhow`
    #[cfg_attr(track_caller, track_caller)]
    fn context<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Compatibility re-export of wrap_err_with for interop with `anyhow`
    #[cfg_attr(track_caller, track_caller)]
    fn with_context<D, F>(self, f: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;
}

/// Provides the [`ok_or_eyre`][OptionExt::ok_or_eyre] method for [`Option`].
///
/// This trait is sealed and cannot be implemented for types outside of
/// `eyre`.
///
/// # Example
///
/// ```
/// # #[cfg(not(feature = "auto-install"))]
/// # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
/// use eyre::OptionExt;
///
/// let option: Option<()> = None;
///
/// let result = option.ok_or_eyre("static str error");
///
/// assert_eq!(result.unwrap_err().to_string(), "static str error");
/// ```
///
/// # `ok_or_eyre` vs `ok_or_else`
///
/// If string interpolation is required for the generated [report][Report],
/// use [`ok_or_else`][Option::ok_or_else] instead,
/// invoking [`eyre!`] to perform string interpolation:
///
/// ```
/// # #[cfg(not(feature = "auto-install"))]
/// # eyre::set_hook(Box::new(eyre::DefaultHandler::default_with)).unwrap();
/// use eyre::eyre;
///
/// let option: Option<()> = None;
///
/// let result = option.ok_or_else(|| eyre!("{} error", "dynamic"));
///
/// assert_eq!(result.unwrap_err().to_string(), "dynamic error");
/// ```
///
/// `ok_or_eyre` incurs no runtime cost, as the error object
/// is constructed from the provided static argument
/// only in the `None` case.
pub trait OptionExt<T>: context::private::Sealed {
    /// Transform the [`Option<T>`] into a [`Result<T, E>`],
    /// mapping [`Some(v)`][Option::Some] to [`Ok(v)`][Result::Ok]
    /// and [`None`] to [`Report`].
    ///
    /// `ok_or_eyre` allows for eyre [`Report`] error objects
    /// to be lazily created from static messages in the `None` case.
    ///
    /// For dynamic error messages, use [`ok_or_else`][Option::ok_or_else],
    /// invoking [`eyre!`] in the closure to perform string interpolation.
    fn ok_or_eyre<M>(self, message: M) -> crate::Result<T>
    where
        M: Debug + Display + Send + Sync + 'static;
}

/// Provides the `context` method for `Option` when porting from `anyhow`
///
/// This trait is sealed and cannot be implemented for types outside of
/// `eyre`.
///
/// ## Why Doesn't `Eyre` impl `WrapErr` for `Option`?
///
/// `eyre` doesn't impl `WrapErr` for `Option` because `wrap_err` implies that you're creating a
/// new error that saves the previous error as its `source`. Calling `wrap_err` on an `Option` is
/// meaningless because there is no source error. `anyhow` avoids this issue by using a different
/// mental model where you're adding "context" to an error, though this not a mental model for
/// error handling that `eyre` agrees with.
///
/// Instead, `eyre` encourages users to think of each error as distinct, where the previous error
/// is the context being saved by the new error, which is backwards compared to anyhow's model. In
/// this model you're encouraged to use combinators provided by `std` for `Option` to convert an
/// option to a `Result`
///
/// # Example
///
/// Instead of:
///
/// ```rust
/// use eyre::ContextCompat;
///
/// fn get_thing(mut things: impl Iterator<Item = u32>) -> eyre::Result<u32> {
///     things
///         .find(|&thing| thing == 42)
///         .context("the thing wasnt in the list")
/// }
/// ```
///
/// We encourage you to use this:
///
/// ```rust
/// use eyre::eyre;
///
/// fn get_thing(mut things: impl Iterator<Item = u32>) -> eyre::Result<u32> {
///     things
///         .find(|&thing| thing == 42)
///         .ok_or_else(|| eyre!("the thing wasnt in the list"))
/// }
/// ```
pub trait ContextCompat<T>: context::private::Sealed {
    /// Compatibility version of `wrap_err` for creating new errors with new source on `Option`
    /// when porting from `anyhow`
    #[cfg_attr(track_caller, track_caller)]
    fn context<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Compatibility version of `wrap_err_with` for creating new errors with new source on `Option`
    /// when porting from `anyhow`
    #[cfg_attr(track_caller, track_caller)]
    fn with_context<D, F>(self, f: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Compatibility re-export of `context` for porting from `anyhow` to `eyre`
    #[cfg_attr(track_caller, track_caller)]
    fn wrap_err<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Compatibility re-export of `with_context` for porting from `anyhow` to `eyre`
    #[cfg_attr(track_caller, track_caller)]
    fn wrap_err_with<D, F>(self, f: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;
}

/// Equivalent to Ok::<_, eyre::Error>(value).
///
/// This simplifies creation of an eyre::Result in places where type inference
/// cannot deduce the `E` type of the result &mdash; without needing to write
/// `Ok::<_, eyre::Error>(value)`.
///
/// One might think that `eyre::Result::Ok(value)` would work in such cases
/// but it does not.
///
/// ```console
/// error[E0282]: type annotations needed for `std::result::Result<i32, E>`
///   --> src/main.rs:11:13
///    |
/// 11 |     let _ = eyre::Result::Ok(1);
///    |         -   ^^^^^^^^^^^^^^^^ cannot infer type for type parameter `E` declared on the enum `Result`
///    |         |
///    |         consider giving this pattern the explicit type `std::result::Result<i32, E>`, where the type parameter `E` is specified
/// ```
#[allow(non_snake_case)]
pub fn Ok<T>(t: T) -> Result<T> {
    Result::Ok(t)
}

// Not public API. Referenced by macro-generated code.
#[doc(hidden)]
pub mod private {
    use crate::Report;
    use alloc::fmt;
    use core::fmt::{Arguments, Debug, Display};

    pub use alloc::format;
    pub use core::format_args;
    pub use core::result::Result::Err;

    #[doc(hidden)]
    pub mod kind {
        pub use crate::kind::{AdhocKind, TraitKind};

        pub use crate::kind::BoxedKind;
    }

    #[cfg_attr(track_caller, track_caller)]
    pub fn new_adhoc<M>(message: M) -> Report
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Report::from_adhoc(message)
    }

    #[doc(hidden)]
    #[cold]
    #[cfg_attr(track_caller, track_caller)]
    pub fn format_err(args: Arguments<'_>) -> Report {
        #[cfg(eyre_no_fmt_arguments_as_str)]
        let fmt_arguments_as_str: Option<&str> = None;
        #[cfg(not(eyre_no_fmt_arguments_as_str))]
        let fmt_arguments_as_str = args.as_str();

        if let Some(message) = fmt_arguments_as_str {
            // eyre!("literal"), can downcast to &'static str
            Report::msg(message)
        } else {
            // eyre!("interpolate {var}"), can downcast to String
            Report::msg(fmt::format(args))
        }
    }
}
