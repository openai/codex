use core::fmt;

/// The error returned when slice can not be converted into array.
#[derive(Copy, Clone, Debug)]
pub struct IntoArrayError;

impl fmt::Display for IntoArrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Failed to convert into array.")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for IntoArrayError {}

/// The error returned when input and output slices have different length
/// and thus can not be converted to `InOutBuf`.
#[derive(Copy, Clone, Debug)]
pub struct NotEqualError;

impl fmt::Display for NotEqualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Length of input slices is not equal to each other")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for NotEqualError {}

/// Padding error. Usually emitted when size of output buffer is insufficient.
#[cfg(feature = "block-padding")]
#[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
#[derive(Clone, Copy, Debug)]
pub struct PadError;

#[cfg(feature = "block-padding")]
impl fmt::Display for PadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Padding error")
    }
}

#[cfg(feature = "block-padding")]
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for PadError {}

/// Output buffer is smaller than input buffer.
#[derive(Clone, Copy, Debug)]
pub struct OutIsTooSmallError;

impl fmt::Display for OutIsTooSmallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Output buffer is smaller than input")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for OutIsTooSmallError {}
