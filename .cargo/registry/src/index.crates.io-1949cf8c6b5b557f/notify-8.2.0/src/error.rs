//! Error types

use crate::Config;
use std::error::Error as StdError;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::{self, fmt, io};

/// Type alias to use this library's `Error` type in a Result
pub type Result<T> = StdResult<T, Error>;

/// Error kinds
#[derive(Debug)]
pub enum ErrorKind {
    /// Generic error
    ///
    /// May be used in cases where a platform specific error is mapped to this type, or for opaque
    /// internal errors.
    Generic(String),

    /// I/O errors.
    Io(io::Error),

    /// A path does not exist.
    PathNotFound,

    /// Attempted to remove a watch that does not exist.
    WatchNotFound,

    /// An invalid value was passed as runtime configuration.
    InvalidConfig(Config),

    /// Can't watch (more) files, limit on the total number of inotify watches reached
    MaxFilesWatch,
}

/// Notify error type.
///
/// Errors are emitted either at creation time of a `Watcher`, or during the event stream. They
/// range from kernel errors to filesystem errors to argument errors.
///
/// Errors can be general, or they can be about specific paths or subtrees. In that later case, the
/// error's `paths` field will be populated.
#[derive(Debug)]
pub struct Error {
    /// Kind of the error.
    pub kind: ErrorKind,

    /// Relevant paths to the error, if any.
    pub paths: Vec<PathBuf>,
}

impl Error {
    /// Adds a path to the error.
    pub fn add_path(mut self, path: PathBuf) -> Self {
        self.paths.push(path);
        self
    }

    /// Replaces the paths for the error.
    pub fn set_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = paths;
        self
    }

    /// Creates a new Error with empty paths given its kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            paths: Vec::new(),
        }
    }

    /// Creates a new generic Error from a message.
    pub fn generic(msg: &str) -> Self {
        Self::new(ErrorKind::Generic(msg.into()))
    }

    /// Creates a new i/o Error from a stdlib `io::Error`.
    pub fn io(err: io::Error) -> Self {
        Self::new(ErrorKind::Io(err))
    }

    /// Similar to [`Error::io`], but specifically handles [`io::ErrorKind::NotFound`].
    pub fn io_watch(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::NotFound {
            Self::path_not_found()
        } else {
            Self::io(err)
        }
    }

    /// Creates a new "path not found" error.
    pub fn path_not_found() -> Self {
        Self::new(ErrorKind::PathNotFound)
    }

    /// Creates a new "watch not found" error.
    pub fn watch_not_found() -> Self {
        Self::new(ErrorKind::WatchNotFound)
    }

    /// Creates a new "invalid config" error from the given `Config`.
    pub fn invalid_config(config: &Config) -> Self {
        Self::new(ErrorKind::InvalidConfig(*config))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let error = match self.kind {
            ErrorKind::PathNotFound => "No path was found.".into(),
            ErrorKind::WatchNotFound => "No watch was found.".into(),
            ErrorKind::InvalidConfig(ref config) => format!("Invalid configuration: {:?}", config),
            ErrorKind::Generic(ref err) => err.clone(),
            ErrorKind::Io(ref err) => err.to_string(),
            ErrorKind::MaxFilesWatch => "OS file watch limit reached.".into(),
        };

        if self.paths.is_empty() {
            write!(f, "{}", error)
        } else {
            write!(f, "{} about {:?}", error, self.paths)
        }
    }
}

impl StdError for Error {
    fn cause(&self) -> Option<&dyn StdError> {
        match self.kind {
            ErrorKind::Io(ref cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::io(err)
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for Error {
    fn from(err: std::sync::mpsc::SendError<T>) -> Self {
        Error::generic(&format!("internal channel disconnect: {:?}", err))
    }
}

impl From<std::sync::mpsc::RecvError> for Error {
    fn from(err: std::sync::mpsc::RecvError) -> Self {
        Error::generic(&format!("internal channel disconnect: {:?}", err))
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Error::generic(&format!("internal mutex poisoned: {:?}", err))
    }
}

#[test]
fn display_formatted_errors() {
    let expected = "Some error";

    assert_eq!(expected, format!("{}", Error::generic(expected)));

    assert_eq!(
        expected,
        format!("{}", Error::io(io::Error::other(expected)))
    );
}
