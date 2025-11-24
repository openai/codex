use http::Method;
use std::{
    fmt,
    fmt::{Debug, Formatter},
};

/// A filter that matches one or more HTTP methods.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MethodFilter(u16);

impl MethodFilter {
    /// Match `CONNECT` requests.
    ///
    /// This is useful for implementing HTTP/2's [extended CONNECT method],
    /// in which the `:protocol` pseudoheader is read
    /// (using [`hyper::ext::Protocol`])
    /// and the connection upgraded to a bidirectional byte stream
    /// (using [`hyper::upgrade::on`]).
    ///
    /// As seen in the [HTTP Upgrade Token Registry],
    /// common uses include WebSockets and proxying UDP or IP –
    /// though note that when using [`WebSocketUpgrade`]
    /// it's more useful to use [`any`](crate::routing::any)
    /// as HTTP/1.1 WebSockets need to support `GET`.
    ///
    /// [extended CONNECT]: https://www.rfc-editor.org/rfc/rfc8441.html#section-4
    /// [HTTP Upgrade Token Registry]: https://www.iana.org/assignments/http-upgrade-tokens/http-upgrade-tokens.xhtml
    /// [`WebSocketUpgrade`]: crate::extract::WebSocketUpgrade
    pub const CONNECT: Self = Self::from_bits(0b0_0000_0001);
    /// Match `DELETE` requests.
    pub const DELETE: Self = Self::from_bits(0b0_0000_0010);
    /// Match `GET` requests.
    pub const GET: Self = Self::from_bits(0b0_0000_0100);
    /// Match `HEAD` requests.
    pub const HEAD: Self = Self::from_bits(0b0_0000_1000);
    /// Match `OPTIONS` requests.
    pub const OPTIONS: Self = Self::from_bits(0b0_0001_0000);
    /// Match `PATCH` requests.
    pub const PATCH: Self = Self::from_bits(0b0_0010_0000);
    /// Match `POST` requests.
    pub const POST: Self = Self::from_bits(0b0_0100_0000);
    /// Match `PUT` requests.
    pub const PUT: Self = Self::from_bits(0b0_1000_0000);
    /// Match `TRACE` requests.
    pub const TRACE: Self = Self::from_bits(0b1_0000_0000);

    const fn bits(&self) -> u16 {
        let bits = self;
        bits.0
    }

    const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub(crate) const fn contains(&self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }

    /// Performs the OR operation between the [`MethodFilter`] in `self` with `other`.
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Error type used when converting a [`Method`] to a [`MethodFilter`] fails.
#[derive(Debug)]
pub struct NoMatchingMethodFilter {
    method: Method,
}

impl NoMatchingMethodFilter {
    /// Get the [`Method`] that couldn't be converted to a [`MethodFilter`].
    pub fn method(&self) -> &Method {
        &self.method
    }
}

impl fmt::Display for NoMatchingMethodFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "no `MethodFilter` for `{}`", self.method.as_str())
    }
}

impl std::error::Error for NoMatchingMethodFilter {}

impl TryFrom<Method> for MethodFilter {
    type Error = NoMatchingMethodFilter;

    fn try_from(m: Method) -> Result<Self, NoMatchingMethodFilter> {
        match m {
            Method::CONNECT => Ok(MethodFilter::CONNECT),
            Method::DELETE => Ok(MethodFilter::DELETE),
            Method::GET => Ok(MethodFilter::GET),
            Method::HEAD => Ok(MethodFilter::HEAD),
            Method::OPTIONS => Ok(MethodFilter::OPTIONS),
            Method::PATCH => Ok(MethodFilter::PATCH),
            Method::POST => Ok(MethodFilter::POST),
            Method::PUT => Ok(MethodFilter::PUT),
            Method::TRACE => Ok(MethodFilter::TRACE),
            other => Err(NoMatchingMethodFilter { method: other }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_http_method() {
        assert_eq!(
            MethodFilter::try_from(Method::CONNECT).unwrap(),
            MethodFilter::CONNECT
        );

        assert_eq!(
            MethodFilter::try_from(Method::DELETE).unwrap(),
            MethodFilter::DELETE
        );

        assert_eq!(
            MethodFilter::try_from(Method::GET).unwrap(),
            MethodFilter::GET
        );

        assert_eq!(
            MethodFilter::try_from(Method::HEAD).unwrap(),
            MethodFilter::HEAD
        );

        assert_eq!(
            MethodFilter::try_from(Method::OPTIONS).unwrap(),
            MethodFilter::OPTIONS
        );

        assert_eq!(
            MethodFilter::try_from(Method::PATCH).unwrap(),
            MethodFilter::PATCH
        );

        assert_eq!(
            MethodFilter::try_from(Method::POST).unwrap(),
            MethodFilter::POST
        );

        assert_eq!(
            MethodFilter::try_from(Method::PUT).unwrap(),
            MethodFilter::PUT
        );

        assert_eq!(
            MethodFilter::try_from(Method::TRACE).unwrap(),
            MethodFilter::TRACE
        );

        assert!(
            MethodFilter::try_from(http::Method::from_bytes(b"CUSTOM").unwrap())
                .unwrap_err()
                .to_string()
                .contains("CUSTOM")
        );
    }
}
