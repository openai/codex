use ascii::{AsciiStr, AsciiString, FromAsciiError};
use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// Status code of a request or response.
#[derive(Eq, PartialEq, Copy, Clone, Debug, Ord, PartialOrd)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// Returns the default reason phrase for this status code.
    /// For example the status code 404 corresponds to "Not Found".
    pub fn default_reason_phrase(&self) -> &'static str {
        match self.0 {
            100 => "Continue",
            101 => "Switching Protocols",
            102 => "Processing",
            103 => "Early Hints",

            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            203 => "Non-Authoritative Information",
            204 => "No Content",
            205 => "Reset Content",
            206 => "Partial Content",
            207 => "Multi-Status",
            208 => "Already Reported",
            226 => "IM Used",

            300 => "Multiple Choices",
            301 => "Moved Permanently",
            302 => "Found",
            303 => "See Other",
            304 => "Not Modified",
            305 => "Use Proxy",
            307 => "Temporary Redirect",
            308 => "Permanent Redirect",

            400 => "Bad Request",
            401 => "Unauthorized",
            402 => "Payment Required",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            406 => "Not Acceptable",
            407 => "Proxy Authentication Required",
            408 => "Request Timeout",
            409 => "Conflict",
            410 => "Gone",
            411 => "Length Required",
            412 => "Precondition Failed",
            413 => "Payload Too Large",
            414 => "URI Too Long",
            415 => "Unsupported Media Type",
            416 => "Range Not Satisfiable",
            417 => "Expectation Failed",
            421 => "Misdirected Request",
            422 => "Unprocessable Entity",
            423 => "Locked",
            424 => "Failed Dependency",
            426 => "Upgrade Required",
            428 => "Precondition Required",
            429 => "Too Many Requests",
            431 => "Request Header Fields Too Large",
            451 => "Unavailable For Legal Reasons",

            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            505 => "HTTP Version Not Supported",
            506 => "Variant Also Negotiates",
            507 => "Insufficient Storage",
            508 => "Loop Detected",
            510 => "Not Extended",
            511 => "Network Authentication Required",
            _ => "Unknown",
        }
    }
}

impl From<i8> for StatusCode {
    fn from(in_code: i8) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<u8> for StatusCode {
    fn from(in_code: u8) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<i16> for StatusCode {
    fn from(in_code: i16) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<u16> for StatusCode {
    fn from(in_code: u16) -> StatusCode {
        StatusCode(in_code)
    }
}

impl From<i32> for StatusCode {
    fn from(in_code: i32) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<u32> for StatusCode {
    fn from(in_code: u32) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl AsRef<u16> for StatusCode {
    fn as_ref(&self) -> &u16 {
        &self.0
    }
}

impl PartialEq<u16> for StatusCode {
    fn eq(&self, other: &u16) -> bool {
        &self.0 == other
    }
}

impl PartialEq<StatusCode> for u16 {
    fn eq(&self, other: &StatusCode) -> bool {
        self == &other.0
    }
}

impl PartialOrd<u16> for StatusCode {
    fn partial_cmp(&self, other: &u16) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialOrd<StatusCode> for u16 {
    fn partial_cmp(&self, other: &StatusCode) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

/// Represents a HTTP header.
#[derive(Debug, Clone)]
pub struct Header {
    pub field: HeaderField,
    pub value: AsciiString,
}

impl Header {
    /// Builds a `Header` from two `Vec<u8>`s or two `&[u8]`s.
    ///
    /// Example:
    ///
    /// ```
    /// let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap();
    /// ```
    #[allow(clippy::result_unit_err)]
    pub fn from_bytes<B1, B2>(header: B1, value: B2) -> Result<Header, ()>
    where
        B1: Into<Vec<u8>> + AsRef<[u8]>,
        B2: Into<Vec<u8>> + AsRef<[u8]>,
    {
        let header = HeaderField::from_bytes(header).or(Err(()))?;
        let value = AsciiString::from_ascii(value).or(Err(()))?;

        Ok(Header {
            field: header,
            value,
        })
    }
}

impl FromStr for Header {
    type Err = ();

    fn from_str(input: &str) -> Result<Header, ()> {
        let mut elems = input.splitn(2, ':');

        let field = elems.next().and_then(|f| f.parse().ok()).ok_or(())?;
        let value = elems
            .next()
            .and_then(|v| AsciiString::from_ascii(v.trim()).ok())
            .ok_or(())?;

        Ok(Header { field, value })
    }
}

impl Display for Header {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "{}: {}", self.field, self.value.as_str())
    }
}

/// Field of a header (eg. `Content-Type`, `Content-Length`, etc.)
///
/// Comparison between two `HeaderField`s ignores case.
#[derive(Debug, Clone, Eq)]
pub struct HeaderField(AsciiString);

impl HeaderField {
    pub fn from_bytes<B>(bytes: B) -> Result<HeaderField, FromAsciiError<B>>
    where
        B: Into<Vec<u8>> + AsRef<[u8]>,
    {
        AsciiString::from_ascii(bytes).map(HeaderField)
    }

    pub fn as_str(&self) -> &AsciiStr {
        &self.0
    }

    pub fn equiv(&self, other: &'static str) -> bool {
        other.eq_ignore_ascii_case(self.as_str().as_str())
    }
}

impl FromStr for HeaderField {
    type Err = ();

    fn from_str(s: &str) -> Result<HeaderField, ()> {
        if s.contains(char::is_whitespace) {
            Err(())
        } else {
            AsciiString::from_ascii(s).map(HeaderField).map_err(|_| ())
        }
    }
}

impl Display for HeaderField {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "{}", self.0.as_str())
    }
}

impl PartialEq for HeaderField {
    fn eq(&self, other: &HeaderField) -> bool {
        let self_str: &str = self.as_str().as_ref();
        let other_str = other.as_str().as_ref();
        self_str.eq_ignore_ascii_case(other_str)
    }
}

/// HTTP request methods
///
/// As per [RFC 7231](https://tools.ietf.org/html/rfc7231#section-4.1) and
/// [RFC 5789](https://tools.ietf.org/html/rfc5789)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Method {
    /// `GET`
    Get,

    /// `HEAD`
    Head,

    /// `POST`
    Post,

    /// `PUT`
    Put,

    /// `DELETE`
    Delete,

    /// `CONNECT`
    Connect,

    /// `OPTIONS`
    Options,

    /// `TRACE`
    Trace,

    /// `PATCH`
    Patch,

    /// Request methods not standardized by the IETF
    NonStandard(AsciiString),
}

impl Method {
    pub fn as_str(&self) -> &str {
        match *self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Connect => "CONNECT",
            Method::Options => "OPTIONS",
            Method::Trace => "TRACE",
            Method::Patch => "PATCH",
            Method::NonStandard(ref s) => s.as_str(),
        }
    }
}

impl FromStr for Method {
    type Err = ();

    fn from_str(s: &str) -> Result<Method, ()> {
        Ok(match s {
            "GET" => Method::Get,
            "HEAD" => Method::Head,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "CONNECT" => Method::Connect,
            "OPTIONS" => Method::Options,
            "TRACE" => Method::Trace,
            "PATCH" => Method::Patch,
            s => {
                let ascii_string = AsciiString::from_ascii(s).map_err(|_| ())?;
                Method::NonStandard(ascii_string)
            }
        })
    }
}

impl Display for Method {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "{}", self.as_str())
    }
}

/// HTTP version (usually 1.0 or 1.1).
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HTTPVersion(pub u8, pub u8);

impl Display for HTTPVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "{}.{}", self.0, self.1)
    }
}

impl Ord for HTTPVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let HTTPVersion(my_major, my_minor) = *self;
        let HTTPVersion(other_major, other_minor) = *other;

        if my_major != other_major {
            return my_major.cmp(&other_major);
        }

        my_minor.cmp(&other_minor)
    }
}

impl PartialOrd for HTTPVersion {
    fn partial_cmp(&self, other: &HTTPVersion) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq<(u8, u8)> for HTTPVersion {
    fn eq(&self, &(major, minor): &(u8, u8)) -> bool {
        self.eq(&HTTPVersion(major, minor))
    }
}

impl PartialEq<HTTPVersion> for (u8, u8) {
    fn eq(&self, other: &HTTPVersion) -> bool {
        let &(major, minor) = self;
        HTTPVersion(major, minor).eq(other)
    }
}

impl PartialOrd<(u8, u8)> for HTTPVersion {
    fn partial_cmp(&self, &(major, minor): &(u8, u8)) -> Option<Ordering> {
        self.partial_cmp(&HTTPVersion(major, minor))
    }
}

impl PartialOrd<HTTPVersion> for (u8, u8) {
    fn partial_cmp(&self, other: &HTTPVersion) -> Option<Ordering> {
        let &(major, minor) = self;
        HTTPVersion(major, minor).partial_cmp(other)
    }
}

impl From<(u8, u8)> for HTTPVersion {
    fn from((major, minor): (u8, u8)) -> HTTPVersion {
        HTTPVersion(major, minor)
    }
}

#[cfg(test)]
mod test {
    use super::Header;
    use httpdate::HttpDate;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_parse_header() {
        let header: Header = "Content-Type: text/html".parse().unwrap();

        assert!(header.field.equiv(&"content-type"));
        assert!(header.value.as_str() == "text/html");

        assert!("hello world".parse::<Header>().is_err());
    }

    #[test]
    fn formats_date_correctly() {
        let http_date = HttpDate::from(SystemTime::UNIX_EPOCH + Duration::from_secs(420895020));

        assert_eq!(http_date.to_string(), "Wed, 04 May 1983 11:17:00 GMT")
    }

    #[test]
    fn test_parse_header_with_doublecolon() {
        let header: Header = "Time: 20: 34".parse().unwrap();

        assert!(header.field.equiv(&"time"));
        assert!(header.value.as_str() == "20: 34");
    }

    // This tests reslstance to RUSTSEC-2020-0031: "HTTP Request smuggling
    // through malformed Transfer Encoding headers"
    // (https://rustsec.org/advisories/RUSTSEC-2020-0031.html).
    #[test]
    fn test_strict_headers() {
        assert!("Transfer-Encoding : chunked".parse::<Header>().is_err());
        assert!(" Transfer-Encoding: chunked".parse::<Header>().is_err());
        assert!("Transfer Encoding: chunked".parse::<Header>().is_err());
        assert!(" Transfer\tEncoding : chunked".parse::<Header>().is_err());
        assert!("Transfer-Encoding: chunked".parse::<Header>().is_ok());
        assert!("Transfer-Encoding: chunked ".parse::<Header>().is_ok());
        assert!("Transfer-Encoding:   chunked ".parse::<Header>().is_ok());
    }
}
