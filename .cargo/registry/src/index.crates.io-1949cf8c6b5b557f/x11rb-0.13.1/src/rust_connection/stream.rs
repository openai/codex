use rustix::fd::{AsFd, BorrowedFd};
use std::io::{IoSlice, Result};
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, IntoRawFd, OwnedFd, RawFd};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use std::os::windows::io::{
    AsRawSocket, AsSocket, BorrowedSocket, IntoRawSocket, OwnedSocket, RawSocket,
};

use crate::utils::RawFdContainer;
use x11rb_protocol::parse_display::ConnectAddress;
use x11rb_protocol::xauth::Family;

/// The kind of operation that one want to poll for.
#[derive(Debug, Clone, Copy)]
pub enum PollMode {
    /// Check if the stream is readable, i.e. there is pending data to be read.
    Readable,

    /// Check if the stream is writable, i.e. some data could be successfully written to it.
    Writable,

    /// Check for both readability and writability.
    ReadAndWritable,
}

impl PollMode {
    /// Does this poll mode include readability?
    pub fn readable(self) -> bool {
        match self {
            PollMode::Readable | PollMode::ReadAndWritable => true,
            PollMode::Writable => false,
        }
    }

    /// Does this poll mode include writability?
    pub fn writable(self) -> bool {
        match self {
            PollMode::Writable | PollMode::ReadAndWritable => true,
            PollMode::Readable => false,
        }
    }
}

/// A trait used to implement the raw communication with the X11 server.
///
/// None of the functions of this trait shall return [`std::io::ErrorKind::Interrupted`].
/// If a system call fails with this error, the implementation should try again.
pub trait Stream {
    /// Waits for level-triggered read and/or write events on the stream.
    ///
    /// This function does not return what caused it to complete the poll.
    /// Instead, callers should try to read or write and check for
    /// [`std::io::ErrorKind::WouldBlock`].
    ///
    /// This function is allowed to spuriously return even if the stream
    /// is neither readable nor writable. However, it shall not do it
    /// continuously, which would cause a 100% CPU usage.
    ///
    /// # Multithreading
    ///
    /// If `Self` is `Send + Sync` and `poll` is used concurrently from more than
    /// one thread, all threads should wake when the stream becomes readable (when
    /// `read` is `true`) or writable (when `write` is `true`).
    fn poll(&self, mode: PollMode) -> Result<()>;

    /// Read some bytes and FDs from this reader without blocking, returning how many bytes
    /// were read.
    ///
    /// This function works like [`std::io::Read::read`], but also supports the reception of file
    /// descriptors. Any received file descriptors are appended to the given `fd_storage`.
    /// Whereas implementation of [`std::io::Read::read`] are allowed to block or not to block,
    /// this method shall never block and return `ErrorKind::WouldBlock` if needed.
    ///
    /// This function does not guarantee that all file descriptors were sent together with the data
    /// with which they are received. However, file descriptors may not be received later than the
    /// data that was sent at the same time. Instead, file descriptors may only be received
    /// earlier.
    ///
    /// # Multithreading
    ///
    /// If `Self` is `Send + Sync` and `read` is used concurrently from more than one thread:
    ///
    /// * Both the data and the file descriptors shall be read in order, but possibly
    /// interleaved across threads.
    /// * Neither the data nor the file descriptors shall be duplicated.
    /// * The returned value shall always be the actual number of bytes read into `buf`.
    fn read(&self, buf: &mut [u8], fd_storage: &mut Vec<RawFdContainer>) -> Result<usize>;

    /// Write a buffer and some FDs into this writer without blocking, returning how many
    /// bytes were written.
    ///
    /// This function works like [`std::io::Write::write`], but also supports sending file
    /// descriptors. The `fds` argument contains the file descriptors to send. The order of file
    /// descriptors is maintained. Whereas implementation of [`std::io::Write::write`] are
    /// allowed to block or not to block, this function must never block and return
    /// `ErrorKind::WouldBlock` if needed.
    ///
    /// This function does not guarantee that all file descriptors are sent together with the data.
    /// Any file descriptors that were sent are removed from the beginning of the given `Vec`.
    ///
    /// There is no guarantee that the given file descriptors are received together with the given
    /// data. File descriptors might be received earlier than their corresponding data. It is not
    /// allowed for file descriptors to be received later than the bytes that were sent at the same
    /// time.
    ///
    /// # Multithreading
    ///
    /// If `Self` is `Send + Sync` and `write` is used concurrently from more than one thread:
    ///
    /// * Both the data and the file descriptors shall be written in order, but possibly
    /// interleaved across threads.
    /// * Neither the data nor the file descriptors shall be duplicated.
    /// * The returned value shall always be the actual number of bytes written from `buf`.
    fn write(&self, buf: &[u8], fds: &mut Vec<RawFdContainer>) -> Result<usize>;

    /// Like `write`, except that it writes from a slice of buffers. Like `write`, this
    /// method must never block.
    ///
    /// This method must behave as a call to `write` with the buffers concatenated would.
    ///
    /// The default implementation calls `write` with the first nonempty buffer provided.
    ///
    /// # Multithreading
    ///
    /// Same as `write`.
    fn write_vectored(&self, bufs: &[IoSlice<'_>], fds: &mut Vec<RawFdContainer>) -> Result<usize> {
        for buf in bufs {
            if !buf.is_empty() {
                return self.write(buf, fds);
            }
        }
        Ok(0)
    }
}

/// A wrapper around a `TcpStream` or `UnixStream`.
///
/// Use by default in `RustConnection` as stream.
#[derive(Debug)]
pub struct DefaultStream {
    inner: DefaultStreamInner,
}

#[cfg(unix)]
type DefaultStreamInner = RawFdContainer;

#[cfg(not(unix))]
type DefaultStreamInner = TcpStream;

/// The address of a peer in a format suitable for xauth.
///
/// These values can be directly given to [`x11rb_protocol::xauth::get_auth`].
type PeerAddr = (Family, Vec<u8>);

impl DefaultStream {
    /// Try to connect to the X11 server described by the given arguments.
    pub fn connect(addr: &ConnectAddress<'_>) -> Result<(Self, PeerAddr)> {
        match addr {
            ConnectAddress::Hostname(host, port) => {
                // connect over TCP
                let stream = TcpStream::connect((*host, *port))?;
                Self::from_tcp_stream(stream)
            }
            #[cfg(unix)]
            ConnectAddress::Socket(path) => {
                // Try abstract unix socket first. If that fails, fall back to normal unix socket
                #[cfg(any(target_os = "linux", target_os = "android"))]
                if let Ok(stream) = connect_abstract_unix_stream(path.as_bytes()) {
                    // TODO: Does it make sense to add a constructor similar to from_unix_stream()?
                    // If this is done: Move the set_nonblocking() from
                    // connect_abstract_unix_stream() to that new function.
                    let stream = DefaultStream { inner: stream };
                    return Ok((stream, peer_addr::local()));
                }

                // connect over Unix domain socket
                let stream = UnixStream::connect(path)?;
                Self::from_unix_stream(stream)
            }
            #[cfg(not(unix))]
            ConnectAddress::Socket(_) => {
                // Unix domain sockets are not supported on Windows
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unix domain sockets are not supported on Windows",
                ))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "The given address family is not implemented",
            )),
        }
    }

    /// Creates a new `Stream` from an already connected `TcpStream`.
    ///
    /// The stream will be set in non-blocking mode.
    ///
    /// This returns the peer address in a format suitable for [`x11rb_protocol::xauth::get_auth`].
    pub fn from_tcp_stream(stream: TcpStream) -> Result<(Self, PeerAddr)> {
        let peer_addr = peer_addr::tcp(&stream.peer_addr()?);
        stream.set_nonblocking(true)?;
        let result = Self {
            inner: stream.into(),
        };
        Ok((result, peer_addr))
    }

    /// Creates a new `Stream` from an already connected `UnixStream`.
    ///
    /// The stream will be set in non-blocking mode.
    ///
    /// This returns the peer address in a format suitable for [`x11rb_protocol::xauth::get_auth`].
    #[cfg(unix)]
    pub fn from_unix_stream(stream: UnixStream) -> Result<(Self, PeerAddr)> {
        stream.set_nonblocking(true)?;
        let result = Self {
            inner: stream.into(),
        };
        Ok((result, peer_addr::local()))
    }

    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

#[cfg(unix)]
impl AsRawFd for DefaultStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(unix)]
impl AsFd for DefaultStream {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

#[cfg(unix)]
impl IntoRawFd for DefaultStream {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

#[cfg(unix)]
impl From<DefaultStream> for OwnedFd {
    fn from(stream: DefaultStream) -> Self {
        stream.inner
    }
}

#[cfg(windows)]
impl AsRawSocket for DefaultStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.inner.as_raw_socket()
    }
}

#[cfg(windows)]
impl AsSocket for DefaultStream {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.inner.as_socket()
    }
}

#[cfg(windows)]
impl IntoRawSocket for DefaultStream {
    fn into_raw_socket(self) -> RawSocket {
        self.inner.into_raw_socket()
    }
}

#[cfg(windows)]
impl From<DefaultStream> for OwnedSocket {
    fn from(stream: DefaultStream) -> Self {
        stream.inner.into()
    }
}

#[cfg(unix)]
fn do_write(
    stream: &DefaultStream,
    bufs: &[IoSlice<'_>],
    fds: &mut Vec<RawFdContainer>,
) -> Result<usize> {
    use rustix::io::Errno;
    use rustix::net::{sendmsg, SendAncillaryBuffer, SendAncillaryMessage, SendFlags};

    fn sendmsg_wrapper(
        fd: BorrowedFd<'_>,
        iov: &[IoSlice<'_>],
        cmsgs: &mut SendAncillaryBuffer<'_, '_, '_>,
        flags: SendFlags,
    ) -> Result<usize> {
        loop {
            match sendmsg(fd, iov, cmsgs, flags) {
                Ok(n) => return Ok(n),
                // try again
                Err(Errno::INTR) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }

    let fd = stream.as_fd();

    let res = if !fds.is_empty() {
        let fds = fds.iter().map(|fd| fd.as_fd()).collect::<Vec<_>>();
        let rights = SendAncillaryMessage::ScmRights(&fds);

        let mut cmsg_space = vec![0u8; rights.size()];
        let mut cmsg_buffer = SendAncillaryBuffer::new(&mut cmsg_space);
        assert!(cmsg_buffer.push(rights));

        sendmsg_wrapper(fd, bufs, &mut cmsg_buffer, SendFlags::empty())?
    } else {
        sendmsg_wrapper(fd, bufs, &mut Default::default(), SendFlags::empty())?
    };

    // We successfully sent all FDs
    fds.clear();

    Ok(res)
}

impl Stream for DefaultStream {
    fn poll(&self, mode: PollMode) -> Result<()> {
        use rustix::event::{poll, PollFd, PollFlags};
        use rustix::io::Errno;

        let mut poll_flags = PollFlags::empty();
        if mode.readable() {
            poll_flags |= PollFlags::IN;
        }
        if mode.writable() {
            poll_flags |= PollFlags::OUT;
        }
        let fd = self.as_fd();
        let mut poll_fds = [PollFd::from_borrowed_fd(fd, poll_flags)];
        loop {
            match poll(&mut poll_fds, -1) {
                Ok(_) => break,
                Err(Errno::INTR) => {}
                Err(e) => return Err(e.into()),
            }
        }
        // Let the errors (POLLERR) be handled when trying to read or write.
        Ok(())
    }

    fn read(&self, buf: &mut [u8], fd_storage: &mut Vec<RawFdContainer>) -> Result<usize> {
        #[cfg(unix)]
        {
            use rustix::io::Errno;
            use rustix::net::{recvmsg, RecvAncillaryBuffer, RecvAncillaryMessage};
            use std::io::IoSliceMut;

            // 1024 bytes on the stack should be enough for more file descriptors than the X server will ever
            // send, as well as the header for the ancillary data. If you can find a case where this can
            // overflow with an actual production X11 server, I'll buy you a steak dinner.
            let mut cmsg = [0u8; 1024];
            let mut iov = [IoSliceMut::new(buf)];
            let mut cmsg_buffer = RecvAncillaryBuffer::new(&mut cmsg);

            let fd = self.as_fd();
            let msg = loop {
                match recvmsg(fd, &mut iov, &mut cmsg_buffer, recvmsg::flags()) {
                    Ok(msg) => break msg,
                    // try again
                    Err(Errno::INTR) => {}
                    Err(e) => return Err(e.into()),
                }
            };

            let fds_received = cmsg_buffer
                .drain()
                .filter_map(|cmsg| match cmsg {
                    RecvAncillaryMessage::ScmRights(r) => Some(r),
                    _ => None,
                })
                .flatten();

            let mut cloexec_error = Ok(());
            fd_storage.extend(recvmsg::after_recvmsg(fds_received, &mut cloexec_error));
            cloexec_error?;

            Ok(msg.bytes)
        }
        #[cfg(not(unix))]
        {
            use std::io::Read;
            // No FDs are read, so nothing needs to be done with fd_storage
            let _ = fd_storage;
            loop {
                // Use `impl Read for &TcpStream` to avoid needing a mutable `TcpStream`.
                match (&mut &self.inner).read(buf) {
                    Ok(n) => return Ok(n),
                    // try again
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }

    fn write(&self, buf: &[u8], fds: &mut Vec<RawFdContainer>) -> Result<usize> {
        #[cfg(unix)]
        {
            do_write(self, &[IoSlice::new(buf)], fds)
        }
        #[cfg(not(unix))]
        {
            use std::io::{Error, ErrorKind, Write};
            if !fds.is_empty() {
                return Err(Error::new(ErrorKind::Other, "FD passing is unsupported"));
            }
            loop {
                // Use `impl Write for &TcpStream` to avoid needing a mutable `TcpStream`.
                match (&mut &self.inner).write(buf) {
                    Ok(n) => return Ok(n),
                    // try again
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }

    fn write_vectored(&self, bufs: &[IoSlice<'_>], fds: &mut Vec<RawFdContainer>) -> Result<usize> {
        #[cfg(unix)]
        {
            do_write(self, bufs, fds)
        }
        #[cfg(not(unix))]
        {
            use std::io::{Error, ErrorKind, Write};
            if !fds.is_empty() {
                return Err(Error::new(ErrorKind::Other, "FD passing is unsupported"));
            }
            loop {
                // Use `impl Write for &TcpStream` to avoid needing a mutable `TcpStream`.
                match (&mut &self.inner).write_vectored(bufs) {
                    Ok(n) => return Ok(n),
                    // try again
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn connect_abstract_unix_stream(
    path: &[u8],
) -> std::result::Result<RawFdContainer, rustix::io::Errno> {
    use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
    use rustix::net::{
        connect_unix, socket_with, AddressFamily, SocketAddrUnix, SocketFlags, SocketType,
    };

    let socket = socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )?;

    connect_unix(&socket, &SocketAddrUnix::new_abstract_name(path)?)?;

    // Make the FD non-blocking
    fcntl_setfl(&socket, fcntl_getfl(&socket)? | OFlags::NONBLOCK)?;

    Ok(socket)
}

/// Helper code to make sure that received FDs are marked as CLOEXEC
#[cfg(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "linux",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod recvmsg {
    use super::RawFdContainer;
    use rustix::net::RecvFlags;

    pub(crate) fn flags() -> RecvFlags {
        RecvFlags::CMSG_CLOEXEC
    }

    pub(crate) fn after_recvmsg<'a>(
        fds: impl Iterator<Item = RawFdContainer> + 'a,
        _cloexec_error: &'a mut Result<(), rustix::io::Errno>,
    ) -> impl Iterator<Item = RawFdContainer> + 'a {
        fds
    }
}

/// Helper code to make sure that received FDs are marked as CLOEXEC
#[cfg(all(
    unix,
    not(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))
))]
mod recvmsg {
    use super::RawFdContainer;
    use rustix::io::{fcntl_getfd, fcntl_setfd, FdFlags};
    use rustix::net::RecvFlags;

    pub(crate) fn flags() -> RecvFlags {
        RecvFlags::empty()
    }

    pub(crate) fn after_recvmsg<'a>(
        fds: impl Iterator<Item = RawFdContainer> + 'a,
        cloexec_error: &'a mut rustix::io::Result<()>,
    ) -> impl Iterator<Item = RawFdContainer> + 'a {
        fds.map(move |fd| {
            if let Err(e) =
                fcntl_getfd(&fd).and_then(|flags| fcntl_setfd(&fd, flags | FdFlags::CLOEXEC))
            {
                *cloexec_error = Err(e);
            }
            fd
        })
    }
}

mod peer_addr {
    use super::{Family, PeerAddr};
    use std::net::{Ipv4Addr, SocketAddr};

    // Get xauth information representing a local connection
    pub(super) fn local() -> PeerAddr {
        let hostname = crate::hostname()
            .to_str()
            .map_or_else(Vec::new, |s| s.as_bytes().to_vec());
        (Family::LOCAL, hostname)
    }

    // Get xauth information representing a TCP connection to the given address
    pub(super) fn tcp(addr: &SocketAddr) -> PeerAddr {
        let ip = match addr {
            SocketAddr::V4(addr) => *addr.ip(),
            SocketAddr::V6(addr) => {
                let ip = addr.ip();
                if ip.is_loopback() {
                    // This is a local connection.
                    // Use LOCALHOST to cause a fall-through in the code below.
                    Ipv4Addr::LOCALHOST
                } else if let Some(ip) = ip.to_ipv4() {
                    // Let the ipv4 code below handle this
                    ip
                } else {
                    // Okay, this is really a v6 address
                    return (Family::INTERNET6, ip.octets().to_vec());
                }
            }
        };

        // Handle the v4 address
        if ip.is_loopback() {
            local()
        } else {
            (Family::INTERNET, ip.octets().to_vec())
        }
    }
}
