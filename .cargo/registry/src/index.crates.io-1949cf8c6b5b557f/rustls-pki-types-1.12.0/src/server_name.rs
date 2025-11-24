//! DNS name validation according to RFC1035, but with underscores allowed.

#[cfg(all(feature = "alloc", feature = "std"))]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::string::{String, ToString};
use core::hash::{Hash, Hasher};
use core::{fmt, mem, str};
#[cfg(feature = "std")]
use std::error::Error as StdError;

/// Encodes ways a client can know the expected name of the server.
///
/// This currently covers knowing the DNS name of the server, but
/// will be extended in the future to supporting privacy-preserving names
/// for the server ("ECH").  For this reason this enum is `non_exhaustive`.
///
/// # Making one
///
/// If you have a DNS name as a `&str`, this type implements `TryFrom<&str>`,
/// so you can do:
///
/// ```
/// # use rustls_pki_types::ServerName;
/// ServerName::try_from("example.com").expect("invalid DNS name");
/// ```
///
/// If you have an owned `String`, you can use `TryFrom` directly:
///
/// ```
/// # use rustls_pki_types::ServerName;
/// let name = "example.com".to_string();
/// #[cfg(feature = "alloc")]
/// ServerName::try_from(name).expect("invalid DNS name");
/// ```
///
/// which will yield a `ServerName<'static>` if successful.
///
/// or, alternatively...
///
/// ```
/// # use rustls_pki_types::ServerName;
/// let x: ServerName = "example.com".try_into().expect("invalid DNS name");
/// ```
#[non_exhaustive]
#[derive(Clone, Eq, Hash, PartialEq)]
pub enum ServerName<'a> {
    /// The server is identified by a DNS name.  The name
    /// is sent in the TLS Server Name Indication (SNI)
    /// extension.
    DnsName(DnsName<'a>),

    /// The server is identified by an IP address. SNI is not
    /// done.
    IpAddress(IpAddr),
}

impl ServerName<'_> {
    /// Produce an owned `ServerName` from this (potentially borrowed) `ServerName`.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> ServerName<'static> {
        match self {
            Self::DnsName(d) => ServerName::DnsName(d.to_owned()),
            Self::IpAddress(i) => ServerName::IpAddress(*i),
        }
    }

    /// Return the string representation of this `ServerName`.
    ///
    /// In the case of a `ServerName::DnsName` instance, this function returns a borrowed `str`.
    /// For a `ServerName::IpAddress` instance it returns an allocated `String`.
    #[cfg(feature = "std")]
    pub fn to_str(&self) -> Cow<'_, str> {
        match self {
            Self::DnsName(d) => d.as_ref().into(),
            Self::IpAddress(i) => std::net::IpAddr::from(*i).to_string().into(),
        }
    }
}

impl fmt::Debug for ServerName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DnsName(d) => f.debug_tuple("DnsName").field(&d.as_ref()).finish(),
            Self::IpAddress(i) => f.debug_tuple("IpAddress").field(i).finish(),
        }
    }
}

#[cfg(feature = "alloc")]
impl TryFrom<String> for ServerName<'static> {
    type Error = InvalidDnsNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match DnsName::try_from_string(value) {
            Ok(dns) => Ok(Self::DnsName(dns)),
            Err(value) => match IpAddr::try_from(value.as_str()) {
                Ok(ip) => Ok(Self::IpAddress(ip)),
                Err(_) => Err(InvalidDnsNameError),
            },
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for ServerName<'a> {
    type Error = InvalidDnsNameError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        match str::from_utf8(value) {
            Ok(s) => Self::try_from(s),
            Err(_) => Err(InvalidDnsNameError),
        }
    }
}

/// Attempt to make a ServerName from a string by parsing as a DNS name or IP address.
impl<'a> TryFrom<&'a str> for ServerName<'a> {
    type Error = InvalidDnsNameError;
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        match DnsName::try_from(s) {
            Ok(dns) => Ok(Self::DnsName(dns)),
            Err(InvalidDnsNameError) => match IpAddr::try_from(s) {
                Ok(ip) => Ok(Self::IpAddress(ip)),
                Err(_) => Err(InvalidDnsNameError),
            },
        }
    }
}

impl From<IpAddr> for ServerName<'_> {
    fn from(addr: IpAddr) -> Self {
        Self::IpAddress(addr)
    }
}

#[cfg(feature = "std")]
impl From<std::net::IpAddr> for ServerName<'_> {
    fn from(addr: std::net::IpAddr) -> Self {
        Self::IpAddress(addr.into())
    }
}

impl From<Ipv4Addr> for ServerName<'_> {
    fn from(v4: Ipv4Addr) -> Self {
        Self::IpAddress(IpAddr::V4(v4))
    }
}

impl From<Ipv6Addr> for ServerName<'_> {
    fn from(v6: Ipv6Addr) -> Self {
        Self::IpAddress(IpAddr::V6(v6))
    }
}

#[cfg(feature = "std")]
impl From<std::net::Ipv4Addr> for ServerName<'_> {
    fn from(v4: std::net::Ipv4Addr) -> Self {
        Self::IpAddress(IpAddr::V4(v4.into()))
    }
}

#[cfg(feature = "std")]
impl From<std::net::Ipv6Addr> for ServerName<'_> {
    fn from(v6: std::net::Ipv6Addr) -> Self {
        Self::IpAddress(IpAddr::V6(v6.into()))
    }
}

/// A type which encapsulates a string (borrowed or owned) that is a syntactically valid DNS name.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DnsName<'a>(DnsNameInner<'a>);

impl<'a> DnsName<'a> {
    /// Produce a borrowed `DnsName` from this owned `DnsName`.
    pub fn borrow(&'a self) -> Self {
        Self(match self {
            Self(DnsNameInner::Borrowed(s)) => DnsNameInner::Borrowed(s),
            #[cfg(feature = "alloc")]
            Self(DnsNameInner::Owned(s)) => DnsNameInner::Borrowed(s.as_str()),
        })
    }

    /// Copy this object to produce an owned `DnsName`, smashing the case to lowercase
    /// in one operation.
    #[cfg(feature = "alloc")]
    pub fn to_lowercase_owned(&self) -> DnsName<'static> {
        DnsName(DnsNameInner::Owned(self.as_ref().to_ascii_lowercase()))
    }

    /// Produce an owned `DnsName` from this (potentially borrowed) `DnsName`.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> DnsName<'static> {
        DnsName(DnsNameInner::Owned(match self {
            Self(DnsNameInner::Borrowed(s)) => s.to_string(),
            #[cfg(feature = "alloc")]
            Self(DnsNameInner::Owned(s)) => s.clone(),
        }))
    }

    #[cfg(feature = "alloc")]
    fn try_from_string(s: String) -> Result<Self, String> {
        match validate(s.as_bytes()) {
            Ok(_) => Ok(Self(DnsNameInner::Owned(s))),
            Err(_) => Err(s),
        }
    }

    /// Produces a borrowed [`DnsName`] from a borrowed [`str`].
    pub const fn try_from_str(s: &str) -> Result<DnsName<'_>, InvalidDnsNameError> {
        match validate(s.as_bytes()) {
            Ok(_) => Ok(DnsName(DnsNameInner::Borrowed(s))),
            Err(err) => Err(err),
        }
    }
}

#[cfg(feature = "alloc")]
impl TryFrom<String> for DnsName<'static> {
    type Error = InvalidDnsNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from_string(value).map_err(|_| InvalidDnsNameError)
    }
}

impl<'a> TryFrom<&'a str> for DnsName<'a> {
    type Error = InvalidDnsNameError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        DnsName::try_from_str(value)
    }
}

impl<'a> TryFrom<&'a [u8]> for DnsName<'a> {
    type Error = InvalidDnsNameError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        validate(value)?;
        Ok(Self(DnsNameInner::Borrowed(str::from_utf8(value).unwrap())))
    }
}

impl AsRef<str> for DnsName<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self(DnsNameInner::Borrowed(s)) => s,
            #[cfg(feature = "alloc")]
            Self(DnsNameInner::Owned(s)) => s.as_str(),
        }
    }
}

#[derive(Clone, Eq)]
enum DnsNameInner<'a> {
    Borrowed(&'a str),
    #[cfg(feature = "alloc")]
    Owned(String),
}

impl PartialEq<Self> for DnsNameInner<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Borrowed(s), Self::Borrowed(o)) => s.eq_ignore_ascii_case(o),
            #[cfg(feature = "alloc")]
            (Self::Borrowed(s), Self::Owned(o)) => s.eq_ignore_ascii_case(o.as_str()),
            #[cfg(feature = "alloc")]
            (Self::Owned(s), Self::Borrowed(o)) => s.eq_ignore_ascii_case(o),
            #[cfg(feature = "alloc")]
            (Self::Owned(s), Self::Owned(o)) => s.eq_ignore_ascii_case(o.as_str()),
        }
    }
}

impl Hash for DnsNameInner<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let s = match self {
            Self::Borrowed(s) => s,
            #[cfg(feature = "alloc")]
            Self::Owned(s) => s.as_str(),
        };

        s.chars().for_each(|c| c.to_ascii_lowercase().hash(state));
    }
}

impl fmt::Debug for DnsNameInner<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Borrowed(s) => f.write_fmt(format_args!("{:?}", s)),
            #[cfg(feature = "alloc")]
            Self::Owned(s) => f.write_fmt(format_args!("{:?}", s)),
        }
    }
}

/// The provided input could not be parsed because
/// it is not a syntactically-valid DNS Name.
#[derive(Debug)]
pub struct InvalidDnsNameError;

impl fmt::Display for InvalidDnsNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("invalid dns name")
    }
}

#[cfg(feature = "std")]
impl StdError for InvalidDnsNameError {}

const fn validate(input: &[u8]) -> Result<(), InvalidDnsNameError> {
    enum State {
        Start,
        Next,
        NumericOnly { len: usize },
        NextAfterNumericOnly,
        Subsequent { len: usize },
        Hyphen { len: usize },
    }

    use State::*;
    let mut state = Start;

    /// "Labels must be 63 characters or less."
    const MAX_LABEL_LENGTH: usize = 63;

    /// https://devblogs.microsoft.com/oldnewthing/20120412-00/?p=7873
    const MAX_NAME_LENGTH: usize = 253;

    if input.len() > MAX_NAME_LENGTH {
        return Err(InvalidDnsNameError);
    }

    let mut idx = 0;
    while idx < input.len() {
        let ch = input[idx];
        state = match (state, ch) {
            (Start | Next | NextAfterNumericOnly | Hyphen { .. }, b'.') => {
                return Err(InvalidDnsNameError);
            }
            (Subsequent { .. }, b'.') => Next,
            (NumericOnly { .. }, b'.') => NextAfterNumericOnly,
            (Subsequent { len } | NumericOnly { len } | Hyphen { len }, _)
                if len >= MAX_LABEL_LENGTH =>
            {
                return Err(InvalidDnsNameError);
            }
            (Start | Next | NextAfterNumericOnly, b'0'..=b'9') => NumericOnly { len: 1 },
            (NumericOnly { len }, b'0'..=b'9') => NumericOnly { len: len + 1 },
            (Start | Next | NextAfterNumericOnly, b'a'..=b'z' | b'A'..=b'Z' | b'_') => {
                Subsequent { len: 1 }
            }
            (Subsequent { len } | NumericOnly { len } | Hyphen { len }, b'-') => {
                Hyphen { len: len + 1 }
            }
            (
                Subsequent { len } | NumericOnly { len } | Hyphen { len },
                b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9',
            ) => Subsequent { len: len + 1 },
            _ => return Err(InvalidDnsNameError),
        };
        idx += 1;
    }

    if matches!(
        state,
        Start | Hyphen { .. } | NumericOnly { .. } | NextAfterNumericOnly
    ) {
        return Err(InvalidDnsNameError);
    }

    Ok(())
}

/// `no_std` implementation of `std::net::IpAddr`.
///
/// Note: because we intend to replace this type with `core::net::IpAddr` as soon as it is
/// stabilized, the identity of this type should not be considered semver-stable. However, the
/// attached interfaces are stable; they form a subset of those provided by `core::net::IpAddr`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IpAddr {
    /// An Ipv4 address.
    V4(Ipv4Addr),
    /// An Ipv6 address.
    V6(Ipv6Addr),
}

impl TryFrom<&str> for IpAddr {
    type Error = AddrParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match Ipv4Addr::try_from(value) {
            Ok(v4) => Ok(Self::V4(v4)),
            Err(_) => match Ipv6Addr::try_from(value) {
                Ok(v6) => Ok(Self::V6(v6)),
                Err(e) => Err(e),
            },
        }
    }
}

#[cfg(feature = "std")]
impl From<std::net::IpAddr> for IpAddr {
    fn from(addr: std::net::IpAddr) -> Self {
        match addr {
            std::net::IpAddr::V4(v4) => Self::V4(v4.into()),
            std::net::IpAddr::V6(v6) => Self::V6(v6.into()),
        }
    }
}

#[cfg(feature = "std")]
impl From<IpAddr> for std::net::IpAddr {
    fn from(value: IpAddr) -> Self {
        match value {
            IpAddr::V4(v4) => Self::from(std::net::Ipv4Addr::from(v4)),
            IpAddr::V6(v6) => Self::from(std::net::Ipv6Addr::from(v6)),
        }
    }
}

#[cfg(feature = "std")]
impl From<std::net::Ipv4Addr> for IpAddr {
    fn from(v4: std::net::Ipv4Addr) -> Self {
        Self::V4(v4.into())
    }
}

#[cfg(feature = "std")]
impl From<std::net::Ipv6Addr> for IpAddr {
    fn from(v6: std::net::Ipv6Addr) -> Self {
        Self::V6(v6.into())
    }
}

/// `no_std` implementation of `std::net::Ipv4Addr`.
///
/// Note: because we intend to replace this type with `core::net::Ipv4Addr` as soon as it is
/// stabilized, the identity of this type should not be considered semver-stable. However, the
/// attached interfaces are stable; they form a subset of those provided by `core::net::Ipv4Addr`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ipv4Addr([u8; 4]);

impl TryFrom<&str> for Ipv4Addr {
    type Error = AddrParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // don't try to parse if too long
        if value.len() > 15 {
            Err(AddrParseError(AddrKind::Ipv4))
        } else {
            Parser::new(value.as_bytes()).parse_with(|p| p.read_ipv4_addr(), AddrKind::Ipv4)
        }
    }
}

#[cfg(feature = "std")]
impl From<std::net::Ipv4Addr> for Ipv4Addr {
    fn from(addr: std::net::Ipv4Addr) -> Self {
        Self(addr.octets())
    }
}

#[cfg(feature = "std")]
impl From<Ipv4Addr> for std::net::Ipv4Addr {
    fn from(value: Ipv4Addr) -> Self {
        Self::from(value.0)
    }
}

impl AsRef<[u8; 4]> for Ipv4Addr {
    fn as_ref(&self) -> &[u8; 4] {
        &self.0
    }
}

/// `no_std` implementation of `std::net::Ipv6Addr`.
///
/// Note: because we intend to replace this type with `core::net::Ipv6Addr` as soon as it is
/// stabilized, the identity of this type should not be considered semver-stable. However, the
/// attached interfaces are stable; they form a subset of those provided by `core::net::Ipv6Addr`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ipv6Addr([u8; 16]);

impl TryFrom<&str> for Ipv6Addr {
    type Error = AddrParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Parser::new(value.as_bytes()).parse_with(|p| p.read_ipv6_addr(), AddrKind::Ipv6)
    }
}

impl From<[u16; 8]> for Ipv6Addr {
    fn from(value: [u16; 8]) -> Self {
        // Adapted from `std::net::Ipv6Addr::new()`
        let addr16 = [
            value[0].to_be(),
            value[1].to_be(),
            value[2].to_be(),
            value[3].to_be(),
            value[4].to_be(),
            value[5].to_be(),
            value[6].to_be(),
            value[7].to_be(),
        ];
        Self(
            // All elements in `addr16` are big endian.
            // SAFETY: `[u16; 8]` is always safe to transmute to `[u8; 16]`.
            unsafe { mem::transmute::<[u16; 8], [u8; 16]>(addr16) },
        )
    }
}

#[cfg(feature = "std")]
impl From<std::net::Ipv6Addr> for Ipv6Addr {
    fn from(addr: std::net::Ipv6Addr) -> Self {
        Self(addr.octets())
    }
}

#[cfg(feature = "std")]
impl From<Ipv6Addr> for std::net::Ipv6Addr {
    fn from(value: Ipv6Addr) -> Self {
        Self::from(value.0)
    }
}

impl AsRef<[u8; 16]> for Ipv6Addr {
    fn as_ref(&self) -> &[u8; 16] {
        &self.0
    }
}

// Adapted from core, 2023-11-23
//
// https://github.com/rust-lang/rust/blob/fc13ca6d70f7381513c22443fc5aaee1d151ea45/library/core/src/net/parser.rs#L34
mod parser {
    use super::{AddrParseError, Ipv4Addr, Ipv6Addr};

    pub(super) struct Parser<'a> {
        // Parsing as ASCII, so can use byte array.
        state: &'a [u8],
    }

    impl<'a> Parser<'a> {
        pub(super) fn new(input: &'a [u8]) -> Self {
            Parser { state: input }
        }

        /// Run a parser, and restore the pre-parse state if it fails.
        fn read_atomically<T, F>(&mut self, inner: F) -> Option<T>
        where
            F: FnOnce(&mut Parser<'_>) -> Option<T>,
        {
            let state = self.state;
            let result = inner(self);
            if result.is_none() {
                self.state = state;
            }
            result
        }

        /// Run a parser, but fail if the entire input wasn't consumed.
        /// Doesn't run atomically.
        pub(super) fn parse_with<T, F>(
            &mut self,
            inner: F,
            kind: AddrKind,
        ) -> Result<T, AddrParseError>
        where
            F: FnOnce(&mut Parser<'_>) -> Option<T>,
        {
            let result = inner(self);
            if self.state.is_empty() { result } else { None }.ok_or(AddrParseError(kind))
        }

        /// Peek the next character from the input
        fn peek_char(&self) -> Option<char> {
            self.state.first().map(|&b| char::from(b))
        }

        /// Read the next character from the input
        fn read_char(&mut self) -> Option<char> {
            self.state.split_first().map(|(&b, tail)| {
                self.state = tail;
                char::from(b)
            })
        }

        #[must_use]
        /// Read the next character from the input if it matches the target.
        fn read_given_char(&mut self, target: char) -> Option<()> {
            self.read_atomically(|p| {
                p.read_char()
                    .and_then(|c| if c == target { Some(()) } else { None })
            })
        }

        /// Helper for reading separators in an indexed loop. Reads the separator
        /// character iff index > 0, then runs the parser. When used in a loop,
        /// the separator character will only be read on index > 0 (see
        /// read_ipv4_addr for an example)
        fn read_separator<T, F>(&mut self, sep: char, index: usize, inner: F) -> Option<T>
        where
            F: FnOnce(&mut Parser<'_>) -> Option<T>,
        {
            self.read_atomically(move |p| {
                if index > 0 {
                    p.read_given_char(sep)?;
                }
                inner(p)
            })
        }

        // Read a number off the front of the input in the given radix, stopping
        // at the first non-digit character or eof. Fails if the number has more
        // digits than max_digits or if there is no number.
        fn read_number<T: ReadNumberHelper>(
            &mut self,
            radix: u32,
            max_digits: Option<usize>,
            allow_zero_prefix: bool,
        ) -> Option<T> {
            self.read_atomically(move |p| {
                let mut result = T::ZERO;
                let mut digit_count = 0;
                let has_leading_zero = p.peek_char() == Some('0');

                while let Some(digit) = p.read_atomically(|p| p.read_char()?.to_digit(radix)) {
                    result = result.checked_mul(radix)?;
                    result = result.checked_add(digit)?;
                    digit_count += 1;
                    if let Some(max_digits) = max_digits {
                        if digit_count > max_digits {
                            return None;
                        }
                    }
                }

                if digit_count == 0 || (!allow_zero_prefix && has_leading_zero && digit_count > 1) {
                    None
                } else {
                    Some(result)
                }
            })
        }

        /// Read an IPv4 address.
        pub(super) fn read_ipv4_addr(&mut self) -> Option<Ipv4Addr> {
            self.read_atomically(|p| {
                let mut groups = [0; 4];

                for (i, slot) in groups.iter_mut().enumerate() {
                    *slot = p.read_separator('.', i, |p| {
                        // Disallow octal number in IP string.
                        // https://tools.ietf.org/html/rfc6943#section-3.1.1
                        p.read_number(10, Some(3), false)
                    })?;
                }

                Some(Ipv4Addr(groups))
            })
        }

        /// Read an IPv6 Address.
        pub(super) fn read_ipv6_addr(&mut self) -> Option<Ipv6Addr> {
            /// Read a chunk of an IPv6 address into `groups`. Returns the number
            /// of groups read, along with a bool indicating if an embedded
            /// trailing IPv4 address was read. Specifically, read a series of
            /// colon-separated IPv6 groups (0x0000 - 0xFFFF), with an optional
            /// trailing embedded IPv4 address.
            fn read_groups(p: &mut Parser<'_>, groups: &mut [u16]) -> (usize, bool) {
                let limit = groups.len();

                for (i, slot) in groups.iter_mut().enumerate() {
                    // Try to read a trailing embedded IPv4 address. There must be
                    // at least two groups left.
                    if i < limit - 1 {
                        let ipv4 = p.read_separator(':', i, |p| p.read_ipv4_addr());

                        if let Some(v4_addr) = ipv4 {
                            let [one, two, three, four] = v4_addr.0;
                            groups[i] = u16::from_be_bytes([one, two]);
                            groups[i + 1] = u16::from_be_bytes([three, four]);
                            return (i + 2, true);
                        }
                    }

                    let group = p.read_separator(':', i, |p| p.read_number(16, Some(4), true));

                    match group {
                        Some(g) => *slot = g,
                        None => return (i, false),
                    }
                }
                (groups.len(), false)
            }

            self.read_atomically(|p| {
                // Read the front part of the address; either the whole thing, or up
                // to the first ::
                let mut head = [0; 8];
                let (head_size, head_ipv4) = read_groups(p, &mut head);

                if head_size == 8 {
                    return Some(head.into());
                }

                // IPv4 part is not allowed before `::`
                if head_ipv4 {
                    return None;
                }

                // Read `::` if previous code parsed less than 8 groups.
                // `::` indicates one or more groups of 16 bits of zeros.
                p.read_given_char(':')?;
                p.read_given_char(':')?;

                // Read the back part of the address. The :: must contain at least one
                // set of zeroes, so our max length is 7.
                let mut tail = [0; 7];
                let limit = 8 - (head_size + 1);
                let (tail_size, _) = read_groups(p, &mut tail[..limit]);

                // Concat the head and tail of the IP address
                head[(8 - tail_size)..8].copy_from_slice(&tail[..tail_size]);

                Some(head.into())
            })
        }
    }

    trait ReadNumberHelper: Sized {
        const ZERO: Self;
        fn checked_mul(&self, other: u32) -> Option<Self>;
        fn checked_add(&self, other: u32) -> Option<Self>;
    }

    macro_rules! impl_helper {
        ($($t:ty)*) => ($(impl ReadNumberHelper for $t {
            const ZERO: Self = 0;
            #[inline]
            fn checked_mul(&self, other: u32) -> Option<Self> {
                Self::checked_mul(*self, other.try_into().ok()?)
            }
            #[inline]
            fn checked_add(&self, other: u32) -> Option<Self> {
                Self::checked_add(*self, other.try_into().ok()?)
            }
        })*)
    }

    impl_helper! { u8 u16 u32 }

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub(super) enum AddrKind {
        Ipv4,
        Ipv6,
    }
}

use parser::{AddrKind, Parser};

/// Failure to parse an IP address
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AddrParseError(AddrKind);

impl core::fmt::Display for AddrParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str(match self.0 {
            AddrKind::Ipv4 => "invalid IPv4 address syntax",
            AddrKind::Ipv6 => "invalid IPv6 address syntax",
        })
    }
}

#[cfg(feature = "std")]
impl ::std::error::Error for AddrParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "alloc")]
    use alloc::format;

    #[cfg(feature = "alloc")]
    static TESTS: &[(&str, bool)] = &[
        ("", false),
        ("localhost", true),
        ("LOCALHOST", true),
        (".localhost", false),
        ("..localhost", false),
        ("1.2.3.4", false),
        ("127.0.0.1", false),
        ("absolute.", true),
        ("absolute..", false),
        ("multiple.labels.absolute.", true),
        ("foo.bar.com", true),
        ("infix-hyphen-allowed.com", true),
        ("-prefixhypheninvalid.com", false),
        ("suffixhypheninvalid--", false),
        ("suffixhypheninvalid-.com", false),
        ("foo.lastlabelendswithhyphen-", false),
        ("infix_underscore_allowed.com", true),
        ("_prefixunderscorevalid.com", true),
        ("labelendswithnumber1.bar.com", true),
        ("xn--bcher-kva.example", true),
        (
            "sixtythreesixtythreesixtythreesixtythreesixtythreesixtythreesix.com",
            true,
        ),
        (
            "sixtyfoursixtyfoursixtyfoursixtyfoursixtyfoursixtyfoursixtyfours.com",
            false,
        ),
        (
            "012345678901234567890123456789012345678901234567890123456789012.com",
            true,
        ),
        (
            "0123456789012345678901234567890123456789012345678901234567890123.com",
            false,
        ),
        (
            "01234567890123456789012345678901234567890123456789012345678901-.com",
            false,
        ),
        (
            "012345678901234567890123456789012345678901234567890123456789012-.com",
            false,
        ),
        ("numeric-only-final-label.1", false),
        ("numeric-only-final-label.absolute.1.", false),
        ("1starts-with-number.com", true),
        ("1Starts-with-number.com", true),
        ("1.2.3.4.com", true),
        ("123.numeric-only-first-label", true),
        ("a123b.com", true),
        ("numeric-only-middle-label.4.com", true),
        ("1000-sans.badssl.com", true),
        (
            "twohundredandfiftythreecharacters.twohundredandfiftythreecharacters.twohundredandfiftythreecharacters.twohundredandfiftythreecharacters.twohundredandfiftythreecharacters.twohundredandfiftythreecharacters.twohundredandfiftythreecharacters.twohundredandfi",
            true,
        ),
        (
            "twohundredandfiftyfourcharacters.twohundredandfiftyfourcharacters.twohundredandfiftyfourcharacters.twohundredandfiftyfourcharacters.twohundredandfiftyfourcharacters.twohundredandfiftyfourcharacters.twohundredandfiftyfourcharacters.twohundredandfiftyfourc",
            false,
        ),
    ];

    #[cfg(feature = "alloc")]
    #[test]
    fn test_validation() {
        for (input, expected) in TESTS {
            #[cfg(feature = "std")]
            println!("test: {:?} expected valid? {:?}", input, expected);
            let name_ref = DnsName::try_from(*input);
            assert_eq!(*expected, name_ref.is_ok());
            let name = DnsName::try_from(input.to_string());
            assert_eq!(*expected, name.is_ok());
        }
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn error_is_debug() {
        assert_eq!(format!("{:?}", InvalidDnsNameError), "InvalidDnsNameError");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn error_is_display() {
        assert_eq!(format!("{}", InvalidDnsNameError), "invalid dns name");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn dns_name_is_debug() {
        let example = DnsName::try_from("example.com".to_string()).unwrap();
        assert_eq!(format!("{:?}", example), "DnsName(\"example.com\")");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn dns_name_traits() {
        let example = DnsName::try_from("example.com".to_string()).unwrap();
        assert_eq!(example, example); // PartialEq

        #[cfg(feature = "std")]
        {
            use std::collections::HashSet;
            let mut h = HashSet::<DnsName>::new();
            h.insert(example);
        }
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn try_from_ascii_rejects_bad_utf8() {
        assert_eq!(
            format!("{:?}", DnsName::try_from(&b"\x80"[..])),
            "Err(InvalidDnsNameError)"
        );
    }

    const fn ipv4_address(
        ip_address: &str,
        octets: [u8; 4],
    ) -> (&str, Result<Ipv4Addr, AddrParseError>) {
        (ip_address, Ok(Ipv4Addr(octets)))
    }

    const IPV4_ADDRESSES: &[(&str, Result<Ipv4Addr, AddrParseError>)] = &[
        // Valid IPv4 addresses
        ipv4_address("0.0.0.0", [0, 0, 0, 0]),
        ipv4_address("1.1.1.1", [1, 1, 1, 1]),
        ipv4_address("205.0.0.0", [205, 0, 0, 0]),
        ipv4_address("0.205.0.0", [0, 205, 0, 0]),
        ipv4_address("0.0.205.0", [0, 0, 205, 0]),
        ipv4_address("0.0.0.205", [0, 0, 0, 205]),
        ipv4_address("0.0.0.20", [0, 0, 0, 20]),
        // Invalid IPv4 addresses
        ("", Err(AddrParseError(AddrKind::Ipv4))),
        ("...", Err(AddrParseError(AddrKind::Ipv4))),
        (".0.0.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.0.0.", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.0.", Err(AddrParseError(AddrKind::Ipv4))),
        ("256.0.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.256.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.256.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.0.256", Err(AddrParseError(AddrKind::Ipv4))),
        ("1..1.1.1", Err(AddrParseError(AddrKind::Ipv4))),
        ("1.1..1.1", Err(AddrParseError(AddrKind::Ipv4))),
        ("1.1.1..1", Err(AddrParseError(AddrKind::Ipv4))),
        ("025.0.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.025.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.025.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.0.025", Err(AddrParseError(AddrKind::Ipv4))),
        ("1234.0.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.1234.0.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.1234.0", Err(AddrParseError(AddrKind::Ipv4))),
        ("0.0.0.1234", Err(AddrParseError(AddrKind::Ipv4))),
    ];

    #[test]
    fn parse_ipv4_address_test() {
        for &(ip_address, expected_result) in IPV4_ADDRESSES {
            assert_eq!(Ipv4Addr::try_from(ip_address), expected_result);
        }
    }

    const fn ipv6_address(
        ip_address: &str,
        octets: [u8; 16],
    ) -> (&str, Result<Ipv6Addr, AddrParseError>) {
        (ip_address, Ok(Ipv6Addr(octets)))
    }

    const IPV6_ADDRESSES: &[(&str, Result<Ipv6Addr, AddrParseError>)] = &[
        // Valid IPv6 addresses
        ipv6_address(
            "2a05:d018:076c:b685:e8ab:afd3:af51:3aed",
            [
                0x2a, 0x05, 0xd0, 0x18, 0x07, 0x6c, 0xb6, 0x85, 0xe8, 0xab, 0xaf, 0xd3, 0xaf, 0x51,
                0x3a, 0xed,
            ],
        ),
        ipv6_address(
            "2A05:D018:076C:B685:E8AB:AFD3:AF51:3AED",
            [
                0x2a, 0x05, 0xd0, 0x18, 0x07, 0x6c, 0xb6, 0x85, 0xe8, 0xab, 0xaf, 0xd3, 0xaf, 0x51,
                0x3a, 0xed,
            ],
        ),
        ipv6_address(
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            [
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff,
            ],
        ),
        ipv6_address(
            "FFFF:FFFF:FFFF:FFFF:FFFF:FFFF:FFFF:FFFF",
            [
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff,
            ],
        ),
        ipv6_address(
            "FFFF:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            [
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff,
            ],
        ),
        // Wrong hexadecimal characters on different positions
        (
            "ffgf:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:gfff:ffff:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:fffg:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffgf:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:gfff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff:fgff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff:ffff:ffgf:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff:ffff:ffgf:fffg",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Wrong colons on uncompressed addresses
        (
            ":ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff::ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff::ffff:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff::ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff::ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff::ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff:ffff::ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff::ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // More colons than allowed
        (
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff:",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        (
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // v Invalid hexadecimal
        (
            "ga05:d018:076c:b685:e8ab:afd3:af51:3aed",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Cannot start with colon
        (
            ":a05:d018:076c:b685:e8ab:afd3:af51:3aed",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Cannot end with colon
        (
            "2a05:d018:076c:b685:e8ab:afd3:af51:3ae:",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Cannot have more than seven colons
        (
            "2a05:d018:076c:b685:e8ab:afd3:af51:3a::",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Cannot contain two colons in a row
        (
            "2a05::018:076c:b685:e8ab:afd3:af51:3aed",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // v Textual block size is longer
        (
            "2a056:d018:076c:b685:e8ab:afd3:af51:3ae",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // v Textual block size is shorter
        (
            "2a0:d018:076c:b685:e8ab:afd3:af51:3aed ",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Shorter IPv6 address
        (
            "d018:076c:b685:e8ab:afd3:af51:3aed",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
        // Longer IPv6 address
        (
            "2a05:d018:076c:b685:e8ab:afd3:af51:3aed3aed",
            Err(AddrParseError(AddrKind::Ipv6)),
        ),
    ];

    #[test]
    fn parse_ipv6_address_test() {
        for &(ip_address, expected_result) in IPV6_ADDRESSES {
            assert_eq!(Ipv6Addr::try_from(ip_address), expected_result);
        }
    }

    #[test]
    fn try_from_ascii_ip_address_test() {
        const IP_ADDRESSES: &[(&str, Result<IpAddr, AddrParseError>)] = &[
            // Valid IPv4 addresses
            ("127.0.0.1", Ok(IpAddr::V4(Ipv4Addr([127, 0, 0, 1])))),
            // Invalid IPv4 addresses
            (
                // Ends with a dot; misses one octet
                "127.0.0.",
                Err(AddrParseError(AddrKind::Ipv6)),
            ),
            // Valid IPv6 addresses
            (
                "0000:0000:0000:0000:0000:0000:0000:0001",
                Ok(IpAddr::V6(Ipv6Addr([
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ]))),
            ),
            // Something else
            (
                // A hostname
                "example.com",
                Err(AddrParseError(AddrKind::Ipv6)),
            ),
        ];
        for &(ip_address, expected_result) in IP_ADDRESSES {
            assert_eq!(IpAddr::try_from(ip_address), expected_result)
        }
    }

    #[test]
    fn try_from_ascii_str_ip_address_test() {
        const IP_ADDRESSES: &[(&str, Result<IpAddr, AddrParseError>)] = &[
            // Valid IPv4 addresses
            ("127.0.0.1", Ok(IpAddr::V4(Ipv4Addr([127, 0, 0, 1])))),
            // Invalid IPv4 addresses
            (
                // Ends with a dot; misses one octet
                "127.0.0.",
                Err(AddrParseError(AddrKind::Ipv6)),
            ),
            // Valid IPv6 addresses
            (
                "0000:0000:0000:0000:0000:0000:0000:0001",
                Ok(IpAddr::V6(Ipv6Addr([
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ]))),
            ),
            // Something else
            (
                // A hostname
                "example.com",
                Err(AddrParseError(AddrKind::Ipv6)),
            ),
        ];
        for &(ip_address, expected_result) in IP_ADDRESSES {
            assert_eq!(IpAddr::try_from(ip_address), expected_result)
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn to_str() {
        let domain_str = "example.com";
        let domain_servername = ServerName::try_from(domain_str).unwrap();
        assert_eq!(domain_str, domain_servername.to_str());

        let ipv4_str = "127.0.0.1";
        let ipv4_servername = ServerName::try_from("127.0.0.1").unwrap();
        assert_eq!(ipv4_str, ipv4_servername.to_str());

        let ipv6_str = "::1";
        let ipv6_servername = ServerName::try_from(ipv6_str).unwrap();
        assert_eq!("::1", ipv6_servername.to_str());
    }
}
