use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::{cmp, fmt, str};

use bytes::Bytes;

use super::{ErrorKind, InvalidUri, Port, URI_CHARS};
use crate::byte_str::ByteStr;

/// Represents the authority component of a URI.
#[derive(Clone)]
pub struct Authority {
    pub(super) data: ByteStr,
}

impl Authority {
    pub(super) fn empty() -> Self {
        Authority {
            data: ByteStr::new(),
        }
    }

    // Not public while `bytes` is unstable.
    pub(super) fn from_shared(s: Bytes) -> Result<Self, InvalidUri> {
        // Precondition on create_authority: trivially satisfied by the
        // identity closure
        create_authority(s, |s| s)
    }

    /// Attempt to convert an `Authority` from a static string.
    ///
    /// This function will not perform any copying, and the string will be
    /// checked if it is empty or contains an invalid character.
    ///
    /// # Panics
    ///
    /// This function panics if the argument contains invalid characters or
    /// is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::Authority;
    /// let authority = Authority::from_static("example.com");
    /// assert_eq!(authority.host(), "example.com");
    /// ```
    pub fn from_static(src: &'static str) -> Self {
        Authority::from_shared(Bytes::from_static(src.as_bytes()))
            .expect("static str is not valid authority")
    }

    /// Attempt to convert a `Bytes` buffer to a `Authority`.
    ///
    /// This will try to prevent a copy if the type passed is the type used
    /// internally, and will copy the data if it is not.
    pub fn from_maybe_shared<T>(src: T) -> Result<Self, InvalidUri>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return Authority::from_shared(src);
        });

        Authority::try_from(src.as_ref())
    }

    // Note: this may return an *empty* Authority. You might want `parse_non_empty`.
    // Postcondition: for all Ok() returns, s[..ret.unwrap()] is valid UTF-8 where
    // ret is the return value.
    pub(super) fn parse(s: &[u8]) -> Result<usize, InvalidUri> {
        let mut colon_cnt = 0u32;
        let mut start_bracket = false;
        let mut end_bracket = false;
        let mut has_percent = false;
        let mut end = s.len();
        let mut at_sign_pos = None;
        const MAX_COLONS: u32 = 8; // e.g., [FEDC:BA98:7654:3210:FEDC:BA98:7654:3210]:80

        // Among other things, this loop checks that every byte in s up to the
        // first '/', '?', or '#' is a valid URI character (or in some contexts,
        // a '%'). This means that each such byte is a valid single-byte UTF-8
        // code point.
        for (i, &b) in s.iter().enumerate() {
            match URI_CHARS[b as usize] {
                b'/' | b'?' | b'#' => {
                    end = i;
                    break;
                }
                b':' => {
                    if colon_cnt >= MAX_COLONS {
                        return Err(ErrorKind::InvalidAuthority.into());
                    }
                    colon_cnt += 1;
                }
                b'[' => {
                    if has_percent || start_bracket {
                        // Something other than the userinfo has a `%`, so reject it.
                        return Err(ErrorKind::InvalidAuthority.into());
                    }
                    start_bracket = true;
                }
                b']' => {
                    if (!start_bracket) || end_bracket {
                        return Err(ErrorKind::InvalidAuthority.into());
                    }
                    end_bracket = true;

                    // Those were part of an IPv6 hostname, so forget them...
                    colon_cnt = 0;
                    has_percent = false;
                }
                b'@' => {
                    at_sign_pos = Some(i);

                    // Those weren't a port colon, but part of the
                    // userinfo, so it needs to be forgotten.
                    colon_cnt = 0;
                    has_percent = false;
                }
                0 if b == b'%' => {
                    // Per https://tools.ietf.org/html/rfc3986#section-3.2.1 and
                    // https://url.spec.whatwg.org/#authority-state
                    // the userinfo can have a percent-encoded username and password,
                    // so record that a `%` was found. If this turns out to be
                    // part of the userinfo, this flag will be cleared.
                    // Also per https://tools.ietf.org/html/rfc6874, percent-encoding can
                    // be used to indicate a zone identifier.
                    // If the flag hasn't been cleared at the end, that means this
                    // was part of the hostname (and not part of an IPv6 address), and
                    // will fail with an error.
                    has_percent = true;
                }
                0 => {
                    return Err(ErrorKind::InvalidUriChar.into());
                }
                _ => {}
            }
        }

        if start_bracket ^ end_bracket {
            return Err(ErrorKind::InvalidAuthority.into());
        }

        if colon_cnt > 1 {
            // Things like 'localhost:8080:3030' are rejected.
            return Err(ErrorKind::InvalidAuthority.into());
        }

        if end > 0 && at_sign_pos == Some(end - 1) {
            // If there's nothing after an `@`, this is bonkers.
            return Err(ErrorKind::InvalidAuthority.into());
        }

        if has_percent {
            // Something after the userinfo has a `%`, so reject it.
            return Err(ErrorKind::InvalidAuthority.into());
        }

        Ok(end)
    }

    // Parse bytes as an Authority, not allowing an empty string.
    //
    // This should be used by functions that allow a user to parse
    // an `Authority` by itself.
    //
    // Postcondition: for all Ok() returns, s[..ret.unwrap()] is valid UTF-8 where
    // ret is the return value.
    fn parse_non_empty(s: &[u8]) -> Result<usize, InvalidUri> {
        if s.is_empty() {
            return Err(ErrorKind::Empty.into());
        }
        Authority::parse(s)
    }

    /// Get the host of this `Authority`.
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
    /// ```
    /// # use http::uri::*;
    /// let authority: Authority = "example.org:80".parse().unwrap();
    ///
    /// assert_eq!(authority.host(), "example.org");
    /// ```
    #[inline]
    pub fn host(&self) -> &str {
        host(self.as_str())
    }

    /// Get the port part of this `Authority`.
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
    /// Authority with port
    ///
    /// ```
    /// # use http::uri::Authority;
    /// let authority: Authority = "example.org:80".parse().unwrap();
    ///
    /// let port = authority.port().unwrap();
    /// assert_eq!(port.as_u16(), 80);
    /// assert_eq!(port.as_str(), "80");
    /// ```
    ///
    /// Authority without port
    ///
    /// ```
    /// # use http::uri::Authority;
    /// let authority: Authority = "example.org".parse().unwrap();
    ///
    /// assert!(authority.port().is_none());
    /// ```
    pub fn port(&self) -> Option<Port<&str>> {
        let bytes = self.as_str();
        bytes
            .rfind(':')
            .and_then(|i| Port::from_str(&bytes[i + 1..]).ok())
    }

    /// Get the port of this `Authority` as a `u16`.
    ///
    /// # Example
    ///
    /// ```
    /// # use http::uri::Authority;
    /// let authority: Authority = "example.org:80".parse().unwrap();
    ///
    /// assert_eq!(authority.port_u16(), Some(80));
    /// ```
    pub fn port_u16(&self) -> Option<u16> {
        self.port().map(|p| p.as_u16())
    }

    /// Return a str representation of the authority
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.data[..]
    }
}

// Purposefully not public while `bytes` is unstable.
// impl TryFrom<Bytes> for Authority

impl AsRef<str> for Authority {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for Authority {
    fn eq(&self, other: &Authority) -> bool {
        self.data.eq_ignore_ascii_case(&other.data)
    }
}

impl Eq for Authority {}

/// Case-insensitive equality
///
/// # Examples
///
/// ```
/// # use http::uri::Authority;
/// let authority: Authority = "HELLO.com".parse().unwrap();
/// assert_eq!(authority, "hello.coM");
/// assert_eq!("hello.com", authority);
/// ```
impl PartialEq<str> for Authority {
    fn eq(&self, other: &str) -> bool {
        self.data.eq_ignore_ascii_case(other)
    }
}

impl PartialEq<Authority> for str {
    fn eq(&self, other: &Authority) -> bool {
        self.eq_ignore_ascii_case(other.as_str())
    }
}

impl<'a> PartialEq<Authority> for &'a str {
    fn eq(&self, other: &Authority) -> bool {
        self.eq_ignore_ascii_case(other.as_str())
    }
}

impl<'a> PartialEq<&'a str> for Authority {
    fn eq(&self, other: &&'a str) -> bool {
        self.data.eq_ignore_ascii_case(other)
    }
}

impl PartialEq<String> for Authority {
    fn eq(&self, other: &String) -> bool {
        self.data.eq_ignore_ascii_case(other.as_str())
    }
}

impl PartialEq<Authority> for String {
    fn eq(&self, other: &Authority) -> bool {
        self.as_str().eq_ignore_ascii_case(other.as_str())
    }
}

/// Case-insensitive ordering
///
/// # Examples
///
/// ```
/// # use http::uri::Authority;
/// let authority: Authority = "DEF.com".parse().unwrap();
/// assert!(authority < "ghi.com");
/// assert!(authority > "abc.com");
/// ```
impl PartialOrd for Authority {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

impl PartialOrd<str> for Authority {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

impl PartialOrd<Authority> for str {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

impl<'a> PartialOrd<Authority> for &'a str {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

impl<'a> PartialOrd<&'a str> for Authority {
    fn partial_cmp(&self, other: &&'a str) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

impl PartialOrd<String> for Authority {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        let left = self.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

impl PartialOrd<Authority> for String {
    fn partial_cmp(&self, other: &Authority) -> Option<cmp::Ordering> {
        let left = self.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        let right = other.data.as_bytes().iter().map(|b| b.to_ascii_lowercase());
        left.partial_cmp(right)
    }
}

/// Case-insensitive hashing
///
/// # Examples
///
/// ```
/// # use http::uri::Authority;
/// # use std::hash::{Hash, Hasher};
/// # use std::collections::hash_map::DefaultHasher;
///
/// let a: Authority = "HELLO.com".parse().unwrap();
/// let b: Authority = "hello.coM".parse().unwrap();
///
/// let mut s = DefaultHasher::new();
/// a.hash(&mut s);
/// let a = s.finish();
///
/// let mut s = DefaultHasher::new();
/// b.hash(&mut s);
/// let b = s.finish();
///
/// assert_eq!(a, b);
/// ```
impl Hash for Authority {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.data.len().hash(state);
        for &b in self.data.as_bytes() {
            state.write_u8(b.to_ascii_lowercase());
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for Authority {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &'a [u8]) -> Result<Self, Self::Error> {
        // parse first, and only turn into Bytes if valid

        // Preconditon on create_authority: copy_from_slice() copies all of
        // bytes from the [u8] parameter into a new Bytes
        create_authority(s, Bytes::copy_from_slice)
    }
}

impl<'a> TryFrom<&'a str> for Authority {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}

impl TryFrom<Vec<u8>> for Authority {
    type Error = InvalidUri;

    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        Authority::from_shared(vec.into())
    }
}

impl TryFrom<String> for Authority {
    type Error = InvalidUri;

    #[inline]
    fn try_from(t: String) -> Result<Self, Self::Error> {
        Authority::from_shared(t.into())
    }
}

impl FromStr for Authority {
    type Err = InvalidUri;

    fn from_str(s: &str) -> Result<Self, InvalidUri> {
        TryFrom::try_from(s)
    }
}

impl fmt::Debug for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn host(auth: &str) -> &str {
    let host_port = auth
        .rsplit('@')
        .next()
        .expect("split always has at least 1 item");

    if host_port.as_bytes()[0] == b'[' {
        let i = host_port
            .find(']')
            .expect("parsing should validate brackets");
        // ..= ranges aren't available in 1.20, our minimum Rust version...
        &host_port[0..i + 1]
    } else {
        host_port
            .split(':')
            .next()
            .expect("split always has at least 1 item")
    }
}

// Precondition: f converts all of the bytes in the passed in B into the
// returned Bytes.
fn create_authority<B, F>(b: B, f: F) -> Result<Authority, InvalidUri>
where
    B: AsRef<[u8]>,
    F: FnOnce(B) -> Bytes,
{
    let s = b.as_ref();
    let authority_end = Authority::parse_non_empty(s)?;

    if authority_end != s.len() {
        return Err(ErrorKind::InvalidUriChar.into());
    }

    let bytes = f(b);

    Ok(Authority {
        // Safety: the postcondition on parse_non_empty() and the check against
        // s.len() ensure that b is valid UTF-8. The precondition on f ensures
        // that this is carried through to bytes.
        data: unsafe { ByteStr::from_utf8_unchecked(bytes) },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_string_is_error() {
        let err = Authority::parse_non_empty(b"").unwrap_err();
        assert_eq!(err.0, ErrorKind::Empty);
    }

    #[test]
    fn equal_to_self_of_same_authority() {
        let authority1: Authority = "example.com".parse().unwrap();
        let authority2: Authority = "EXAMPLE.COM".parse().unwrap();
        assert_eq!(authority1, authority2);
        assert_eq!(authority2, authority1);
    }

    #[test]
    fn not_equal_to_self_of_different_authority() {
        let authority1: Authority = "example.com".parse().unwrap();
        let authority2: Authority = "test.com".parse().unwrap();
        assert_ne!(authority1, authority2);
        assert_ne!(authority2, authority1);
    }

    #[test]
    fn equates_with_a_str() {
        let authority: Authority = "example.com".parse().unwrap();
        assert_eq!(&authority, "EXAMPLE.com");
        assert_eq!("EXAMPLE.com", &authority);
        assert_eq!(authority, "EXAMPLE.com");
        assert_eq!("EXAMPLE.com", authority);
    }

    #[test]
    fn from_static_equates_with_a_str() {
        let authority = Authority::from_static("example.com");
        assert_eq!(authority, "example.com");
    }

    #[test]
    fn not_equal_with_a_str_of_a_different_authority() {
        let authority: Authority = "example.com".parse().unwrap();
        assert_ne!(&authority, "test.com");
        assert_ne!("test.com", &authority);
        assert_ne!(authority, "test.com");
        assert_ne!("test.com", authority);
    }

    #[test]
    fn equates_with_a_string() {
        let authority: Authority = "example.com".parse().unwrap();
        assert_eq!(authority, "EXAMPLE.com".to_string());
        assert_eq!("EXAMPLE.com".to_string(), authority);
    }

    #[test]
    fn equates_with_a_string_of_a_different_authority() {
        let authority: Authority = "example.com".parse().unwrap();
        assert_ne!(authority, "test.com".to_string());
        assert_ne!("test.com".to_string(), authority);
    }

    #[test]
    fn compares_to_self() {
        let authority1: Authority = "abc.com".parse().unwrap();
        let authority2: Authority = "def.com".parse().unwrap();
        assert!(authority1 < authority2);
        assert!(authority2 > authority1);
    }

    #[test]
    fn compares_with_a_str() {
        let authority: Authority = "def.com".parse().unwrap();
        // with ref
        assert!(&authority < "ghi.com");
        assert!("ghi.com" > &authority);
        assert!(&authority > "abc.com");
        assert!("abc.com" < &authority);

        // no ref
        assert!(authority < "ghi.com");
        assert!("ghi.com" > authority);
        assert!(authority > "abc.com");
        assert!("abc.com" < authority);
    }

    #[test]
    fn compares_with_a_string() {
        let authority: Authority = "def.com".parse().unwrap();
        assert!(authority < "ghi.com".to_string());
        assert!("ghi.com".to_string() > authority);
        assert!(authority > "abc.com".to_string());
        assert!("abc.com".to_string() < authority);
    }

    #[test]
    fn allows_percent_in_userinfo() {
        let authority_str = "a%2f:b%2f@example.com";
        let authority: Authority = authority_str.parse().unwrap();
        assert_eq!(authority, authority_str);
    }

    #[test]
    fn rejects_percent_in_hostname() {
        let err = Authority::parse_non_empty(b"example%2f.com").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);

        let err = Authority::parse_non_empty(b"a%2f:b%2f@example%2f.com").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);
    }

    #[test]
    fn allows_percent_in_ipv6_address() {
        let authority_str = "[fe80::1:2:3:4%25eth0]";
        let result: Authority = authority_str.parse().unwrap();
        assert_eq!(result, authority_str);
    }

    #[test]
    fn reject_obviously_invalid_ipv6_address() {
        let err = Authority::parse_non_empty(b"[0:1:2:3:4:5:6:7:8:9:10:11:12:13:14]").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);
    }

    #[test]
    fn rejects_percent_outside_ipv6_address() {
        let err = Authority::parse_non_empty(b"1234%20[fe80::1:2:3:4]").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);

        let err = Authority::parse_non_empty(b"[fe80::1:2:3:4]%20").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);
    }

    #[test]
    fn rejects_invalid_utf8() {
        let err = Authority::try_from([0xc0u8].as_ref()).unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidUriChar);

        let err = Authority::from_shared(Bytes::from_static([0xc0u8].as_ref())).unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidUriChar);
    }

    #[test]
    fn rejects_invalid_use_of_brackets() {
        let err = Authority::parse_non_empty(b"[]@[").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);

        // reject tie-fighter
        let err = Authority::parse_non_empty(b"]o[").unwrap_err();
        assert_eq!(err.0, ErrorKind::InvalidAuthority);
    }
}
