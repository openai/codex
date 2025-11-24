use std::borrow::Cow;
use std::ffi::{OsStr, OsString};

use crate::types::{Dsn, ParseDsnError};

/// Helper trait to convert a string into an `Option<Dsn>`.
///
/// This converts a value into a DSN by parsing.  The empty string or
/// null values result in no DSN being parsed.
pub trait IntoDsn {
    /// Converts the value into a `Result<Option<Dsn>, E>`.
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError>;
}

impl<I: IntoDsn> IntoDsn for Option<I> {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        match self {
            Some(into_dsn) => into_dsn.into_dsn(),
            None => Ok(None),
        }
    }
}

impl IntoDsn for () {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        Ok(None)
    }
}

impl<'a> IntoDsn for &'a str {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        if self.is_empty() {
            Ok(None)
        } else {
            self.parse().map(Some)
        }
    }
}

impl<'a> IntoDsn for Cow<'a, str> {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        let x: &str = &self;
        x.into_dsn()
    }
}

impl<'a> IntoDsn for &'a OsStr {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        self.to_string_lossy().into_dsn()
    }
}

impl IntoDsn for OsString {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        self.as_os_str().into_dsn()
    }
}

impl IntoDsn for String {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        self.as_str().into_dsn()
    }
}

impl<'a> IntoDsn for &'a Dsn {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        Ok(Some(self.clone()))
    }
}

impl IntoDsn for Dsn {
    fn into_dsn(self) -> Result<Option<Dsn>, ParseDsnError> {
        Ok(Some(self))
    }
}
