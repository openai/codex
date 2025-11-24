//! This library provides a convenient derive macro for the standard library's
//! [`core::fmt::Display`] trait.
//!
//! [`core::fmt::Display`]: https://doc.rust-lang.org/std/fmt/trait.Display.html
//!
//! ```toml
//! [dependencies]
//! displaydoc = "0.2"
//! ```
//!
//! *Compiler support: requires rustc 1.56+*
//!
//! <br>
//!
//! ## Example
//!
//! *Demonstration alongside the [`Error`][std::error::Error] derive macro from [`thiserror`](https://docs.rs/thiserror/1.0.25/thiserror/index.html),
//! to propagate source locations from [`io::Error`][std::io::Error] with the `#[source]` attribute:*
//! ```rust
//! use std::io;
//! use displaydoc::Display;
//! use thiserror::Error;
//!
//! #[derive(Display, Error, Debug)]
//! pub enum DataStoreError {
//!     /// data store disconnected
//!     Disconnect(#[source] io::Error),
//!     /// the data for key `{0}` is not available
//!     Redaction(String),
//!     /// invalid header (expected {expected:?}, found {found:?})
//!     InvalidHeader {
//!         expected: String,
//!         found: String,
//!     },
//!     /// unknown data store error
//!     Unknown,
//! }
//!
//! let error = DataStoreError::Redaction("CLASSIFIED CONTENT".to_string());
//! assert!("the data for key `CLASSIFIED CONTENT` is not available" == &format!("{}", error));
//! ```
//! *Note that although [`io::Error`][std::io::Error] implements `Display`, we do not add it to the
//! generated message for `DataStoreError::Disconnect`, since it is already made available via
//! `#[source]`. See further context on avoiding duplication in error reports at the rust blog
//! [here](https://github.com/yaahc/blog.rust-lang.org/blob/master/posts/inside-rust/2021-05-15-What-the-error-handling-project-group-is-working-towards.md#duplicate-information-issue).*
//!
//! <br>
//!
//! ## Details
//!
//! - A `fmt::Display` impl is generated for your enum if you provide
//!   a docstring comment on each variant as shown above in the example. The
//!   `Display` derive macro supports a shorthand for interpolating fields from
//!   the error:
//!     - `/// {var}` ⟶ `write!("{}", self.var)`
//!     - `/// {0}` ⟶ `write!("{}", self.0)`
//!     - `/// {var:?}` ⟶ `write!("{:?}", self.var)`
//!     - `/// {0:?}` ⟶ `write!("{:?}", self.0)`
//! - This also works with structs and [generic types][crate::Display#generic-type-parameters]:
//! ```rust
//! # use displaydoc::Display;
//! /// oh no, an error: {0}
//! #[derive(Display)]
//! pub struct Error<E>(pub E);
//!
//! let error: Error<&str> = Error("muahaha i am an error");
//! assert!("oh no, an error: muahaha i am an error" == &format!("{}", error));
//! ```
//!
//! - Two optional attributes can be added to your types next to the derive:
//!
//!     - `#[ignore_extra_doc_attributes]` makes the macro ignore any doc
//!       comment attributes (or `///` lines) after the first. Multi-line
//!       comments using `///` are otherwise treated as an error, so use this
//!       attribute or consider switching to block doc comments (`/** */`).
//!
//!     - `#[prefix_enum_doc_attributes]` combines the doc comment message on
//!       your enum itself with the messages for each variant, in the format
//!       “enum: variant”. When added to an enum, the doc comment on the enum
//!       becomes mandatory. When added to any other type, it has no effect.
//!
//! - In case you want to have an independent doc comment, the
//!   `#[displaydoc("...")` atrribute may be used on the variant or struct to
//!   override it.
//!
//! <br>
//!
//! ## FAQ
//!
//! 1. **Is this crate `no_std` compatible?**
//!     * Yes! This crate implements the [`core::fmt::Display`] trait, not the [`std::fmt::Display`] trait, so it should work in `std` and `no_std` environments. Just add `default-features = false`.
//!
//! 2. **Does this crate work with `Path` and `PathBuf` via the `Display` trait?**
//!     * Yuuup. This crate uses @dtolnay's [autoref specialization technique](https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md) to add a special trait for types to get the display impl. It then specializes for `Path` and `PathBuf`, and when either of these types are found, it calls `self.display()` to get a `std::path::Display<'_>` type which can be used with the `Display` format specifier!
#![doc(html_root_url = "https://docs.rs/displaydoc/0.2.3")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(
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
#![allow(clippy::try_err)]

#[allow(unused_extern_crates)]
extern crate proc_macro;

mod attr;
mod expand;
mod fmt;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// [Custom `#[derive(...)]` macro](https://doc.rust-lang.org/edition-guide/rust-2018/macros/custom-derive.html)
/// for implementing [`fmt::Display`][core::fmt::Display] via doc comment attributes.
///
/// ### Generic Type Parameters
///
/// Type parameters to an enum or struct using this macro should *not* need to
/// have an explicit `Display` constraint at the struct or enum definition
/// site. A `Display` implementation for the `derive`d struct or enum is
/// generated assuming each type parameter implements `Display`, but that should
/// be possible without adding the constraint to the struct definition itself:
/// ```rust
/// use displaydoc::Display;
///
/// /// oh no, an error: {0}
/// #[derive(Display)]
/// pub struct Error<E>(pub E);
///
/// // No need to require `E: Display`, since `displaydoc::Display` adds that implicitly.
/// fn generate_error<E>(e: E) -> Error<E> { Error(e) }
///
/// assert!("oh no, an error: muahaha" == &format!("{}", generate_error("muahaha")));
/// ```
///
/// ### Using [`Debug`][core::fmt::Debug] Implementations with Type Parameters
/// However, if a type parameter must instead be constrained with the
/// [`Debug`][core::fmt::Debug] trait so that some field may be printed with
/// `{:?}`, that constraint must currently still also be specified redundantly
/// at the struct or enum definition site. If a struct or enum field is being
/// formatted with `{:?}` via [`displaydoc`][crate], and a generic type
/// parameter must implement `Debug` to do that, then that struct or enum
/// definition will need to propagate the `Debug` constraint to every type
/// parameter it's instantiated with:
/// ```rust
/// use core::fmt::Debug;
/// use displaydoc::Display;
///
/// /// oh no, an error: {0:?}
/// #[derive(Display)]
/// pub struct Error<E: Debug>(pub E);
///
/// // `E: Debug` now has to propagate to callers.
/// fn generate_error<E: Debug>(e: E) -> Error<E> { Error(e) }
///
/// assert!("oh no, an error: \"cool\"" == &format!("{}", generate_error("cool")));
///
/// // Try this with a struct that doesn't impl `Display` at all, unlike `str`.
/// #[derive(Debug)]
/// pub struct Oh;
/// assert!("oh no, an error: Oh" == &format!("{}", generate_error(Oh)));
/// ```
#[proc_macro_derive(
    Display,
    attributes(ignore_extra_doc_attributes, prefix_enum_doc_attributes, displaydoc)
)]
pub fn derive_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::derive(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
