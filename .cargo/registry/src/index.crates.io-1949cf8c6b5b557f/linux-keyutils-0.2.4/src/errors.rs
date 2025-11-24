use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result;

#[cfg(feature = "std")]
use std::error::Error;

/// Error type for this library, optionally implements `std::error::Error`.
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyError {
    /// The keyring wasn't available for modification by the user.
    AccessDenied,

    /// The key quota for this user would be exceeded by creating
    /// this key or linking it to the keyring.
    QuotaExceeded,

    /// One or more of type, description, and payload points outside
    /// process's accessible address space.
    BadAddress,

    /// Provided bad arguments
    InvalidArguments,

    /// The keyring has expired.
    KeyExpired,

    /// The keyring has been revoked.
    KeyRevoked,

    /// The attempt to generate a new key was rejected.
    KeyRejected,

    /// The keyring doesn't exist.
    KeyringDoesNotExist,

    /// They key does not exist
    KeyDoesNotExist,

    /// Insufficient memory to create a key.
    OutOfMemory,

    /// Invalid Description
    InvalidDescription,

    /// An invalid identifier was returned
    InvalidIdentifier,

    /// Operation not supported
    OperationNotSupported,

    /// Write to destination failed
    WriteError,

    /// Unknown - catch all, return this instead of panicing
    Unknown(i32),
}

impl Display for KeyError {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter) -> Result {
        <KeyError as Debug>::fmt(self, f)
    }
}

#[cfg(feature = "std")]
impl Error for KeyError {}

impl KeyError {
    /// Obtain the KeyError derived from checking errno
    pub fn from_errno() -> KeyError {
        match unsafe { *libc::__errno_location() } {
            // Create Errors
            libc::EACCES => KeyError::AccessDenied,
            libc::EDQUOT => KeyError::QuotaExceeded,
            libc::EFAULT => KeyError::BadAddress,
            libc::EINVAL => KeyError::InvalidArguments,
            libc::EKEYEXPIRED => KeyError::KeyExpired,
            libc::EKEYREVOKED => KeyError::KeyRevoked,
            libc::EKEYREJECTED => KeyError::KeyRejected,
            libc::ENOMEM => KeyError::OutOfMemory,
            libc::ENOKEY => KeyError::KeyDoesNotExist,
            libc::ENOTSUP => KeyError::OperationNotSupported,

            // Unknown, provide error code for debugging
            x => KeyError::Unknown(x),
        }
    }
}
