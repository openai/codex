// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::{error, fmt};

/// An error that could occur interacting with the secret service dbus interface.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An error occurred decrypting a response message.
    /// The type of the error will depend on which crypto is being used.
    Crypto(Box<dyn (error::Error) + Send + Sync>),
    /// A bad path was handed to the secret service.
    Path(String),
    /// The response value of a secret service call couldn't be parsed.
    Parse,
    /// A call into the secret service provider failed.
    Dbus(dbus::Error),
    /// A secret service interface was locked and can't return any
    /// information about its contents.
    Locked,
    /// No object was found in the object for the request.
    NoResult,
    /// An authorization prompt was dismissed, but is required to continue.
    Prompt,
    /// A secret service provider, or a session to connect to one,
    /// was not found on the system.
    Unavailable,
    /// The provided secret was not text/plain
    UnsupportedSecretFormat,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Crypto(err) => write!(f, "Crypto error: {err}"),
            Error::Path(err) => write!(f, "DBus object path error: {err}"),
            Error::Dbus(err) => write!(f, "DBus error: {err}"),
            Error::Locked => f.write_str("Secret Service: object locked"),
            Error::NoResult => f.write_str("Secret Service: no result found"),
            Error::Prompt => f.write_str("Secret Service: unlock prompt was dismissed"),
            Error::Unavailable => f.write_str("No DBus session or Secret Service provider found"),
            Error::UnsupportedSecretFormat => f.write_str("Secrets must have MIME type text/plain"),
            _ => write!(f, "Unexpected Error: {self:?}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::Dbus(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<dbus::Error> for Error {
    fn from(err: dbus::Error) -> Error {
        Error::Dbus(err)
    }
}

impl From<String> for Error {
    // dbus parse errors return strings
    fn from(s: String) -> Error {
        Error::Path(s)
    }
}

#[cfg(feature = "crypto-rust")]
impl From<aes::cipher::block_padding::UnpadError> for Error {
    fn from(err: aes::cipher::block_padding::UnpadError) -> Error {
        Error::Crypto(Box::new(err))
    }
}

#[cfg(feature = "crypto-openssl")]
impl From<openssl::error::ErrorStack> for Error {
    fn from(err: openssl::error::ErrorStack) -> Error {
        Error::Crypto(Box::new(err))
    }
}

#[cfg(feature = "crypto-openssl")]
impl From<openssl::error::Error> for Error {
    fn from(err: openssl::error::Error) -> Error {
        Error::Crypto(Box::new(err))
    }
}
