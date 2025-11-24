//! Unix pipe.
//!
//! See the [`new`] function for documentation.

use std::io;
use std::os::fd::RawFd;

pub(crate) fn new_raw() -> io::Result<[RawFd; 2]> {
    let mut fds: [RawFd; 2] = [-1, -1];

    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "illumos",
        target_os = "redox",
        target_os = "solaris",
        target_os = "vita",
    ))]
    unsafe {
        if libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    #[cfg(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "nto",
    ))]
    unsafe {
        // For platforms that don't have `pipe2(2)` we need to manually set the
        // correct flags on the file descriptor.
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }

        for fd in &fds {
            if libc::fcntl(*fd, libc::F_SETFL, libc::O_NONBLOCK) != 0
                || libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC) != 0
            {
                let err = io::Error::last_os_error();
                // Don't leak file descriptors. Can't handle closing error though.
                let _ = libc::close(fds[0]);
                let _ = libc::close(fds[1]);
                return Err(err);
            }
        }
    }

    Ok(fds)
}

cfg_os_ext! {
use std::fs::File;
use std::io::{IoSlice, IoSliceMut, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
use std::process::{ChildStderr, ChildStdin, ChildStdout};

use crate::io_source::IoSource;
use crate::{event, Interest, Registry, Token};

/// Create a new non-blocking Unix pipe.
///
/// This is a wrapper around Unix's [`pipe(2)`] system call and can be used as
/// inter-process or thread communication channel.
///
/// This channel may be created before forking the process and then one end used
/// in each process, e.g. the parent process has the sending end to send command
/// to the child process.
///
/// [`pipe(2)`]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/pipe.html
///
/// # Events
///
/// The [`Sender`] can be registered with [`WRITABLE`] interest to receive
/// [writable events], the [`Receiver`] with [`READABLE`] interest. Once data is
/// written to the `Sender` the `Receiver` will receive an [readable event].
///
/// In addition to those events, events will also be generated if the other side
/// is dropped. To check if the `Sender` is dropped you'll need to check
/// [`is_read_closed`] on events for the `Receiver`, if it returns true the
/// `Sender` is dropped. On the `Sender` end check [`is_write_closed`], if it
/// returns true the `Receiver` was dropped. Also see the second example below.
///
/// [`WRITABLE`]: Interest::WRITABLE
/// [writable events]: event::Event::is_writable
/// [`READABLE`]: Interest::READABLE
/// [readable event]: event::Event::is_readable
/// [`is_read_closed`]: event::Event::is_read_closed
/// [`is_write_closed`]: event::Event::is_write_closed
///
/// # Deregistering
///
/// Both `Sender` and `Receiver` will deregister themselves when dropped,
/// **iff** the file descriptors are not duplicated (via [`dup(2)`]).
///
/// [`dup(2)`]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/dup.html
///
/// # Examples
///
/// Simple example that writes data into the sending end and read it from the
/// receiving end.
///
/// ```
/// use std::io::{self, Read, Write};
///
/// use mio::{Poll, Events, Interest, Token};
/// use mio::unix::pipe;
///
/// // Unique tokens for the two ends of the channel.
/// const PIPE_RECV: Token = Token(0);
/// const PIPE_SEND: Token = Token(1);
///
/// # fn main() -> io::Result<()> {
/// // Create our `Poll` instance and the `Events` container.
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(8);
///
/// // Create a new pipe.
/// let (mut sender, mut receiver) = pipe::new()?;
///
/// // Register both ends of the channel.
/// poll.registry().register(&mut receiver, PIPE_RECV, Interest::READABLE)?;
/// poll.registry().register(&mut sender, PIPE_SEND, Interest::WRITABLE)?;
///
/// const MSG: &[u8; 11] = b"Hello world";
///
/// loop {
///     poll.poll(&mut events, None)?;
///
///     for event in events.iter() {
///         match event.token() {
///             PIPE_SEND => sender.write(MSG)
///                 .and_then(|n| if n != MSG.len() {
///                         // We'll consider a short write an error in this
///                         // example. NOTE: we can't use `write_all` with
///                         // non-blocking I/O.
///                         Err(io::ErrorKind::WriteZero.into())
///                     } else {
///                         Ok(())
///                     })?,
///             PIPE_RECV => {
///                 let mut buf = [0; 11];
///                 let n = receiver.read(&mut buf)?;
///                 println!("received: {:?}", &buf[0..n]);
///                 assert_eq!(n, MSG.len());
///                 assert_eq!(&buf, &*MSG);
///                 return Ok(());
///             },
///             _ => unreachable!(),
///         }
///     }
/// }
/// # }
/// ```
///
/// Example that receives an event once the `Sender` is dropped.
///
/// ```
/// # use std::io;
/// #
/// # use mio::{Poll, Events, Interest, Token};
/// # use mio::unix::pipe;
/// #
/// # const PIPE_RECV: Token = Token(0);
/// # const PIPE_SEND: Token = Token(1);
/// #
/// # fn main() -> io::Result<()> {
/// // Same setup as in the example above.
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(8);
///
/// let (mut sender, mut receiver) = pipe::new()?;
///
/// poll.registry().register(&mut receiver, PIPE_RECV, Interest::READABLE)?;
/// poll.registry().register(&mut sender, PIPE_SEND, Interest::WRITABLE)?;
///
/// // Drop the sender.
/// drop(sender);
///
/// poll.poll(&mut events, None)?;
///
/// for event in events.iter() {
///     match event.token() {
///         PIPE_RECV if event.is_read_closed() => {
///             // Detected that the sender was dropped.
///             println!("Sender dropped!");
///             return Ok(());
///         },
///         _ => unreachable!(),
///     }
/// }
/// # unreachable!();
/// # }
/// ```
pub fn new() -> io::Result<(Sender, Receiver)> {
    let fds = new_raw()?;
    // SAFETY: `new_raw` initialised the `fds` above.
    let r = unsafe { Receiver::from_raw_fd(fds[0]) };
    let w = unsafe { Sender::from_raw_fd(fds[1]) };
    Ok((w, r))
}

/// Sending end of an Unix pipe.
///
/// See [`new`] for documentation, including examples.
#[derive(Debug)]
pub struct Sender {
    inner: IoSource<File>,
}

impl Sender {
    /// Set the `Sender` into or out of non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        set_nonblocking(self.inner.as_raw_fd(), nonblocking)
    }

    /// Execute an I/O operation ensuring that the socket receives more events
    /// if it hits a [`WouldBlock`] error.
    ///
    /// # Notes
    ///
    /// This method is required to be called for **all** I/O operations to
    /// ensure the user will receive events once the socket is ready again after
    /// returning a [`WouldBlock`] error.
    ///
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use std::io;
    /// use std::os::fd::AsRawFd;
    /// use mio::unix::pipe;
    ///
    /// let (sender, receiver) = pipe::new()?;
    ///
    /// // Wait until the sender is writable...
    ///
    /// // Write to the sender using a direct libc call, of course the
    /// // `io::Write` implementation would be easier to use.
    /// let buf = b"hello";
    /// let n = sender.try_io(|| {
    ///     let buf_ptr = &buf as *const _ as *const _;
    ///     let res = unsafe { libc::write(sender.as_raw_fd(), buf_ptr, buf.len()) };
    ///     if res != -1 {
    ///         Ok(res as usize)
    ///     } else {
    ///         // If EAGAIN or EWOULDBLOCK is set by libc::write, the closure
    ///         // should return `WouldBlock` error.
    ///         Err(io::Error::last_os_error())
    ///     }
    /// })?;
    /// eprintln!("write {} bytes", n);
    ///
    /// // Wait until the receiver is readable...
    ///
    /// // Read from the receiver using a direct libc call, of course the
    /// // `io::Read` implementation would be easier to use.
    /// let mut buf = [0; 512];
    /// let n = receiver.try_io(|| {
    ///     let buf_ptr = &mut buf as *mut _ as *mut _;
    ///     let res = unsafe { libc::read(receiver.as_raw_fd(), buf_ptr, buf.len()) };
    ///     if res != -1 {
    ///         Ok(res as usize)
    ///     } else {
    ///         // If EAGAIN or EWOULDBLOCK is set by libc::read, the closure
    ///         // should return `WouldBlock` error.
    ///         Err(io::Error::last_os_error())
    ///     }
    /// })?;
    /// eprintln!("read {} bytes", n);
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_io<F, T>(&self, f: F) -> io::Result<T>
    where
        F: FnOnce() -> io::Result<T>,
    {
        self.inner.do_io(|_| f())
    }
}

impl event::Source for Sender {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.inner.deregister(registry)
    }
}

impl Write for Sender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.write(buf))
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.write_vectored(bufs))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.do_io(|mut sender| sender.flush())
    }
}

impl Write for &Sender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.write(buf))
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.write_vectored(bufs))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.do_io(|mut sender| sender.flush())
    }
}

/// # Notes
///
/// The underlying pipe is **not** set to non-blocking.
impl From<ChildStdin> for Sender {
    fn from(stdin: ChildStdin) -> Sender {
        // Safety: `ChildStdin` is guaranteed to be a valid file descriptor.
        unsafe { Sender::from_raw_fd(stdin.into_raw_fd()) }
    }
}

impl FromRawFd for Sender {
    unsafe fn from_raw_fd(fd: RawFd) -> Sender {
        Sender {
            inner: IoSource::new(File::from_raw_fd(fd)),
        }
    }
}

impl AsRawFd for Sender {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for Sender {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

impl From<Sender> for OwnedFd {
    fn from(sender: Sender) -> Self {
        sender.inner.into_inner().into()
    }
}

impl AsFd for Sender {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl From<OwnedFd> for Sender {
    fn from(fd: OwnedFd) -> Self {
        Sender {
            inner: IoSource::new(File::from(fd)),
        }
    }
}

/// Receiving end of an Unix pipe.
///
/// See [`new`] for documentation, including examples.
#[derive(Debug)]
pub struct Receiver {
    inner: IoSource<File>,
}

impl Receiver {
    /// Set the `Receiver` into or out of non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        set_nonblocking(self.inner.as_raw_fd(), nonblocking)
    }

    /// Execute an I/O operation ensuring that the socket receives more events
    /// if it hits a [`WouldBlock`] error.
    ///
    /// # Notes
    ///
    /// This method is required to be called for **all** I/O operations to
    /// ensure the user will receive events once the socket is ready again after
    /// returning a [`WouldBlock`] error.
    ///
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use std::io;
    /// use std::os::fd::AsRawFd;
    /// use mio::unix::pipe;
    ///
    /// let (sender, receiver) = pipe::new()?;
    ///
    /// // Wait until the sender is writable...
    ///
    /// // Write to the sender using a direct libc call, of course the
    /// // `io::Write` implementation would be easier to use.
    /// let buf = b"hello";
    /// let n = sender.try_io(|| {
    ///     let buf_ptr = &buf as *const _ as *const _;
    ///     let res = unsafe { libc::write(sender.as_raw_fd(), buf_ptr, buf.len()) };
    ///     if res != -1 {
    ///         Ok(res as usize)
    ///     } else {
    ///         // If EAGAIN or EWOULDBLOCK is set by libc::write, the closure
    ///         // should return `WouldBlock` error.
    ///         Err(io::Error::last_os_error())
    ///     }
    /// })?;
    /// eprintln!("write {} bytes", n);
    ///
    /// // Wait until the receiver is readable...
    ///
    /// // Read from the receiver using a direct libc call, of course the
    /// // `io::Read` implementation would be easier to use.
    /// let mut buf = [0; 512];
    /// let n = receiver.try_io(|| {
    ///     let buf_ptr = &mut buf as *mut _ as *mut _;
    ///     let res = unsafe { libc::read(receiver.as_raw_fd(), buf_ptr, buf.len()) };
    ///     if res != -1 {
    ///         Ok(res as usize)
    ///     } else {
    ///         // If EAGAIN or EWOULDBLOCK is set by libc::read, the closure
    ///         // should return `WouldBlock` error.
    ///         Err(io::Error::last_os_error())
    ///     }
    /// })?;
    /// eprintln!("read {} bytes", n);
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_io<F, T>(&self, f: F) -> io::Result<T>
    where
        F: FnOnce() -> io::Result<T>,
    {
        self.inner.do_io(|_| f())
    }
}

impl event::Source for Receiver {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.inner.deregister(registry)
    }
}

impl Read for Receiver {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.read(buf))
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.read_vectored(bufs))
    }
}

impl Read for &Receiver {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.read(buf))
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut sender| sender.read_vectored(bufs))
    }
}

/// # Notes
///
/// The underlying pipe is **not** set to non-blocking.
impl From<ChildStdout> for Receiver {
    fn from(stdout: ChildStdout) -> Receiver {
        // Safety: `ChildStdout` is guaranteed to be a valid file descriptor.
        unsafe { Receiver::from_raw_fd(stdout.into_raw_fd()) }
    }
}

/// # Notes
///
/// The underlying pipe is **not** set to non-blocking.
impl From<ChildStderr> for Receiver {
    fn from(stderr: ChildStderr) -> Receiver {
        // Safety: `ChildStderr` is guaranteed to be a valid file descriptor.
        unsafe { Receiver::from_raw_fd(stderr.into_raw_fd()) }
    }
}

impl IntoRawFd for Receiver {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

impl AsRawFd for Receiver {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl FromRawFd for Receiver {
    unsafe fn from_raw_fd(fd: RawFd) -> Receiver {
        Receiver {
            inner: IoSource::new(File::from_raw_fd(fd)),
        }
    }
}

impl From<Receiver> for OwnedFd {
    fn from(receiver: Receiver) -> Self {
        receiver.inner.into_inner().into()
    }
}

impl AsFd for Receiver {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl From<OwnedFd> for Receiver {
    fn from(fd: OwnedFd) -> Self {
        Receiver {
            inner: IoSource::new(File::from(fd)),
        }
    }
}

#[cfg(not(any(target_os = "aix", target_os = "illumos", target_os = "solaris", target_os = "vita")))]
fn set_nonblocking(fd: RawFd, nonblocking: bool) -> io::Result<()> {
    let value = nonblocking as libc::c_int;
    if unsafe { libc::ioctl(fd, libc::FIONBIO, &value) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "aix", target_os = "illumos", target_os = "solaris", target_os = "vita"))]
fn set_nonblocking(fd: RawFd, nonblocking: bool) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }

    let nflags = if nonblocking {
        flags | libc::O_NONBLOCK
    } else {
        flags & !libc::O_NONBLOCK
    };

    if flags != nflags {
        if unsafe { libc::fcntl(fd, libc::F_SETFL, nflags) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}
} // `cfg_os_ext!`.
