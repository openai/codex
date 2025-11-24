use std::fmt;

use syn::Lit;

use crate::ast::NestedMeta;
use crate::{FromMeta, Result};

use self::Override::*;

/// A value which can inherit a default value or have an explicit value specified.
///
/// # Usage
/// This type is meant for attributes like `default` in `darling`, which can take the following forms:
///
/// * `#[darling(default)]`
/// * `#[darling(default="path::to::fn")]`
///
/// In a struct collecting input for this attribute, that would be written as:
///
/// ```rust,ignore
/// use darling::{util::Override, FromField};
/// #[derive(FromField)]
/// #[darling(attributes(darling))]
/// pub struct Options {
///    default: Option<Override<syn::Path>>,
/// }
///
/// impl Options {
///     fn hydrate(self) -> Option<syn::Path> {
///         self.default.map(|ov| ov.unwrap_or(syn::parse_path("::Default::default").unwrap()))
///     }
/// }
/// ```
///
/// The `word` format (with no associated value), would produce `Override::Inherit`, while a list
/// or value format would produce `Override::Explicit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Override<T> {
    /// Inherit the eventual value from an external source.
    Inherit,

    /// Explicitly set the value.
    Explicit(T),
}

impl<T> Override<T> {
    /// Converts from `Override<T>` to `Override<&T>`.
    ///
    /// Produces a new `Override`, containing a reference into the original, leaving the original in place.
    pub fn as_ref(&self) -> Override<&T> {
        match *self {
            Inherit => Inherit,
            Explicit(ref val) => Explicit(val),
        }
    }

    /// Converts from `Override<T>` to `Override<&mut T>`.
    ///
    /// Produces a new `Override`, containing a mutable reference into the original.
    pub fn as_mut(&mut self) -> Override<&mut T> {
        match *self {
            Inherit => Inherit,
            Explicit(ref mut val) => Explicit(val),
        }
    }

    /// Returns `true` if the override is an `Explicit` value.
    pub fn is_explicit(&self) -> bool {
        match *self {
            Inherit => false,
            Explicit(_) => true,
        }
    }

    /// Converts from `Override<T>` to `Option<T>`.
    pub fn explicit(self) -> Option<T> {
        match self {
            Inherit => None,
            Explicit(val) => Some(val),
        }
    }

    /// Unwraps an override, yielding the content of an `Explicit`. Otherwise, it returns `optb`.
    pub fn unwrap_or(self, optb: T) -> T {
        match self {
            Inherit => optb,
            Explicit(val) => val,
        }
    }

    /// Unwraps an override, yielding the content of an `Explicit`. Otherwise, it calls `op`.
    pub fn unwrap_or_else<F>(self, op: F) -> T
    where
        F: FnOnce() -> T,
    {
        match self {
            Inherit => op(),
            Explicit(val) => val,
        }
    }
}

impl<T: Default> Override<T> {
    /// Returns the contained value or the default value of `T`.
    pub fn unwrap_or_default(self) -> T {
        match self {
            Inherit => Default::default(),
            Explicit(val) => val,
        }
    }
}

impl<T> Default for Override<T> {
    fn default() -> Self {
        Inherit
    }
}

impl<T> From<Option<T>> for Override<T> {
    fn from(v: Option<T>) -> Self {
        match v {
            None => Inherit,
            Some(val) => Explicit(val),
        }
    }
}

impl<T: fmt::Display> fmt::Display for Override<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Inherit => write!(f, "Inherit"),
            Explicit(ref val) => write!(f, "Explicit `{}`", val),
        }
    }
}

/// Parses a `Meta`. A bare word will produce `Override::Inherit`, while
/// any value will be forwarded to `T::from_meta`.
impl<T: FromMeta> FromMeta for Override<T> {
    fn from_word() -> Result<Self> {
        Ok(Inherit)
    }

    fn from_list(items: &[NestedMeta]) -> Result<Self> {
        Ok(Explicit(FromMeta::from_list(items)?))
    }

    fn from_value(lit: &Lit) -> Result<Self> {
        Ok(Explicit(FromMeta::from_value(lit)?))
    }

    fn from_char(value: char) -> Result<Self> {
        Ok(Explicit(FromMeta::from_char(value)?))
    }

    fn from_string(value: &str) -> Result<Self> {
        Ok(Explicit(FromMeta::from_string(value)?))
    }

    fn from_bool(value: bool) -> Result<Self> {
        Ok(Explicit(FromMeta::from_bool(value)?))
    }
}
