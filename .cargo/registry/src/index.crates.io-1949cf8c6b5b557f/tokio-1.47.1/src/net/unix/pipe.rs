//! Unix pipe types.

use crate::io::interest::Interest;
use crate::io::{AsyncRead, AsyncWrite, PollEvented, ReadBuf, Ready};

use mio::unix::pipe as mio_pipe;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

cfg_io_util! {
    use bytes::BufMut;
}

/// Creates a new anonymous Unix pipe.
///
/// This function will open a new pipe and associate both pipe ends with the default
/// event loop.
///
/// If you need to create a pipe for communication with a spawned process, you can
/// use [`Stdio::piped()`] instead.
///
/// [`Stdio::piped()`]: std::process::Stdio::piped
///
/// # Errors
///
/// If creating a pipe fails, this function will return with the related OS error.
///
/// # Examples
///
/// Create a pipe and pass the writing end to a spawned process.
///
/// ```no_run
/// use tokio::net::unix::pipe;
/// use tokio::process::Command;
/// # use tokio::io::AsyncReadExt;
/// # use std::error::Error;
///
/// # async fn dox() -> Result<(), Box<dyn Error>> {
/// let (tx, mut rx) = pipe::pipe()?;
/// let mut buffer = String::new();
///
/// let status = Command::new("echo")
///     .arg("Hello, world!")
///     .stdout(tx.into_blocking_fd()?)
///     .status();
/// rx.read_to_string(&mut buffer).await?;
///
/// assert!(status.await?.success());
/// assert_eq!(buffer, "Hello, world!\n");
/// # Ok(())
/// # }
/// ```
///
/// # Panics
///
/// This function panics if it is not called from within a runtime with
/// IO enabled.
///
/// The runtime is usually set implicitly when this function is called
/// from a future driven by a tokio runtime, otherwise runtime can be set
/// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
pub fn pipe() -> io::Result<(Sender, Receiver)> {
    let (tx, rx) = mio_pipe::new()?;
    Ok((Sender::from_mio(tx)?, Receiver::from_mio(rx)?))
}

/// Options and flags which can be used to configure how a FIFO file is opened.
///
/// This builder allows configuring how to create a pipe end from a FIFO file.
/// Generally speaking, when using `OpenOptions`, you'll first call [`new`],
/// then chain calls to methods to set each option, then call either
/// [`open_receiver`] or [`open_sender`], passing the path of the FIFO file you
/// are trying to open. This will give you a [`io::Result`] with a pipe end
/// inside that you can further operate on.
///
/// [`new`]: OpenOptions::new
/// [`open_receiver`]: OpenOptions::open_receiver
/// [`open_sender`]: OpenOptions::open_sender
///
/// # Examples
///
/// Opening a pair of pipe ends from a FIFO file:
///
/// ```no_run
/// use tokio::net::unix::pipe;
/// # use std::error::Error;
///
/// const FIFO_NAME: &str = "path/to/a/fifo";
///
/// # async fn dox() -> Result<(), Box<dyn Error>> {
/// let rx = pipe::OpenOptions::new().open_receiver(FIFO_NAME)?;
/// let tx = pipe::OpenOptions::new().open_sender(FIFO_NAME)?;
/// # Ok(())
/// # }
/// ```
///
/// Opening a [`Sender`] on Linux when you are sure the file is a FIFO:
///
/// ```ignore
/// use tokio::net::unix::pipe;
/// use nix::{unistd::mkfifo, sys::stat::Mode};
/// # use std::error::Error;
///
/// // Our program has exclusive access to this path.
/// const FIFO_NAME: &str = "path/to/a/new/fifo";
///
/// # async fn dox() -> Result<(), Box<dyn Error>> {
/// mkfifo(FIFO_NAME, Mode::S_IRWXU)?;
/// let tx = pipe::OpenOptions::new()
///     .read_write(true)
///     .unchecked(true)
///     .open_sender(FIFO_NAME)?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct OpenOptions {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    read_write: bool,
    unchecked: bool,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    ///
    /// All options are initially set to `false`.
    pub fn new() -> OpenOptions {
        OpenOptions {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            read_write: false,
            unchecked: false,
        }
    }

    /// Sets the option for read-write access.
    ///
    /// This option, when true, will indicate that a FIFO file will be opened
    /// in read-write access mode. This operation is not defined by the POSIX
    /// standard and is only guaranteed to work on Linux.
    ///
    /// # Examples
    ///
    /// Opening a [`Sender`] even if there are no open reading ends:
    ///
    /// ```ignore
    /// use tokio::net::unix::pipe;
    ///
    /// let tx = pipe::OpenOptions::new()
    ///     .read_write(true)
    ///     .open_sender("path/to/a/fifo");
    /// ```
    ///
    /// Opening a resilient [`Receiver`] i.e. a reading pipe end which will not
    /// fail with [`UnexpectedEof`] during reading if all writing ends of the
    /// pipe close the FIFO file.
    ///
    /// [`UnexpectedEof`]: std::io::ErrorKind::UnexpectedEof
    ///
    /// ```ignore
    /// use tokio::net::unix::pipe;
    ///
    /// let tx = pipe::OpenOptions::new()
    ///     .read_write(true)
    ///     .open_receiver("path/to/a/fifo");
    /// ```
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "linux", target_os = "android"))))]
    pub fn read_write(&mut self, value: bool) -> &mut Self {
        self.read_write = value;
        self
    }

    /// Sets the option to skip the check for FIFO file type.
    ///
    /// By default, [`open_receiver`] and [`open_sender`] functions will check
    /// if the opened file is a FIFO file. Set this option to `true` if you are
    /// sure the file is a FIFO file.
    ///
    /// [`open_receiver`]: OpenOptions::open_receiver
    /// [`open_sender`]: OpenOptions::open_sender
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use nix::{unistd::mkfifo, sys::stat::Mode};
    /// # use std::error::Error;
    ///
    /// // Our program has exclusive access to this path.
    /// const FIFO_NAME: &str = "path/to/a/new/fifo";
    ///
    /// # async fn dox() -> Result<(), Box<dyn Error>> {
    /// mkfifo(FIFO_NAME, Mode::S_IRWXU)?;
    /// let rx = pipe::OpenOptions::new()
    ///     .unchecked(true)
    ///     .open_receiver(FIFO_NAME)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn unchecked(&mut self, value: bool) -> &mut Self {
        self.unchecked = value;
        self
    }

    /// Creates a [`Receiver`] from a FIFO file with the options specified by `self`.
    ///
    /// This function will open the FIFO file at the specified path, possibly
    /// check if it is a pipe, and associate the pipe with the default event
    /// loop for reading.
    ///
    /// # Errors
    ///
    /// If the file type check fails, this function will fail with `io::ErrorKind::InvalidInput`.
    /// This function may also fail with other standard OS errors.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn open_receiver<P: AsRef<Path>>(&self, path: P) -> io::Result<Receiver> {
        let file = self.open(path.as_ref(), PipeEnd::Receiver)?;
        Receiver::from_file_unchecked(file)
    }

    /// Creates a [`Sender`] from a FIFO file with the options specified by `self`.
    ///
    /// This function will open the FIFO file at the specified path, possibly
    /// check if it is a pipe, and associate the pipe with the default event
    /// loop for writing.
    ///
    /// # Errors
    ///
    /// If the file type check fails, this function will fail with `io::ErrorKind::InvalidInput`.
    /// If the file is not opened in read-write access mode and the file is not
    /// currently open for reading, this function will fail with `ENXIO`.
    /// This function may also fail with other standard OS errors.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn open_sender<P: AsRef<Path>>(&self, path: P) -> io::Result<Sender> {
        let file = self.open(path.as_ref(), PipeEnd::Sender)?;
        Sender::from_file_unchecked(file)
    }

    fn open(&self, path: &Path, pipe_end: PipeEnd) -> io::Result<File> {
        let mut options = std::fs::OpenOptions::new();
        options
            .read(pipe_end == PipeEnd::Receiver)
            .write(pipe_end == PipeEnd::Sender)
            .custom_flags(libc::O_NONBLOCK);

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if self.read_write {
            options.read(true).write(true);
        }

        let file = options.open(path)?;

        if !self.unchecked && !is_pipe(file.as_fd())? {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a pipe"));
        }

        Ok(file)
    }
}

impl Default for OpenOptions {
    fn default() -> OpenOptions {
        OpenOptions::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PipeEnd {
    Sender,
    Receiver,
}

/// Writing end of a Unix pipe.
///
/// It can be constructed from a FIFO file with [`OpenOptions::open_sender`].
///
/// Opening a named pipe for writing involves a few steps.
/// Call to [`OpenOptions::open_sender`] might fail with an error indicating
/// different things:
///
/// * [`io::ErrorKind::NotFound`] - There is no file at the specified path.
/// * [`io::ErrorKind::InvalidInput`] - The file exists, but it is not a FIFO.
/// * [`ENXIO`] - The file is a FIFO, but no process has it open for reading.
///   Sleep for a while and try again.
/// * Other OS errors not specific to opening FIFO files.
///
/// Opening a `Sender` from a FIFO file should look like this:
///
/// ```no_run
/// use tokio::net::unix::pipe;
/// use tokio::time::{self, Duration};
///
/// const FIFO_NAME: &str = "path/to/a/fifo";
///
/// # async fn dox() -> Result<(), Box<dyn std::error::Error>> {
/// // Wait for a reader to open the file.
/// let tx = loop {
///     match pipe::OpenOptions::new().open_sender(FIFO_NAME) {
///         Ok(tx) => break tx,
///         Err(e) if e.raw_os_error() == Some(libc::ENXIO) => {},
///         Err(e) => return Err(e.into()),
///     }
///
///     time::sleep(Duration::from_millis(50)).await;
/// };
/// # Ok(())
/// # }
/// ```
///
/// On Linux, it is possible to create a `Sender` without waiting in a sleeping
/// loop. This is done by opening a named pipe in read-write access mode with
/// `OpenOptions::read_write`. This way, a `Sender` can at the same time hold
/// both a writing end and a reading end, and the latter allows to open a FIFO
/// without [`ENXIO`] error since the pipe is open for reading as well.
///
/// `Sender` cannot be used to read from a pipe, so in practice the read access
/// is only used when a FIFO is opened. However, using a `Sender` in read-write
/// mode **may lead to lost data**, because written data will be dropped by the
/// system as soon as all pipe ends are closed. To avoid lost data you have to
/// make sure that a reading end has been opened before dropping a `Sender`.
///
/// Note that using read-write access mode with FIFO files is not defined by
/// the POSIX standard and it is only guaranteed to work on Linux.
///
/// ```ignore
/// use tokio::io::AsyncWriteExt;
/// use tokio::net::unix::pipe;
///
/// const FIFO_NAME: &str = "path/to/a/fifo";
///
/// # async fn dox() -> Result<(), Box<dyn std::error::Error>> {
/// let mut tx = pipe::OpenOptions::new()
///     .read_write(true)
///     .open_sender(FIFO_NAME)?;
///
/// // Asynchronously write to the pipe before a reader.
/// tx.write_all(b"hello world").await?;
/// # Ok(())
/// # }
/// ```
///
/// [`ENXIO`]: https://docs.rs/libc/latest/libc/constant.ENXIO.html
#[derive(Debug)]
pub struct Sender {
    io: PollEvented<mio_pipe::Sender>,
}

impl Sender {
    fn from_mio(mio_tx: mio_pipe::Sender) -> io::Result<Sender> {
        let io = PollEvented::new_with_interest(mio_tx, Interest::WRITABLE)?;
        Ok(Sender { io })
    }

    /// Creates a new `Sender` from a [`File`].
    ///
    /// This function is intended to construct a pipe from a [`File`] representing
    /// a special FIFO file. It will check if the file is a pipe and has write access,
    /// set it in non-blocking mode and perform the conversion.
    ///
    /// # Errors
    ///
    /// Fails with `io::ErrorKind::InvalidInput` if the file is not a pipe or it
    /// does not have write access. Also fails with any standard OS error if it occurs.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_file(file: File) -> io::Result<Sender> {
        Sender::from_owned_fd(file.into())
    }

    /// Creates a new `Sender` from an [`OwnedFd`].
    ///
    /// This function is intended to construct a pipe from an [`OwnedFd`] representing
    /// an anonymous pipe or a special FIFO file. It will check if the file descriptor
    /// is a pipe and has write access, set it in non-blocking mode and perform the
    /// conversion.
    ///
    /// # Errors
    ///
    /// Fails with `io::ErrorKind::InvalidInput` if the file descriptor is not a pipe
    /// or it does not have write access. Also fails with any standard OS error if it
    /// occurs.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_owned_fd(owned_fd: OwnedFd) -> io::Result<Sender> {
        if !is_pipe(owned_fd.as_fd())? {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a pipe"));
        }

        let flags = get_file_flags(owned_fd.as_fd())?;
        if has_write_access(flags) {
            set_nonblocking(owned_fd.as_fd(), flags)?;
            Sender::from_owned_fd_unchecked(owned_fd)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not in O_WRONLY or O_RDWR access mode",
            ))
        }
    }

    /// Creates a new `Sender` from a [`File`] without checking pipe properties.
    ///
    /// This function is intended to construct a pipe from a File representing
    /// a special FIFO file. The conversion assumes nothing about the underlying
    /// file; it is left up to the user to make sure it is opened with write access,
    /// represents a pipe and is set in non-blocking mode.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::fs::OpenOptions;
    /// use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
    /// # use std::error::Error;
    ///
    /// const FIFO_NAME: &str = "path/to/a/fifo";
    ///
    /// # async fn dox() -> Result<(), Box<dyn Error>> {
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .custom_flags(libc::O_NONBLOCK)
    ///     .open(FIFO_NAME)?;
    /// if file.metadata()?.file_type().is_fifo() {
    ///     let tx = pipe::Sender::from_file_unchecked(file)?;
    ///     /* use the Sender */
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_file_unchecked(file: File) -> io::Result<Sender> {
        Sender::from_owned_fd_unchecked(file.into())
    }

    /// Creates a new `Sender` from an [`OwnedFd`] without checking pipe properties.
    ///
    /// This function is intended to construct a pipe from an [`OwnedFd`] representing
    /// an anonymous pipe or a special FIFO file. The conversion assumes nothing about
    /// the underlying pipe; it is left up to the user to make sure that the file
    /// descriptor represents the writing end of a pipe and the pipe is set in
    /// non-blocking mode.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_owned_fd_unchecked(owned_fd: OwnedFd) -> io::Result<Sender> {
        // Safety: OwnedFd represents a valid, open file descriptor.
        let mio_tx = unsafe { mio_pipe::Sender::from_raw_fd(owned_fd.into_raw_fd()) };
        Sender::from_mio(mio_tx)
    }

    /// Waits for any of the requested ready states.
    ///
    /// This function can be used instead of [`writable()`] to check the returned
    /// ready set for [`Ready::WRITABLE`] and [`Ready::WRITE_CLOSED`] events.
    ///
    /// The function may complete without the pipe being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// [`writable()`]: Self::writable
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        let event = self.io.registration().readiness(interest).await?;
        Ok(event.ready)
    }

    /// Waits for the pipe to become writable.
    ///
    /// This function is equivalent to `ready(Interest::WRITABLE)` and is usually
    /// paired with [`try_write()`].
    ///
    /// [`try_write()`]: Self::try_write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Open a writing end of a fifo
    ///     let tx = pipe::OpenOptions::new().open_sender("path/to/a/fifo")?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         tx.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match tx.try_write(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn writable(&self) -> io::Result<()> {
        self.ready(Interest::WRITABLE).await?;
        Ok(())
    }

    /// Polls for write readiness.
    ///
    /// If the pipe is not currently ready for writing, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the pipe
    /// becomes ready for writing, `Waker::wake` will be called on the waker.
    ///
    /// Note that on multiple calls to `poll_write_ready` or `poll_write`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup.
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`writable`] is not feasible. Where possible, using [`writable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// [`writable`]: Self::writable
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the pipe is not ready for writing.
    /// * `Poll::Ready(Ok(()))` if the pipe is ready for writing.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    pub fn poll_write_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_write_ready(cx).map_ok(|_| ())
    }

    /// Tries to write a buffer to the pipe, returning how many bytes were
    /// written.
    ///
    /// The function will attempt to write the entire contents of `buf`, but
    /// only part of the buffer may be written. If the length of `buf` is not
    /// greater than `PIPE_BUF` (an OS constant, 4096 under Linux), then the
    /// write is guaranteed to be atomic, i.e. either the entire content of
    /// `buf` will be written or this method will fail with `WouldBlock`. There
    /// is no such guarantee if `buf` is larger than `PIPE_BUF`.
    ///
    /// This function is usually paired with [`writable`].
    ///
    /// [`writable`]: Self::writable
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the pipe is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Open a writing end of a fifo
    ///     let tx = pipe::OpenOptions::new().open_sender("path/to/a/fifo")?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         tx.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match tx.try_write(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || (&*self.io).write(buf))
    }

    /// Tries to write several buffers to the pipe, returning how many bytes
    /// were written.
    ///
    /// Data is written from each buffer in order, with the final buffer read
    /// from possible being only partially consumed. This method behaves
    /// equivalently to a single call to [`try_write()`] with concatenated
    /// buffers.
    ///
    /// If the total length of buffers is not greater than `PIPE_BUF` (an OS
    /// constant, 4096 under Linux), then the write is guaranteed to be atomic,
    /// i.e. either the entire contents of buffers will be written or this
    /// method will fail with `WouldBlock`. There is no such guarantee if the
    /// total length of buffers is greater than `PIPE_BUF`.
    ///
    /// This function is usually paired with [`writable`].
    ///
    /// [`try_write()`]: Self::try_write()
    /// [`writable`]: Self::writable
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the pipe is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Open a writing end of a fifo
    ///     let tx = pipe::OpenOptions::new().open_sender("path/to/a/fifo")?;
    ///
    ///     let bufs = [io::IoSlice::new(b"hello "), io::IoSlice::new(b"world")];
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         tx.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match tx.try_write_vectored(&bufs) {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_write_vectored(&self, buf: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || (&*self.io).write_vectored(buf))
    }

    /// Converts the pipe into an [`OwnedFd`] in blocking mode.
    ///
    /// This function will deregister this pipe end from the event loop, set
    /// it in blocking mode and perform the conversion.
    pub fn into_blocking_fd(self) -> io::Result<OwnedFd> {
        let fd = self.into_nonblocking_fd()?;
        set_blocking(&fd)?;
        Ok(fd)
    }

    /// Converts the pipe into an [`OwnedFd`] in nonblocking mode.
    ///
    /// This function will deregister this pipe end from the event loop and
    /// perform the conversion. The returned file descriptor will be in nonblocking
    /// mode.
    pub fn into_nonblocking_fd(self) -> io::Result<OwnedFd> {
        let mio_pipe = self.io.into_inner()?;

        // Safety: the pipe is now deregistered from the event loop
        // and we are the only owner of this pipe end.
        let owned_fd = unsafe { OwnedFd::from_raw_fd(mio_pipe.into_raw_fd()) };

        Ok(owned_fd)
    }
}

impl AsyncWrite for Sender {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsRawFd for Sender {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl AsFd for Sender {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

/// Reading end of a Unix pipe.
///
/// It can be constructed from a FIFO file with [`OpenOptions::open_receiver`].
///
/// # Examples
///
/// Receiving messages from a named pipe in a loop:
///
/// ```no_run
/// use tokio::net::unix::pipe;
/// use tokio::io::{self, AsyncReadExt};
///
/// const FIFO_NAME: &str = "path/to/a/fifo";
///
/// # async fn dox() -> Result<(), Box<dyn std::error::Error>> {
/// let mut rx = pipe::OpenOptions::new().open_receiver(FIFO_NAME)?;
/// loop {
///     let mut msg = vec![0; 256];
///     match rx.read_exact(&mut msg).await {
///         Ok(_) => {
///             /* handle the message */
///         }
///         Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
///             // Writing end has been closed, we should reopen the pipe.
///             rx = pipe::OpenOptions::new().open_receiver(FIFO_NAME)?;
///         }
///         Err(e) => return Err(e.into()),
///     }
/// }
/// # }
/// ```
///
/// On Linux, you can use a `Receiver` in read-write access mode to implement
/// resilient reading from a named pipe. Unlike `Receiver` opened in read-only
/// mode, read from a pipe in read-write mode will not fail with `UnexpectedEof`
/// when the writing end is closed. This way, a `Receiver` can asynchronously
/// wait for the next writer to open the pipe.
///
/// You should not use functions waiting for EOF such as [`read_to_end`] with
/// a `Receiver` in read-write access mode, since it **may wait forever**.
/// `Receiver` in this mode also holds an open writing end, which prevents
/// receiving EOF.
///
/// To set the read-write access mode you can use `OpenOptions::read_write`.
/// Note that using read-write access mode with FIFO files is not defined by
/// the POSIX standard and it is only guaranteed to work on Linux.
///
/// ```ignore
/// use tokio::net::unix::pipe;
/// use tokio::io::AsyncReadExt;
/// # use std::error::Error;
///
/// const FIFO_NAME: &str = "path/to/a/fifo";
///
/// # async fn dox() -> Result<(), Box<dyn Error>> {
/// let mut rx = pipe::OpenOptions::new()
///     .read_write(true)
///     .open_receiver(FIFO_NAME)?;
/// loop {
///     let mut msg = vec![0; 256];
///     rx.read_exact(&mut msg).await?;
///     /* handle the message */
/// }
/// # }
/// ```
///
/// [`read_to_end`]: crate::io::AsyncReadExt::read_to_end
#[derive(Debug)]
pub struct Receiver {
    io: PollEvented<mio_pipe::Receiver>,
}

impl Receiver {
    fn from_mio(mio_rx: mio_pipe::Receiver) -> io::Result<Receiver> {
        let io = PollEvented::new_with_interest(mio_rx, Interest::READABLE)?;
        Ok(Receiver { io })
    }

    /// Creates a new `Receiver` from a [`File`].
    ///
    /// This function is intended to construct a pipe from a [`File`] representing
    /// a special FIFO file. It will check if the file is a pipe and has read access,
    /// set it in non-blocking mode and perform the conversion.
    ///
    /// # Errors
    ///
    /// Fails with `io::ErrorKind::InvalidInput` if the file is not a pipe or it
    /// does not have read access. Also fails with any standard OS error if it occurs.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_file(file: File) -> io::Result<Receiver> {
        Receiver::from_owned_fd(file.into())
    }

    /// Creates a new `Receiver` from an [`OwnedFd`].
    ///
    /// This function is intended to construct a pipe from an [`OwnedFd`] representing
    /// an anonymous pipe or a special FIFO file. It will check if the file descriptor
    /// is a pipe and has read access, set it in non-blocking mode and perform the
    /// conversion.
    ///
    /// # Errors
    ///
    /// Fails with `io::ErrorKind::InvalidInput` if the file descriptor is not a pipe
    /// or it does not have read access. Also fails with any standard OS error if it
    /// occurs.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_owned_fd(owned_fd: OwnedFd) -> io::Result<Receiver> {
        if !is_pipe(owned_fd.as_fd())? {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a pipe"));
        }

        let flags = get_file_flags(owned_fd.as_fd())?;
        if has_read_access(flags) {
            set_nonblocking(owned_fd.as_fd(), flags)?;
            Receiver::from_owned_fd_unchecked(owned_fd)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not in O_RDONLY or O_RDWR access mode",
            ))
        }
    }

    /// Creates a new `Receiver` from a [`File`] without checking pipe properties.
    ///
    /// This function is intended to construct a pipe from a File representing
    /// a special FIFO file. The conversion assumes nothing about the underlying
    /// file; it is left up to the user to make sure it is opened with read access,
    /// represents a pipe and is set in non-blocking mode.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::fs::OpenOptions;
    /// use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
    /// # use std::error::Error;
    ///
    /// const FIFO_NAME: &str = "path/to/a/fifo";
    ///
    /// # async fn dox() -> Result<(), Box<dyn Error>> {
    /// let file = OpenOptions::new()
    ///     .read(true)
    ///     .custom_flags(libc::O_NONBLOCK)
    ///     .open(FIFO_NAME)?;
    /// if file.metadata()?.file_type().is_fifo() {
    ///     let rx = pipe::Receiver::from_file_unchecked(file)?;
    ///     /* use the Receiver */
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_file_unchecked(file: File) -> io::Result<Receiver> {
        Receiver::from_owned_fd_unchecked(file.into())
    }

    /// Creates a new `Receiver` from an [`OwnedFd`] without checking pipe properties.
    ///
    /// This function is intended to construct a pipe from an [`OwnedFd`] representing
    /// an anonymous pipe or a special FIFO file. The conversion assumes nothing about
    /// the underlying pipe; it is left up to the user to make sure that the file
    /// descriptor represents the reading end of a pipe and the pipe is set in
    /// non-blocking mode.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn from_owned_fd_unchecked(owned_fd: OwnedFd) -> io::Result<Receiver> {
        // Safety: OwnedFd represents a valid, open file descriptor.
        let mio_rx = unsafe { mio_pipe::Receiver::from_raw_fd(owned_fd.into_raw_fd()) };
        Receiver::from_mio(mio_rx)
    }

    /// Waits for any of the requested ready states.
    ///
    /// This function can be used instead of [`readable()`] to check the returned
    /// ready set for [`Ready::READABLE`] and [`Ready::READ_CLOSED`] events.
    ///
    /// The function may complete without the pipe being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// [`readable()`]: Self::readable
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        let event = self.io.registration().readiness(interest).await?;
        Ok(event.ready)
    }

    /// Waits for the pipe to become readable.
    ///
    /// This function is equivalent to `ready(Interest::READABLE)` and is usually
    /// paired with [`try_read()`].
    ///
    /// [`try_read()`]: Self::try_read()
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Open a reading end of a fifo
    ///     let rx = pipe::OpenOptions::new().open_receiver("path/to/a/fifo")?;
    ///
    ///     let mut msg = vec![0; 1024];
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         rx.readable().await?;
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match rx.try_read(&mut msg) {
    ///             Ok(n) => {
    ///                 msg.truncate(n);
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     println!("GOT = {:?}", msg);
    ///     Ok(())
    /// }
    /// ```
    pub async fn readable(&self) -> io::Result<()> {
        self.ready(Interest::READABLE).await?;
        Ok(())
    }

    /// Polls for read readiness.
    ///
    /// If the pipe is not currently ready for reading, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the pipe
    /// becomes ready for reading, `Waker::wake` will be called on the waker.
    ///
    /// Note that on multiple calls to `poll_read_ready` or `poll_read`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup.
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`readable`] is not feasible. Where possible, using [`readable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// [`readable`]: Self::readable
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the pipe is not ready for reading.
    /// * `Poll::Ready(Ok(()))` if the pipe is ready for reading.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    pub fn poll_read_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_read_ready(cx).map_ok(|_| ())
    }

    /// Tries to read data from the pipe into the provided buffer, returning how
    /// many bytes were read.
    ///
    /// Reads any pending data from the pipe but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read()` is non-blocking, the buffer does not have to be stored by
    /// the async task and can exist entirely on the stack.
    ///
    /// Usually [`readable()`] is used with this function.
    ///
    /// [`readable()`]: Self::readable()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. If `n` is `0`, then it can indicate one of two scenarios:
    ///
    /// 1. The pipe's writing end is closed and will no longer write data.
    /// 2. The specified buffer was 0 bytes in length.
    ///
    /// If the pipe is not ready to read data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Open a reading end of a fifo
    ///     let rx = pipe::OpenOptions::new().open_receiver("path/to/a/fifo")?;
    ///
    ///     let mut msg = vec![0; 1024];
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         rx.readable().await?;
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match rx.try_read(&mut msg) {
    ///             Ok(n) => {
    ///                 msg.truncate(n);
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     println!("GOT = {:?}", msg);
    ///     Ok(())
    /// }
    /// ```
    pub fn try_read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || (&*self.io).read(buf))
    }

    /// Tries to read data from the pipe into the provided buffers, returning
    /// how many bytes were read.
    ///
    /// Data is copied to fill each buffer in order, with the final buffer
    /// written to possibly being only partially filled. This method behaves
    /// equivalently to a single call to [`try_read()`] with concatenated
    /// buffers.
    ///
    /// Reads any pending data from the pipe but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read_vectored()` is non-blocking, the buffer does not have to be
    /// stored by the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] is used with this function.
    ///
    /// [`try_read()`]: Self::try_read()
    /// [`readable()`]: Self::readable()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. `Ok(0)` indicates the pipe's writing end is
    /// closed and will no longer write data. If the pipe is not ready to read
    /// data `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::unix::pipe;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Open a reading end of a fifo
    ///     let rx = pipe::OpenOptions::new().open_receiver("path/to/a/fifo")?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         rx.readable().await?;
    ///
    ///         // Creating the buffer **after** the `await` prevents it from
    ///         // being stored in the async task.
    ///         let mut buf_a = [0; 512];
    ///         let mut buf_b = [0; 1024];
    ///         let mut bufs = [
    ///             io::IoSliceMut::new(&mut buf_a),
    ///             io::IoSliceMut::new(&mut buf_b),
    ///         ];
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match rx.try_read_vectored(&mut bufs) {
    ///             Ok(0) => break,
    ///             Ok(n) => {
    ///                 println!("read {} bytes", n);
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_read_vectored(&self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || (&*self.io).read_vectored(bufs))
    }

    cfg_io_util! {
        /// Tries to read data from the pipe into the provided buffer, advancing the
        /// buffer's internal cursor, returning how many bytes were read.
        ///
        /// Reads any pending data from the pipe but does not wait for new data
        /// to arrive. On success, returns the number of bytes read. Because
        /// `try_read_buf()` is non-blocking, the buffer does not have to be stored by
        /// the async task and can exist entirely on the stack.
        ///
        /// Usually, [`readable()`] or [`ready()`] is used with this function.
        ///
        /// [`readable()`]: Self::readable
        /// [`ready()`]: Self::ready
        ///
        /// # Return
        ///
        /// If data is successfully read, `Ok(n)` is returned, where `n` is the
        /// number of bytes read. `Ok(0)` indicates the pipe's writing end is
        /// closed and will no longer write data. If the pipe is not ready to read
        /// data `Err(io::ErrorKind::WouldBlock)` is returned.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::net::unix::pipe;
        /// use std::io;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     // Open a reading end of a fifo
        ///     let rx = pipe::OpenOptions::new().open_receiver("path/to/a/fifo")?;
        ///
        ///     loop {
        ///         // Wait for the pipe to be readable
        ///         rx.readable().await?;
        ///
        ///         let mut buf = Vec::with_capacity(4096);
        ///
        ///         // Try to read data, this may still fail with `WouldBlock`
        ///         // if the readiness event is a false positive.
        ///         match rx.try_read_buf(&mut buf) {
        ///             Ok(0) => break,
        ///             Ok(n) => {
        ///                 println!("read {} bytes", n);
        ///             }
        ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        ///                 continue;
        ///             }
        ///             Err(e) => {
        ///                 return Err(e.into());
        ///             }
        ///         }
        ///     }
        ///
        ///     Ok(())
        /// }
        /// ```
        pub fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> io::Result<usize> {
            self.io.registration().try_io(Interest::READABLE, || {
                use std::io::Read;

                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: `mio_pipe::Receiver` uses a `std::fs::File` underneath,
                // which correctly handles reads into uninitialized memory.
                let n = (&*self.io).read(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }

                Ok(n)
            })
        }
    }

    /// Converts the pipe into an [`OwnedFd`] in blocking mode.
    ///
    /// This function will deregister this pipe end from the event loop, set
    /// it in blocking mode and perform the conversion.
    pub fn into_blocking_fd(self) -> io::Result<OwnedFd> {
        let fd = self.into_nonblocking_fd()?;
        set_blocking(&fd)?;
        Ok(fd)
    }

    /// Converts the pipe into an [`OwnedFd`] in nonblocking mode.
    ///
    /// This function will deregister this pipe end from the event loop and
    /// perform the conversion. Returned file descriptor will be in nonblocking
    /// mode.
    pub fn into_nonblocking_fd(self) -> io::Result<OwnedFd> {
        let mio_pipe = self.io.into_inner()?;

        // Safety: the pipe is now deregistered from the event loop
        // and we are the only owner of this pipe end.
        let owned_fd = unsafe { OwnedFd::from_raw_fd(mio_pipe.into_raw_fd()) };

        Ok(owned_fd)
    }
}

impl AsyncRead for Receiver {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Safety: `mio_pipe::Receiver` uses a `std::fs::File` underneath,
        // which correctly handles reads into uninitialized memory.
        unsafe { self.io.poll_read(cx, buf) }
    }
}

impl AsRawFd for Receiver {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl AsFd for Receiver {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

/// Checks if the file descriptor is a pipe or a FIFO.
fn is_pipe(fd: BorrowedFd<'_>) -> io::Result<bool> {
    // Safety: `libc::stat` is C-like struct used for syscalls and all-zero
    // byte pattern forms a valid value.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };

    // Safety: it's safe to call `fstat` with a valid, open file descriptor
    // and a valid pointer to a `stat` struct.
    let r = unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) };

    if r == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok((stat.st_mode as libc::mode_t & libc::S_IFMT) == libc::S_IFIFO)
    }
}

/// Gets file descriptor's flags by fcntl.
fn get_file_flags(fd: BorrowedFd<'_>) -> io::Result<libc::c_int> {
    // Safety: it's safe to use `fcntl` to read flags of a valid, open file descriptor.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(flags)
    }
}

/// Checks for `O_RDONLY` or `O_RDWR` access mode.
fn has_read_access(flags: libc::c_int) -> bool {
    let mode = flags & libc::O_ACCMODE;
    mode == libc::O_RDONLY || mode == libc::O_RDWR
}

/// Checks for `O_WRONLY` or `O_RDWR` access mode.
fn has_write_access(flags: libc::c_int) -> bool {
    let mode = flags & libc::O_ACCMODE;
    mode == libc::O_WRONLY || mode == libc::O_RDWR
}

/// Sets file descriptor's flags with `O_NONBLOCK` by fcntl.
fn set_nonblocking(fd: BorrowedFd<'_>, current_flags: libc::c_int) -> io::Result<()> {
    let flags = current_flags | libc::O_NONBLOCK;

    if flags != current_flags {
        // Safety: it's safe to use `fcntl` to set the `O_NONBLOCK` flag of a valid,
        // open file descriptor.
        let ret = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

/// Removes `O_NONBLOCK` from fd's flags.
fn set_blocking<T: AsRawFd>(fd: &T) -> io::Result<()> {
    // Safety: it's safe to use `fcntl` to read flags of a valid, open file descriptor.
    let previous = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if previous == -1 {
        return Err(io::Error::last_os_error());
    }

    let new = previous & !libc::O_NONBLOCK;

    // Safety: it's safe to use `fcntl` to unset the `O_NONBLOCK` flag of a valid,
    // open file descriptor.
    let r = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, new) };
    if r == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
