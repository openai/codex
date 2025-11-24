//! URI component of request and response lines
//!
//! This module primarily contains the `Uri` type which is a component of all
//! HTTP requests and also reexports this type at the root of the crate. A URI
//! is not always a "full URL" in the sense of something you'd type into a web
//! browser, but HTTP requests may only have paths on servers but may have full
//! schemes and hostnames on clients.
//!
//! # Examples
//!
//! ```
//! use http::Uri;
//!
//! let uri = "/foo/bar?baz".parse::<Uri>().unwrap();
//! assert_eq!(uri.path(), "/foo/bar");
//! assert_eq!(uri.query(), Some("baz"));
//! assert_eq!(uri.host(), None);
//!
//! let uri = "https://www.rust-lang.org/install.html".parse::<Uri>().unwrap();
//! assert_eq!(uri.scheme_str(), Some("https"));
//! assert_eq!(uri.host(), Some("www.rust-lang.org"));
//! assert_eq!(uri.path(), "/install.html");
//! ```

use crate::byte_str::ByteStr;
use std::convert::TryFrom;

use bytes::Bytes;

use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::{self, FromStr};

use self::scheme::Scheme2;

pub use self::authority::Authority;
pub use self::builder::Builder;
pub use self::path::PathAndQuery;
pub use self::port::Port;
pub use self::scheme::Scheme;

mod authority;
mod builder;
mod path;
mod port;
mod scheme;
#[cfg(test)]
mod tests;

/// The URI component of a request.
///
/// For HTTP 1, this is included as part of the request line. From Section 5.3,
/// Request Target:
///
/// > Once an inbound connection is obtained, the client sends an HTTP
/// > request message (Section 3) with a request-target derived from the
/// > target URI.  There are four distinct formats for the request-target,
/// > depending on both the method being requested and whether the request
/// > is to a proxy.
/// >
/// > ```notrust
/// > request-target = origin-form
/// >                / absolute-form
/// >                / authority-form
/// >                / asterisk-form
/// > ```
///
/// The URI is structured as follows:
///
/// ```notrust
/// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
/// |-|   |-------------------------------||--------| |-------------------| |-----|
///  |                  |                       |               |              |
/// scheme          authority                 path            query         fragment
/// ```
///
/// For HTTP 2.0, the URI is encoded using pseudoheaders.
///
/// # Examples
///
/// ```
/// use http::Uri;
///
/// let uri = "/foo/bar?baz".parse::<Uri>().unwrap();
/// assert_eq!(uri.path(), "/foo/bar");
/// assert_eq!(uri.query(), Some("baz"));
/// assert_eq!(uri.host(), None);
///
/// let uri = "https://www.rust-lang.org/install.html".parse::<Uri>().unwrap();
/// assert_eq!(uri.scheme_str(), Some("https"));
/// assert_eq!(uri.host(), Some("www.rust-lang.org"));
/// assert_eq!(uri.path(), "/install.html");
/// ```
#[derive(Clone)]
pub struct Uri {
    scheme: Scheme,
    authority: Authority,
    path_and_query: PathAndQuery,
}

/// The various parts of a URI.
///
/// This struct is used to provide to and retrieve from a URI.
#[derive(Debug, Default)]
pub struct Parts {
    /// The scheme component of a URI
    pub scheme: Option<Scheme>,

    /// The authority component of a URI
    pub authority: Option<Authority>,

    /// The origin-form component of a URI
    pub path_and_query: Option<PathAndQuery>,

    /// Allow extending in the future
    _priv: (),
}

/// An error resulting from a failed attempt to construct a URI.
#[derive(Debug)]
pub struct InvalidUri(ErrorKind);

/// An error resulting from a failed attempt to construct a URI.
#[derive(Debug)]
pub struct InvalidUriParts(InvalidUri);

#[derive(Debug, Eq, PartialEq)]
enum ErrorKind {
    InvalidUriChar,
    InvalidScheme,
    InvalidAuthority,
    InvalidPort,
    InvalidFormat,
    SchemeMissing,
    AuthorityMissing,
    PathAndQueryMissing,
    TooLong,
    Empty,
    SchemeTooLong,
}

// u16::MAX is reserved for None
const MAX_LEN: usize = (u16::MAX - 1) as usize;

// URI_CHARS is a table of valid characters in a URI. An entry in the table is
// 0 for invalid characters. For valid characters the entry is itself (i.e.
// the entry for 33 is b'!' because b'!' == 33u8). An important characteristic
// of this table is that all entries above 127 are invalid. This makes all of the
// valid entries a valid single-byte UTF-8 code point. This means that a slice
// of such valid entries is valid UTF-8.
#[rustfmt::skip]
const URI_CHARS: [u8; 256] = [
    //  0      1      2      3      4      5      6      7      8      9
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //   x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  1x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  2x
        0,     0,     0,  b'!',     0,  b'#',  b'$',     0,  b'&', b'\'', //  3x
     b'(',  b')',  b'*',  b'+',  b',',  b'-',  b'.',  b'/',  b'0',  b'1', //  4x
     b'2',  b'3',  b'4',  b'5',  b'6',  b'7',  b'8',  b'9',  b':',  b';', //  5x
        0,  b'=',     0,  b'?',  b'@',  b'A',  b'B',  b'C',  b'D',  b'E', //  6x
     b'F',  b'G',  b'H',  b'I',  b'J',  b'K',  b'L',  b'M',  b'N',  b'O', //  7x
     b'P',  b'Q',  b'R',  b'S',  b'T',  b'U',  b'V',  b'W',  b'X',  b'Y', //  8x
     b'Z',  b'[',     0,  b']',     0,  b'_',     0,  b'a',  b'b',  b'c', //  9x
     b'd',  b'e',  b'f',  b'g',  b'h',  b'i',  b'j',  b'k',  b'l',  b'm', // 10x
     b'n',  b'o',  b'p',  b'q',  b'r',  b's',  b't',  b'u',  b'v',  b'w', // 11x
     b'x',  b'y',  b'z',     0,     0,     0,  b'~',     0,     0,     0, // 12x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 13x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 14x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 15x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 16x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 17x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 18x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 19x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 20x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 21x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 22x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 23x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, // 24x
        0,     0,     0,     0,     0,     0                              // 25x
];

impl Uri {
    /// Creates a new builder-style object to manufacture a `Uri`.
    ///
    /// This method returns an instance of `Builder` which can be usd to
    /// create a `Uri`.
    ///
    /// # Examples
    ///
    /// ```
    /// use http::Uri;
    ///
    /// let uri = Uri::builder()
    ///     .scheme("https")
    ///     .authority("hyper.rs")
    ///     .path_and_query("/")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Attempt to convert a `Parts` into a `Uri`.
    ///
    /// # Examples
    ///
    /// Relative URI
    ///
    /// ```
    /// # use http::uri::*;
    /// let mut parts = Parts::default();
    /// parts.path_and_query = Some("/foo".parse().unwrap());
    ///
    /// let uri = Uri::from_parts(parts).unwrap();
    ///
    /// assert_eq!(uri.path(), "/foo");
    ///
    /// assert!(uri.scheme().is_none());
    /// assert!(uri.authority().is_none());
    /// ```
    ///
    /// Absolute URI
    ///
    /// ```
    /// # use http::uri::*;
    /// let mut parts = Parts::default();
    /// parts.scheme = Some("http".parse().unwrap());
    /// parts.authority = Some("foo.com".parse().unwrap());
    /// parts.path_and_query = Some("/foo".parse().unwrap());
    ///
    /// let uri = Uri::from_parts(parts).unwrap();
    ///
    /// assert_eq!(uri.scheme().unwrap().as_str(), "http");
    /// assert_eq!(uri.authority().unwrap(), "foo.com");
    /// assert_eq!(uri.path(), "/foo");
    /// ```
    pub fn from_parts(src: Parts) -> Result<Uri, InvalidUriParts> {
        if src.scheme.is_some() {
            if src.authority.is_none() {
                return Err(ErrorKind::AuthorityMissing.into());
            }

            if src.path_and_query.is_none() {
                return Err(ErrorKind::PathAndQueryMissing.into());
            }
        } else if src.authority.is_some() && src.path_and_query.is_some() {
            return Err(ErrorKind::SchemeMissing.into());
        }

        let scheme = match src.scheme {
            Some(scheme) => scheme,
            None => Scheme {
                inner: Scheme2::None,
            },
        };

        let authority = match src.authority {
            Some(authority) => authority,
            None => Authority::empty(),
        };

        let path_and_query = match src.path_and_query {
            Some(path_and_query) => path_and_query,
            None => PathAndQuery::empty(),
        };

        Ok(Uri {
            scheme,
            authority,
            path_and_query,
        })
    }

    /// Attempt to convert a `Bytes` buffer to a `Uri`.
    ///
    /// This will try to prevent a copy if the type passed is the type used
    /// internally, and will copy the data if it is not.
    pub fn from_maybe_shared<T>(src: T) -> Result<Self, InvalidUri>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return Uri::from_shared(src);
        });

        Uri::try_from(src.as_ref())
    }

    // Not public while `bytes` is unstable.
    fn from_shared(s: Bytes) -> Result<Uri, InvalidUri> {
        use self::ErrorKind::*;

        if s.len() > MAX_LEN {
            return Err(TooLong.into());
        }

        match s.len() {
            0 => {
                return Err(Empty.into());
            }
            1 => match s[0] {
                b'/' => {
                    return Ok(Uri {
                        scheme: Scheme::empty(),
                        authority: Authority::empty(),
                        path_and_query: PathAndQuery::slash(),
                    });
                }
                b'*' => {
                    return Ok(Uri {
                        scheme: Scheme::empty(),
                        authority: Authority::empty(),
                        path_and_query: PathAndQuery::star(),
                    });
                }
                _ => {
                    let authority = Authority::from_shared(s)?;

                    return Ok(Uri {
                        scheme: Scheme::empty(),
                        authority,
                        path_and_query: PathAndQuery::empty(),
                    });
                }
            },
            _ => {}
        }

        if s[0] == b'/' {
            return Ok(Uri {
                scheme: Scheme::empty(),
                authority: Authority::empty(),
                path_and_query: PathAndQuery::from_shared(s)?,
            });
        }

        parse_full(s)
    }

    /// Convert a `Uri` from a static string.
    ///
    /// This function will not perform any copying, however the string is
    /// checked to ensure that it is valid.
    ///
    /// # Panics
    ///
    /// This function panics if the argument is an invalid URI.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::Uri;
    /// let uri = Uri::from_static("http://example.com/foo");
    ///
    /// assert_eq!(uri.host().unwrap(), "example.com");
    /// assert_eq!(uri.path(), "/foo");
    /// ```
    pub fn from_static(src: &'static str) -> Self {
        let s = Bytes::from_static(src.as_bytes());
        match Uri::from_shared(s) {
            Ok(uri) => uri,
            Err(e) => panic!("static str is not valid URI: {}", e),
        }
    }

    /// Convert a `Uri` into `Parts`.
    ///
    /// # Note
    ///
    /// This is just an inherent method providing the same functionality as
    /// `let parts: Parts = uri.into()`
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::*;
    /// let uri: Uri = "/foo".parse().unwrap();
    ///
    /// let parts = uri.into_parts();
    ///
    /// assert_eq!(parts.path_and_query.unwrap(), "/foo");
    ///
    /// assert!(parts.scheme.is_none());
    /// assert!(parts.authority.is_none());
    /// ```
    #[inline]
    pub fn into_parts(self) -> Parts {
        self.into()
    }

    /// Returns the path & query components of the Uri
    #[inline]
    pub fn path_and_query(&self) -> Option<&PathAndQuery> {
        if !self.scheme.inner.is_none() || self.authority.data.is_empty() {
            Some(&self.path_and_query)
        } else {
            None
        }
    }

    /// Get the path of this `Uri`.
    ///
    /// Both relative and absolute URIs contain a path component, though it
    /// might be the empty string. The path component is **case sensitive**.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                        |--------|
    ///                                             |
    ///                                           path
    /// ```
    ///
    /// If the URI is `*` then the path component is equal to `*`.
    ///
    /// # Examples
    ///
    /// A relative URI
    ///
    /// ```
    /// # use http::Uri;
    ///
    /// let uri: Uri = "/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.path(), "/hello/world");
    /// ```
    ///
    /// An absolute URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.path(), "/hello/world");
    /// ```
    #[inline]
    pub fn path(&self) -> &str {
        if self.has_path() {
            self.path_and_query.path()
        } else {
            ""
        }
    }

    /// Get the scheme of this `Uri`.
    ///
    /// The URI scheme refers to a specification for assigning identifiers
    /// within that scheme. Only absolute URIs contain a scheme component, but
    /// not all absolute URIs will contain a scheme component.  Although scheme
    /// names are case-insensitive, the canonical form is lowercase.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    /// |-|
    ///  |
    /// scheme
    /// ```
    ///
    /// # Examples
    ///
    /// Absolute URI
    ///
    /// ```
    /// use http::uri::{Scheme, Uri};
    ///
    /// let uri: Uri = "http://example.org/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.scheme(), Some(&Scheme::HTTP));
    /// ```
    ///
    ///
    /// Relative URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "/hello/world".parse().unwrap();
    ///
    /// assert!(uri.scheme().is_none());
    /// ```
    #[inline]
    pub fn scheme(&self) -> Option<&Scheme> {
        if self.scheme.inner.is_none() {
            None
        } else {
            Some(&self.scheme)
        }
    }

    /// Get the scheme of this `Uri` as a `&str`.
    ///
    /// # Example
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.scheme_str(), Some("http"));
    /// ```
    #[inline]
    pub fn scheme_str(&self) -> Option<&str> {
        if self.scheme.inner.is_none() {
            None
        } else {
            Some(self.scheme.as_str())
        }
    }

    /// Get the authority of this `Uri`.
    ///
    /// The authority is a hierarchical element for naming authority such that
    /// the remainder of the URI is delegated to that authority. For HTTP, the
    /// authority consists of the host and port. The host portion of the
    /// authority is **case-insensitive**.
    ///
    /// The authority also includes a `username:password` component, however
    /// the use of this is deprecated and should be avoided.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///       |-------------------------------|
    ///                     |
    ///                 authority
    /// ```
    ///
    /// # Examples
    ///
    /// Absolute URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org:80/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.authority().map(|a| a.as_str()), Some("example.org:80"));
    /// ```
    ///
    ///
    /// Relative URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "/hello/world".parse().unwrap();
    ///
    /// assert!(uri.authority().is_none());
    /// ```
    #[inline]
    pub fn authority(&self) -> Option<&Authority> {
        if self.authority.data.is_empty() {
            None
        } else {
            Some(&self.authority)
        }
    }

    /// Get the host of this `Uri`.
    ///
    /// The host subcomponent of authority is identified by an IP literal
    /// encapsulated within square brackets, an IPv4 address in dotted- decimal
    /// form, or a registered name.  The host subcomponent is **case-insensitive**.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                         |---------|
    ///                              |
    ///                             host
    /// ```
    ///
    /// # Examples
    ///
    /// Absolute URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org:80/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.host(), Some("example.org"));
    /// ```
    ///
    ///
    /// Relative URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "/hello/world".parse().unwrap();
    ///
    /// assert!(uri.host().is_none());
    /// ```
    #[inline]
    pub fn host(&self) -> Option<&str> {
        self.authority().map(|a| a.host())
    }

    /// Get the port part of this `Uri`.
    ///
    /// The port subcomponent of authority is designated by an optional port
    /// number following the host and delimited from it by a single colon (":")
    /// character. It can be turned into a decimal port number with the `as_u16`
    /// method or as a `str` with the `as_str` method.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                     |-|
    ///                                      |
    ///                                     port
    /// ```
    ///
    /// # Examples
    ///
    /// Absolute URI with port
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org:80/hello/world".parse().unwrap();
    ///
    /// let port = uri.port().unwrap();
    /// assert_eq!(port.as_u16(), 80);
    /// ```
    ///
    /// Absolute URI without port
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org/hello/world".parse().unwrap();
    ///
    /// assert!(uri.port().is_none());
    /// ```
    ///
    /// Relative URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "/hello/world".parse().unwrap();
    ///
    /// assert!(uri.port().is_none());
    /// ```
    pub fn port(&self) -> Option<Port<&str>> {
        self.authority().and_then(|a| a.port())
    }

    /// Get the port of this `Uri` as a `u16`.
    ///
    ///
    /// # Example
    ///
    /// ```
    /// # use http::{Uri, uri::Port};
    /// let uri: Uri = "http://example.org:80/hello/world".parse().unwrap();
    ///
    /// assert_eq!(uri.port_u16(), Some(80));
    /// ```
    pub fn port_u16(&self) -> Option<u16> {
        self.port().map(|p| p.as_u16())
    }

    /// Get the query string of this `Uri`, starting after the `?`.
    ///
    /// The query component contains non-hierarchical data that, along with data
    /// in the path component, serves to identify a resource within the scope of
    /// the URI's scheme and naming authority (if any). The query component is
    /// indicated by the first question mark ("?") character and terminated by a
    /// number sign ("#") character or by the end of the URI.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                                   |-------------------|
    ///                                                             |
    ///                                                           query
    /// ```
    ///
    /// # Examples
    ///
    /// Absolute URI
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org/hello/world?key=value".parse().unwrap();
    ///
    /// assert_eq!(uri.query(), Some("key=value"));
    /// ```
    ///
    /// Relative URI with a query string component
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "/hello/world?key=value&foo=bar".parse().unwrap();
    ///
    /// assert_eq!(uri.query(), Some("key=value&foo=bar"));
    /// ```
    ///
    /// Relative URI without a query string component
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "/hello/world".parse().unwrap();
    ///
    /// assert!(uri.query().is_none());
    /// ```
    #[inline]
    pub fn query(&self) -> Option<&str> {
        self.path_and_query.query()
    }

    fn has_path(&self) -> bool {
        !self.path_and_query.data.is_empty() || !self.scheme.inner.is_none()
    }
}

impl<'a> TryFrom<&'a [u8]> for Uri {
    type Error = InvalidUri;

    #[inline]
    fn try_from(t: &'a [u8]) -> Result<Self, Self::Error> {
        Uri::from_shared(Bytes::copy_from_slice(t))
    }
}

impl<'a> TryFrom<&'a str> for Uri {
    type Error = InvalidUri;

    #[inline]
    fn try_from(t: &'a str) -> Result<Self, Self::Error> {
        t.parse()
    }
}

impl<'a> TryFrom<&'a String> for Uri {
    type Error = InvalidUri;

    #[inline]
    fn try_from(t: &'a String) -> Result<Self, Self::Error> {
        t.parse()
    }
}

impl TryFrom<String> for Uri {
    type Error = InvalidUri;

    #[inline]
    fn try_from(t: String) -> Result<Self, Self::Error> {
        Uri::from_shared(Bytes::from(t))
    }
}

impl TryFrom<Vec<u8>> for Uri {
    type Error = InvalidUri;

    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        Uri::from_shared(Bytes::from(vec))
    }
}

impl TryFrom<Parts> for Uri {
    type Error = InvalidUriParts;

    #[inline]
    fn try_from(src: Parts) -> Result<Self, Self::Error> {
        Uri::from_parts(src)
    }
}

impl<'a> TryFrom<&'a Uri> for Uri {
    type Error = crate::Error;

    #[inline]
    fn try_from(src: &'a Uri) -> Result<Self, Self::Error> {
        Ok(src.clone())
    }
}

/// Convert an `Authority` into a `Uri`.
impl From<Authority> for Uri {
    fn from(authority: Authority) -> Self {
        Self {
            scheme: Scheme::empty(),
            authority,
            path_and_query: PathAndQuery::empty(),
        }
    }
}

/// Convert a `PathAndQuery` into a `Uri`.
impl From<PathAndQuery> for Uri {
    fn from(path_and_query: PathAndQuery) -> Self {
        Self {
            scheme: Scheme::empty(),
            authority: Authority::empty(),
            path_and_query,
        }
    }
}

/// Convert a `Uri` into `Parts`
impl From<Uri> for Parts {
    fn from(src: Uri) -> Self {
        let path_and_query = if src.has_path() {
            Some(src.path_and_query)
        } else {
            None
        };

        let scheme = match src.scheme.inner {
            Scheme2::None => None,
            _ => Some(src.scheme),
        };

        let authority = if src.authority.data.is_empty() {
            None
        } else {
            Some(src.authority)
        };

        Parts {
            scheme,
            authority,
            path_and_query,
            _priv: (),
        }
    }
}

fn parse_full(mut s: Bytes) -> Result<Uri, InvalidUri> {
    // Parse the scheme
    let scheme = match Scheme2::parse(&s[..])? {
        Scheme2::None => Scheme2::None,
        Scheme2::Standard(p) => {
            // TODO: use truncate
            let _ = s.split_to(p.len() + 3);
            Scheme2::Standard(p)
        }
        Scheme2::Other(n) => {
            // Grab the protocol
            let mut scheme = s.split_to(n + 3);

            // Strip ://, TODO: truncate
            let _ = scheme.split_off(n);

            // Allocate the ByteStr
            let val = unsafe { ByteStr::from_utf8_unchecked(scheme) };

            Scheme2::Other(Box::new(val))
        }
    };

    // Find the end of the authority. The scheme will already have been
    // extracted.
    let authority_end = Authority::parse(&s[..])?;

    if scheme.is_none() {
        if authority_end != s.len() {
            return Err(ErrorKind::InvalidFormat.into());
        }

        let authority = Authority {
            data: unsafe { ByteStr::from_utf8_unchecked(s) },
        };

        return Ok(Uri {
            scheme: scheme.into(),
            authority,
            path_and_query: PathAndQuery::empty(),
        });
    }

    // Authority is required when absolute
    if authority_end == 0 {
        return Err(ErrorKind::InvalidFormat.into());
    }

    let authority = s.split_to(authority_end);
    let authority = Authority {
        data: unsafe { ByteStr::from_utf8_unchecked(authority) },
    };

    Ok(Uri {
        scheme: scheme.into(),
        authority,
        path_and_query: PathAndQuery::from_shared(s)?,
    })
}

impl FromStr for Uri {
    type Err = InvalidUri;

    #[inline]
    fn from_str(s: &str) -> Result<Uri, InvalidUri> {
        Uri::try_from(s.as_bytes())
    }
}

impl PartialEq for Uri {
    fn eq(&self, other: &Uri) -> bool {
        if self.scheme() != other.scheme() {
            return false;
        }

        if self.authority() != other.authority() {
            return false;
        }

        if self.path() != other.path() {
            return false;
        }

        if self.query() != other.query() {
            return false;
        }

        true
    }
}

impl PartialEq<str> for Uri {
    fn eq(&self, other: &str) -> bool {
        let mut other = other.as_bytes();
        let mut absolute = false;

        if let Some(scheme) = self.scheme() {
            let scheme = scheme.as_str().as_bytes();
            absolute = true;

            if other.len() < scheme.len() + 3 {
                return false;
            }

            if !scheme.eq_ignore_ascii_case(&other[..scheme.len()]) {
                return false;
            }

            other = &other[scheme.len()..];

            if &other[..3] != b"://" {
                return false;
            }

            other = &other[3..];
        }

        if let Some(auth) = self.authority() {
            let len = auth.data.len();
            absolute = true;

            if other.len() < len {
                return false;
            }

            if !auth.data.as_bytes().eq_ignore_ascii_case(&other[..len]) {
                return false;
            }

            other = &other[len..];
        }

        let path = self.path();

        if other.len() < path.len() || path.as_bytes() != &other[..path.len()] {
            if absolute && path == "/" {
                // PathAndQuery can be omitted, fall through
            } else {
                return false;
            }
        } else {
            other = &other[path.len()..];
        }

        if let Some(query) = self.query() {
            if other.is_empty() {
                return query.is_empty();
            }

            if other[0] != b'?' {
                return false;
            }

            other = &other[1..];

            if other.len() < query.len() {
                return false;
            }

            if query.as_bytes() != &other[..query.len()] {
                return false;
            }

            other = &other[query.len()..];
        }

        other.is_empty() || other[0] == b'#'
    }
}

impl PartialEq<Uri> for str {
    fn eq(&self, uri: &Uri) -> bool {
        uri == self
    }
}

impl<'a> PartialEq<&'a str> for Uri {
    fn eq(&self, other: &&'a str) -> bool {
        self == *other
    }
}

impl<'a> PartialEq<Uri> for &'a str {
    fn eq(&self, uri: &Uri) -> bool {
        uri == *self
    }
}

impl Eq for Uri {}

/// Returns a `Uri` representing `/`
impl Default for Uri {
    #[inline]
    fn default() -> Uri {
        Uri {
            scheme: Scheme::empty(),
            authority: Authority::empty(),
            path_and_query: PathAndQuery::slash(),
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = self.scheme() {
            write!(f, "{}://", scheme)?;
        }

        if let Some(authority) = self.authority() {
            write!(f, "{}", authority)?;
        }

        write!(f, "{}", self.path())?;

        if let Some(query) = self.query() {
            write!(f, "?{}", query)?;
        }

        Ok(())
    }
}

impl fmt::Debug for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl From<ErrorKind> for InvalidUri {
    fn from(src: ErrorKind) -> InvalidUri {
        InvalidUri(src)
    }
}

impl From<ErrorKind> for InvalidUriParts {
    fn from(src: ErrorKind) -> InvalidUriParts {
        InvalidUriParts(src.into())
    }
}

impl InvalidUri {
    fn s(&self) -> &str {
        match self.0 {
            ErrorKind::InvalidUriChar => "invalid uri character",
            ErrorKind::InvalidScheme => "invalid scheme",
            ErrorKind::InvalidAuthority => "invalid authority",
            ErrorKind::InvalidPort => "invalid port",
            ErrorKind::InvalidFormat => "invalid format",
            ErrorKind::SchemeMissing => "scheme missing",
            ErrorKind::AuthorityMissing => "authority missing",
            ErrorKind::PathAndQueryMissing => "path missing",
            ErrorKind::TooLong => "uri too long",
            ErrorKind::Empty => "empty string",
            ErrorKind::SchemeTooLong => "scheme too long",
        }
    }
}

impl fmt::Display for InvalidUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.s().fmt(f)
    }
}

impl Error for InvalidUri {}

impl fmt::Display for InvalidUriParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for InvalidUriParts {}

impl Hash for Uri {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        if !self.scheme.inner.is_none() {
            self.scheme.hash(state);
            state.write_u8(0xff);
        }

        if let Some(auth) = self.authority() {
            auth.hash(state);
        }

        Hash::hash_slice(self.path().as_bytes(), state);

        if let Some(query) = self.query() {
            b'?'.hash(state);
            Hash::hash_slice(query.as_bytes(), state);
        }
    }
}
