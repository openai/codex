use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use bytes::Bytes;

use super::{ErrorKind, InvalidUri};
use crate::byte_str::ByteStr;

/// Represents the scheme component of a URI
#[derive(Clone)]
pub struct Scheme {
    pub(super) inner: Scheme2,
}

#[derive(Clone, Debug)]
pub(super) enum Scheme2<T = Box<ByteStr>> {
    None,
    Standard(Protocol),
    Other(T),
}

#[derive(Copy, Clone, Debug)]
pub(super) enum Protocol {
    Http,
    Https,
}

impl Scheme {
    /// HTTP protocol scheme
    pub const HTTP: Scheme = Scheme {
        inner: Scheme2::Standard(Protocol::Http),
    };

    /// HTTP protocol over TLS.
    pub const HTTPS: Scheme = Scheme {
        inner: Scheme2::Standard(Protocol::Https),
    };

    pub(super) fn empty() -> Self {
        Scheme {
            inner: Scheme2::None,
        }
    }

    /// Return a str representation of the scheme
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::*;
    /// let scheme: Scheme = "http".parse().unwrap();
    /// assert_eq!(scheme.as_str(), "http");
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        use self::Protocol::*;
        use self::Scheme2::*;

        match self.inner {
            Standard(Http) => "http",
            Standard(Https) => "https",
            Other(ref v) => &v[..],
            None => unreachable!(),
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for Scheme {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &'a [u8]) -> Result<Self, Self::Error> {
        use self::Scheme2::*;

        match Scheme2::parse_exact(s)? {
            None => Err(ErrorKind::InvalidScheme.into()),
            Standard(p) => Ok(Standard(p).into()),
            Other(_) => {
                let bytes = Bytes::copy_from_slice(s);

                // Safety: postcondition on parse_exact() means that s and
                // hence bytes are valid UTF-8.
                let string = unsafe { ByteStr::from_utf8_unchecked(bytes) };

                Ok(Other(Box::new(string)).into())
            }
        }
    }
}

impl<'a> TryFrom<&'a str> for Scheme {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}

impl FromStr for Scheme {
    type Err = InvalidUri;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TryFrom::try_from(s)
    }
}

impl fmt::Debug for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for Scheme {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for Scheme {
    fn eq(&self, other: &Scheme) -> bool {
        use self::Protocol::*;
        use self::Scheme2::*;

        match (&self.inner, &other.inner) {
            (&Standard(Http), &Standard(Http)) => true,
            (&Standard(Https), &Standard(Https)) => true,
            (Other(a), Other(b)) => a.eq_ignore_ascii_case(b),
            (&None, _) | (_, &None) => unreachable!(),
            _ => false,
        }
    }
}

impl Eq for Scheme {}

/// Case-insensitive equality
///
/// # Examples
///
/// ```
/// # use http::uri::Scheme;
/// let scheme: Scheme = "HTTP".parse().unwrap();
/// assert_eq!(scheme, *"http");
/// ```
impl PartialEq<str> for Scheme {
    fn eq(&self, other: &str) -> bool {
        self.as_str().eq_ignore_ascii_case(other)
    }
}

/// Case-insensitive equality
impl PartialEq<Scheme> for str {
    fn eq(&self, other: &Scheme) -> bool {
        other == self
    }
}

/// Case-insensitive hashing
impl Hash for Scheme {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        match self.inner {
            Scheme2::None => (),
            Scheme2::Standard(Protocol::Http) => state.write_u8(1),
            Scheme2::Standard(Protocol::Https) => state.write_u8(2),
            Scheme2::Other(ref other) => {
                other.len().hash(state);
                for &b in other.as_bytes() {
                    state.write_u8(b.to_ascii_lowercase());
                }
            }
        }
    }
}

impl<T> Scheme2<T> {
    pub(super) fn is_none(&self) -> bool {
        matches!(*self, Scheme2::None)
    }
}

// Require the scheme to not be too long in order to enable further
// optimizations later.
const MAX_SCHEME_LEN: usize = 64;

// scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
//
// SCHEME_CHARS is a table of valid characters in the scheme part of a URI.  An
// entry in the table is 0 for invalid characters. For valid characters the
// entry is itself (i.e.  the entry for 43 is b'+' because b'+' == 43u8). An
// important characteristic of this table is that all entries above 127 are
// invalid. This makes all of the valid entries a valid single-byte UTF-8 code
// point. This means that a slice of such valid entries is valid UTF-8.
#[rustfmt::skip]
const SCHEME_CHARS: [u8; 256] = [
    //  0      1      2      3      4      5      6      7      8      9
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //   x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  1x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  2x
        0,     0,     0,     0,     0,     0,     0,     0,     0,     0, //  3x
        0,     0,     0,  b'+',     0,  b'-',  b'.',     0,  b'0',  b'1', //  4x
     b'2',  b'3',  b'4',  b'5',  b'6',  b'7',  b'8',  b'9',  b':',     0, //  5x
        0,     0,     0,     0,     0,  b'A',  b'B',  b'C',  b'D',  b'E', //  6x
     b'F',  b'G',  b'H',  b'I',  b'J',  b'K',  b'L',  b'M',  b'N',  b'O', //  7x
     b'P',  b'Q',  b'R',  b'S',  b'T',  b'U',  b'V',  b'W',  b'X',  b'Y', //  8x
     b'Z',     0,     0,     0,     0,     0,     0,  b'a',  b'b',  b'c', //  9x
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

impl Scheme2<usize> {
    // Postcondition: On all Ok() returns, s is valid UTF-8
    fn parse_exact(s: &[u8]) -> Result<Scheme2<()>, InvalidUri> {
        match s {
            b"http" => Ok(Protocol::Http.into()),
            b"https" => Ok(Protocol::Https.into()),
            _ => {
                if s.len() > MAX_SCHEME_LEN {
                    return Err(ErrorKind::SchemeTooLong.into());
                }

                // check that each byte in s is a SCHEME_CHARS which implies
                // that it is a valid single byte UTF-8 code point.
                for &b in s {
                    match SCHEME_CHARS[b as usize] {
                        b':' => {
                            // Don't want :// here
                            return Err(ErrorKind::InvalidScheme.into());
                        }
                        0 => {
                            return Err(ErrorKind::InvalidScheme.into());
                        }
                        _ => {}
                    }
                }

                Ok(Scheme2::Other(()))
            }
        }
    }

    pub(super) fn parse(s: &[u8]) -> Result<Scheme2<usize>, InvalidUri> {
        if s.len() >= 7 {
            // Check for HTTP
            if s[..7].eq_ignore_ascii_case(b"http://") {
                // Prefix will be striped
                return Ok(Protocol::Http.into());
            }
        }

        if s.len() >= 8 {
            // Check for HTTPs
            if s[..8].eq_ignore_ascii_case(b"https://") {
                return Ok(Protocol::Https.into());
            }
        }

        if s.len() > 3 {
            for i in 0..s.len() {
                let b = s[i];

                match SCHEME_CHARS[b as usize] {
                    b':' => {
                        // Not enough data remaining
                        if s.len() < i + 3 {
                            break;
                        }

                        // Not a scheme
                        if &s[i + 1..i + 3] != b"//" {
                            break;
                        }

                        if i > MAX_SCHEME_LEN {
                            return Err(ErrorKind::SchemeTooLong.into());
                        }

                        // Return scheme
                        return Ok(Scheme2::Other(i));
                    }
                    // Invalid scheme character, abort
                    0 => break,
                    _ => {}
                }
            }
        }

        Ok(Scheme2::None)
    }
}

impl Protocol {
    pub(super) fn len(&self) -> usize {
        match *self {
            Protocol::Http => 4,
            Protocol::Https => 5,
        }
    }
}

impl<T> From<Protocol> for Scheme2<T> {
    fn from(src: Protocol) -> Self {
        Scheme2::Standard(src)
    }
}

#[doc(hidden)]
impl From<Scheme2> for Scheme {
    fn from(src: Scheme2) -> Self {
        Scheme { inner: src }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn scheme_eq_to_str() {
        assert_eq!(&scheme("http"), "http");
        assert_eq!(&scheme("https"), "https");
        assert_eq!(&scheme("ftp"), "ftp");
        assert_eq!(&scheme("my+funky+scheme"), "my+funky+scheme");
    }

    #[test]
    fn invalid_scheme_is_error() {
        Scheme::try_from("my_funky_scheme").expect_err("Unexpectedly valid Scheme");

        // Invalid UTF-8
        Scheme::try_from([0xC0].as_ref()).expect_err("Unexpectedly valid Scheme");
    }

    fn scheme(s: &str) -> Scheme {
        s.parse().expect(&format!("Invalid scheme: {}", s))
    }
}
