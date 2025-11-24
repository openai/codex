use std::{fmt, str::FromStr};

use crate::{Error, Result};

/// Authentication mechanisms
///
/// See <https://dbus.freedesktop.org/doc/dbus-specification.html#auth-mechanisms>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMechanism {
    /// This is the recommended authentication mechanism on platforms where credentials can be
    /// transferred out-of-band, in particular Unix platforms that can perform credentials-passing
    /// over the `unix:` transport.
    External,

    /// This mechanism is designed to establish that a client has the ability to read a private
    /// file owned by the user being authenticated.
    Cookie,

    /// Does not perform any authentication at all, and should not be accepted by message buses.
    /// However, it might sometimes be useful for non-message-bus uses of D-Bus.
    Anonymous,
}

impl fmt::Display for AuthMechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mech = match self {
            AuthMechanism::External => "EXTERNAL",
            AuthMechanism::Cookie => "DBUS_COOKIE_SHA1",
            AuthMechanism::Anonymous => "ANONYMOUS",
        };
        write!(f, "{mech}")
    }
}

impl FromStr for AuthMechanism {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "EXTERNAL" => Ok(AuthMechanism::External),
            "DBUS_COOKIE_SHA1" => Ok(AuthMechanism::Cookie),
            "ANONYMOUS" => Ok(AuthMechanism::Anonymous),
            _ => Err(Error::Handshake(format!("Unknown mechanism: {s}"))),
        }
    }
}
