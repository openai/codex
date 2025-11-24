// Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::{error, fmt};
use zbus::zvariant;

/// An error that could occur interacting with the secret service dbus interface.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An error occured decrypting a response message.
    Crypto(&'static str),
    /// A call into the secret service provider failed.
    Zbus(zbus::Error),
    /// A call into a standard dbus interface failed.
    ZbusFdo(zbus::fdo::Error),
    /// Serializing or deserializing a dbus message failed.
    Zvariant(zvariant::Error),
    /// A secret service interface was locked and can't return any
    /// information about its contents.
    Locked,
    /// No object was found in the object for the request.
    NoResult,
    /// An authorization prompt was dismissed, but is required to continue.
    Prompt,
    /// A secret service provider, or a session to connect to one, was found
    /// on the system.
    Unavailable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Crypto(err) => write!(f, "Crypto error: {err}"),
            Error::Zbus(err) => write!(f, "zbus error: {err}"),
            Error::ZbusFdo(err) => write!(f, "zbus fdo error: {err}"),
            Error::Zvariant(err) => write!(f, "zbus serde error: {err}"),
            Error::Locked => f.write_str("SS Error: object locked"),
            Error::NoResult => f.write_str("SS error: result not returned from SS API"),
            Error::Prompt => f.write_str("SS error: prompt dismissed"),
            Error::Unavailable => f.write_str("no secret service provider or dbus session found"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::Zbus(ref err) => Some(err),
            Error::ZbusFdo(ref err) => Some(err),
            Error::Zvariant(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<zbus::Error> for Error {
    fn from(err: zbus::Error) -> Error {
        Error::Zbus(err)
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(err: zbus::fdo::Error) -> Error {
        Error::ZbusFdo(err)
    }
}

impl From<zvariant::Error> for Error {
    fn from(err: zvariant::Error) -> Error {
        Error::Zvariant(err)
    }
}
