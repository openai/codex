/*!

Platform-independent error model.

There is an escape hatch here for surfacing platform-specific
error information returned by the platform-specific storage provider,
but the concrete objects returned must be `Send` so they can be
moved from one thread to another. (Since most platform errors
are integer error codes, this requirement
is not much of a burden on the platform-specific store providers.)
 */

use crate::Credential;

#[derive(Debug)]
/// Each variant of the `Error` enum provides a summary of the error.
/// More details, if relevant, are contained in the associated value,
/// which may be platform-specific.
///
/// This enum is non-exhaustive so that more values can be added to it
/// without a SemVer break. Clients should always have default handling
/// for variants they don't understand.
#[non_exhaustive]
pub enum Error {
    /// This indicates runtime failure in the underlying
    /// platform storage system.  The details of the failure can
    /// be retrieved from the attached platform error.
    PlatformFailure(Box<dyn std::error::Error + Send + Sync>),
    /// This indicates that the underlying secure storage
    /// holding saved items could not be accessed.  Typically this
    /// is because of access rules in the platform; for example, it
    /// might be that the credential store is locked.  The underlying
    /// platform error will typically give the reason.
    NoStorageAccess(Box<dyn std::error::Error + Send + Sync>),
    /// This indicates that there is no underlying credential
    /// entry in the platform for this entry.  Either one was
    /// never set, or it was deleted.
    NoEntry,
    /// This indicates that the retrieved password blob was not
    /// a UTF-8 string.  The underlying bytes are available
    /// for examination in the attached value.
    BadEncoding(Vec<u8>),
    /// This indicates that one of the entry's credential
    /// attributes exceeded a
    /// length limit in the underlying platform.  The
    /// attached values give the name of the attribute and
    /// the platform length limit that was exceeded.
    TooLong(String, u32),
    /// This indicates that one of the entry's required credential
    /// attributes was invalid.  The
    /// attached value gives the name of the attribute
    /// and the reason it's invalid.
    Invalid(String, String),
    /// This indicates that there is more than one credential found in the store
    /// that matches the entry.  Its value is a vector of the matching credentials.
    Ambiguous(Vec<Box<Credential>>),
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::PlatformFailure(err) => write!(f, "Platform secure storage failure: {err}"),
            Error::NoStorageAccess(err) => {
                write!(f, "Couldn't access platform secure storage: {err}")
            }
            Error::NoEntry => write!(f, "No matching entry found in secure storage"),
            Error::BadEncoding(_) => write!(f, "Data is not UTF-8 encoded"),
            Error::TooLong(name, len) => write!(
                f,
                "Attribute '{name}' is longer than platform limit of {len} chars"
            ),
            Error::Invalid(attr, reason) => {
                write!(f, "Attribute {attr} is invalid: {reason}")
            }
            Error::Ambiguous(items) => {
                write!(
                    f,
                    "Entry is matched by {} credentials: {items:?}",
                    items.len(),
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::PlatformFailure(err) => Some(err.as_ref()),
            Error::NoStorageAccess(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

/// Try to interpret a byte vector as a password string
pub fn decode_password(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(|err| Error::BadEncoding(err.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_password() {
        // malformed sequences here taken from:
        // https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
        for bytes in [b"\x80".to_vec(), b"\xbf".to_vec(), b"\xed\xa0\xa0".to_vec()] {
            match decode_password(bytes.clone()) {
                Err(Error::BadEncoding(str)) => assert_eq!(str, bytes),
                Err(other) => panic!("Bad password ({bytes:?}) decode gave wrong error: {other}"),
                Ok(s) => panic!("Bad password ({bytes:?}) decode gave results: {s:?}"),
            }
        }
    }
}
