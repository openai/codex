use proc_macro2::Span;
use syn::{spanned::Spanned, Meta};

use crate::{FromMeta, Result};

/// A meta-item that can be present as a word - with no value - or absent.
///
/// # Defaulting
/// Like `Option`, `Flag` does not require `#[darling(default)]` to be optional.
/// If the caller does not include the property, then an absent `Flag` will be included
/// in the receiver struct.
///
/// # Spans
/// `Flag` keeps the span where its word was seen.
/// This enables attaching custom error messages to the word, such as in the case of two
/// conflicting flags being present.
///
/// # Example
/// ```ignore
/// #[derive(FromMeta)]
/// #[darling(and_then = Self::not_both)]
/// struct Demo {
///     flag_a: Flag,
///     flag_b: Flag,
/// }
///
/// impl Demo {
///     fn not_both(self) -> Result<Self> {
///         if self.flag_a.is_present() && self.flag_b.is_present() {
///             Err(Error::custom("Cannot set flag_a and flag_b").with_span(&self.flag_b.span()))
///         } else {
///             Ok(self)
///         }
///     }
/// }
/// ```
///
/// The above struct would then produce the following error.
///
/// ```ignore
/// #[example(flag_a, flag_b)]
/// //                ^^^^^^ Cannot set flag_a and flag_b
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Flag(Option<Span>);

impl Flag {
    /// Creates a new `Flag` which corresponds to the presence of a value.
    pub fn present() -> Self {
        Flag(Some(Span::call_site()))
    }

    /// Check if the flag is present.
    pub fn is_present(&self) -> bool {
        self.0.is_some()
    }

    #[deprecated(since = "0.14.0", note = "Use Flag::is_present")]
    pub fn is_some(&self) -> bool {
        self.is_present()
    }

    /// Get the span of the flag, or [`Span::call_site`] if the flag was not present.
    pub fn span(&self) -> Span {
        self.0.unwrap_or_else(Span::call_site)
    }
}

impl FromMeta for Flag {
    fn from_none() -> Option<Self> {
        Some(Flag(None))
    }

    fn from_meta(mi: &syn::Meta) -> Result<Self> {
        if let Meta::Path(p) = mi {
            Ok(Flag(Some(p.span())))
        } else {
            // The implementation for () will produce an error for all non-path meta items;
            // call it to make sure the span behaviors and error messages are the same.
            Err(<()>::from_meta(mi).unwrap_err())
        }
    }
}

impl From<Flag> for bool {
    fn from(flag: Flag) -> Self {
        flag.is_present()
    }
}

impl From<bool> for Flag {
    fn from(v: bool) -> Self {
        if v {
            Flag::present()
        } else {
            Flag(None)
        }
    }
}
