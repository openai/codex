/*!
The ignore crate provides a fast recursive directory iterator that respects
various filters such as globs, file types and `.gitignore` files. The precise
matching rules and precedence is explained in the documentation for
`WalkBuilder`.

Secondarily, this crate exposes gitignore and file type matchers for use cases
that demand more fine-grained control.

# Example

This example shows the most basic usage of this crate. This code will
recursively traverse the current directory while automatically filtering out
files and directories according to ignore globs found in files like
`.ignore` and `.gitignore`:


```rust,no_run
use ignore::Walk;

for result in Walk::new("./") {
    // Each item yielded by the iterator is either a directory entry or an
    // error, so either print the path or the error.
    match result {
        Ok(entry) => println!("{}", entry.path().display()),
        Err(err) => println!("ERROR: {}", err),
    }
}
```

# Example: advanced

By default, the recursive directory iterator will ignore hidden files and
directories. This can be disabled by building the iterator with `WalkBuilder`:

```rust,no_run
use ignore::WalkBuilder;

for result in WalkBuilder::new("./").hidden(false).build() {
    println!("{:?}", result);
}
```

See the documentation for `WalkBuilder` for many other options.
*/

#![deny(missing_docs)]

use std::path::{Path, PathBuf};

pub use crate::walk::{
    DirEntry, ParallelVisitor, ParallelVisitorBuilder, Walk, WalkBuilder,
    WalkParallel, WalkState,
};

mod default_types;
mod dir;
pub mod gitignore;
pub mod overrides;
mod pathutil;
pub mod types;
mod walk;

/// Represents an error that can occur when parsing a gitignore file.
#[derive(Debug)]
pub enum Error {
    /// A collection of "soft" errors. These occur when adding an ignore
    /// file partially succeeded.
    Partial(Vec<Error>),
    /// An error associated with a specific line number.
    WithLineNumber {
        /// The line number.
        line: u64,
        /// The underlying error.
        err: Box<Error>,
    },
    /// An error associated with a particular file path.
    WithPath {
        /// The file path.
        path: PathBuf,
        /// The underlying error.
        err: Box<Error>,
    },
    /// An error associated with a particular directory depth when recursively
    /// walking a directory.
    WithDepth {
        /// The directory depth.
        depth: usize,
        /// The underlying error.
        err: Box<Error>,
    },
    /// An error that occurs when a file loop is detected when traversing
    /// symbolic links.
    Loop {
        /// The ancestor file path in the loop.
        ancestor: PathBuf,
        /// The child file path in the loop.
        child: PathBuf,
    },
    /// An error that occurs when doing I/O, such as reading an ignore file.
    Io(std::io::Error),
    /// An error that occurs when trying to parse a glob.
    Glob {
        /// The original glob that caused this error. This glob, when
        /// available, always corresponds to the glob provided by an end user.
        /// e.g., It is the glob as written in a `.gitignore` file.
        ///
        /// (This glob may be distinct from the glob that is actually
        /// compiled, after accounting for `gitignore` semantics.)
        glob: Option<String>,
        /// The underlying glob error as a string.
        err: String,
    },
    /// A type selection for a file type that is not defined.
    UnrecognizedFileType(String),
    /// A user specified file type definition could not be parsed.
    InvalidDefinition,
}

impl Clone for Error {
    fn clone(&self) -> Error {
        match *self {
            Error::Partial(ref errs) => Error::Partial(errs.clone()),
            Error::WithLineNumber { line, ref err } => {
                Error::WithLineNumber { line, err: err.clone() }
            }
            Error::WithPath { ref path, ref err } => {
                Error::WithPath { path: path.clone(), err: err.clone() }
            }
            Error::WithDepth { depth, ref err } => {
                Error::WithDepth { depth, err: err.clone() }
            }
            Error::Loop { ref ancestor, ref child } => Error::Loop {
                ancestor: ancestor.clone(),
                child: child.clone(),
            },
            Error::Io(ref err) => match err.raw_os_error() {
                Some(e) => Error::Io(std::io::Error::from_raw_os_error(e)),
                None => {
                    Error::Io(std::io::Error::new(err.kind(), err.to_string()))
                }
            },
            Error::Glob { ref glob, ref err } => {
                Error::Glob { glob: glob.clone(), err: err.clone() }
            }
            Error::UnrecognizedFileType(ref err) => {
                Error::UnrecognizedFileType(err.clone())
            }
            Error::InvalidDefinition => Error::InvalidDefinition,
        }
    }
}

impl Error {
    /// Returns true if this is a partial error.
    ///
    /// A partial error occurs when only some operations failed while others
    /// may have succeeded. For example, an ignore file may contain an invalid
    /// glob among otherwise valid globs.
    pub fn is_partial(&self) -> bool {
        match *self {
            Error::Partial(_) => true,
            Error::WithLineNumber { ref err, .. } => err.is_partial(),
            Error::WithPath { ref err, .. } => err.is_partial(),
            Error::WithDepth { ref err, .. } => err.is_partial(),
            _ => false,
        }
    }

    /// Returns true if this error is exclusively an I/O error.
    pub fn is_io(&self) -> bool {
        match *self {
            Error::Partial(ref errs) => errs.len() == 1 && errs[0].is_io(),
            Error::WithLineNumber { ref err, .. } => err.is_io(),
            Error::WithPath { ref err, .. } => err.is_io(),
            Error::WithDepth { ref err, .. } => err.is_io(),
            Error::Loop { .. } => false,
            Error::Io(_) => true,
            Error::Glob { .. } => false,
            Error::UnrecognizedFileType(_) => false,
            Error::InvalidDefinition => false,
        }
    }

    /// Inspect the original [`std::io::Error`] if there is one.
    ///
    /// [`None`] is returned if the [`Error`] doesn't correspond to an
    /// [`std::io::Error`]. This might happen, for example, when the error was
    /// produced because a cycle was found in the directory tree while
    /// following symbolic links.
    ///
    /// This method returns a borrowed value that is bound to the lifetime of the [`Error`]. To
    /// obtain an owned value, the [`into_io_error`] can be used instead.
    ///
    /// > This is the original [`std::io::Error`] and is _not_ the same as
    /// > [`impl From<Error> for std::io::Error`][impl] which contains
    /// > additional context about the error.
    ///
    /// [`None`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html#variant.None
    /// [`std::io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    /// [`From`]: https://doc.rust-lang.org/stable/std/convert/trait.From.html
    /// [`Error`]: struct.Error.html
    /// [`into_io_error`]: struct.Error.html#method.into_io_error
    /// [impl]: struct.Error.html#impl-From%3CError%3E
    pub fn io_error(&self) -> Option<&std::io::Error> {
        match *self {
            Error::Partial(ref errs) => {
                if errs.len() == 1 {
                    errs[0].io_error()
                } else {
                    None
                }
            }
            Error::WithLineNumber { ref err, .. } => err.io_error(),
            Error::WithPath { ref err, .. } => err.io_error(),
            Error::WithDepth { ref err, .. } => err.io_error(),
            Error::Loop { .. } => None,
            Error::Io(ref err) => Some(err),
            Error::Glob { .. } => None,
            Error::UnrecognizedFileType(_) => None,
            Error::InvalidDefinition => None,
        }
    }

    /// Similar to [`io_error`] except consumes self to convert to the original
    /// [`std::io::Error`] if one exists.
    ///
    /// [`io_error`]: struct.Error.html#method.io_error
    /// [`std::io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    pub fn into_io_error(self) -> Option<std::io::Error> {
        match self {
            Error::Partial(mut errs) => {
                if errs.len() == 1 {
                    errs.remove(0).into_io_error()
                } else {
                    None
                }
            }
            Error::WithLineNumber { err, .. } => err.into_io_error(),
            Error::WithPath { err, .. } => err.into_io_error(),
            Error::WithDepth { err, .. } => err.into_io_error(),
            Error::Loop { .. } => None,
            Error::Io(err) => Some(err),
            Error::Glob { .. } => None,
            Error::UnrecognizedFileType(_) => None,
            Error::InvalidDefinition => None,
        }
    }

    /// Returns a depth associated with recursively walking a directory (if
    /// this error was generated from a recursive directory iterator).
    pub fn depth(&self) -> Option<usize> {
        match *self {
            Error::WithPath { ref err, .. } => err.depth(),
            Error::WithDepth { depth, .. } => Some(depth),
            _ => None,
        }
    }

    /// Turn an error into a tagged error with the given file path.
    fn with_path<P: AsRef<Path>>(self, path: P) -> Error {
        Error::WithPath {
            path: path.as_ref().to_path_buf(),
            err: Box::new(self),
        }
    }

    /// Turn an error into a tagged error with the given depth.
    fn with_depth(self, depth: usize) -> Error {
        Error::WithDepth { depth, err: Box::new(self) }
    }

    /// Turn an error into a tagged error with the given file path and line
    /// number. If path is empty, then it is omitted from the error.
    fn tagged<P: AsRef<Path>>(self, path: P, lineno: u64) -> Error {
        let errline =
            Error::WithLineNumber { line: lineno, err: Box::new(self) };
        if path.as_ref().as_os_str().is_empty() {
            return errline;
        }
        errline.with_path(path)
    }

    /// Build an error from a walkdir error.
    fn from_walkdir(err: walkdir::Error) -> Error {
        let depth = err.depth();
        if let (Some(anc), Some(child)) = (err.loop_ancestor(), err.path()) {
            return Error::WithDepth {
                depth,
                err: Box::new(Error::Loop {
                    ancestor: anc.to_path_buf(),
                    child: child.to_path_buf(),
                }),
            };
        }
        let path = err.path().map(|p| p.to_path_buf());
        let mut ig_err = Error::Io(std::io::Error::from(err));
        if let Some(path) = path {
            ig_err = Error::WithPath { path, err: Box::new(ig_err) };
        }
        ig_err
    }
}

impl std::error::Error for Error {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match *self {
            Error::Partial(_) => "partial error",
            Error::WithLineNumber { ref err, .. } => err.description(),
            Error::WithPath { ref err, .. } => err.description(),
            Error::WithDepth { ref err, .. } => err.description(),
            Error::Loop { .. } => "file system loop found",
            Error::Io(ref err) => err.description(),
            Error::Glob { ref err, .. } => err,
            Error::UnrecognizedFileType(_) => "unrecognized file type",
            Error::InvalidDefinition => "invalid definition",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Error::Partial(ref errs) => {
                let msgs: Vec<String> =
                    errs.iter().map(|err| err.to_string()).collect();
                write!(f, "{}", msgs.join("\n"))
            }
            Error::WithLineNumber { line, ref err } => {
                write!(f, "line {}: {}", line, err)
            }
            Error::WithPath { ref path, ref err } => {
                write!(f, "{}: {}", path.display(), err)
            }
            Error::WithDepth { ref err, .. } => err.fmt(f),
            Error::Loop { ref ancestor, ref child } => write!(
                f,
                "File system loop found: \
                           {} points to an ancestor {}",
                child.display(),
                ancestor.display()
            ),
            Error::Io(ref err) => err.fmt(f),
            Error::Glob { glob: None, ref err } => write!(f, "{}", err),
            Error::Glob { glob: Some(ref glob), ref err } => {
                write!(f, "error parsing glob '{}': {}", glob, err)
            }
            Error::UnrecognizedFileType(ref ty) => {
                write!(f, "unrecognized file type: {}", ty)
            }
            Error::InvalidDefinition => write!(
                f,
                "invalid definition (format is type:glob, e.g., \
                           html:*.html)"
            ),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

#[derive(Debug, Default)]
struct PartialErrorBuilder(Vec<Error>);

impl PartialErrorBuilder {
    fn push(&mut self, err: Error) {
        self.0.push(err);
    }

    fn push_ignore_io(&mut self, err: Error) {
        if !err.is_io() {
            self.push(err);
        }
    }

    fn maybe_push(&mut self, err: Option<Error>) {
        if let Some(err) = err {
            self.push(err);
        }
    }

    fn maybe_push_ignore_io(&mut self, err: Option<Error>) {
        if let Some(err) = err {
            self.push_ignore_io(err);
        }
    }

    fn into_error_option(mut self) -> Option<Error> {
        if self.0.is_empty() {
            None
        } else if self.0.len() == 1 {
            Some(self.0.pop().unwrap())
        } else {
            Some(Error::Partial(self.0))
        }
    }
}

/// The result of a glob match.
///
/// The type parameter `T` typically refers to a type that provides more
/// information about a particular match. For example, it might identify
/// the specific gitignore file and the specific glob pattern that caused
/// the match.
#[derive(Clone, Debug)]
pub enum Match<T> {
    /// The path didn't match any glob.
    None,
    /// The highest precedent glob matched indicates the path should be
    /// ignored.
    Ignore(T),
    /// The highest precedent glob matched indicates the path should be
    /// whitelisted.
    Whitelist(T),
}

impl<T> Match<T> {
    /// Returns true if the match result didn't match any globs.
    pub fn is_none(&self) -> bool {
        match *self {
            Match::None => true,
            Match::Ignore(_) | Match::Whitelist(_) => false,
        }
    }

    /// Returns true if the match result implies the path should be ignored.
    pub fn is_ignore(&self) -> bool {
        match *self {
            Match::Ignore(_) => true,
            Match::None | Match::Whitelist(_) => false,
        }
    }

    /// Returns true if the match result implies the path should be
    /// whitelisted.
    pub fn is_whitelist(&self) -> bool {
        match *self {
            Match::Whitelist(_) => true,
            Match::None | Match::Ignore(_) => false,
        }
    }

    /// Inverts the match so that `Ignore` becomes `Whitelist` and
    /// `Whitelist` becomes `Ignore`. A non-match remains the same.
    pub fn invert(self) -> Match<T> {
        match self {
            Match::None => Match::None,
            Match::Ignore(t) => Match::Whitelist(t),
            Match::Whitelist(t) => Match::Ignore(t),
        }
    }

    /// Return the value inside this match if it exists.
    pub fn inner(&self) -> Option<&T> {
        match *self {
            Match::None => None,
            Match::Ignore(ref t) => Some(t),
            Match::Whitelist(ref t) => Some(t),
        }
    }

    /// Apply the given function to the value inside this match.
    ///
    /// If the match has no value, then return the match unchanged.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Match<U> {
        match self {
            Match::None => Match::None,
            Match::Ignore(t) => Match::Ignore(f(t)),
            Match::Whitelist(t) => Match::Whitelist(f(t)),
        }
    }

    /// Return the match if it is not none. Otherwise, return other.
    pub fn or(self, other: Self) -> Self {
        if self.is_none() {
            other
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };

    /// A convenient result type alias.
    pub(crate) type Result<T> =
        std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    macro_rules! err {
        ($($tt:tt)*) => {
            Box::<dyn std::error::Error + Send + Sync>::from(format!($($tt)*))
        }
    }

    /// A simple wrapper for creating a temporary directory that is
    /// automatically deleted when it's dropped.
    ///
    /// We use this in lieu of tempfile because tempfile brings in too many
    /// dependencies.
    #[derive(Debug)]
    pub struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    impl TempDir {
        /// Create a new empty temporary directory under the system's configured
        /// temporary directory.
        pub fn new() -> Result<TempDir> {
            use std::sync::atomic::{AtomicUsize, Ordering};

            static TRIES: usize = 100;
            static COUNTER: AtomicUsize = AtomicUsize::new(0);

            let tmpdir = env::temp_dir();
            for _ in 0..TRIES {
                let count = COUNTER.fetch_add(1, Ordering::SeqCst);
                let path = tmpdir.join("rust-ignore").join(count.to_string());
                if path.is_dir() {
                    continue;
                }
                fs::create_dir_all(&path).map_err(|e| {
                    err!("failed to create {}: {}", path.display(), e)
                })?;
                return Ok(TempDir(path));
            }
            Err(err!("failed to create temp dir after {} tries", TRIES))
        }

        /// Return the underlying path to this temporary directory.
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
}
