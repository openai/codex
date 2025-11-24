use crate::builder::Str;

/// [`Arg`][crate::Arg] or [`ArgGroup`][crate::ArgGroup] identifier
///
/// This is used for accessing the value in [`ArgMatches`][crate::ArgMatches] or defining
/// relationships between `Arg`s and `ArgGroup`s with functions like
/// [`Arg::conflicts_with`][crate::Arg::conflicts_with].
#[derive(Default, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Id(Str);

impl Id {
    pub(crate) const HELP: &'static str = "help";
    pub(crate) const VERSION: &'static str = "version";
    pub(crate) const EXTERNAL: &'static str = "";

    pub(crate) fn from_static_ref(name: &'static str) -> Self {
        Self(Str::from_static_ref(name))
    }

    /// Get the raw string of the `Id`
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn as_internal_str(&self) -> &Str {
        &self.0
    }
}

impl From<&'_ Id> for Id {
    fn from(id: &'_ Id) -> Self {
        id.clone()
    }
}

impl From<Str> for Id {
    fn from(name: Str) -> Self {
        Self(name)
    }
}

impl From<&'_ Str> for Id {
    fn from(name: &'_ Str) -> Self {
        Self(name.into())
    }
}

#[cfg(feature = "string")]
impl From<String> for Id {
    fn from(name: String) -> Self {
        Self(name.into())
    }
}

#[cfg(feature = "string")]
impl From<&'_ String> for Id {
    fn from(name: &'_ String) -> Self {
        Self(name.into())
    }
}

impl From<&'static str> for Id {
    fn from(name: &'static str) -> Self {
        Self(name.into())
    }
}

impl From<&'_ &'static str> for Id {
    fn from(name: &'_ &'static str) -> Self {
        Self(name.into())
    }
}

impl From<Id> for Str {
    fn from(name: Id) -> Self {
        name.0
    }
}

impl From<Id> for String {
    fn from(name: Id) -> Self {
        Str::from(name).into()
    }
}

impl std::fmt::Display for Id {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

impl std::fmt::Debug for Id {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl AsRef<str> for Id {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Id {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<str> for Id {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        PartialEq::eq(self.as_str(), other)
    }
}
impl PartialEq<Id> for str {
    #[inline]
    fn eq(&self, other: &Id) -> bool {
        PartialEq::eq(self, other.as_str())
    }
}

impl PartialEq<&'_ str> for Id {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        PartialEq::eq(self.as_str(), *other)
    }
}
impl PartialEq<Id> for &'_ str {
    #[inline]
    fn eq(&self, other: &Id) -> bool {
        PartialEq::eq(*self, other.as_str())
    }
}

impl PartialEq<Str> for Id {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        PartialEq::eq(self.as_str(), other.as_str())
    }
}
impl PartialEq<Id> for Str {
    #[inline]
    fn eq(&self, other: &Id) -> bool {
        PartialEq::eq(self.as_str(), other.as_str())
    }
}

impl PartialEq<String> for Id {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        PartialEq::eq(self.as_str(), other.as_str())
    }
}
impl PartialEq<Id> for String {
    #[inline]
    fn eq(&self, other: &Id) -> bool {
        PartialEq::eq(other, self)
    }
}
