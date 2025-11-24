//! Processor for [RFC 6570] URI Template.
//!
//! [RFC 6570]: https://www.rfc-editor.org/rfc/rfc6570.html
//!
//! # Usage
//!
//! 1. Prepare a template.
//!     * You can create a template as [`UriTemplateStr`]
#![cfg_attr(
    feature = "alloc",
    doc = "      type (borrowed) or [`UriTemplateString`] type (owned)."
)]
#![cfg_attr(not(feature = "alloc"), doc = "      type.")]
//! 2. Prepare a context.
//!     * Create a value of type that implements [`Context`] trait.
#![cfg_attr(
    feature = "alloc",
    doc = "    * Or, if you use [`SimpleContext`], insert key-value pairs into it."
)]
//! 3. Expand.
//!     * Pass the context to [`UriTemplateStr::expand`] method of the template.
//! 4. Use the result.
//!     * Returned [`Expanded`] object can be directly printed since it
//!       implements [`Display`][`core::fmt::Display`] trait. Or, you can call
//!       `.to_string()` method of the `alloc::string::ToString` trait to
//!       convert it to a `String`.
//!
//! # Examples
//!
//! ## Custom context type
//!
//! For details, see [the documentation of `context` module][`context`].
//!
//! ```
//! # use iri_string::template::Error;
//! use core::fmt;
//! use iri_string::spec::{IriSpec, Spec, UriSpec};
//! use iri_string::template::UriTemplateStr;
//! use iri_string::template::context::{Context, VarName, Visitor};
//!
//! struct UserInfo {
//!     username: &'static str,
//!     utf8_available: bool,
//! }
//!
//! impl Context for UserInfo {
//!     fn visit<V: Visitor>(
//!         &self,
//!         visitor: V,
//!     ) -> V::Result {
//!         match visitor.var_name().as_str() {
//!             "username" => visitor.visit_string(self.username),
//!             "utf8" => {
//!                 if self.utf8_available {
//!                     // U+2713 CHECK MARK
//!                     visitor.visit_string("\u{2713}")
//!                 } else {
//!                     visitor.visit_undefined()
//!                 }
//!             }
//!             _ => visitor.visit_undefined()
//!         }
//!     }
//! }
//!
//! let context = UserInfo {
//!     username: "foo",
//!     utf8_available: true,
//! };
//!
//! let template = UriTemplateStr::new("/users/{username}{?utf8}")?;
//!
//! # #[cfg(feature = "alloc")] {
//! assert_eq!(
//!     template.expand::<UriSpec, _>(&context)?.to_string(),
//!     "/users/foo?utf8=%E2%9C%93"
//! );
//! assert_eq!(
//!     template.expand::<IriSpec, _>(&context)?.to_string(),
//!     "/users/foo?utf8=\u{2713}"
//! );
//! # }
//! # Ok::<_, Error>(())
//! ```
//!
//! ## `SimpleContext` type (enabled by `alloc` feature flag)
//!
//! ```
//! # use iri_string::template::Error;
//! # #[cfg(feature = "alloc")] {
//! use iri_string::spec::{IriSpec, UriSpec};
//! use iri_string::template::UriTemplateStr;
//! use iri_string::template::simple_context::SimpleContext;
//!
//! let mut context = SimpleContext::new();
//! context.insert("username", "foo");
//! // U+2713 CHECK MARK
//! context.insert("utf8", "\u{2713}");
//!
//! let template = UriTemplateStr::new("/users/{username}{?utf8}")?;
//!
//! assert_eq!(
//!     template.expand::<UriSpec, _>(&context)?.to_string(),
//!     "/users/foo?utf8=%E2%9C%93"
//! );
//! assert_eq!(
//!     template.expand::<IriSpec, _>(&context)?.to_string(),
//!     "/users/foo?utf8=\u{2713}"
//! );
//! # }
//! # Ok::<_, Error>(())
//! ```
//!
#![cfg_attr(
    feature = "alloc",
    doc = "[`SimpleContext`]: `simple_context::SimpleContext`"
)]
mod components;
pub mod context;
mod error;
mod expand;
mod parser;
#[cfg(feature = "alloc")]
pub mod simple_context;
mod string;

pub use self::context::{Context, DynamicContext};
#[cfg(feature = "alloc")]
pub use self::error::CreationError;
pub use self::error::Error;
pub use self::expand::Expanded;
#[cfg(feature = "alloc")]
pub use self::string::UriTemplateString;
pub use self::string::{UriTemplateStr, UriTemplateVariables};

/// Deprecated old name of [`template::context::VarName`].
///
/// [`template::context::VarName`]: `components::VarName`
#[deprecated(
    since = "0.7.1",
    note = "renamed (moved) to `template::context::VarName`"
)]
pub type VarName<'a> = self::components::VarName<'a>;

/// Variable value type.
#[derive(Debug, Clone, Copy)]
enum ValueType {
    /// Undefined (i.e. null).
    Undefined,
    /// String value.
    String,
    /// List.
    List,
    /// Associative array.
    Assoc,
}

impl ValueType {
    /// Returns the value type for an undefined variable.
    #[inline]
    #[must_use]
    pub const fn undefined() -> Self {
        ValueType::Undefined
    }

    /// Returns the value type for a string variable.
    #[inline]
    #[must_use]
    pub const fn string() -> Self {
        ValueType::String
    }

    /// Returns the value type for an empty list variable.
    #[inline]
    #[must_use]
    pub const fn empty_list() -> Self {
        ValueType::Undefined
    }

    /// Returns the value type for a nonempty list variable.
    #[inline]
    #[must_use]
    pub const fn nonempty_list() -> Self {
        ValueType::List
    }

    /// Returns the value type for an empty associative array variable.
    #[inline]
    #[must_use]
    pub const fn empty_assoc() -> Self {
        ValueType::Undefined
    }

    /// Returns the value type for a nonempty associative array variable.
    #[inline]
    #[must_use]
    pub const fn nonempty_assoc() -> Self {
        ValueType::Assoc
    }
}
