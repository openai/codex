//! The HTTP request method
//!
//! This module contains HTTP-method related structs and errors and such. The
//! main type of this module, `Method`, is also reexported at the root of the
//! crate as `http::Method` and is intended for import through that location
//! primarily.
//!
//! # Examples
//!
//! ```
//! use http::Method;
//!
//! assert_eq!(Method::GET, Method::from_bytes(b"GET").unwrap());
//! assert!(Method::GET.is_idempotent());
//! assert_eq!(Method::POST.as_str(), "POST");
//! ```

use self::extension::{AllocatedExtension, InlineExtension};
use self::Inner::*;

use std::convert::TryFrom;
use std::error::Error;
use std::str::FromStr;
use std::{fmt, str};

/// The Request Method (VERB)
///
/// This type also contains constants for a number of common HTTP methods such
/// as GET, POST, etc.
///
/// Currently includes 8 variants representing the 8 methods defined in
/// [RFC 7230](https://tools.ietf.org/html/rfc7231#section-4.1), plus PATCH,
/// and an Extension variant for all extensions.
///
/// # Examples
///
/// ```
/// use http::Method;
///
/// assert_eq!(Method::GET, Method::from_bytes(b"GET").unwrap());
/// assert!(Method::GET.is_idempotent());
/// assert_eq!(Method::POST.as_str(), "POST");
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Method(Inner);

/// A possible error value when converting `Method` from bytes.
pub struct InvalidMethod {
    _priv: (),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Inner {
    Options,
    Get,
    Post,
    Put,
    Delete,
    Head,
    Trace,
    Connect,
    Patch,
    // If the extension is short enough, store it inline
    ExtensionInline(InlineExtension),
    // Otherwise, allocate it
    ExtensionAllocated(AllocatedExtension),
}

impl Method {
    /// GET
    pub const GET: Method = Method(Get);

    /// POST
    pub const POST: Method = Method(Post);

    /// PUT
    pub const PUT: Method = Method(Put);

    /// DELETE
    pub const DELETE: Method = Method(Delete);

    /// HEAD
    pub const HEAD: Method = Method(Head);

    /// OPTIONS
    pub const OPTIONS: Method = Method(Options);

    /// CONNECT
    pub const CONNECT: Method = Method(Connect);

    /// PATCH
    pub const PATCH: Method = Method(Patch);

    /// TRACE
    pub const TRACE: Method = Method(Trace);

    /// Converts a slice of bytes to an HTTP method.
    pub fn from_bytes(src: &[u8]) -> Result<Method, InvalidMethod> {
        match src.len() {
            0 => Err(InvalidMethod::new()),
            3 => match src {
                b"GET" => Ok(Method(Get)),
                b"PUT" => Ok(Method(Put)),
                _ => Method::extension_inline(src),
            },
            4 => match src {
                b"POST" => Ok(Method(Post)),
                b"HEAD" => Ok(Method(Head)),
                _ => Method::extension_inline(src),
            },
            5 => match src {
                b"PATCH" => Ok(Method(Patch)),
                b"TRACE" => Ok(Method(Trace)),
                _ => Method::extension_inline(src),
            },
            6 => match src {
                b"DELETE" => Ok(Method(Delete)),
                _ => Method::extension_inline(src),
            },
            7 => match src {
                b"OPTIONS" => Ok(Method(Options)),
                b"CONNECT" => Ok(Method(Connect)),
                _ => Method::extension_inline(src),
            },
            _ => {
                if src.len() <= InlineExtension::MAX {
                    Method::extension_inline(src)
                } else {
                    let allocated = AllocatedExtension::new(src)?;

                    Ok(Method(ExtensionAllocated(allocated)))
                }
            }
        }
    }

    fn extension_inline(src: &[u8]) -> Result<Method, InvalidMethod> {
        let inline = InlineExtension::new(src)?;

        Ok(Method(ExtensionInline(inline)))
    }

    /// Whether a method is considered "safe", meaning the request is
    /// essentially read-only.
    ///
    /// See [the spec](https://tools.ietf.org/html/rfc7231#section-4.2.1)
    /// for more words.
    pub fn is_safe(&self) -> bool {
        matches!(self.0, Get | Head | Options | Trace)
    }

    /// Whether a method is considered "idempotent", meaning the request has
    /// the same result if executed multiple times.
    ///
    /// See [the spec](https://tools.ietf.org/html/rfc7231#section-4.2.2) for
    /// more words.
    pub fn is_idempotent(&self) -> bool {
        match self.0 {
            Put | Delete => true,
            _ => self.is_safe(),
        }
    }

    /// Return a &str representation of the HTTP method
    #[inline]
    pub fn as_str(&self) -> &str {
        match self.0 {
            Options => "OPTIONS",
            Get => "GET",
            Post => "POST",
            Put => "PUT",
            Delete => "DELETE",
            Head => "HEAD",
            Trace => "TRACE",
            Connect => "CONNECT",
            Patch => "PATCH",
            ExtensionInline(ref inline) => inline.as_str(),
            ExtensionAllocated(ref allocated) => allocated.as_str(),
        }
    }
}

impl AsRef<str> for Method {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'a> PartialEq<&'a Method> for Method {
    #[inline]
    fn eq(&self, other: &&'a Method) -> bool {
        self == *other
    }
}

impl<'a> PartialEq<Method> for &'a Method {
    #[inline]
    fn eq(&self, other: &Method) -> bool {
        *self == other
    }
}

impl PartialEq<str> for Method {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_ref() == other
    }
}

impl PartialEq<Method> for str {
    #[inline]
    fn eq(&self, other: &Method) -> bool {
        self == other.as_ref()
    }
}

impl<'a> PartialEq<&'a str> for Method {
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        self.as_ref() == *other
    }
}

impl<'a> PartialEq<Method> for &'a str {
    #[inline]
    fn eq(&self, other: &Method) -> bool {
        *self == other.as_ref()
    }
}

impl fmt::Debug for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl fmt::Display for Method {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(self.as_ref())
    }
}

impl Default for Method {
    #[inline]
    fn default() -> Method {
        Method::GET
    }
}

impl<'a> From<&'a Method> for Method {
    #[inline]
    fn from(t: &'a Method) -> Self {
        t.clone()
    }
}

impl<'a> TryFrom<&'a [u8]> for Method {
    type Error = InvalidMethod;

    #[inline]
    fn try_from(t: &'a [u8]) -> Result<Self, Self::Error> {
        Method::from_bytes(t)
    }
}

impl<'a> TryFrom<&'a str> for Method {
    type Error = InvalidMethod;

    #[inline]
    fn try_from(t: &'a str) -> Result<Self, Self::Error> {
        TryFrom::try_from(t.as_bytes())
    }
}

impl FromStr for Method {
    type Err = InvalidMethod;

    #[inline]
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        TryFrom::try_from(t)
    }
}

impl InvalidMethod {
    fn new() -> InvalidMethod {
        InvalidMethod { _priv: () }
    }
}

impl fmt::Debug for InvalidMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidMethod")
            // skip _priv noise
            .finish()
    }
}

impl fmt::Display for InvalidMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid HTTP method")
    }
}

impl Error for InvalidMethod {}

mod extension {
    use super::InvalidMethod;
    use std::str;

    #[derive(Clone, PartialEq, Eq, Hash)]
    // Invariant: the first self.1 bytes of self.0 are valid UTF-8.
    pub struct InlineExtension([u8; InlineExtension::MAX], u8);

    #[derive(Clone, PartialEq, Eq, Hash)]
    // Invariant: self.0 contains valid UTF-8.
    pub struct AllocatedExtension(Box<[u8]>);

    impl InlineExtension {
        // Method::from_bytes() assumes this is at least 7
        pub const MAX: usize = 15;

        pub fn new(src: &[u8]) -> Result<InlineExtension, InvalidMethod> {
            let mut data: [u8; InlineExtension::MAX] = Default::default();

            write_checked(src, &mut data)?;

            // Invariant: write_checked ensures that the first src.len() bytes
            // of data are valid UTF-8.
            Ok(InlineExtension(data, src.len() as u8))
        }

        pub fn as_str(&self) -> &str {
            let InlineExtension(ref data, len) = self;
            // Safety: the invariant of InlineExtension ensures that the first
            // len bytes of data contain valid UTF-8.
            unsafe { str::from_utf8_unchecked(&data[..*len as usize]) }
        }
    }

    impl AllocatedExtension {
        pub fn new(src: &[u8]) -> Result<AllocatedExtension, InvalidMethod> {
            let mut data: Vec<u8> = vec![0; src.len()];

            write_checked(src, &mut data)?;

            // Invariant: data is exactly src.len() long and write_checked
            // ensures that the first src.len() bytes of data are valid UTF-8.
            Ok(AllocatedExtension(data.into_boxed_slice()))
        }

        pub fn as_str(&self) -> &str {
            // Safety: the invariant of AllocatedExtension ensures that self.0
            // contains valid UTF-8.
            unsafe { str::from_utf8_unchecked(&self.0) }
        }
    }

    // From the RFC 9110 HTTP Semantics, section 9.1, the HTTP method is case-sensitive and can
    // contain the following characters:
    //
    // ```
    // method = token
    // token = 1*tchar
    // tchar = "!" / "#" / "$" / "%" / "&" / "'" / "*" / "+" / "-" / "." /
    //     "^" / "_" / "`" / "|" / "~" / DIGIT / ALPHA
    // ```
    //
    // https://datatracker.ietf.org/doc/html/rfc9110#section-9.1
    //
    // Note that this definition means that any &[u8] that consists solely of valid
    // characters is also valid UTF-8 because the valid method characters are a
    // subset of the valid 1 byte UTF-8 encoding.
    #[rustfmt::skip]
    const METHOD_CHARS: [u8; 256] = [
        //  0      1      2      3      4      5      6      7      8      9
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', //   x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', //  1x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', //  2x
        b'\0', b'\0', b'\0',  b'!', b'\0',  b'#',  b'$',  b'%',  b'&', b'\'', //  3x
        b'\0', b'\0',  b'*',  b'+', b'\0',  b'-',  b'.', b'\0',  b'0',  b'1', //  4x
         b'2',  b'3',  b'4',  b'5',  b'6',  b'7',  b'8',  b'9', b'\0', b'\0', //  5x
        b'\0', b'\0', b'\0', b'\0', b'\0',  b'A',  b'B',  b'C',  b'D',  b'E', //  6x
         b'F',  b'G',  b'H',  b'I',  b'J',  b'K',  b'L',  b'M',  b'N',  b'O', //  7x
         b'P',  b'Q',  b'R',  b'S',  b'T',  b'U',  b'V',  b'W',  b'X',  b'Y', //  8x
         b'Z', b'\0', b'\0', b'\0',  b'^',  b'_',  b'`',  b'a',  b'b',  b'c', //  9x
         b'd',  b'e',  b'f',  b'g',  b'h',  b'i',  b'j',  b'k',  b'l',  b'm', // 10x
         b'n',  b'o',  b'p',  b'q',  b'r',  b's',  b't',  b'u',  b'v',  b'w', // 11x
         b'x',  b'y',  b'z', b'\0',  b'|', b'\0',  b'~', b'\0', b'\0', b'\0', // 12x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 13x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 14x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 15x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 16x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 17x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 18x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 19x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 20x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 21x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 22x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 23x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', b'\0', // 24x
        b'\0', b'\0', b'\0', b'\0', b'\0', b'\0'                              // 25x
    ];

    // write_checked ensures (among other things) that the first src.len() bytes
    // of dst are valid UTF-8
    fn write_checked(src: &[u8], dst: &mut [u8]) -> Result<(), InvalidMethod> {
        for (i, &b) in src.iter().enumerate() {
            let b = METHOD_CHARS[b as usize];

            if b == 0 {
                return Err(InvalidMethod::new());
            }

            dst[i] = b;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_method_eq() {
        assert_eq!(Method::GET, Method::GET);
        assert_eq!(Method::GET, "GET");
        assert_eq!(&Method::GET, "GET");

        assert_eq!("GET", Method::GET);
        assert_eq!("GET", &Method::GET);

        assert_eq!(&Method::GET, Method::GET);
        assert_eq!(Method::GET, &Method::GET);
    }

    #[test]
    fn test_invalid_method() {
        assert!(Method::from_str("").is_err());
        assert!(Method::from_bytes(b"").is_err());
        assert!(Method::from_bytes(&[0xC0]).is_err()); // invalid utf-8
        assert!(Method::from_bytes(&[0x10]).is_err()); // invalid method characters
    }

    #[test]
    fn test_is_idempotent() {
        assert!(Method::OPTIONS.is_idempotent());
        assert!(Method::GET.is_idempotent());
        assert!(Method::PUT.is_idempotent());
        assert!(Method::DELETE.is_idempotent());
        assert!(Method::HEAD.is_idempotent());
        assert!(Method::TRACE.is_idempotent());

        assert!(!Method::POST.is_idempotent());
        assert!(!Method::CONNECT.is_idempotent());
        assert!(!Method::PATCH.is_idempotent());
    }

    #[test]
    fn test_extension_method() {
        assert_eq!(Method::from_str("WOW").unwrap(), "WOW");
        assert_eq!(Method::from_str("wOw!!").unwrap(), "wOw!!");

        let long_method = "This_is_a_very_long_method.It_is_valid_but_unlikely.";
        assert_eq!(Method::from_str(long_method).unwrap(), long_method);

        let longest_inline_method = [b'A'; InlineExtension::MAX];
        assert_eq!(
            Method::from_bytes(&longest_inline_method).unwrap(),
            Method(ExtensionInline(
                InlineExtension::new(&longest_inline_method).unwrap()
            ))
        );
        let shortest_allocated_method = [b'A'; InlineExtension::MAX + 1];
        assert_eq!(
            Method::from_bytes(&shortest_allocated_method).unwrap(),
            Method(ExtensionAllocated(
                AllocatedExtension::new(&shortest_allocated_method).unwrap()
            ))
        );
    }

    #[test]
    fn test_extension_method_chars() {
        const VALID_METHOD_CHARS: &str =
            "!#$%&'*+-.^_`|~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

        for c in VALID_METHOD_CHARS.chars() {
            let c = c.to_string();

            assert_eq!(
                Method::from_str(&c).unwrap(),
                c.as_str(),
                "testing {c} is a valid method character"
            );
        }
    }
}
