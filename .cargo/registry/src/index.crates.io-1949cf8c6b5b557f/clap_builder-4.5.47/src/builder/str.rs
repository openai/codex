/// A UTF-8-encoded fixed string
///
/// <div class="warning">
///
/// **NOTE:** To support dynamic values (i.e. `String`), enable the `string`
/// feature
///
/// </div>
#[derive(Default, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Str {
    name: Inner,
}

impl Str {
    #[cfg(feature = "string")]
    pub(crate) fn from_string(name: String) -> Self {
        Self {
            name: Inner::from_string(name),
        }
    }

    #[cfg(feature = "string")]
    pub(crate) fn from_ref(name: &str) -> Self {
        Self {
            name: Inner::from_ref(name),
        }
    }

    pub(crate) fn from_static_ref(name: &'static str) -> Self {
        Self {
            name: Inner::from_static_ref(name),
        }
    }

    pub(crate) fn into_inner(self) -> Inner {
        self.name
    }

    /// Get the raw string of the `Str`
    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

impl From<&'_ Str> for Str {
    fn from(id: &'_ Str) -> Self {
        id.clone()
    }
}

#[cfg(feature = "string")]
impl From<String> for Str {
    fn from(name: String) -> Self {
        Self::from_string(name)
    }
}

#[cfg(feature = "string")]
impl From<&'_ String> for Str {
    fn from(name: &'_ String) -> Self {
        Self::from_ref(name.as_str())
    }
}

impl From<&'static str> for Str {
    fn from(name: &'static str) -> Self {
        Self::from_static_ref(name)
    }
}

impl From<&'_ &'static str> for Str {
    fn from(name: &'_ &'static str) -> Self {
        Self::from_static_ref(name)
    }
}

impl From<Str> for String {
    fn from(name: Str) -> Self {
        name.name.into_string()
    }
}

impl From<Str> for Vec<u8> {
    fn from(name: Str) -> Self {
        String::from(name).into()
    }
}

impl From<Str> for std::ffi::OsString {
    fn from(name: Str) -> Self {
        String::from(name).into()
    }
}

impl From<Str> for std::path::PathBuf {
    fn from(name: Str) -> Self {
        String::from(name).into()
    }
}

impl std::fmt::Display for Str {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

impl std::fmt::Debug for Str {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl std::ops::Deref for Str {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Str {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for Str {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<std::ffi::OsStr> for Str {
    #[inline]
    fn as_ref(&self) -> &std::ffi::OsStr {
        (**self).as_ref()
    }
}

impl AsRef<std::path::Path> for Str {
    #[inline]
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self)
    }
}

impl std::borrow::Borrow<str> for Str {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<str> for Str {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        PartialEq::eq(self.as_str(), other)
    }
}
impl PartialEq<Str> for str {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        PartialEq::eq(self, other.as_str())
    }
}

impl PartialEq<&'_ str> for Str {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        PartialEq::eq(self.as_str(), *other)
    }
}
impl PartialEq<Str> for &'_ str {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        PartialEq::eq(*self, other.as_str())
    }
}

impl PartialEq<std::ffi::OsStr> for Str {
    #[inline]
    fn eq(&self, other: &std::ffi::OsStr) -> bool {
        PartialEq::eq(self.as_str(), other)
    }
}
impl PartialEq<Str> for std::ffi::OsStr {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        PartialEq::eq(self, other.as_str())
    }
}

impl PartialEq<&'_ std::ffi::OsStr> for Str {
    #[inline]
    fn eq(&self, other: &&std::ffi::OsStr) -> bool {
        PartialEq::eq(self.as_str(), *other)
    }
}
impl PartialEq<Str> for &'_ std::ffi::OsStr {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        PartialEq::eq(*self, other.as_str())
    }
}

impl PartialEq<String> for Str {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        PartialEq::eq(self.as_str(), other.as_str())
    }
}
impl PartialEq<Str> for String {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        PartialEq::eq(self.as_str(), other.as_str())
    }
}

#[cfg(feature = "string")]
pub(crate) mod inner {
    #[derive(Clone)]
    pub(crate) enum Inner {
        Static(&'static str),
        Owned(Box<str>),
    }

    impl Inner {
        pub(crate) fn from_string(name: String) -> Self {
            Self::Owned(name.into_boxed_str())
        }

        pub(crate) fn from_ref(name: &str) -> Self {
            Self::Owned(Box::from(name))
        }

        pub(crate) fn from_static_ref(name: &'static str) -> Self {
            Self::Static(name)
        }

        pub(crate) fn as_str(&self) -> &str {
            match self {
                Self::Static(s) => s,
                Self::Owned(s) => s.as_ref(),
            }
        }

        pub(crate) fn into_string(self) -> String {
            match self {
                Self::Static(s) => s.to_owned(),
                Self::Owned(s) => s.into(),
            }
        }
    }
}

#[cfg(not(feature = "string"))]
pub(crate) mod inner {
    #[derive(Clone)]
    pub(crate) struct Inner(pub(crate) &'static str);

    impl Inner {
        pub(crate) fn from_static_ref(name: &'static str) -> Self {
            Self(name)
        }

        pub(crate) fn as_str(&self) -> &str {
            self.0
        }

        pub(crate) fn into_string(self) -> String {
            self.as_str().to_owned()
        }
    }
}

pub(crate) use inner::Inner;

impl Default for Inner {
    fn default() -> Self {
        Self::from_static_ref("")
    }
}

impl PartialEq for Inner {
    fn eq(&self, other: &Inner) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialOrd for Inner {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Inner {
    fn cmp(&self, other: &Inner) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Eq for Inner {}

impl std::hash::Hash for Inner {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
