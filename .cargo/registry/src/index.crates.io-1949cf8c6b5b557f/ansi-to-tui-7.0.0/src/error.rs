/// This enum stores the error types
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    /// Stack is empty (should never happen)
    #[error("Internal error: stack is empty")]
    NomError(String),

    /// Error parsing the input as utf-8
    #[cfg(feature = "simd")]
    /// Cannot determine the foreground or background
    #[error("{0:?}")]
    Utf8Error(#[from] simdutf8::basic::Utf8Error),

    #[cfg(not(feature = "simd"))]
    /// Cannot determine the foreground or background
    #[error("{0:?}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(e: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self::NomError(format!("{:?}", e))
    }
}
