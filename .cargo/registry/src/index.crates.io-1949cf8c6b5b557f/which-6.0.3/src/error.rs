use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    /// An executable binary with that name was not found
    CannotFindBinaryPath,
    /// There was nowhere to search and the provided name wasn't an absolute path
    CannotGetCurrentDirAndPathListEmpty,
    /// Failed to canonicalize the path found
    CannotCanonicalize,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::CannotFindBinaryPath => write!(f, "cannot find binary path"),
            Error::CannotGetCurrentDirAndPathListEmpty => write!(
                f,
                "no path to search and provided name is not an absolute path"
            ),
            Error::CannotCanonicalize => write!(f, "cannot canonicalize path"),
        }
    }
}
