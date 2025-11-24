use std::error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::DirEntry;

/// An error produced by recursively walking a directory.
///
/// This error type is a light wrapper around [`std::io::Error`]. In
/// particular, it adds the following information:
///
/// * The depth at which the error occurred in the file tree, relative to the
/// root.
/// * The path, if any, associated with the IO error.
/// * An indication that a loop occurred when following symbolic links. In this
/// case, there is no underlying IO error.
///
/// To maintain good ergonomics, this type has a
/// [`impl From<Error> for std::io::Error`][impl] defined which preserves the original context.
/// This allows you to use an [`io::Result`] with methods in this crate if you don't care about
/// accessing the underlying error data in a structured form.
///
/// [`std::io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
/// [`io::Result`]: https://doc.rust-lang.org/stable/std/io/type.Result.html
/// [impl]: struct.Error.html#impl-From%3CError%3E
#[derive(Debug)]
pub struct Error {
    depth: usize,
    inner: ErrorInner,
}

#[derive(Debug)]
enum ErrorInner {
    Io { path: Option<PathBuf>, err: io::Error },
    Loop { ancestor: PathBuf, child: PathBuf },
}

impl Error {
    /// Returns the path associated with this error if one exists.
    ///
    /// For example, if an error occurred while opening a directory handle,
    /// the error will include the path passed to [`std::fs::read_dir`].
    ///
    /// [`std::fs::read_dir`]: https://doc.rust-lang.org/stable/std/fs/fn.read_dir.html
    pub fn path(&self) -> Option<&Path> {
        match self.inner {
            ErrorInner::Io { path: None, .. } => None,
            ErrorInner::Io { path: Some(ref path), .. } => Some(path),
            ErrorInner::Loop { ref child, .. } => Some(child),
        }
    }

    /// Returns the path at which a cycle was detected.
    ///
    /// If no cycle was detected, [`None`] is returned.
    ///
    /// A cycle is detected when a directory entry is equivalent to one of
    /// its ancestors.
    ///
    /// To get the path to the child directory entry in the cycle, use the
    /// [`path`] method.
    ///
    /// [`None`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html#variant.None
    /// [`path`]: struct.Error.html#path
    pub fn loop_ancestor(&self) -> Option<&Path> {
        match self.inner {
            ErrorInner::Loop { ref ancestor, .. } => Some(ancestor),
            _ => None,
        }
    }

    /// Returns the depth at which this error occurred relative to the root.
    ///
    /// The smallest depth is `0` and always corresponds to the path given to
    /// the [`new`] function on [`WalkDir`]. Its direct descendents have depth
    /// `1`, and their descendents have depth `2`, and so on.
    ///
    /// [`new`]: struct.WalkDir.html#method.new
    /// [`WalkDir`]: struct.WalkDir.html
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Inspect the original [`io::Error`] if there is one.
    ///
    /// [`None`] is returned if the [`Error`] doesn't correspond to an
    /// [`io::Error`]. This might happen, for example, when the error was
    /// produced because a cycle was found in the directory tree while
    /// following symbolic links.
    ///
    /// This method returns a borrowed value that is bound to the lifetime of the [`Error`]. To
    /// obtain an owned value, the [`into_io_error`] can be used instead.
    ///
    /// > This is the original [`io::Error`] and is _not_ the same as
    /// > [`impl From<Error> for std::io::Error`][impl] which contains additional context about the
    /// error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io;
    /// use std::path::Path;
    ///
    /// use walkdir::WalkDir;
    ///
    /// for entry in WalkDir::new("foo") {
    ///     match entry {
    ///         Ok(entry) => println!("{}", entry.path().display()),
    ///         Err(err) => {
    ///             let path = err.path().unwrap_or(Path::new("")).display();
    ///             println!("failed to access entry {}", path);
    ///             if let Some(inner) = err.io_error() {
    ///                 match inner.kind() {
    ///                     io::ErrorKind::InvalidData => {
    ///                         println!(
    ///                             "entry contains invalid data: {}",
    ///                             inner)
    ///                     }
    ///                     io::ErrorKind::PermissionDenied => {
    ///                         println!(
    ///                             "Missing permission to read entry: {}",
    ///                             inner)
    ///                     }
    ///                     _ => {
    ///                         println!(
    ///                             "Unexpected error occurred: {}",
    ///                             inner)
    ///                     }
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// [`None`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html#variant.None
    /// [`io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    /// [`From`]: https://doc.rust-lang.org/stable/std/convert/trait.From.html
    /// [`Error`]: struct.Error.html
    /// [`into_io_error`]: struct.Error.html#method.into_io_error
    /// [impl]: struct.Error.html#impl-From%3CError%3E
    pub fn io_error(&self) -> Option<&io::Error> {
        match self.inner {
            ErrorInner::Io { ref err, .. } => Some(err),
            ErrorInner::Loop { .. } => None,
        }
    }

    /// Similar to [`io_error`] except consumes self to convert to the original
    /// [`io::Error`] if one exists.
    ///
    /// [`io_error`]: struct.Error.html#method.io_error
    /// [`io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    pub fn into_io_error(self) -> Option<io::Error> {
        match self.inner {
            ErrorInner::Io { err, .. } => Some(err),
            ErrorInner::Loop { .. } => None,
        }
    }

    pub(crate) fn from_path(
        depth: usize,
        pb: PathBuf,
        err: io::Error,
    ) -> Self {
        Error { depth, inner: ErrorInner::Io { path: Some(pb), err } }
    }

    pub(crate) fn from_entry(dent: &DirEntry, err: io::Error) -> Self {
        Error {
            depth: dent.depth(),
            inner: ErrorInner::Io {
                path: Some(dent.path().to_path_buf()),
                err,
            },
        }
    }

    pub(crate) fn from_io(depth: usize, err: io::Error) -> Self {
        Error { depth, inner: ErrorInner::Io { path: None, err } }
    }

    pub(crate) fn from_loop(
        depth: usize,
        ancestor: &Path,
        child: &Path,
    ) -> Self {
        Error {
            depth,
            inner: ErrorInner::Loop {
                ancestor: ancestor.to_path_buf(),
                child: child.to_path_buf(),
            },
        }
    }
}

impl error::Error for Error {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match self.inner {
            ErrorInner::Io { ref err, .. } => err.description(),
            ErrorInner::Loop { .. } => "file system loop found",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        self.source()
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.inner {
            ErrorInner::Io { ref err, .. } => Some(err),
            ErrorInner::Loop { .. } => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            ErrorInner::Io { path: None, ref err } => err.fmt(f),
            ErrorInner::Io { path: Some(ref path), ref err } => write!(
                f,
                "IO error for operation on {}: {}",
                path.display(),
                err
            ),
            ErrorInner::Loop { ref ancestor, ref child } => write!(
                f,
                "File system loop found: \
                 {} points to an ancestor {}",
                child.display(),
                ancestor.display()
            ),
        }
    }
}

impl From<Error> for io::Error {
    /// Convert the [`Error`] to an [`io::Error`], preserving the original
    /// [`Error`] as the ["inner error"]. Note that this also makes the display
    /// of the error include the context.
    ///
    /// This is different from [`into_io_error`] which returns the original
    /// [`io::Error`].
    ///
    /// [`Error`]: struct.Error.html
    /// [`io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    /// ["inner error"]: https://doc.rust-lang.org/std/io/struct.Error.html#method.into_inner
    /// [`into_io_error`]: struct.WalkDir.html#method.into_io_error
    fn from(walk_err: Error) -> io::Error {
        let kind = match walk_err {
            Error { inner: ErrorInner::Io { ref err, .. }, .. } => err.kind(),
            Error { inner: ErrorInner::Loop { .. }, .. } => {
                io::ErrorKind::Other
            }
        };
        io::Error::new(kind, walk_err)
    }
}
