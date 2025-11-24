//! Error types.

use core::fmt;

/// This error is returned by the [`StreamCipher`][crate::stream::StreamCipher]
/// trait methods.
///
/// Usually it's used in cases when stream cipher has reached the end
/// of a keystream, but also can be used if lengths of provided input
/// and output buffers are not equal.
#[derive(Copy, Clone, Debug)]
pub struct StreamCipherError;

impl fmt::Display for StreamCipherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Loop Error")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for StreamCipherError {}

/// The error type returned when a cipher position can not be represented
/// by the requested type.
#[derive(Copy, Clone, Debug)]
pub struct OverflowError;

impl fmt::Display for OverflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Overflow Error")
    }
}

impl From<OverflowError> for StreamCipherError {
    fn from(_: OverflowError) -> StreamCipherError {
        StreamCipherError
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for OverflowError {}
