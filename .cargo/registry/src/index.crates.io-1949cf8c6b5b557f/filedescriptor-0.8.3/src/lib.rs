//! The purpose of this crate is to make it a bit more ergonomic for portable
//! applications that need to work with the platform level `RawFd` and
//! `RawHandle` types.
//!
//! Rather than conditionally using `RawFd` and `RawHandle`, the `FileDescriptor`
//! type can be used to manage ownership, duplicate, read and write.
//!
//! ## FileDescriptor
//!
//! This is a bit of a contrived example, but demonstrates how to avoid
//! the conditional code that would otherwise be required to deal with
//! calling `as_raw_fd` and `as_raw_handle`:
//!
//! ```
//! use filedescriptor::{FileDescriptor, FromRawFileDescriptor, Result};
//! use std::io::Write;
//!
//! fn get_stdout() -> Result<FileDescriptor> {
//!   let stdout = std::io::stdout();
//!   let handle = stdout.lock();
//!   FileDescriptor::dup(&handle)
//! }
//!
//! fn print_something() -> Result<()> {
//!    get_stdout()?.write(b"hello")?;
//!    Ok(())
//! }
//! ```
//!
//! ## Pipe
//! The `Pipe` type makes it more convenient to create a pipe and manage
//! the lifetime of both the read and write ends of that pipe.
//!
//! ```
//! use filedescriptor::{Pipe, Error};
//! use std::io::{Read, Write};
//!
//! let mut pipe = Pipe::new()?;
//! pipe.write.write(b"hello")?;
//! drop(pipe.write);
//!
//! let mut s = String::new();
//! pipe.read.read_to_string(&mut s)?;
//! assert_eq!(s, "hello");
//! # Ok::<(), Error>(())
//! ```
//!
//! ## Socketpair
//! The `socketpair` function returns a pair of connected `SOCK_STREAM`
//! sockets and functions both on posix and windows systems.
//!
//! ```
//! use std::io::{Read, Write};
//! use filedescriptor::Error;
//!
//! let (mut a, mut b) = filedescriptor::socketpair()?;
//! a.write(b"hello")?;
//! drop(a);
//!
//! let mut s = String::new();
//! b.read_to_string(&mut s)?;
//! assert_eq!(s, "hello");
//! # Ok::<(), Error>(())
//! ```
//!
//! ## Polling
//! The `mio` crate offers powerful and scalable IO multiplexing, but there
//! are some situations where `mio` doesn't fit.  The `filedescriptor` crate
//! offers a `poll(2)` compatible interface suitable for testing the readiness
//! of a set of file descriptors.  On unix systems this is a very thin wrapper
//! around `poll(2)`, except on macOS where it is actually a wrapper around
//! the `select(2)` interface.  On Windows systems the winsock `WSAPoll`
//! function is used instead.
//!
//! ```
//! use filedescriptor::*;
//! use std::time::Duration;
//! use std::io::{Read, Write};
//!
//! let (mut a, mut b) = filedescriptor::socketpair()?;
//! let mut poll_array = [pollfd {
//!    fd: a.as_socket_descriptor(),
//!    events: POLLIN,
//!    revents: 0
//! }];
//! // sleeps for 20 milliseconds because `a` is not yet ready
//! assert_eq!(poll(&mut poll_array, Some(Duration::from_millis(20)))?, 0);
//!
//! b.write(b"hello")?;
//!
//! // Now a is ready for read
//! assert_eq!(poll(&mut poll_array, Some(Duration::from_millis(20)))?, 1);
//!
//! # Ok::<(), Error>(())
//! ```

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use crate::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use crate::windows::*;

use thiserror::Error;
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to create a pipe")]
    Pipe(#[source] std::io::Error),
    #[error("failed to create a socketpair")]
    Socketpair(#[source] std::io::Error),
    #[error("failed to create a socket")]
    Socket(#[source] std::io::Error),
    #[error("failed to bind a socket")]
    Bind(#[source] std::io::Error),
    #[error("failed to fetch socket name")]
    Getsockname(#[source] std::io::Error),
    #[error("failed to set socket to listen mode")]
    Listen(#[source] std::io::Error),
    #[error("failed to connect socket")]
    Connect(#[source] std::io::Error),
    #[error("failed to accept socket")]
    Accept(#[source] std::io::Error),
    #[error("fcntl read failed")]
    Fcntl(#[source] std::io::Error),
    #[error("failed to set cloexec")]
    Cloexec(#[source] std::io::Error),
    #[error("failed to change non-blocking mode")]
    FionBio(#[source] std::io::Error),
    #[error("poll failed")]
    Poll(#[source] std::io::Error),
    #[error("dup of fd {fd} failed")]
    Dup { fd: i64, source: std::io::Error },
    #[error("dup of fd {src_fd} to fd {dest_fd} failed")]
    Dup2 {
        src_fd: i64,
        dest_fd: i64,
        source: std::io::Error,
    },
    #[error("Illegal fd value {0}")]
    IllegalFdValue(i64),
    #[error("fd value {0} too large to use with select(2)")]
    FdValueOutsideFdSetSize(i64),
    #[error("Only socket descriptors can change their non-blocking mode on Windows")]
    OnlySocketsNonBlocking,
    #[error("SetStdHandle failed")]
    SetStdHandle(#[source] std::io::Error),

    #[error("IoError")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// `AsRawFileDescriptor` is a platform independent trait for returning
/// a non-owning reference to the underlying platform file descriptor
/// type.
pub trait AsRawFileDescriptor {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor;
}

/// `IntoRawFileDescriptor` is a platform independent trait for converting
/// an instance into the underlying platform file descriptor type.
pub trait IntoRawFileDescriptor {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor;
}

/// `FromRawFileDescriptor` is a platform independent trait for creating
/// an instance from the underlying platform file descriptor type.
/// Because the platform file descriptor type has no inherent ownership
/// management, the `from_raw_file_descriptor` function is marked as unsafe
/// to indicate that care must be taken by the caller to ensure that it
/// is used appropriately.
pub trait FromRawFileDescriptor {
    unsafe fn from_raw_file_descriptor(fd: RawFileDescriptor) -> Self;
}

pub trait AsRawSocketDescriptor {
    fn as_socket_descriptor(&self) -> SocketDescriptor;
}
pub trait IntoRawSocketDescriptor {
    fn into_socket_descriptor(self) -> SocketDescriptor;
}
pub trait FromRawSocketDescriptor {
    unsafe fn from_socket_descriptor(fd: SocketDescriptor) -> Self;
}

/// `OwnedHandle` allows managing the lifetime of the platform `RawFileDescriptor`
/// type.  It is exposed in the interface of this crate primarily for convenience
/// on Windows where the system handle type is used for a variety of objects
/// that don't support reading and writing.
#[derive(Debug)]
pub struct OwnedHandle {
    handle: RawFileDescriptor,
    handle_type: HandleType,
}

impl OwnedHandle {
    /// Create a new handle from some object that is convertible into
    /// the system `RawFileDescriptor` type.  This consumes the parameter
    /// and replaces it with an `OwnedHandle` instance.
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let handle = f.into_raw_file_descriptor();
        Self {
            handle,
            handle_type: Self::probe_handle_type(handle),
        }
    }

    /// Attempt to duplicate the underlying handle and return an
    /// `OwnedHandle` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn try_clone(&self) -> Result<Self> {
        Self::dup_impl(self, self.handle_type)
    }

    /// Attempt to duplicate the underlying handle from an object that is
    /// representable as the system `RawFileDescriptor` type and return an
    /// `OwnedHandle` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Result<Self> {
        Self::dup_impl(f, Default::default())
    }
}

/// `FileDescriptor` is a thin wrapper on top of the `OwnedHandle` type that
/// exposes the ability to Read and Write to the platform `RawFileDescriptor`.
///
/// This is a bit of a contrived example, but demonstrates how to avoid
/// the conditional code that would otherwise be required to deal with
/// calling `as_raw_fd` and `as_raw_handle`:
///
/// ```
/// use filedescriptor::{FileDescriptor, FromRawFileDescriptor, Result};
/// use std::io::Write;
///
/// fn get_stdout() -> Result<FileDescriptor> {
///   let stdout = std::io::stdout();
///   let handle = stdout.lock();
///   FileDescriptor::dup(&handle)
/// }
///
/// fn print_something() -> Result<()> {
///    get_stdout()?.write(b"hello")?;
///    Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FileDescriptor {
    handle: OwnedHandle,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StdioDescriptor {
    Stdin,
    Stdout,
    Stderr,
}

impl FileDescriptor {
    /// Create a new descriptor from some object that is convertible into
    /// the system `RawFileDescriptor` type.  This consumes the parameter
    /// and replaces it with a `FileDescriptor` instance.
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let handle = OwnedHandle::new(f);
        Self { handle }
    }

    /// Attempt to duplicate the underlying handle from an object that is
    /// representable as the system `RawFileDescriptor` type and return a
    /// `FileDescriptor` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Result<Self> {
        OwnedHandle::dup(f).map(|handle| Self { handle })
    }

    /// Attempt to duplicate the underlying handle and return a
    /// `FileDescriptor` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn try_clone(&self) -> Result<Self> {
        self.handle.try_clone().map(|handle| Self { handle })
    }

    /// A convenience method for creating a `std::process::Stdio` object
    /// to be used for eg: redirecting the stdio streams of a child
    /// process.  The `Stdio` is created using a duplicated handle so
    /// that the source handle remains alive.
    pub fn as_stdio(&self) -> Result<std::process::Stdio> {
        self.as_stdio_impl()
    }

    /// A convenience method for creating a `std::fs::File` object.
    /// The `File` is created using a duplicated handle so
    /// that the source handle remains alive.
    pub fn as_file(&self) -> Result<std::fs::File> {
        self.as_file_impl()
    }

    /// Attempt to change the non-blocking IO mode of the file descriptor.
    /// Not all kinds of file descriptor can be placed in non-blocking mode
    /// on all systems, and some file descriptors will claim to be in
    /// non-blocking mode but it will have no effect.
    /// File descriptors based on sockets are the most portable type
    /// that can be successfully made non-blocking.
    pub fn set_non_blocking(&mut self, non_blocking: bool) -> Result<()> {
        self.set_non_blocking_impl(non_blocking)
    }

    /// Attempt to redirect stdio to the underlying handle and return
    /// a `FileDescriptor` wrapped around the original stdio source.
    /// Since the redirection requires kernel resources that may not be
    /// available, this is a potentially fallible operation.
    /// Supports stdin, stdout, and stderr redirections.
    pub fn redirect_stdio<F: AsRawFileDescriptor>(f: &F, stdio: StdioDescriptor) -> Result<Self> {
        Self::redirect_stdio_impl(f, stdio)
    }
}

/// Represents the readable and writable ends of a pair of descriptors
/// connected via a kernel pipe.
///
/// ```
/// use filedescriptor::{Pipe, Error};
/// use std::io::{Read,Write};
///
/// let mut pipe = Pipe::new()?;
/// pipe.write.write(b"hello")?;
/// drop(pipe.write);
///
/// let mut s = String::new();
/// pipe.read.read_to_string(&mut s)?;
/// assert_eq!(s, "hello");
/// # Ok::<(), Error>(())
/// ```
pub struct Pipe {
    /// The readable end of the pipe
    pub read: FileDescriptor,
    /// The writable end of the pipe
    pub write: FileDescriptor,
}

use std::time::Duration;

/// Examines a set of FileDescriptors to see if some of them are ready for I/O,
/// or if certain events have occurred on them.
///
/// This uses the system native readiness checking mechanism, which on Windows
/// means that it does NOT use IOCP and that this only works with sockets on
/// Windows.  If you need IOCP then the `mio` crate is recommended for a much
/// more scalable solution.
///
/// On macOS, the `poll(2)` implementation has problems when used with eg: pty
/// descriptors, so this implementation of poll uses the `select(2)` interface
/// under the covers.  That places a limit on the maximum file descriptor value
/// that can be passed to poll.  If a file descriptor is out of range then an
/// error will returned.  This limitation could potentially be lifted in the
/// future.
///
/// On Windows, `WSAPoll` is used to implement readiness checking, which has
/// the consequence that it can only be used with sockets.
///
/// If `duration` is `None`, then `poll` will block until any of the requested
/// events are ready.  Otherwise, `duration` specifies how long to wait for
/// readiness before giving up.
///
/// The return value is the number of entries that were satisfied; `0` means
/// that none were ready after waiting for the specified duration.
///
/// The `pfd` array is mutated and the `revents` field is updated to indicate
/// which of the events were received.
pub fn poll(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize> {
    poll_impl(pfd, duration)
}

/// Create a pair of connected sockets
///
/// This implementation creates a pair of SOCK_STREAM sockets.
pub fn socketpair() -> Result<(FileDescriptor, FileDescriptor)> {
    socketpair_impl()
}
