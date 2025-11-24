use crate::builder::Str;

/// A UTF-8-encoded fixed string
///
/// <div class="warning">
///
/// **NOTE:** To support dynamic values (i.e. `OsString`), enable the `string`
/// feature
///
/// </div>
#[derive(Default, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct OsStr {
    name: Inner,
}

impl OsStr {
    #[cfg(feature = "string")]
    pub(crate) fn from_string(name: std::ffi::OsString) -> Self {
        Self {
            name: Inner::from_string(name),
        }
    }

    #[cfg(feature = "string")]
    pub(crate) fn from_ref(name: &std::ffi::OsStr) -> Self {
        Self {
            name: Inner::from_ref(name),
        }
    }

    pub(crate) fn from_static_ref(name: &'static std::ffi::OsStr) -> Self {
        Self {
            name: Inner::from_static_ref(name),
        }
    }

    /// Get the raw string as an `std::ffi::OsStr`
    pub fn as_os_str(&self) -> &std::ffi::OsStr {
        self.name.as_os_str()
    }

    /// Get the raw string as an `OsString`
    pub fn to_os_string(&self) -> std::ffi::OsString {
        self.as_os_str().to_owned()
    }
}

impl From<&'_ OsStr> for OsStr {
    fn from(id: &'_ OsStr) -> Self {
        id.clone()
    }
}

#[cfg(feature = "string")]
impl From<Str> for OsStr {
    fn from(id: Str) -> Self {
        match id.into_inner() {
            crate::builder::StrInner::Static(s) => Self::from_static_ref(std::ffi::OsStr::new(s)),
            crate::builder::StrInner::Owned(s) => Self::from_ref(std::ffi::OsStr::new(s.as_ref())),
        }
    }
}

#[cfg(not(feature = "string"))]
impl From<Str> for OsStr {
    fn from(id: Str) -> Self {
        Self::from_static_ref(std::ffi::OsStr::new(id.into_inner().0))
    }
}

impl From<&'_ Str> for OsStr {
    fn from(id: &'_ Str) -> Self {
        id.clone().into()
    }
}

#[cfg(feature = "string")]
impl From<std::ffi::OsString> for OsStr {
    fn from(name: std::ffi::OsString) -> Self {
        Self::from_string(name)
    }
}

#[cfg(feature = "string")]
impl From<&'_ std::ffi::OsString> for OsStr {
    fn from(name: &'_ std::ffi::OsString) -> Self {
        Self::from_ref(name.as_os_str())
    }
}

#[cfg(feature = "string")]
impl From<String> for OsStr {
    fn from(name: String) -> Self {
        Self::from_string(name.into())
    }
}

#[cfg(feature = "string")]
impl From<&'_ String> for OsStr {
    fn from(name: &'_ String) -> Self {
        Self::from_ref(name.as_str().as_ref())
    }
}

impl From<&'static std::ffi::OsStr> for OsStr {
    fn from(name: &'static std::ffi::OsStr) -> Self {
        Self::from_static_ref(name)
    }
}

impl From<&'_ &'static std::ffi::OsStr> for OsStr {
    fn from(name: &'_ &'static std::ffi::OsStr) -> Self {
        Self::from_static_ref(name)
    }
}

impl From<&'static str> for OsStr {
    fn from(name: &'static str) -> Self {
        Self::from_static_ref(name.as_ref())
    }
}

impl From<&'_ &'static str> for OsStr {
    fn from(name: &'_ &'static str) -> Self {
        Self::from_static_ref((*name).as_ref())
    }
}

impl From<OsStr> for std::ffi::OsString {
    fn from(name: OsStr) -> Self {
        name.name.into_os_string()
    }
}

impl From<OsStr> for std::path::PathBuf {
    fn from(name: OsStr) -> Self {
        std::ffi::OsString::from(name).into()
    }
}

impl std::fmt::Debug for OsStr {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_os_str(), f)
    }
}

impl std::ops::Deref for OsStr {
    type Target = std::ffi::OsStr;

    #[inline]
    fn deref(&self) -> &std::ffi::OsStr {
        self.as_os_str()
    }
}

impl AsRef<std::ffi::OsStr> for OsStr {
    #[inline]
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.as_os_str()
    }
}

impl AsRef<std::path::Path> for OsStr {
    #[inline]
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self)
    }
}

impl std::borrow::Borrow<std::ffi::OsStr> for OsStr {
    #[inline]
    fn borrow(&self) -> &std::ffi::OsStr {
        self.as_os_str()
    }
}

impl PartialEq<str> for OsStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        PartialEq::eq(self.as_os_str(), other)
    }
}
impl PartialEq<OsStr> for str {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        PartialEq::eq(self, other.as_os_str())
    }
}

impl PartialEq<&'_ str> for OsStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        PartialEq::eq(self.as_os_str(), *other)
    }
}
impl PartialEq<OsStr> for &'_ str {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        PartialEq::eq(*self, other.as_os_str())
    }
}

impl PartialEq<&'_ std::ffi::OsStr> for OsStr {
    #[inline]
    fn eq(&self, other: &&std::ffi::OsStr) -> bool {
        PartialEq::eq(self.as_os_str(), *other)
    }
}
impl PartialEq<OsStr> for &'_ std::ffi::OsStr {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        PartialEq::eq(*self, other.as_os_str())
    }
}

impl PartialEq<String> for OsStr {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        PartialEq::eq(self.as_os_str(), other.as_str())
    }
}
impl PartialEq<OsStr> for String {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        PartialEq::eq(self.as_str(), other.as_os_str())
    }
}

impl PartialEq<std::ffi::OsString> for OsStr {
    #[inline]
    fn eq(&self, other: &std::ffi::OsString) -> bool {
        PartialEq::eq(self.as_os_str(), other.as_os_str())
    }
}
impl PartialEq<OsStr> for std::ffi::OsString {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        PartialEq::eq(self.as_os_str(), other.as_os_str())
    }
}

#[cfg(feature = "string")]
pub(crate) mod inner {
    #[derive(Clone)]
    pub(crate) enum Inner {
        Static(&'static std::ffi::OsStr),
        Owned(Box<std::ffi::OsStr>),
    }

    impl Inner {
        pub(crate) fn from_string(name: std::ffi::OsString) -> Self {
            Self::Owned(name.into_boxed_os_str())
        }

        pub(crate) fn from_ref(name: &std::ffi::OsStr) -> Self {
            Self::Owned(Box::from(name))
        }

        pub(crate) fn from_static_ref(name: &'static std::ffi::OsStr) -> Self {
            Self::Static(name)
        }

        pub(crate) fn as_os_str(&self) -> &std::ffi::OsStr {
            match self {
                Self::Static(s) => s,
                Self::Owned(s) => s.as_ref(),
            }
        }

        pub(crate) fn into_os_string(self) -> std::ffi::OsString {
            self.as_os_str().to_owned()
        }
    }
}

#[cfg(not(feature = "string"))]
pub(crate) mod inner {
    #[derive(Clone)]
    pub(crate) struct Inner(&'static std::ffi::OsStr);

    impl Inner {
        pub(crate) fn from_static_ref(name: &'static std::ffi::OsStr) -> Self {
            Self(name)
        }

        pub(crate) fn as_os_str(&self) -> &std::ffi::OsStr {
            self.0
        }

        pub(crate) fn into_os_string(self) -> std::ffi::OsString {
            self.as_os_str().to_owned()
        }
    }
}

pub(crate) use inner::Inner;

impl Default for Inner {
    fn default() -> Self {
        Self::from_static_ref(std::ffi::OsStr::new(""))
    }
}

impl PartialEq for Inner {
    fn eq(&self, other: &Inner) -> bool {
        self.as_os_str() == other.as_os_str()
    }
}

impl PartialOrd for Inner {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Inner {
    fn cmp(&self, other: &Inner) -> std::cmp::Ordering {
        self.as_os_str().cmp(other.as_os_str())
    }
}

impl Eq for Inner {}

impl std::hash::Hash for Inner {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_os_str().hash(state);
    }
}
