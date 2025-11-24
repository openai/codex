//! [![github]](https://github.com/dtolnay/anyhow)&ensp;[![crates-io]](https://crates.io/crates/anyhow)&ensp;[![docs-rs]](https://docs.rs/anyhow)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! This library provides [`anyhow::Error`][Error], a trait object based error
//! type for easy idiomatic error handling in Rust applications.
//!
//! <br>
//!
//! # Details
//!
//! - Use `Result<T, anyhow::Error>`, or equivalently `anyhow::Result<T>`, as
//!   the return type of any fallible function.
//!
//!   Within the function, use `?` to easily propagate any error that implements
//!   the [`std::error::Error`] trait.
//!
//!   ```
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
//!   use anyhow::Result;
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
//! - Attach context to help the person troubleshooting the error understand
//!   where things went wrong. A low-level error like "No such file or
//!   directory" can be annoying to debug without more context about what higher
//!   level step the application was in the middle of.
//!
//!   ```
//!   # struct It;
//!   #
//!   # impl It {
//!   #     fn detach(&self) -> Result<()> {
//!   #         unimplemented!()
//!   #     }
//!   # }
//!   #
//!   use anyhow::{Context, Result};
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
//!       it.detach().context("Failed to detach the important thing")?;
//!
//!       let content = std::fs::read(path)
//!           .with_context(|| format!("Failed to read instrs from {}", path))?;
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
//! - Downcasting is supported and can be by value, by shared reference, or by
//!   mutable reference as needed.
//!
//!   ```
//!   # use anyhow::anyhow;
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
//!   # let error = anyhow!("...");
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
//! - If using Rust &ge; 1.65, a backtrace is captured and printed with the
//!   error if the underlying error type does not already provide its own. In
//!   order to see backtraces, they must be enabled through the environment
//!   variables described in [`std::backtrace`]:
//!
//!   - If you want panics and errors to both have backtraces, set
//!     `RUST_BACKTRACE=1`;
//!   - If you want only errors to have backtraces, set `RUST_LIB_BACKTRACE=1`;
//!   - If you want only panics to have backtraces, set `RUST_BACKTRACE=1` and
//!     `RUST_LIB_BACKTRACE=0`.
//!
//!   [`std::backtrace`]: std::backtrace#environment-variables
//!
//! - Anyhow works with any error type that has an impl of `std::error::Error`,
//!   including ones defined in your crate. We do not bundle a `derive(Error)`
//!   macro but you can write the impls yourself or use a standalone macro like
//!   [thiserror].
//!
//!   [thiserror]: https://github.com/dtolnay/thiserror
//!
//!   ```
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
//! - One-off error messages can be constructed using the `anyhow!` macro, which
//!   supports string interpolation and produces an `anyhow::Error`.
//!
//!   ```
//!   # use anyhow::{anyhow, Result};
//!   #
//!   # fn demo() -> Result<()> {
//!   #     let missing = "...";
//!   return Err(anyhow!("Missing attribute: {}", missing));
//!   #     Ok(())
//!   # }
//!   ```
//!
//!   A `bail!` macro is provided as a shorthand for the same early return.
//!
//!   ```
//!   # use anyhow::{bail, Result};
//!   #
//!   # fn demo() -> Result<()> {
//!   #     let missing = "...";
//!   bail!("Missing attribute: {}", missing);
//!   #     Ok(())
//!   # }
//!   ```
//!
//! <br>
//!
//! # No-std support
//!
//! In no_std mode, almost all of the same API is available and works the same
//! way. To depend on Anyhow in no_std mode, disable our default enabled "std"
//! feature in Cargo.toml. A global allocator is required.
//!
//! ```toml
//! [dependencies]
//! anyhow = { version = "1.0", default-features = false }
//! ```
//!
//! With versions of Rust older than 1.81, no_std mode may require an additional
//! `.map_err(Error::msg)` when working with a non-Anyhow error type inside a
//! function that returns Anyhow's error type, as the trait that `?`-based error
//! conversions are defined by is only available in std in those old versions.

#![doc(html_root_url = "https://docs.rs/anyhow/1.0.100")]
#![cfg_attr(error_generic_member_access, feature(error_generic_member_access))]
#![no_std]
#![deny(dead_code, unused_imports, unused_mut)]
#![cfg_attr(
    not(anyhow_no_unsafe_op_in_unsafe_fn_lint),
    deny(unsafe_op_in_unsafe_fn)
)]
#![cfg_attr(anyhow_no_unsafe_op_in_unsafe_fn_lint, allow(unused_unsafe))]
#![allow(
    clippy::doc_markdown,
    clippy::elidable_lifetime_names,
    clippy::enum_glob_use,
    clippy::explicit_auto_deref,
    clippy::extra_unused_type_parameters,
    clippy::incompatible_msrv,
    clippy::let_underscore_untyped,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_doctest_main,
    clippy::needless_lifetimes,
    clippy::new_ret_no_self,
    clippy::redundant_else,
    clippy::return_self_not_must_use,
    clippy::struct_field_names,
    clippy::unused_self,
    clippy::used_underscore_binding,
    clippy::wildcard_imports,
    clippy::wrong_self_convention
)]
#![allow(unknown_lints, mismatched_lifetime_syntaxes)]

#[cfg(all(
    anyhow_nightly_testing,
    feature = "std",
    not(error_generic_member_access)
))]
compile_error!("Build script probe failed to compile.");

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[macro_use]
mod backtrace;
mod chain;
mod context;
mod ensure;
mod error;
mod fmt;
mod kind;
mod macros;
#[cfg(error_generic_member_access)]
mod nightly;
mod ptr;
mod wrapper;

use crate::error::ErrorImpl;
use crate::ptr::Own;
use core::fmt::Display;

#[cfg(all(not(feature = "std"), anyhow_no_core_error))]
use core::fmt::Debug;

#[cfg(feature = "std")]
use std::error::Error as StdError;

#[cfg(not(any(feature = "std", anyhow_no_core_error)))]
use core::error::Error as StdError;

#[cfg(all(not(feature = "std"), anyhow_no_core_error))]
trait StdError: Debug + Display {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

#[doc(no_inline)]
pub use anyhow as format_err;

/// The `Error` type, a wrapper around a dynamic error type.
///
/// `Error` works a lot like `Box<dyn std::error::Error>`, but with these
/// differences:
///
/// - `Error` requires that the error is `Send`, `Sync`, and `'static`.
/// - `Error` guarantees that a backtrace is available, even if the underlying
///   error type does not provide one.
/// - `Error` is represented as a narrow pointer &mdash; exactly one word in
///   size instead of two.
///
/// <br>
///
/// # Display representations
///
/// When you print an error object using "{}" or to_string(), only the outermost
/// underlying error or context is printed, not any of the lower level causes.
/// This is exactly as if you had called the Display impl of the error from
/// which you constructed your anyhow::Error.
///
/// ```console
/// Failed to read instrs from ./path/to/instrs.json
/// ```
///
/// To print causes as well using anyhow's default formatting of causes, use the
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
/// ```
///
/// and if there is a backtrace available:
///
/// ```console
/// Error: Failed to read instrs from ./path/to/instrs.json
///
/// Caused by:
///     No such file or directory (os error 2)
///
/// Stack backtrace:
///    0: <E as anyhow::context::ext::StdError>::ext_context
///              at /git/anyhow/src/backtrace.rs:26
///    1: core::result::Result<T,E>::map_err
///              at /git/rustc/src/libcore/result.rs:596
///    2: anyhow::context::<impl anyhow::Context<T,E> for core::result::Result<T,E>>::with_context
///              at /git/anyhow/src/context.rs:58
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
///     context: "Failed to read instrs from ./path/to/instrs.json",
///     source: Os {
///         code: 2,
///         kind: NotFound,
///         message: "No such file or directory",
///     },
/// }
/// ```
///
/// If none of the built-in representations are appropriate and you would prefer
/// to render the error and its cause chain yourself, it can be done something
/// like this:
///
/// ```
/// use anyhow::{Context, Result};
///
/// fn main() {
///     if let Err(err) = try_main() {
///         eprintln!("ERROR: {}", err);
///         err.chain().skip(1).for_each(|cause| eprintln!("because: {}", cause));
///         std::process::exit(1);
///     }
/// }
///
/// fn try_main() -> Result<()> {
///     # const IGNORE: &str = stringify! {
///     ...
///     # };
///     # Ok(())
/// }
/// ```
#[repr(transparent)]
pub struct Error {
    inner: Own<ErrorImpl>,
}

/// Iterator of a chain of source errors.
///
/// This type is the iterator returned by [`Error::chain`].
///
/// # Example
///
/// ```
/// use anyhow::Error;
/// use std::io;
///
/// pub fn underlying_io_error_kind(error: &Error) -> Option<io::ErrorKind> {
///     for cause in error.chain() {
///         if let Some(io_error) = cause.downcast_ref::<io::Error>() {
///             return Some(io_error.kind());
///         }
///     }
///     None
/// }
/// ```
#[cfg(any(feature = "std", not(anyhow_no_core_error)))]
#[derive(Clone)]
pub struct Chain<'a> {
    state: crate::chain::ChainState<'a>,
}

/// `Result<T, Error>`
///
/// This is a reasonable return type to use throughout your application but also
/// for `fn main`; if you do, failures will be printed along with any
/// [context][Context] and a backtrace if one was captured.
///
/// `anyhow::Result` may be used with one *or* two type parameters.
///
/// ```rust
/// use anyhow::Result;
///
/// # const IGNORE: &str = stringify! {
/// fn demo1() -> Result<T> {...}
///            // ^ equivalent to std::result::Result<T, anyhow::Error>
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
/// use anyhow::Result;
///
/// fn main() -> Result<()> {
///     # return Ok(());
///     let config = std::fs::read_to_string("cluster.json")?;
///     let map: ClusterMap = serde_json::from_str(&config)?;
///     println!("cluster info: {:#?}", map);
///     Ok(())
/// }
/// ```
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Provides the `context` method for `Result`.
///
/// This trait is sealed and cannot be implemented for types outside of
/// `anyhow`.
///
/// <br>
///
/// # Example
///
/// ```
/// use anyhow::{Context, Result};
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
///     it.detach().context("Failed to detach the important thing")?;
///
///     let path = &it.path;
///     let content = fs::read(path)
///         .with_context(|| format!("Failed to read instrs from {}", path.display()))?;
///
///     Ok(content)
/// }
/// ```
///
/// When printed, the outermost context would be printed first and the lower
/// level underlying causes would be enumerated below.
///
/// ```console
/// Error: Failed to read instrs from ./path/to/instrs.json
///
/// Caused by:
///     No such file or directory (os error 2)
/// ```
///
/// Refer to the [Display representations] documentation for other forms in
/// which this context chain can be rendered.
///
/// [Display representations]: Error#display-representations
///
/// <br>
///
/// # Effect on downcasting
///
/// After attaching context of type `C` onto an error of type `E`, the resulting
/// `anyhow::Error` may be downcast to `C` **or** to `E`.
///
/// That is, in codebases that rely on downcasting, Anyhow's context supports
/// both of the following use cases:
///
///   - **Attaching context whose type is insignificant onto errors whose type
///     is used in downcasts.**
///
///     In other error libraries whose context is not designed this way, it can
///     be risky to introduce context to existing code because new context might
///     break existing working downcasts. In Anyhow, any downcast that worked
///     before adding context will continue to work after you add a context, so
///     you should freely add human-readable context to errors wherever it would
///     be helpful.
///
///     ```
///     # use anyhow::bail;
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
///     use anyhow::{Context, Result};
///
///     fn do_it() -> Result<()> {
///         helper().context("Failed to complete the work")?;
///         # const IGNORE: &str = stringify! {
///         ...
///         # };
///         # unreachable!()
///     }
///
///     fn main() {
///         let err = do_it().unwrap_err();
///         if let Some(e) = err.downcast_ref::<SuspiciousError>() {
///             // If helper() returned SuspiciousError, this downcast will
///             // correctly succeed even with the context in between.
///             # return;
///         }
///         # panic!("expected downcast to succeed");
///     }
///     ```
///
///   - **Attaching context whose type is used in downcasts onto errors whose
///     type is insignificant.**
///
///     Some codebases prefer to use machine-readable context to categorize
///     lower level errors in a way that will be actionable to higher levels of
///     the application.
///
///     ```
///     # use anyhow::bail;
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
///     use anyhow::{Context, Result};
///
///     fn do_it() -> Result<()> {
///         helper().context(HelperFailed)?;
///         # const IGNORE: &str = stringify! {
///         ...
///         # };
///         # unreachable!()
///     }
///
///     fn main() {
///         let err = do_it().unwrap_err();
///         if let Some(e) = err.downcast_ref::<HelperFailed>() {
///             // If helper failed, this downcast will succeed because
///             // HelperFailed is the context that has been attached to
///             // that error.
///             # return;
///         }
///         # panic!("expected downcast to succeed");
///     }
///     ```
pub trait Context<T, E>: context::private::Sealed {
    /// Wrap the error value with additional context.
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static;

    /// Wrap the error value with additional context that is evaluated lazily
    /// only once an error does occur.
    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

/// Equivalent to `Ok::<_, anyhow::Error>(value)`.
///
/// This simplifies creation of an `anyhow::Result` in places where type
/// inference cannot deduce the `E` type of the result &mdash; without needing
/// to write`Ok::<_, anyhow::Error>(value)`.
///
/// One might think that `anyhow::Result::Ok(value)` would work in such cases
/// but it does not.
///
/// ```console
/// error[E0282]: type annotations needed for `std::result::Result<i32, E>`
///   --> src/main.rs:11:13
///    |
/// 11 |     let _ = anyhow::Result::Ok(1);
///    |         -   ^^^^^^^^^^^^^^^^^^ cannot infer type for type parameter `E` declared on the enum `Result`
///    |         |
///    |         consider giving this pattern the explicit type `std::result::Result<i32, E>`, where the type parameter `E` is specified
/// ```
#[allow(non_snake_case)]
pub fn Ok<T>(value: T) -> Result<T> {
    Result::Ok(value)
}

// Not public API. Referenced by macro-generated code.
#[doc(hidden)]
pub mod __private {
    use self::not::Bool;
    use crate::Error;
    use alloc::fmt;
    use core::fmt::Arguments;

    #[doc(hidden)]
    pub use crate::ensure::{BothDebug, NotBothDebug};
    #[doc(hidden)]
    pub use alloc::format;
    #[doc(hidden)]
    pub use core::result::Result::Err;
    #[doc(hidden)]
    pub use core::{concat, format_args, stringify};

    #[doc(hidden)]
    pub mod kind {
        #[doc(hidden)]
        pub use crate::kind::{AdhocKind, TraitKind};

        #[cfg(any(feature = "std", not(anyhow_no_core_error)))]
        #[doc(hidden)]
        pub use crate::kind::BoxedKind;
    }

    #[doc(hidden)]
    #[inline]
    #[cold]
    pub fn format_err(args: Arguments) -> Error {
        #[cfg(anyhow_no_fmt_arguments_as_str)]
        let fmt_arguments_as_str = None::<&str>;
        #[cfg(not(anyhow_no_fmt_arguments_as_str))]
        let fmt_arguments_as_str = args.as_str();

        if let Some(message) = fmt_arguments_as_str {
            // anyhow!("literal"), can downcast to &'static str
            Error::msg(message)
        } else {
            // anyhow!("interpolate {var}"), can downcast to String
            Error::msg(fmt::format(args))
        }
    }

    #[doc(hidden)]
    #[inline]
    #[cold]
    #[must_use]
    pub fn must_use(error: Error) -> Error {
        error
    }

    #[doc(hidden)]
    #[inline]
    pub fn not(cond: impl Bool) -> bool {
        cond.not()
    }

    mod not {
        #[doc(hidden)]
        pub trait Bool {
            fn not(self) -> bool;
        }

        impl Bool for bool {
            #[inline]
            fn not(self) -> bool {
                !self
            }
        }

        impl Bool for &bool {
            #[inline]
            fn not(self) -> bool {
                !*self
            }
        }
    }
}
