/*!
This module re-exports a "dumb" version of `std::borrow::Cow`.

It doesn't have any of the generic goodness and doesn't support dynamically
sized types. It's just either a `T` or a `&T`.

We have pretty simplistic needs, and we use this simpler type that works
in core-only mode.
*/

#[derive(Clone, Debug)]
pub(crate) enum DumbCow<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T> DumbCow<'a, T> {
    pub(crate) fn borrowed(&self) -> DumbCow<'_, T> {
        match *self {
            DumbCow::Owned(ref this) => DumbCow::Borrowed(this),
            DumbCow::Borrowed(ref this) => DumbCow::Borrowed(this),
        }
    }
}

impl<'a, T> core::ops::Deref for DumbCow<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match *self {
            DumbCow::Owned(ref t) => t,
            DumbCow::Borrowed(t) => t,
        }
    }
}

impl<'a, T: core::fmt::Display> core::fmt::Display for DumbCow<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(core::ops::Deref::deref(self), f)
    }
}

/// A `Cow`, but can be used in core-only mode.
///
/// In core-only, the `Owned` variant doesn't exist.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub(crate) enum StringCow<'a> {
    #[cfg(feature = "alloc")]
    Owned(alloc::string::String),
    Borrowed(&'a str),
}

impl<'a> StringCow<'a> {
    /// Returns this `String` or `&str` as a `&str`.
    ///
    /// Like `std::borrow::Cow`, the lifetime of the string slice returned
    /// is tied to `StringCow`, and _not_ the original lifetime of the string
    /// slice.
    pub(crate) fn as_str<'s>(&'s self) -> &'s str {
        match *self {
            #[cfg(feature = "alloc")]
            StringCow::Owned(ref s) => s,
            StringCow::Borrowed(s) => s,
        }
    }

    /// Converts this cow into an "owned" variant, copying if necessary.
    ///
    /// If this cow is already an "owned" variant, then this is a no-op.
    #[cfg(feature = "alloc")]
    pub(crate) fn into_owned(self) -> StringCow<'static> {
        use alloc::string::ToString;

        match self {
            StringCow::Owned(string) => StringCow::Owned(string),
            StringCow::Borrowed(string) => {
                StringCow::Owned(string.to_string())
            }
        }
    }
}

impl<'a> core::ops::Deref for StringCow<'a> {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<'a> core::fmt::Display for StringCow<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}

#[cfg(feature = "alloc")]
impl From<alloc::string::String> for StringCow<'static> {
    fn from(string: alloc::string::String) -> StringCow<'static> {
        StringCow::Owned(string)
    }
}

impl<'a> From<&'a str> for StringCow<'a> {
    fn from(string: &'a str) -> StringCow<'a> {
        StringCow::Borrowed(string)
    }
}
