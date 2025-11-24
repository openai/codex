//! URI/IRI builder.
//!
//! See the documentation of [`Builder`] type.

use core::fmt::{self, Display as _, Write as _};
use core::marker::PhantomData;

#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::ToString;

use crate::format::Censored;
#[cfg(feature = "alloc")]
use crate::format::{ToDedicatedString, ToStringFallible};
use crate::normalize::{self, NormalizationMode, PathCharacteristic, PctCaseNormalized};
use crate::parser::str::{find_split, prior_byte2};
use crate::parser::validate as parser;
use crate::spec::Spec;
use crate::types::{RiAbsoluteStr, RiReferenceStr, RiRelativeStr, RiStr};
#[cfg(feature = "alloc")]
use crate::types::{RiAbsoluteString, RiReferenceString, RiRelativeString, RiString};
use crate::validate::Error;

/// Port builder.
///
/// This type is intended to be created by `From` trait implementations, and
/// to be passed to [`Builder::port`] method.
#[derive(Debug, Clone)]
pub struct PortBuilder<'a>(PortBuilderRepr<'a>);

impl Default for PortBuilder<'_> {
    #[inline]
    fn default() -> Self {
        Self(PortBuilderRepr::Empty)
    }
}

impl From<u8> for PortBuilder<'_> {
    #[inline]
    fn from(v: u8) -> Self {
        Self(PortBuilderRepr::Integer(v.into()))
    }
}

impl From<u16> for PortBuilder<'_> {
    #[inline]
    fn from(v: u16) -> Self {
        Self(PortBuilderRepr::Integer(v))
    }
}

impl<'a> From<&'a str> for PortBuilder<'a> {
    #[inline]
    fn from(v: &'a str) -> Self {
        Self(PortBuilderRepr::String(v))
    }
}

#[cfg(feature = "alloc")]
impl<'a> From<&'a alloc::string::String> for PortBuilder<'a> {
    #[inline]
    fn from(v: &'a alloc::string::String) -> Self {
        Self(PortBuilderRepr::String(v.as_str()))
    }
}

/// Internal representation of a port builder.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
enum PortBuilderRepr<'a> {
    /// Empty port.
    Empty,
    /// Port as an integer.
    ///
    /// Note that RFC 3986 accepts any number of digits as a port, but
    /// practically (at least in TCP/IP) `u16` is enough.
    Integer(u16),
    /// Port as a string.
    String(&'a str),
}

/// Userinfo builder.
///
/// This type is intended to be created by `From` trait implementations, and
/// to be passed to [`Builder::userinfo`] method.
#[derive(Clone)]
pub struct UserinfoBuilder<'a>(UserinfoRepr<'a>);

impl Default for UserinfoBuilder<'_> {
    #[inline]
    fn default() -> Self {
        Self(UserinfoRepr::None)
    }
}

impl fmt::Debug for UserinfoBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("UserinfoBuilder");
        if let Some((user, password)) = self.to_user_password() {
            debug.field("user", &user);
            // > Applications should not render as clear text any data after
            // > the first colon (":") character found within a userinfo
            // > subcomponent unless the data after the colon is the empty
            // > string (indicating no password).
            if matches!(password, None | Some("")) {
                debug.field("password", &password);
            } else {
                debug.field("password", &Some(Censored));
            }
        }
        debug.finish()
    }
}

impl<'a> UserinfoBuilder<'a> {
    /// Decomposes the userinfo into `user` and `password`.
    #[must_use]
    fn to_user_password(&self) -> Option<(&'a str, Option<&'a str>)> {
        match &self.0 {
            UserinfoRepr::None => None,
            UserinfoRepr::Direct(s) => match find_split(s, b':') {
                None => Some((s, None)),
                Some((user, password)) => Some((user, Some(password))),
            },
            UserinfoRepr::UserPass(user, password) => Some((*user, *password)),
        }
    }
}

impl<'a> From<&'a str> for UserinfoBuilder<'a> {
    #[inline]
    fn from(direct: &'a str) -> Self {
        Self(UserinfoRepr::Direct(direct))
    }
}

impl<'a> From<(&'a str, &'a str)> for UserinfoBuilder<'a> {
    #[inline]
    fn from((user, password): (&'a str, &'a str)) -> Self {
        Self(UserinfoRepr::UserPass(user, Some(password)))
    }
}

impl<'a> From<(&'a str, Option<&'a str>)> for UserinfoBuilder<'a> {
    #[inline]
    fn from((user, password): (&'a str, Option<&'a str>)) -> Self {
        Self(UserinfoRepr::UserPass(user, password))
    }
}

#[cfg(feature = "alloc")]
impl<'a> From<&'a alloc::string::String> for UserinfoBuilder<'a> {
    #[inline]
    fn from(v: &'a alloc::string::String) -> Self {
        Self::from(v.as_str())
    }
}

/// Internal representation of a userinfo builder.
#[derive(Clone, Copy)]
enum UserinfoRepr<'a> {
    /// Not specified (absent).
    None,
    /// Direct `userinfo` content.
    Direct(&'a str),
    /// User name and password.
    UserPass(&'a str, Option<&'a str>),
}

/// URI/IRI authority builder.
#[derive(Default, Debug, Clone)]
struct AuthorityBuilder<'a> {
    /// Host.
    host: HostRepr<'a>,
    /// Port.
    port: PortBuilder<'a>,
    /// Userinfo.
    userinfo: UserinfoBuilder<'a>,
}

impl AuthorityBuilder<'_> {
    /// Writes the authority to the given formatter.
    fn fmt_write_to<S: Spec>(&self, f: &mut fmt::Formatter<'_>, normalize: bool) -> fmt::Result {
        match &self.userinfo.0 {
            UserinfoRepr::None => {}
            UserinfoRepr::Direct(userinfo) => {
                if normalize {
                    PctCaseNormalized::<S>::new(userinfo).fmt(f)?;
                } else {
                    userinfo.fmt(f)?;
                }
                f.write_char('@')?;
            }
            UserinfoRepr::UserPass(user, password) => {
                if normalize {
                    PctCaseNormalized::<S>::new(user).fmt(f)?;
                } else {
                    f.write_str(user)?;
                }
                if let Some(password) = password {
                    f.write_char(':')?;
                    if normalize {
                        PctCaseNormalized::<S>::new(password).fmt(f)?;
                    } else {
                        password.fmt(f)?;
                    }
                }
                f.write_char('@')?;
            }
        }

        match self.host {
            HostRepr::String(host) => {
                if normalize {
                    normalize::normalize_host_port::<S>(f, host)?;
                } else {
                    f.write_str(host)?;
                }
            }
            #[cfg(feature = "std")]
            HostRepr::IpAddr(ipaddr) => match ipaddr {
                std::net::IpAddr::V4(v) => v.fmt(f)?,
                std::net::IpAddr::V6(v) => write!(f, "[{v}]")?,
            },
        }

        match self.port.0 {
            PortBuilderRepr::Empty => {}
            PortBuilderRepr::Integer(v) => write!(f, ":{v}")?,
            PortBuilderRepr::String(v) => {
                // Omit empty port if the normalization is enabled.
                if !(v.is_empty() && normalize) {
                    write!(f, ":{v}")?;
                }
            }
        }

        Ok(())
    }
}

/// Host representation.
#[derive(Debug, Clone, Copy)]
enum HostRepr<'a> {
    /// Direct string representation.
    String(&'a str),
    #[cfg(feature = "std")]
    /// Dedicated IP address type.
    IpAddr(std::net::IpAddr),
}

impl Default for HostRepr<'_> {
    #[inline]
    fn default() -> Self {
        Self::String("")
    }
}

/// URI/IRI reference builder.
///
/// # Usage
///
/// 1. Create builder by [`Builder::new()`][`Self::new`].
/// 2. Set (or unset) components and set normalization mode as you wish.
/// 3. Validate by [`Builder::build()`][`Self::build`] and get [`Built`] value.
/// 4. Use [`core::fmt::Display`] trait to serialize the resulting [`Built`],
///    or use [`From`]/[`Into`] traits to convert into an allocated string types.
///
/// ```
/// # use iri_string::validate::Error;
/// use iri_string::build::Builder;
/// # #[cfg(not(feature = "alloc"))]
/// # use iri_string::types::IriStr;
/// # #[cfg(feature = "alloc")]
/// use iri_string::types::{IriStr, IriString};
///
/// // 1. Create builder.
/// let mut builder = Builder::new();
///
/// // 2. Set (or unset) component and normalization mode.
/// builder.scheme("http");
/// builder.host("example.com");
/// builder.path("/foo/../");
/// builder.normalize();
///
/// // 3. Validate and create the result.
/// let built = builder.build::<IriStr>()?;
///
/// # #[cfg(feature = "alloc")] {
/// // 4a. Serialize by `Display` trait (or `ToString`).
/// let s = built.to_string();
/// assert_eq!(s, "http://example.com/");
/// # }
///
/// # #[cfg(feature = "alloc")] {
/// // 4b. Convert into an allocated string types.
/// // Thanks to pre-validation by `.build::<IriStr>()`, this conversion is infallible!
/// let s: IriString = built.into();
/// assert_eq!(s, "http://example.com/");
/// # }
///
/// # Ok::<_, Error>(())
/// ```
#[derive(Default, Debug, Clone)]
pub struct Builder<'a> {
    /// Scheme.
    scheme: Option<&'a str>,
    /// Authority.
    authority: Option<AuthorityBuilder<'a>>,
    /// Path.
    path: &'a str,
    /// Query (without the leading `?`).
    query: Option<&'a str>,
    /// Fragment (without the leading `#`).
    fragment: Option<&'a str>,
    /// Normalization mode.
    normalize: bool,
}

impl<'a> Builder<'a> {
    /// Creates a builder with empty data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let builder = Builder::new();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes the authority to the given formatter.
    ///
    /// Don't expose this as public, since this method does not validate.
    ///
    /// # Preconditions
    ///
    /// The IRI string to be built should be a valid IRI reference.
    /// Callers are responsible to validate the component values before calling
    /// this method.
    fn fmt_write_to<S: Spec>(
        &self,
        f: &mut fmt::Formatter<'_>,
        path_is_absolute: bool,
    ) -> fmt::Result {
        if let Some(scheme) = self.scheme {
            // Write the scheme.
            if self.normalize {
                normalize::normalize_scheme(f, scheme)?;
            } else {
                f.write_str(scheme)?;
            }
            f.write_char(':')?;
        }

        if let Some(authority) = &self.authority {
            f.write_str("//")?;
            authority.fmt_write_to::<S>(f, self.normalize)?;
        }

        if !self.normalize {
            // No normalization.
            f.write_str(self.path)?;
        } else if self.scheme.is_some() || self.authority.is_some() || path_is_absolute {
            // Apply full syntax-based normalization.
            let op = normalize::NormalizationOp {
                mode: NormalizationMode::Default,
            };
            normalize::PathToNormalize::from_single_path(self.path).fmt_write_normalize::<S, _>(
                f,
                op,
                self.authority.is_some(),
            )?;
        } else {
            // The IRI reference starts with `path` component, and the path is relative.
            // Skip path segment normalization.
            PctCaseNormalized::<S>::new(self.path).fmt(f)?;
        }

        if let Some(query) = self.query {
            f.write_char('?')?;
            if self.normalize {
                normalize::normalize_query::<S>(f, query)?;
            } else {
                f.write_str(query)?;
            }
        }

        if let Some(fragment) = self.fragment {
            f.write_char('#')?;
            if self.normalize {
                normalize::normalize_fragment::<S>(f, fragment)?;
            } else {
                f.write_str(fragment)?;
            }
        }

        Ok(())
    }

    /// Builds the proxy object that can be converted to the desired IRI string type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriStr;
    /// # #[cfg(feature = "alloc")]
    /// use iri_string::types::IriString;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.scheme("http");
    /// builder.host("example.com");
    /// builder.path("/foo/bar");
    ///
    /// let built = builder.build::<IriStr>()?;
    ///
    /// # #[cfg(feature = "alloc")] {
    /// // The returned value implements `core::fmt::Display` and
    /// // `core::string::ToString`.
    /// assert_eq!(built.to_string(), "http://example.com/foo/bar");
    ///
    /// // The returned value implements `Into<{iri_owned_string_type}>`.
    /// let iri = IriString::from(built);
    /// // `let iri: IriString = built.into();` is also OK.
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn build<T>(self) -> Result<Built<'a, T>, Error>
    where
        T: ?Sized + Buildable<'a>,
    {
        <T as private::Sealed<'a>>::validate_builder(self)
    }
}

// Setters does not return `&mut Self` or `Self` since it introduces needless
// ambiguity for users.
// For example, if setters return something and allows method chaining, can you
// correctly explain what happens with the code below without reading document?
//
// ```text
// let mut builder = Builder::new().foo("foo").bar("bar");
// let baz = builder.baz("baz").clone().build();
// // Should the result be foo+bar+qux, or foo+bar+baz+qux?
// let qux = builder.qux("qux").build();
// ```
impl<'a> Builder<'a> {
    /// Sets the scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.scheme("foo");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "foo:");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn scheme(&mut self, v: &'a str) {
        self.scheme = Some(v);
    }

    /// Unsets the scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.scheme("foo");
    /// builder.unset_scheme();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_scheme(&mut self) {
        self.scheme = None;
    }

    /// Sets the path.
    ///
    /// Note that no methods are provided to "unset" path since every IRI
    /// references has a path component (although it can be empty).
    /// If you want to "unset" the path, just set the empty string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.path("foo/bar");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "foo/bar");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn path(&mut self, v: &'a str) {
        self.path = v;
    }

    /// Initializes the authority builder.
    #[inline]
    fn authority_builder(&mut self) -> &mut AuthorityBuilder<'a> {
        self.authority.get_or_insert_with(AuthorityBuilder::default)
    }

    /// Unsets the authority.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.host("example.com");
    /// builder.unset_authority();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_authority(&mut self) {
        self.authority = None;
    }

    /// Sets the userinfo.
    ///
    /// `userinfo` component always have `user` part (but it can be empty).
    ///
    /// Note that `("", None)` is considered as an empty userinfo, rather than
    /// unset userinfo.
    /// Also note that the user part cannot have colon characters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.userinfo("user:pass");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//user:pass@");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// You can specify `(user, password)` pair.
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.userinfo(("user", Some("pass")));
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(
    ///     builder.clone().build::<IriReferenceStr>()?.to_string(),
    ///     "//user:pass@"
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// `("", None)` is considered as an empty userinfo.
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.userinfo(("", None));
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//@");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn userinfo<T: Into<UserinfoBuilder<'a>>>(&mut self, v: T) {
        self.authority_builder().userinfo = v.into();
    }

    /// Unsets the port.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.userinfo("user:pass");
    /// // Note that this does not unset the entire authority.
    /// // Now empty authority is set.
    /// builder.unset_userinfo();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_userinfo(&mut self) {
        self.authority_builder().userinfo = UserinfoBuilder::default();
    }

    /// Sets the reg-name or IP address (i.e. host) without port.
    ///
    /// Note that no methods are provided to "unset" host.
    /// Depending on your situation, set empty string as a reg-name, or unset
    /// the authority entirely by [`unset_authority`][`Self::unset_authority`]
    /// method.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.host("example.com");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//example.com");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn host(&mut self, v: &'a str) {
        self.authority_builder().host = HostRepr::String(v);
    }

    /// Sets the IP address as a host.
    ///
    /// Note that no methods are provided to "unset" host.
    /// Depending on your situation, set empty string as a reg-name, or unset
    /// the authority entirely by [`unset_authority`][`Self::unset_authority`]
    /// method.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "std")] {
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.ip_address(std::net::Ipv4Addr::new(192, 0, 2, 0));
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//192.0.2.0");
    /// # }
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[cfg(feature = "std")]
    #[inline]
    pub fn ip_address<T: Into<std::net::IpAddr>>(&mut self, addr: T) {
        self.authority_builder().host = HostRepr::IpAddr(addr.into());
    }

    /// Sets the port.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.port(80_u16);
    /// // Accepts other types that implements `Into<PortBuilder<'a>>`.
    /// //builder.port(80_u8);
    /// //builder.port("80");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//:80");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn port<T: Into<PortBuilder<'a>>>(&mut self, v: T) {
        self.authority_builder().port = v.into();
    }

    /// Unsets the port.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.port(80_u16);
    /// // Note that this does not unset the entire authority.
    /// // Now empty authority is set.
    /// builder.unset_port();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "//");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_port(&mut self) {
        self.authority_builder().port = PortBuilder::default();
    }

    /// Sets the query.
    ///
    /// The string after `?` should be specified.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.query("q=example");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "?q=example");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn query(&mut self, v: &'a str) {
        self.query = Some(v);
    }

    /// Unsets the query.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.query("q=example");
    /// builder.unset_query();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_query(&mut self) {
        self.query = None;
    }

    /// Sets the fragment.
    ///
    /// The string after `#` should be specified.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.fragment("anchor");
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "#anchor");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn fragment(&mut self, v: &'a str) {
        self.fragment = Some(v);
    }

    /// Unsets the fragment.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.fragment("anchor");
    /// builder.unset_fragment();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_fragment(&mut self) {
        self.fragment = None;
    }

    /// Stop normalizing the result.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.scheme("http");
    /// // `%75%73%65%72` is "user".
    /// builder.userinfo("%75%73%65%72");
    /// builder.host("EXAMPLE.COM");
    /// builder.port("");
    /// builder.path("/foo/../%2e%2e/bar/%2e/baz/.");
    ///
    /// builder.unset_normalize();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(
    ///     iri.to_string(),
    ///     "http://%75%73%65%72@EXAMPLE.COM:/foo/../%2e%2e/bar/%2e/baz/."
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn unset_normalize(&mut self) {
        self.normalize = false;
    }

    /// Normalizes the result using RFC 3986 syntax-based normalization and
    /// WHATWG URL Standard algorithm.
    ///
    /// # Normalization
    ///
    /// If `scheme` or `authority` component is present or the path is absolute,
    /// the build result will fully normalized using full syntax-based normalization:
    ///
    /// * case normalization ([RFC 3986 6.2.2.1]),
    /// * percent-encoding normalization ([RFC 3986 6.2.2.2]), and
    /// * path segment normalization ([RFC 3986 6.2.2.2]).
    ///
    /// However, if both `scheme` and `authority` is absent and the path is relative
    /// (including empty), i.e. the IRI reference to be built starts with the
    /// relative `path` component, path segment normalization will be omitted.
    /// This is because the path segment normalization depends on presence or
    /// absense of the `authority` components, and will remove extra `..`
    /// segments which should not be ignored.
    ///
    /// Note that `path` must already be empty or start with a slash **before
    /// the normalizaiton** if `authority` is present.
    ///
    /// # WHATWG URL Standard
    ///
    /// If you need to avoid WHATWG URL Standard serialization, use
    /// [`Built::ensure_rfc3986_normalizable`] method to test if the result is
    /// normalizable without WHATWG spec.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::build::Builder;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let mut builder = Builder::new();
    /// builder.scheme("http");
    /// // `%75%73%65%72` is "user".
    /// builder.userinfo("%75%73%65%72");
    /// builder.host("EXAMPLE.COM");
    /// builder.port("");
    /// builder.path("/foo/../%2e%2e/bar/%2e/baz/.");
    ///
    /// builder.normalize();
    ///
    /// let iri = builder.build::<IriReferenceStr>()?;
    /// # #[cfg(feature = "alloc")] {
    /// assert_eq!(iri.to_string(), "http://user@example.com/bar/baz/");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn normalize(&mut self) {
        self.normalize = true;
    }
}

/// [`Display`]-able IRI build result.
///
/// The value of this type can generate an IRI using [`From`]/[`Into`] traits or
/// [`Display`] trait.
///
/// # Security consideration
///
/// This can be stringified or directly printed by `std::fmt::Display`, but note
/// that this `Display` **does not hide the password part**. Be careful **not to
/// print the value using `Display for Built<_>` in public context**.
///
/// [`From`]: `core::convert::From`
/// [`Into`]: `core::convert::Into`
/// [`Display`]: `core::fmt::Display`
#[derive(Debug)]
pub struct Built<'a, T: ?Sized> {
    /// Builder with the validated content.
    builder: Builder<'a>,
    /// Whether the path is absolute.
    path_is_absolute: bool,
    /// String type.
    _ty_str: PhantomData<fn() -> T>,
}

impl<T: ?Sized> Clone for Built<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            builder: self.builder.clone(),
            path_is_absolute: self.path_is_absolute,
            _ty_str: PhantomData,
        }
    }
}

/// Implements conversions to a string.
macro_rules! impl_stringifiers {
    ($borrowed:ident, $owned:ident) => {
        impl<S: Spec> Built<'_, $borrowed<S>> {
            /// Returns Ok`(())` if the IRI is normalizable by the RFC 3986 algorithm.
            #[inline]
            pub fn ensure_rfc3986_normalizable(&self) -> Result<(), normalize::Error> {
                if self.builder.authority.is_none() {
                    let path = normalize::PathToNormalize::from_single_path(self.builder.path);
                    path.ensure_rfc3986_normalizable_with_authority_absent()?;
                }
                Ok(())
            }
        }

        impl<S: Spec> fmt::Display for Built<'_, $borrowed<S>> {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.builder.fmt_write_to::<S>(f, self.path_is_absolute)
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: Spec> ToDedicatedString for Built<'_, $borrowed<S>> {
            type Target = $owned<S>;

            #[inline]
            fn try_to_dedicated_string(&self) -> Result<Self::Target, TryReserveError> {
                let s = self.try_to_string()?;
                Ok(TryFrom::try_from(s)
                    .expect("[validity] the IRI to be built is already validated"))
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: Spec> From<Built<'_, $borrowed<S>>> for $owned<S> {
            #[inline]
            fn from(builder: Built<'_, $borrowed<S>>) -> Self {
                (&builder).into()
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: Spec> From<&Built<'_, $borrowed<S>>> for $owned<S> {
            #[inline]
            fn from(builder: &Built<'_, $borrowed<S>>) -> Self {
                let s = builder.to_string();
                Self::try_from(s).expect("[validity] the IRI to be built is already validated")
            }
        }
    };
}

impl_stringifiers!(RiReferenceStr, RiReferenceString);
impl_stringifiers!(RiStr, RiString);
impl_stringifiers!(RiAbsoluteStr, RiAbsoluteString);
impl_stringifiers!(RiRelativeStr, RiRelativeString);

/// A trait for borrowed IRI string types buildable by the [`Builder`].
pub trait Buildable<'a>: private::Sealed<'a> {}

impl<'a, S: Spec> private::Sealed<'a> for RiReferenceStr<S> {
    fn validate_builder(builder: Builder<'a>) -> Result<Built<'a, Self>, Error> {
        let path_is_absolute = validate_builder_for_iri_reference::<S>(&builder)?;

        Ok(Built {
            builder,
            path_is_absolute,
            _ty_str: PhantomData,
        })
    }
}
impl<S: Spec> Buildable<'_> for RiReferenceStr<S> {}

impl<'a, S: Spec> private::Sealed<'a> for RiStr<S> {
    fn validate_builder(builder: Builder<'a>) -> Result<Built<'a, Self>, Error> {
        if builder.scheme.is_none() {
            return Err(Error::new());
        }
        let path_is_absolute = validate_builder_for_iri_reference::<S>(&builder)?;

        Ok(Built {
            builder,
            path_is_absolute,
            _ty_str: PhantomData,
        })
    }
}
impl<S: Spec> Buildable<'_> for RiStr<S> {}

impl<'a, S: Spec> private::Sealed<'a> for RiAbsoluteStr<S> {
    fn validate_builder(builder: Builder<'a>) -> Result<Built<'a, Self>, Error> {
        if builder.scheme.is_none() {
            return Err(Error::new());
        }
        if builder.fragment.is_some() {
            return Err(Error::new());
        }
        let path_is_absolute = validate_builder_for_iri_reference::<S>(&builder)?;

        Ok(Built {
            builder,
            path_is_absolute,
            _ty_str: PhantomData,
        })
    }
}
impl<S: Spec> Buildable<'_> for RiAbsoluteStr<S> {}

impl<'a, S: Spec> private::Sealed<'a> for RiRelativeStr<S> {
    fn validate_builder(builder: Builder<'a>) -> Result<Built<'a, Self>, Error> {
        if builder.scheme.is_some() {
            return Err(Error::new());
        }
        let path_is_absolute = validate_builder_for_iri_reference::<S>(&builder)?;

        Ok(Built {
            builder,
            path_is_absolute,
            _ty_str: PhantomData,
        })
    }
}
impl<S: Spec> Buildable<'_> for RiRelativeStr<S> {}

/// Checks whether the builder output is valid IRI reference.
///
/// Returns whether the path is absolute.
fn validate_builder_for_iri_reference<S: Spec>(builder: &Builder<'_>) -> Result<bool, Error> {
    if let Some(scheme) = builder.scheme {
        parser::validate_scheme(scheme)?;
    }

    if let Some(authority) = &builder.authority {
        match &authority.userinfo.0 {
            UserinfoRepr::None => {}
            UserinfoRepr::Direct(userinfo) => {
                parser::validate_userinfo::<S>(userinfo)?;
            }
            UserinfoRepr::UserPass(user, password) => {
                // `user` is not allowed to have a colon, since the characters
                // after the colon is parsed as the password.
                if user.contains(':') {
                    return Err(Error::new());
                }

                // Note that the syntax of components inside `authority`
                // (`user` and `password`) is not specified by RFC 3986.
                parser::validate_userinfo::<S>(user)?;
                if let Some(password) = password {
                    parser::validate_userinfo::<S>(password)?;
                }
            }
        }

        match authority.host {
            HostRepr::String(s) => parser::validate_host::<S>(s)?,
            #[cfg(feature = "std")]
            HostRepr::IpAddr(_) => {}
        }

        if let PortBuilderRepr::String(s) = authority.port.0 {
            if !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::new());
            }
        }
    }

    let path_is_absolute: bool;
    let mut is_path_acceptable;
    if builder.normalize {
        if builder.scheme.is_some() || builder.authority.is_some() || builder.path.starts_with('/')
        {
            if builder.authority.is_some() {
                // Note that the path should already be in an absolute form before normalization.
                is_path_acceptable = builder.path.is_empty() || builder.path.starts_with('/');
            } else {
                is_path_acceptable = true;
            }
            let op = normalize::NormalizationOp {
                mode: NormalizationMode::Default,
            };
            let path_characteristic = PathCharacteristic::from_path_to_display::<S>(
                &normalize::PathToNormalize::from_single_path(builder.path),
                op,
                builder.authority.is_some(),
            );
            path_is_absolute = path_characteristic.is_absolute();
            is_path_acceptable = is_path_acceptable
                && match path_characteristic {
                    PathCharacteristic::CommonAbsolute | PathCharacteristic::CommonRelative => true,
                    PathCharacteristic::StartsWithDoubleSlash
                    | PathCharacteristic::RelativeFirstSegmentHasColon => {
                        builder.scheme.is_some() || builder.authority.is_some()
                    }
                };
        } else {
            path_is_absolute = false;
            // If the path is relative (where neither scheme nor authority is
            // available), the first segment should not contain a colon.
            is_path_acceptable = prior_byte2(builder.path.as_bytes(), b'/', b':') != Some(b':');
        }
    } else {
        path_is_absolute = builder.path.starts_with('/');
        is_path_acceptable = if builder.authority.is_some() {
            // The path should be absolute or empty.
            path_is_absolute || builder.path.is_empty()
        } else if builder.scheme.is_some() || path_is_absolute {
            // The path should not start with '//'.
            !builder.path.starts_with("//")
        } else {
            // If the path is relative (where neither scheme nor authority is
            // available), the first segment should not contain a colon.
            prior_byte2(builder.path.as_bytes(), b'/', b':') != Some(b':')
        };
    }
    if !is_path_acceptable {
        return Err(Error::new());
    }

    if let Some(query) = builder.query {
        parser::validate_query::<S>(query)?;
    }

    if let Some(fragment) = builder.fragment {
        parser::validate_fragment::<S>(fragment)?;
    }

    Ok(path_is_absolute)
}

/// Private module to put the trait to seal.
mod private {
    use super::{Builder, Built, Error};

    /// A trait for types buildable by the [`Builder`].
    pub trait Sealed<'a> {
        /// Validates the content of the builder and returns the validated type if possible.
        fn validate_builder(builder: Builder<'a>) -> Result<Built<'a, Self>, Error>;
    }
}
