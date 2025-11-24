use std::fmt;

use super::{ErrorKind, InvalidUri};

/// The port component of a URI.
pub struct Port<T> {
    port: u16,
    repr: T,
}

impl<T> Port<T> {
    /// Returns the port number as a `u16`.
    ///
    /// # Examples
    ///
    /// Port as `u16`.
    ///
    /// ```
    /// # use http::uri::Authority;
    /// let authority: Authority = "example.org:80".parse().unwrap();
    ///
    /// let port = authority.port().unwrap();
    /// assert_eq!(port.as_u16(), 80);
    /// ```
    pub const fn as_u16(&self) -> u16 {
        self.port
    }
}

impl<T> Port<T>
where
    T: AsRef<str>,
{
    /// Converts a `str` to a port number.
    ///
    /// The supplied `str` must be a valid u16.
    pub(crate) fn from_str(bytes: T) -> Result<Self, InvalidUri> {
        bytes
            .as_ref()
            .parse::<u16>()
            .map(|port| Port { port, repr: bytes })
            .map_err(|_| ErrorKind::InvalidPort.into())
    }

    /// Returns the port number as a `str`.
    ///
    /// # Examples
    ///
    /// Port as `str`.
    ///
    /// ```
    /// # use http::uri::Authority;
    /// let authority: Authority = "example.org:80".parse().unwrap();
    ///
    /// let port = authority.port().unwrap();
    /// assert_eq!(port.as_str(), "80");
    /// ```
    pub fn as_str(&self) -> &str {
        self.repr.as_ref()
    }
}

impl<T> fmt::Debug for Port<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Port").field(&self.port).finish()
    }
}

impl<T> fmt::Display for Port<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use `u16::fmt` so that it respects any formatting flags that
        // may have been set (like padding, align, etc).
        fmt::Display::fmt(&self.port, f)
    }
}

impl<T> From<Port<T>> for u16 {
    fn from(port: Port<T>) -> Self {
        port.as_u16()
    }
}

impl<T> AsRef<str> for Port<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<T, U> PartialEq<Port<U>> for Port<T> {
    fn eq(&self, other: &Port<U>) -> bool {
        self.port == other.port
    }
}

impl<T> PartialEq<u16> for Port<T> {
    fn eq(&self, other: &u16) -> bool {
        self.port == *other
    }
}

impl<T> PartialEq<Port<T>> for u16 {
    fn eq(&self, other: &Port<T>) -> bool {
        other.port == *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partialeq_port() {
        let port_a = Port::from_str("8080").unwrap();
        let port_b = Port::from_str("8080").unwrap();
        assert_eq!(port_a, port_b);
    }

    #[test]
    fn partialeq_port_different_reprs() {
        let port_a = Port {
            repr: "8081",
            port: 8081,
        };
        let port_b = Port {
            repr: String::from("8081"),
            port: 8081,
        };
        assert_eq!(port_a, port_b);
        assert_eq!(port_b, port_a);
    }

    #[test]
    fn partialeq_u16() {
        let port = Port::from_str("8080").unwrap();
        // test equals in both directions
        assert_eq!(port, 8080);
        assert_eq!(8080, port);
    }

    #[test]
    fn u16_from_port() {
        let port = Port::from_str("8080").unwrap();
        assert_eq!(8080, u16::from(port));
    }
}
