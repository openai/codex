// Copyright 2015 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::io::{self, Read, Write};
#[cfg(not(target_os = "redox"))]
use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
#[cfg(not(target_os = "nto"))]
use std::net::Ipv6Addr;
use std::net::{self, Ipv4Addr, Shutdown};
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, IntoRawFd};
#[cfg(windows)]
use std::os::windows::io::{FromRawSocket, IntoRawSocket};
use std::time::Duration;

use crate::sys::{self, c_int, getsockopt, setsockopt, Bool};
#[cfg(all(unix, not(target_os = "redox")))]
use crate::MsgHdrMut;
use crate::{Domain, Protocol, SockAddr, TcpKeepalive, Type};
#[cfg(not(target_os = "redox"))]
use crate::{MaybeUninitSlice, MsgHdr, RecvFlags};

/// Owned wrapper around a system socket.
///
/// This type simply wraps an instance of a file descriptor (`c_int`) on Unix
/// and an instance of `SOCKET` on Windows. This is the main type exported by
/// this crate and is intended to mirror the raw semantics of sockets on
/// platforms as closely as possible. Almost all methods correspond to
/// precisely one libc or OS API call which is essentially just a "Rustic
/// translation" of what's below.
///
/// ## Converting to and from other types
///
/// This type can be freely converted into the network primitives provided by
/// the standard library, such as [`TcpStream`] or [`UdpSocket`], using the
/// [`From`] trait, see the example below.
///
/// [`TcpStream`]: std::net::TcpStream
/// [`UdpSocket`]: std::net::UdpSocket
///
/// # Notes
///
/// Some methods that set options on `Socket` require two system calls to set
/// their options without overwriting previously set options. We do this by
/// first getting the current settings, applying the desired changes, and then
/// updating the settings. This means that the operation is **not** atomic. This
/// can lead to a data race when two threads are changing options in parallel.
///
/// # Examples
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// use std::net::{SocketAddr, TcpListener};
/// use socket2::{Socket, Domain, Type};
///
/// // create a TCP listener
/// let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;
///
/// let address: SocketAddr = "[::1]:12345".parse().unwrap();
/// let address = address.into();
/// socket.bind(&address)?;
/// socket.listen(128)?;
///
/// let listener: TcpListener = socket.into();
/// // ...
/// # drop(listener);
/// # Ok(()) }
/// ```
pub struct Socket {
    inner: Inner,
}

/// Store a `TcpStream` internally to take advantage of its niche optimizations on Unix platforms.
pub(crate) type Inner = std::net::TcpStream;

impl Socket {
    /// # Safety
    ///
    /// The caller must ensure `raw` is a valid file descriptor/socket. NOTE:
    /// this should really be marked `unsafe`, but this being an internal
    /// function, often passed as mapping function, it's makes it very
    /// inconvenient to mark it as `unsafe`.
    pub(crate) fn from_raw(raw: sys::Socket) -> Socket {
        Socket {
            inner: unsafe {
                // SAFETY: the caller must ensure that `raw` is a valid file
                // descriptor, but when it isn't it could return I/O errors, or
                // potentially close a fd it doesn't own. All of that isn't
                // memory unsafe, so it's not desired but never memory unsafe or
                // causes UB.
                //
                // However there is one exception. We use `TcpStream` to
                // represent the `Socket` internally (see `Inner` type),
                // `TcpStream` has a layout optimisation that doesn't allow for
                // negative file descriptors (as those are always invalid).
                // Violating this assumption (fd never negative) causes UB,
                // something we don't want. So check for that we have this
                // `assert!`.
                #[cfg(unix)]
                assert!(raw >= 0, "tried to create a `Socket` with an invalid fd");
                sys::socket_from_raw(raw)
            },
        }
    }

    pub(crate) fn as_raw(&self) -> sys::Socket {
        sys::socket_as_raw(&self.inner)
    }

    pub(crate) fn into_raw(self) -> sys::Socket {
        sys::socket_into_raw(self.inner)
    }

    /// Creates a new socket and sets common flags.
    ///
    /// This function corresponds to `socket(2)` on Unix and `WSASocketW` on
    /// Windows.
    ///
    /// On Unix-like systems, the close-on-exec flag is set on the new socket.
    /// Additionally, on Apple platforms `SOCK_NOSIGPIPE` is set. On Windows,
    /// the socket is made non-inheritable.
    ///
    /// [`Socket::new_raw`] can be used if you don't want these flags to be set.
    #[doc = man_links!(socket(2))]
    pub fn new(domain: Domain, ty: Type, protocol: Option<Protocol>) -> io::Result<Socket> {
        let ty = set_common_type(ty);
        Socket::new_raw(domain, ty, protocol).and_then(set_common_flags)
    }

    /// Creates a new socket ready to be configured.
    ///
    /// This function corresponds to `socket(2)` on Unix and `WSASocketW` on
    /// Windows and simply creates a new socket, no other configuration is done.
    pub fn new_raw(domain: Domain, ty: Type, protocol: Option<Protocol>) -> io::Result<Socket> {
        let protocol = protocol.map_or(0, |p| p.0);
        sys::socket(domain.0, ty.0, protocol).map(Socket::from_raw)
    }

    /// Creates a pair of sockets which are connected to each other.
    ///
    /// This function corresponds to `socketpair(2)`.
    ///
    /// This function sets the same flags as in done for [`Socket::new`],
    /// [`Socket::pair_raw`] can be used if you don't want to set those flags.
    #[doc = man_links!(unix: socketpair(2))]
    #[cfg(all(feature = "all", unix))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", unix))))]
    pub fn pair(
        domain: Domain,
        ty: Type,
        protocol: Option<Protocol>,
    ) -> io::Result<(Socket, Socket)> {
        let ty = set_common_type(ty);
        let (a, b) = Socket::pair_raw(domain, ty, protocol)?;
        let a = set_common_flags(a)?;
        let b = set_common_flags(b)?;
        Ok((a, b))
    }

    /// Creates a pair of sockets which are connected to each other.
    ///
    /// This function corresponds to `socketpair(2)`.
    #[cfg(all(feature = "all", unix))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", unix))))]
    pub fn pair_raw(
        domain: Domain,
        ty: Type,
        protocol: Option<Protocol>,
    ) -> io::Result<(Socket, Socket)> {
        let protocol = protocol.map_or(0, |p| p.0);
        sys::socketpair(domain.0, ty.0, protocol)
            .map(|[a, b]| (Socket::from_raw(a), Socket::from_raw(b)))
    }

    /// Binds this socket to the specified address.
    ///
    /// This function directly corresponds to the `bind(2)` function on Windows
    /// and Unix.
    #[doc = man_links!(bind(2))]
    pub fn bind(&self, address: &SockAddr) -> io::Result<()> {
        sys::bind(self.as_raw(), address)
    }

    /// Initiate a connection on this socket to the specified address.
    ///
    /// This function directly corresponds to the `connect(2)` function on
    /// Windows and Unix.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    #[doc = man_links!(connect(2))]
    ///
    /// # Notes
    ///
    /// When using a non-blocking connect (by setting the socket into
    /// non-blocking mode before calling this function), socket option can't be
    /// set *while connecting*. This will cause errors on Windows. Socket
    /// options can be safely set before and after connecting the socket.
    ///
    /// On Cygwin, a Unix domain socket connect blocks until the server accepts
    /// it. If the behavior is not expected, try [`Socket::set_no_peercred`]
    /// (Cygwin only).
    #[allow(rustdoc::broken_intra_doc_links)] // Socket::set_no_peercred
    pub fn connect(&self, address: &SockAddr) -> io::Result<()> {
        sys::connect(self.as_raw(), address)
    }

    /// Initiate a connection on this socket to the specified address, only
    /// only waiting for a certain period of time for the connection to be
    /// established.
    ///
    /// Unlike many other methods on `Socket`, this does *not* correspond to a
    /// single C function. It sets the socket to nonblocking mode, connects via
    /// connect(2), and then waits for the connection to complete with poll(2)
    /// on Unix and select on Windows. When the connection is complete, the
    /// socket is set back to blocking mode. On Unix, this will loop over
    /// `EINTR` errors.
    ///
    /// # Warnings
    ///
    /// The non-blocking state of the socket is overridden by this function -
    /// it will be returned in blocking mode on success, and in an indeterminate
    /// state on failure.
    ///
    /// If the connection request times out, it may still be processing in the
    /// background - a second call to `connect` or `connect_timeout` may fail.
    pub fn connect_timeout(&self, addr: &SockAddr, timeout: Duration) -> io::Result<()> {
        self.set_nonblocking(true)?;
        let res = self.connect(addr);
        self.set_nonblocking(false)?;

        match res {
            Ok(()) => return Ok(()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            #[cfg(unix)]
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        sys::poll_connect(self, timeout)
    }

    /// Mark a socket as ready to accept incoming connection requests using
    /// [`Socket::accept()`].
    ///
    /// This function directly corresponds to the `listen(2)` function on
    /// Windows and Unix.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    #[doc = man_links!(listen(2))]
    pub fn listen(&self, backlog: c_int) -> io::Result<()> {
        sys::listen(self.as_raw(), backlog)
    }

    /// Accept a new incoming connection from this listener.
    ///
    /// This function uses `accept4(2)` on platforms that support it and
    /// `accept(2)` platforms that do not.
    ///
    /// This function sets the same flags as in done for [`Socket::new`],
    /// [`Socket::accept_raw`] can be used if you don't want to set those flags.
    #[doc = man_links!(accept(2))]
    ///
    /// # Notes
    ///
    /// On Cygwin, a Unix domain socket connect blocks until the server accepts
    /// it. If the behavior is not expected, try [`Socket::set_no_peercred`]
    /// (Cygwin only).
    #[allow(rustdoc::broken_intra_doc_links)] // Socket::set_no_peercred
    pub fn accept(&self) -> io::Result<(Socket, SockAddr)> {
        // Use `accept4` on platforms that support it.
        #[cfg(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "cygwin",
        ))]
        return self._accept4(libc::SOCK_CLOEXEC);

        // Fall back to `accept` on platforms that do not support `accept4`.
        #[cfg(not(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "cygwin",
        )))]
        {
            let (socket, addr) = self.accept_raw()?;
            let socket = set_common_flags(socket)?;
            // `set_common_flags` does not disable inheritance on Windows because `Socket::new`
            // unlike `accept` is able to create the socket with inheritance disabled.
            #[cfg(windows)]
            socket._set_no_inherit(true)?;
            Ok((socket, addr))
        }
    }

    /// Accept a new incoming connection from this listener.
    ///
    /// This function directly corresponds to the `accept(2)` function on
    /// Windows and Unix.
    pub fn accept_raw(&self) -> io::Result<(Socket, SockAddr)> {
        sys::accept(self.as_raw()).map(|(inner, addr)| (Socket::from_raw(inner), addr))
    }

    /// Returns the socket address of the local half of this socket.
    ///
    /// This function directly corresponds to the `getsockname(2)` function on
    /// Windows and Unix.
    #[doc = man_links!(getsockname(2))]
    ///
    /// # Notes
    ///
    /// Depending on the OS this may return an error if the socket is not
    /// [bound].
    ///
    /// [bound]: Socket::bind
    pub fn local_addr(&self) -> io::Result<SockAddr> {
        sys::getsockname(self.as_raw())
    }

    /// Returns the socket address of the remote peer of this socket.
    ///
    /// This function directly corresponds to the `getpeername(2)` function on
    /// Windows and Unix.
    #[doc = man_links!(getpeername(2))]
    ///
    /// # Notes
    ///
    /// This returns an error if the socket is not [`connect`ed].
    ///
    /// [`connect`ed]: Socket::connect
    pub fn peer_addr(&self) -> io::Result<SockAddr> {
        sys::getpeername(self.as_raw())
    }

    /// Returns the [`Type`] of this socket by checking the `SO_TYPE` option on
    /// this socket.
    pub fn r#type(&self) -> io::Result<Type> {
        unsafe { getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_TYPE).map(Type) }
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// # Notes
    ///
    /// On Unix this uses `F_DUPFD_CLOEXEC` and thus sets the `FD_CLOEXEC` on
    /// the returned socket.
    ///
    /// On Windows this uses `WSA_FLAG_NO_HANDLE_INHERIT` setting inheriting to
    /// false.
    ///
    /// On Windows this can **not** be used function cannot be used on a
    /// QOS-enabled socket, see
    /// <https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsaduplicatesocketw>.
    pub fn try_clone(&self) -> io::Result<Socket> {
        sys::try_clone(self.as_raw()).map(Socket::from_raw)
    }

    /// Returns true if this socket is set to nonblocking mode, false otherwise.
    ///
    /// # Notes
    ///
    /// On Unix this corresponds to calling `fcntl` returning the value of
    /// `O_NONBLOCK`.
    ///
    /// On Windows it is not possible retrieve the nonblocking mode status.
    #[cfg(all(feature = "all", unix))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", unix))))]
    pub fn nonblocking(&self) -> io::Result<bool> {
        sys::nonblocking(self.as_raw())
    }

    /// Moves this socket into or out of nonblocking mode.
    ///
    /// # Notes
    ///
    /// On Unix this corresponds to calling `fcntl` (un)setting `O_NONBLOCK`.
    ///
    /// On Windows this corresponds to calling `ioctlsocket` (un)setting
    /// `FIONBIO`.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        sys::set_nonblocking(self.as_raw(), nonblocking)
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value.
    #[doc = man_links!(shutdown(2))]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        sys::shutdown(self.as_raw(), how)
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected.
    ///
    /// The [`connect`] method will connect this socket to a remote address.
    /// This method might fail if the socket is not connected.
    #[doc = man_links!(recv(2))]
    ///
    /// [`connect`]: Socket::connect
    ///
    /// # Safety
    ///
    /// Normally casting a `&mut [u8]` to `&mut [MaybeUninit<u8>]` would be
    /// unsound, as that allows us to write uninitialised bytes to the buffer.
    /// However this implementation promises to not write uninitialised bytes to
    /// the `buf`fer and passes it directly to `recv(2)` system call. This
    /// promise ensures that this function can be called using a `buf`fer of
    /// type `&mut [u8]`.
    ///
    /// Note that the [`io::Read::read`] implementation calls this function with
    /// a `buf`fer of type `&mut [u8]`, allowing initialised buffers to be used
    /// without using `unsafe`.
    pub fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.recv_with_flags(buf, 0)
    }

    /// Receives out-of-band (OOB) data on the socket from the remote address to
    /// which it is connected by setting the `MSG_OOB` flag for this call.
    ///
    /// For more information, see [`recv`], [`out_of_band_inline`].
    ///
    /// [`recv`]: Socket::recv
    /// [`out_of_band_inline`]: Socket::out_of_band_inline
    #[cfg_attr(target_os = "redox", allow(rustdoc::broken_intra_doc_links))]
    pub fn recv_out_of_band(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.recv_with_flags(buf, sys::MSG_OOB)
    }

    /// Identical to [`recv`] but allows for specification of arbitrary flags to
    /// the underlying `recv` call.
    ///
    /// [`recv`]: Socket::recv
    pub fn recv_with_flags(
        &self,
        buf: &mut [MaybeUninit<u8>],
        flags: sys::c_int,
    ) -> io::Result<usize> {
        sys::recv(self.as_raw(), buf, flags)
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected. Unlike [`recv`] this allows passing multiple buffers.
    ///
    /// The [`connect`] method will connect this socket to a remote address.
    /// This method might fail if the socket is not connected.
    ///
    /// In addition to the number of bytes read, this function returns the flags
    /// for the received message. See [`RecvFlags`] for more information about
    /// the returned flags.
    #[doc = man_links!(recvmsg(2))]
    ///
    /// [`recv`]: Socket::recv
    /// [`connect`]: Socket::connect
    ///
    /// # Safety
    ///
    /// Normally casting a `IoSliceMut` to `MaybeUninitSlice` would be unsound,
    /// as that allows us to write uninitialised bytes to the buffer. However
    /// this implementation promises to not write uninitialised bytes to the
    /// `bufs` and passes it directly to `recvmsg(2)` system call. This promise
    /// ensures that this function can be called using `bufs` of type `&mut
    /// [IoSliceMut]`.
    ///
    /// Note that the [`io::Read::read_vectored`] implementation calls this
    /// function with `buf`s of type `&mut [IoSliceMut]`, allowing initialised
    /// buffers to be used without using `unsafe`.
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn recv_vectored(
        &self,
        bufs: &mut [MaybeUninitSlice<'_>],
    ) -> io::Result<(usize, RecvFlags)> {
        self.recv_vectored_with_flags(bufs, 0)
    }

    /// Identical to [`recv_vectored`] but allows for specification of arbitrary
    /// flags to the underlying `recvmsg`/`WSARecv` call.
    ///
    /// [`recv_vectored`]: Socket::recv_vectored
    ///
    /// # Safety
    ///
    /// `recv_from_vectored` makes the same safety guarantees regarding `bufs`
    /// as [`recv_vectored`].
    ///
    /// [`recv_vectored`]: Socket::recv_vectored
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn recv_vectored_with_flags(
        &self,
        bufs: &mut [MaybeUninitSlice<'_>],
        flags: c_int,
    ) -> io::Result<(usize, RecvFlags)> {
        sys::recv_vectored(self.as_raw(), bufs, flags)
    }

    /// Receives data on the socket from the remote adress to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    ///
    /// # Safety
    ///
    /// `peek` makes the same safety guarantees regarding the `buf`fer as
    /// [`recv`].
    ///
    /// [`recv`]: Socket::recv
    pub fn peek(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.recv_with_flags(buf, sys::MSG_PEEK)
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read and the address from whence the data came.
    #[doc = man_links!(recvfrom(2))]
    ///
    /// # Safety
    ///
    /// `recv_from` makes the same safety guarantees regarding the `buf`fer as
    /// [`recv`].
    ///
    /// [`recv`]: Socket::recv
    pub fn recv_from(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<(usize, SockAddr)> {
        self.recv_from_with_flags(buf, 0)
    }

    /// Identical to [`recv_from`] but allows for specification of arbitrary
    /// flags to the underlying `recvfrom` call.
    ///
    /// [`recv_from`]: Socket::recv_from
    pub fn recv_from_with_flags(
        &self,
        buf: &mut [MaybeUninit<u8>],
        flags: c_int,
    ) -> io::Result<(usize, SockAddr)> {
        sys::recv_from(self.as_raw(), buf, flags)
    }

    /// Receives data from the socket. Returns the amount of bytes read, the
    /// [`RecvFlags`] and the remote address from the data is coming. Unlike
    /// [`recv_from`] this allows passing multiple buffers.
    #[doc = man_links!(recvmsg(2))]
    ///
    /// [`recv_from`]: Socket::recv_from
    ///
    /// # Safety
    ///
    /// `recv_from_vectored` makes the same safety guarantees regarding `bufs`
    /// as [`recv_vectored`].
    ///
    /// [`recv_vectored`]: Socket::recv_vectored
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn recv_from_vectored(
        &self,
        bufs: &mut [MaybeUninitSlice<'_>],
    ) -> io::Result<(usize, RecvFlags, SockAddr)> {
        self.recv_from_vectored_with_flags(bufs, 0)
    }

    /// Identical to [`recv_from_vectored`] but allows for specification of
    /// arbitrary flags to the underlying `recvmsg`/`WSARecvFrom` call.
    ///
    /// [`recv_from_vectored`]: Socket::recv_from_vectored
    ///
    /// # Safety
    ///
    /// `recv_from_vectored` makes the same safety guarantees regarding `bufs`
    /// as [`recv_vectored`].
    ///
    /// [`recv_vectored`]: Socket::recv_vectored
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn recv_from_vectored_with_flags(
        &self,
        bufs: &mut [MaybeUninitSlice<'_>],
        flags: c_int,
    ) -> io::Result<(usize, RecvFlags, SockAddr)> {
        sys::recv_from_vectored(self.as_raw(), bufs, flags)
    }

    /// Receives data from the socket, without removing it from the queue.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recvfrom` system call.
    ///
    /// On success, returns the number of bytes peeked and the address from
    /// whence the data came.
    ///
    /// # Safety
    ///
    /// `peek_from` makes the same safety guarantees regarding the `buf`fer as
    /// [`recv`].
    ///
    /// # Note: Datagram Sockets
    /// For datagram sockets, the behavior of this method when `buf` is smaller than
    /// the datagram at the head of the receive queue differs between Windows and
    /// Unix-like platforms (Linux, macOS, BSDs, etc: colloquially termed "*nix").
    ///
    /// On *nix platforms, the datagram is truncated to the length of `buf`.
    ///
    /// On Windows, an error corresponding to `WSAEMSGSIZE` will be returned.
    ///
    /// For consistency between platforms, be sure to provide a sufficiently large buffer to avoid
    /// truncation; the exact size required depends on the underlying protocol.
    ///
    /// If you just want to know the sender of the data, try [`peek_sender`].
    ///
    /// [`recv`]: Socket::recv
    /// [`peek_sender`]: Socket::peek_sender
    pub fn peek_from(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<(usize, SockAddr)> {
        self.recv_from_with_flags(buf, sys::MSG_PEEK)
    }

    /// Retrieve the sender for the data at the head of the receive queue.
    ///
    /// This is equivalent to calling [`peek_from`] with a zero-sized buffer,
    /// but suppresses the `WSAEMSGSIZE` error on Windows.
    ///
    /// [`peek_from`]: Socket::peek_from
    pub fn peek_sender(&self) -> io::Result<SockAddr> {
        sys::peek_sender(self.as_raw())
    }

    /// Receive a message from a socket using a message structure.
    ///
    /// This is not supported on Windows as calling `WSARecvMsg` (the `recvmsg`
    /// equivalent) is not straight forward on Windows. See
    /// <https://github.com/microsoft/Windows-classic-samples/blob/7cbd99ac1d2b4a0beffbaba29ea63d024ceff700/Samples/Win7Samples/netds/winsock/recvmsg/rmmc.cpp>
    /// for an example (in C++).
    #[doc = man_links!(recvmsg(2))]
    #[cfg(all(unix, not(target_os = "redox")))]
    #[cfg_attr(docsrs, doc(cfg(all(unix, not(target_os = "redox")))))]
    pub fn recvmsg(&self, msg: &mut MsgHdrMut<'_, '_, '_>, flags: sys::c_int) -> io::Result<usize> {
        sys::recvmsg(self.as_raw(), msg, flags)
    }

    /// Sends data on the socket to a connected peer.
    ///
    /// This is typically used on TCP sockets or datagram sockets which have
    /// been connected.
    ///
    /// On success returns the number of bytes that were sent.
    #[doc = man_links!(send(2))]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.send_with_flags(buf, 0)
    }

    /// Identical to [`send`] but allows for specification of arbitrary flags to the underlying
    /// `send` call.
    ///
    /// [`send`]: Socket::send
    pub fn send_with_flags(&self, buf: &[u8], flags: c_int) -> io::Result<usize> {
        sys::send(self.as_raw(), buf, flags)
    }

    /// Send data to the connected peer. Returns the amount of bytes written.
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn send_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_vectored_with_flags(bufs, 0)
    }

    /// Identical to [`send_vectored`] but allows for specification of arbitrary
    /// flags to the underlying `sendmsg`/`WSASend` call.
    #[doc = man_links!(sendmsg(2))]
    ///
    /// [`send_vectored`]: Socket::send_vectored
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn send_vectored_with_flags(
        &self,
        bufs: &[IoSlice<'_>],
        flags: c_int,
    ) -> io::Result<usize> {
        sys::send_vectored(self.as_raw(), bufs, flags)
    }

    /// Sends out-of-band (OOB) data on the socket to connected peer
    /// by setting the `MSG_OOB` flag for this call.
    ///
    /// For more information, see [`send`], [`out_of_band_inline`].
    ///
    /// [`send`]: Socket::send
    /// [`out_of_band_inline`]: Socket::out_of_band_inline
    #[cfg_attr(target_os = "redox", allow(rustdoc::broken_intra_doc_links))]
    pub fn send_out_of_band(&self, buf: &[u8]) -> io::Result<usize> {
        self.send_with_flags(buf, sys::MSG_OOB)
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// This is typically used on UDP or datagram-oriented sockets.
    #[doc = man_links!(sendto(2))]
    pub fn send_to(&self, buf: &[u8], addr: &SockAddr) -> io::Result<usize> {
        self.send_to_with_flags(buf, addr, 0)
    }

    /// Identical to [`send_to`] but allows for specification of arbitrary flags
    /// to the underlying `sendto` call.
    ///
    /// [`send_to`]: Socket::send_to
    pub fn send_to_with_flags(
        &self,
        buf: &[u8],
        addr: &SockAddr,
        flags: c_int,
    ) -> io::Result<usize> {
        sys::send_to(self.as_raw(), buf, addr, flags)
    }

    /// Send data to a peer listening on `addr`. Returns the amount of bytes
    /// written.
    #[doc = man_links!(sendmsg(2))]
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn send_to_vectored(&self, bufs: &[IoSlice<'_>], addr: &SockAddr) -> io::Result<usize> {
        self.send_to_vectored_with_flags(bufs, addr, 0)
    }

    /// Identical to [`send_to_vectored`] but allows for specification of
    /// arbitrary flags to the underlying `sendmsg`/`WSASendTo` call.
    ///
    /// [`send_to_vectored`]: Socket::send_to_vectored
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn send_to_vectored_with_flags(
        &self,
        bufs: &[IoSlice<'_>],
        addr: &SockAddr,
        flags: c_int,
    ) -> io::Result<usize> {
        sys::send_to_vectored(self.as_raw(), bufs, addr, flags)
    }

    /// Send a message on a socket using a message structure.
    #[doc = man_links!(sendmsg(2))]
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn sendmsg(&self, msg: &MsgHdr<'_, '_, '_>, flags: sys::c_int) -> io::Result<usize> {
        sys::sendmsg(self.as_raw(), msg, flags)
    }
}

/// Set `SOCK_CLOEXEC` and `NO_HANDLE_INHERIT` on the `ty`pe on platforms that
/// support it.
#[inline(always)]
const fn set_common_type(ty: Type) -> Type {
    // On platforms that support it set `SOCK_CLOEXEC`.
    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
    ))]
    let ty = ty._cloexec();

    // On windows set `NO_HANDLE_INHERIT`.
    #[cfg(windows)]
    let ty = ty._no_inherit();

    ty
}

/// Set `FD_CLOEXEC` and `NOSIGPIPE` on the `socket` for platforms that need it.
#[inline(always)]
#[allow(clippy::unnecessary_wraps)]
fn set_common_flags(socket: Socket) -> io::Result<Socket> {
    // On platforms that don't have `SOCK_CLOEXEC` use `FD_CLOEXEC`.
    #[cfg(all(
        unix,
        not(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "hurd",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "espidf",
            target_os = "vita",
            target_os = "cygwin",
        ))
    ))]
    socket._set_cloexec(true)?;

    // On Apple platforms set `NOSIGPIPE`.
    #[cfg(any(
        target_os = "ios",
        target_os = "visionos",
        target_os = "macos",
        target_os = "tvos",
        target_os = "watchos",
    ))]
    socket._set_nosigpipe(true)?;

    Ok(socket)
}

/// A local interface specified by its index or an address assigned to it.
///
/// `Index(0)` and `Address(Ipv4Addr::UNSPECIFIED)` are equivalent and indicate
/// that an appropriate interface should be selected by the system.
#[cfg(not(any(
    target_os = "haiku",
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "solaris",
)))]
#[derive(Debug)]
pub enum InterfaceIndexOrAddress {
    /// An interface index.
    Index(u32),
    /// An address assigned to an interface.
    Address(Ipv4Addr),
}

/// Socket options get/set using `SOL_SOCKET`.
///
/// Additional documentation can be found in documentation of the OS.
/// * Linux: <https://man7.org/linux/man-pages/man7/socket.7.html>
/// * Windows: <https://docs.microsoft.com/en-us/windows/win32/winsock/sol-socket-socket-options>
impl Socket {
    /// Get the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see [`set_broadcast`].
    ///
    /// [`set_broadcast`]: Socket::set_broadcast
    pub fn broadcast(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_BROADCAST)
                .map(|broadcast| broadcast != 0)
        }
    }

    /// Set the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_BROADCAST,
                broadcast as c_int,
            )
        }
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        match unsafe { getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_ERROR) } {
            Ok(0) => Ok(None),
            Ok(errno) => Ok(Some(io::Error::from_raw_os_error(errno))),
            Err(err) => Err(err),
        }
    }

    /// Get the value of the `SO_KEEPALIVE` option on this socket.
    ///
    /// For more information about this option, see [`set_keepalive`].
    ///
    /// [`set_keepalive`]: Socket::set_keepalive
    pub fn keepalive(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<Bool>(self.as_raw(), sys::SOL_SOCKET, sys::SO_KEEPALIVE)
                .map(|keepalive| keepalive != 0)
        }
    }

    /// Set value for the `SO_KEEPALIVE` option on this socket.
    ///
    /// Enable sending of keep-alive messages on connection-oriented sockets.
    pub fn set_keepalive(&self, keepalive: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_KEEPALIVE,
                keepalive as c_int,
            )
        }
    }

    /// Get the value of the `SO_LINGER` option on this socket.
    ///
    /// For more information about this option, see [`set_linger`].
    ///
    /// [`set_linger`]: Socket::set_linger
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        unsafe {
            getsockopt::<sys::linger>(self.as_raw(), sys::SOL_SOCKET, sys::SO_LINGER)
                .map(from_linger)
        }
    }

    /// Set value for the `SO_LINGER` option on this socket.
    ///
    /// If `linger` is not `None`, a close(2) or shutdown(2) will not return
    /// until all queued messages for the socket have been successfully sent or
    /// the linger timeout has been reached. Otherwise, the call returns
    /// immediately and the closing is done in the background. When the socket
    /// is closed as part of exit(2), it always lingers in the background.
    ///
    /// # Notes
    ///
    /// On most OSs the duration only has a precision of seconds and will be
    /// silently truncated.
    ///
    /// On Apple platforms (e.g. macOS, iOS, etc) this uses `SO_LINGER_SEC`.
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        let linger = into_linger(linger);
        unsafe { setsockopt(self.as_raw(), sys::SOL_SOCKET, sys::SO_LINGER, linger) }
    }

    /// Get value for the `SO_OOBINLINE` option on this socket.
    ///
    /// For more information about this option, see [`set_out_of_band_inline`].
    ///
    /// [`set_out_of_band_inline`]: Socket::set_out_of_band_inline
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn out_of_band_inline(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_OOBINLINE)
                .map(|oob_inline| oob_inline != 0)
        }
    }

    /// Set value for the `SO_OOBINLINE` option on this socket.
    ///
    /// If this option is enabled, out-of-band data is directly placed into the
    /// receive data stream. Otherwise, out-of-band data is passed only when the
    /// `MSG_OOB` flag is set during receiving. As per RFC6093, TCP sockets
    /// using the Urgent mechanism are encouraged to set this flag.
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_os = "redox"))))]
    pub fn set_out_of_band_inline(&self, oob_inline: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_OOBINLINE,
                oob_inline as c_int,
            )
        }
    }

    /// Get value for the `SO_PASSCRED` option on this socket.
    ///
    /// For more information about this option, see [`set_passcred`].
    ///
    /// [`set_passcred`]: Socket::set_passcred
    #[cfg(any(target_os = "linux", target_os = "cygwin"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "linux", target_os = "cygwin"))))]
    pub fn passcred(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_PASSCRED)
                .map(|passcred| passcred != 0)
        }
    }

    /// Set value for the `SO_PASSCRED` option on this socket.
    ///
    /// If this option is enabled, enables the receiving of the `SCM_CREDENTIALS`
    /// control messages.
    #[cfg(any(target_os = "linux", target_os = "cygwin"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "linux", target_os = "cygwin"))))]
    pub fn set_passcred(&self, passcred: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_PASSCRED,
                passcred as c_int,
            )
        }
    }

    /// Get value for the `SO_RCVBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_recv_buffer_size`].
    ///
    /// [`set_recv_buffer_size`]: Socket::set_recv_buffer_size
    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_RCVBUF)
                .map(|size| size as usize)
        }
    }

    /// Set value for the `SO_RCVBUF` option on this socket.
    ///
    /// Changes the size of the operating system's receive buffer associated
    /// with the socket.
    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_RCVBUF,
                size as c_int,
            )
        }
    }

    /// Get value for the `SO_RCVTIMEO` option on this socket.
    ///
    /// If the returned timeout is `None`, then `read` and `recv` calls will
    /// block indefinitely.
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        sys::timeout_opt(self.as_raw(), sys::SOL_SOCKET, sys::SO_RCVTIMEO)
    }

    /// Set value for the `SO_RCVTIMEO` option on this socket.
    ///
    /// If `timeout` is `None`, then `read` and `recv` calls will block
    /// indefinitely.
    pub fn set_read_timeout(&self, duration: Option<Duration>) -> io::Result<()> {
        sys::set_timeout_opt(self.as_raw(), sys::SOL_SOCKET, sys::SO_RCVTIMEO, duration)
    }

    /// Get the value of the `SO_REUSEADDR` option on this socket.
    ///
    /// For more information about this option, see [`set_reuse_address`].
    ///
    /// [`set_reuse_address`]: Socket::set_reuse_address
    pub fn reuse_address(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_REUSEADDR)
                .map(|reuse| reuse != 0)
        }
    }

    /// Set value for the `SO_REUSEADDR` option on this socket.
    ///
    /// This indicates that further calls to `bind` may allow reuse of local
    /// addresses. For IPv4 sockets this means that a socket may bind even when
    /// there's a socket already listening on this port.
    pub fn set_reuse_address(&self, reuse: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_REUSEADDR,
                reuse as c_int,
            )
        }
    }

    /// Get the value of the `SO_SNDBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_send_buffer_size`].
    ///
    /// [`set_send_buffer_size`]: Socket::set_send_buffer_size
    pub fn send_buffer_size(&self) -> io::Result<usize> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::SOL_SOCKET, sys::SO_SNDBUF)
                .map(|size| size as usize)
        }
    }

    /// Set value for the `SO_SNDBUF` option on this socket.
    ///
    /// Changes the size of the operating system's send buffer associated with
    /// the socket.
    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::SOL_SOCKET,
                sys::SO_SNDBUF,
                size as c_int,
            )
        }
    }

    /// Get value for the `SO_SNDTIMEO` option on this socket.
    ///
    /// If the returned timeout is `None`, then `write` and `send` calls will
    /// block indefinitely.
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        sys::timeout_opt(self.as_raw(), sys::SOL_SOCKET, sys::SO_SNDTIMEO)
    }

    /// Set value for the `SO_SNDTIMEO` option on this socket.
    ///
    /// If `timeout` is `None`, then `write` and `send` calls will block
    /// indefinitely.
    pub fn set_write_timeout(&self, duration: Option<Duration>) -> io::Result<()> {
        sys::set_timeout_opt(self.as_raw(), sys::SOL_SOCKET, sys::SO_SNDTIMEO, duration)
    }
}

const fn from_linger(linger: sys::linger) -> Option<Duration> {
    if linger.l_onoff == 0 {
        None
    } else {
        Some(Duration::from_secs(linger.l_linger as u64))
    }
}

const fn into_linger(duration: Option<Duration>) -> sys::linger {
    match duration {
        Some(duration) => sys::linger {
            l_onoff: 1,
            l_linger: duration.as_secs() as _,
        },
        None => sys::linger {
            l_onoff: 0,
            l_linger: 0,
        },
    }
}

/// Socket options for IPv4 sockets, get/set using `IPPROTO_IP`.
///
/// Additional documentation can be found in documentation of the OS.
/// * Linux: <https://man7.org/linux/man-pages/man7/ip.7.html>
/// * Windows: <https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options>
impl Socket {
    /// This method is deprecated, use [`crate::Socket::header_included_v4`].
    #[cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    #[deprecated = "Use `Socket::header_included_v4` instead"]
    pub fn header_included(&self) -> io::Result<bool> {
        self.header_included_v4()
    }
    /// Get the value of the `IP_HDRINCL` option on this socket.
    ///
    /// For more information about this option, see [`set_header_included`].
    ///
    /// [`set_header_included`]: Socket::set_header_included
    #[cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    pub fn header_included_v4(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, sys::IP_HDRINCL)
                .map(|included| included != 0)
        }
    }

    /// This method is deprecated, use [`crate::Socket::set_header_included_v4`].
    #[cfg_attr(
        any(target_os = "fuchsia", target_os = "illumos", target_os = "solaris"),
        allow(rustdoc::broken_intra_doc_links)
    )]
    #[cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    #[deprecated = "Use `Socket::set_header_included_v4` instead"]
    pub fn set_header_included(&self, included: bool) -> io::Result<()> {
        self.set_header_included_v4(included)
    }

    /// Set the value of the `IP_HDRINCL` option on this socket.
    ///
    /// If enabled, the user supplies an IP header in front of the user data.
    /// Valid only for [`SOCK_RAW`] sockets; see [raw(7)] for more information.
    /// When this flag is enabled, the values set by `IP_OPTIONS`, [`IP_TTL`],
    /// and [`IP_TOS`] are ignored.
    ///
    /// [`SOCK_RAW`]: Type::RAW
    /// [raw(7)]: https://man7.org/linux/man-pages/man7/raw.7.html
    /// [`IP_TTL`]: Socket::set_ttl
    /// [`IP_TOS`]: Socket::set_tos
    #[cfg_attr(
        any(target_os = "fuchsia", target_os = "illumos", target_os = "solaris"),
        allow(rustdoc::broken_intra_doc_links)
    )]
    #[cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    pub fn set_header_included_v4(&self, included: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_HDRINCL,
                included as c_int,
            )
        }
    }

    /// Get the value of the `IP_TRANSPARENT` option on this socket.
    ///
    /// For more information about this option, see [`set_ip_transparent`].
    ///
    /// [`set_ip_transparent`]: Socket::set_ip_transparent
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub fn ip_transparent(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, libc::IP_TRANSPARENT)
                .map(|transparent| transparent != 0)
        }
    }

    /// Set the value of the `IP_TRANSPARENT` option on this socket.
    ///
    /// Setting this boolean option enables transparent proxying
    /// on this socket.  This socket option allows the calling
    /// application to bind to a nonlocal IP address and operate
    /// both as a client and a server with the foreign address as
    /// the local endpoint.  NOTE: this requires that routing be
    /// set up in a way that packets going to the foreign address
    /// are routed through the TProxy box (i.e., the system
    /// hosting the application that employs the IP_TRANSPARENT
    /// socket option).  Enabling this socket option requires
    /// superuser privileges (the `CAP_NET_ADMIN` capability).
    ///
    /// TProxy redirection with the iptables TPROXY target also
    /// requires that this option be set on the redirected socket.
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub fn set_ip_transparent(&self, transparent: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                libc::IP_TRANSPARENT,
                transparent as c_int,
            )
        }
    }

    /// Join a multicast group using `IP_ADD_MEMBERSHIP` option on this socket.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's [`Ipv4Addr::UNSPECIFIED`] (`INADDR_ANY`) then
    /// an appropriate interface is chosen by the system.
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        let mreq = sys::IpMreq {
            imr_multiaddr: sys::to_in_addr(multiaddr),
            imr_interface: sys::to_in_addr(interface),
        };
        unsafe { setsockopt(self.as_raw(), sys::IPPROTO_IP, sys::IP_ADD_MEMBERSHIP, mreq) }
    }

    /// Leave a multicast group using `IP_DROP_MEMBERSHIP` option on this socket.
    ///
    /// For more information about this option, see [`join_multicast_v4`].
    ///
    /// [`join_multicast_v4`]: Socket::join_multicast_v4
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        let mreq = sys::IpMreq {
            imr_multiaddr: sys::to_in_addr(multiaddr),
            imr_interface: sys::to_in_addr(interface),
        };
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_DROP_MEMBERSHIP,
                mreq,
            )
        }
    }

    /// Join a multicast group using `IP_ADD_MEMBERSHIP` option on this socket.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` specifies
    /// the local interface with which the system should join the multicast
    /// group. See [`InterfaceIndexOrAddress`].
    #[cfg(not(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
        target_os = "cygwin",
    )))]
    pub fn join_multicast_v4_n(
        &self,
        multiaddr: &Ipv4Addr,
        interface: &InterfaceIndexOrAddress,
    ) -> io::Result<()> {
        let mreqn = sys::to_mreqn(multiaddr, interface);
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_ADD_MEMBERSHIP,
                mreqn,
            )
        }
    }

    /// Leave a multicast group using `IP_DROP_MEMBERSHIP` option on this socket.
    ///
    /// For more information about this option, see [`join_multicast_v4_n`].
    ///
    /// [`join_multicast_v4_n`]: Socket::join_multicast_v4_n
    #[cfg(not(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
        target_os = "cygwin",
    )))]
    pub fn leave_multicast_v4_n(
        &self,
        multiaddr: &Ipv4Addr,
        interface: &InterfaceIndexOrAddress,
    ) -> io::Result<()> {
        let mreqn = sys::to_mreqn(multiaddr, interface);
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_DROP_MEMBERSHIP,
                mreqn,
            )
        }
    }

    /// Join a multicast SSM channel using `IP_ADD_SOURCE_MEMBERSHIP` option on this socket.
    ///
    /// This function specifies a new multicast channel for this socket to join.
    /// The group must be a valid SSM group address, the source must be the address of the sender
    /// and `interface` is the address of the local interface with which the system should join the
    /// multicast group. If it's [`Ipv4Addr::UNSPECIFIED`] (`INADDR_ANY`) then
    /// an appropriate interface is chosen by the system.
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "fuchsia",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
    )))]
    pub fn join_ssm_v4(
        &self,
        source: &Ipv4Addr,
        group: &Ipv4Addr,
        interface: &Ipv4Addr,
    ) -> io::Result<()> {
        let mreqs = sys::IpMreqSource {
            imr_multiaddr: sys::to_in_addr(group),
            imr_interface: sys::to_in_addr(interface),
            imr_sourceaddr: sys::to_in_addr(source),
        };
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_ADD_SOURCE_MEMBERSHIP,
                mreqs,
            )
        }
    }

    /// Leave a multicast group using `IP_DROP_SOURCE_MEMBERSHIP` option on this socket.
    ///
    /// For more information about this option, see [`join_ssm_v4`].
    ///
    /// [`join_ssm_v4`]: Socket::join_ssm_v4
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "fuchsia",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
    )))]
    pub fn leave_ssm_v4(
        &self,
        source: &Ipv4Addr,
        group: &Ipv4Addr,
        interface: &Ipv4Addr,
    ) -> io::Result<()> {
        let mreqs = sys::IpMreqSource {
            imr_multiaddr: sys::to_in_addr(group),
            imr_interface: sys::to_in_addr(interface),
            imr_sourceaddr: sys::to_in_addr(source),
        };
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_DROP_SOURCE_MEMBERSHIP,
                mreqs,
            )
        }
    }

    /// Get the value of the `IP_MULTICAST_ALL` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_all_v4`].
    ///
    /// [`set_multicast_all_v4`]: Socket::set_multicast_all_v4
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub fn multicast_all_v4(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, libc::IP_MULTICAST_ALL)
                .map(|all| all != 0)
        }
    }

    /// Set the value of the `IP_MULTICAST_ALL` option for this socket.
    ///
    /// This option can be used to modify the delivery policy of
    /// multicast messages.  The argument is a boolean
    /// (defaults to true).  If set to true, the socket will receive
    /// messages from all the groups that have been joined
    /// globally on the whole system.  Otherwise, it will deliver
    /// messages only from the groups that have been explicitly
    /// joined (for example via the `IP_ADD_MEMBERSHIP` option) on
    /// this particular socket.
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub fn set_multicast_all_v4(&self, all: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                libc::IP_MULTICAST_ALL,
                all as c_int,
            )
        }
    }

    /// Get the value of the `IP_MULTICAST_IF` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_if_v4`].
    ///
    /// [`set_multicast_if_v4`]: Socket::set_multicast_if_v4
    pub fn multicast_if_v4(&self) -> io::Result<Ipv4Addr> {
        unsafe {
            getsockopt(self.as_raw(), sys::IPPROTO_IP, sys::IP_MULTICAST_IF).map(sys::from_in_addr)
        }
    }

    /// Set the value of the `IP_MULTICAST_IF` option for this socket.
    ///
    /// Specifies the interface to use for routing multicast packets.
    pub fn set_multicast_if_v4(&self, interface: &Ipv4Addr) -> io::Result<()> {
        let interface = sys::to_in_addr(interface);
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_MULTICAST_IF,
                interface,
            )
        }
    }

    /// Get the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_loop_v4`].
    ///
    /// [`set_multicast_loop_v4`]: Socket::set_multicast_loop_v4
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, sys::IP_MULTICAST_LOOP)
                .map(|loop_v4| loop_v4 != 0)
        }
    }

    /// Set the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_loop_v4(&self, loop_v4: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_MULTICAST_LOOP,
                loop_v4 as c_int,
            )
        }
    }

    /// Get the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_ttl_v4`].
    ///
    /// [`set_multicast_ttl_v4`]: Socket::set_multicast_ttl_v4
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, sys::IP_MULTICAST_TTL)
                .map(|ttl| ttl as u32)
        }
    }

    /// Set the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_MULTICAST_TTL,
                ttl as c_int,
            )
        }
    }

    /// Get the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`].
    ///
    /// [`set_ttl`]: Socket::set_ttl
    pub fn ttl(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, sys::IP_TTL).map(|ttl| ttl as u32)
        }
    }

    /// Set the value of the `IP_TTL` option for this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        unsafe { setsockopt(self.as_raw(), sys::IPPROTO_IP, sys::IP_TTL, ttl as c_int) }
    }

    /// Set the value of the `IP_TOS` option for this socket.
    ///
    /// This value sets the type-of-service field that is used in every packet
    /// sent from this socket.
    ///
    /// NOTE: <https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options>
    /// documents that not all versions of windows support `IP_TOS`.
    #[cfg(not(any(
        target_os = "fuchsia",
        target_os = "redox",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
    )))]
    pub fn set_tos(&self, tos: u32) -> io::Result<()> {
        unsafe { setsockopt(self.as_raw(), sys::IPPROTO_IP, sys::IP_TOS, tos as c_int) }
    }

    /// Get the value of the `IP_TOS` option for this socket.
    ///
    /// For more information about this option, see [`set_tos`].
    ///
    /// NOTE: <https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options>
    /// documents that not all versions of windows support `IP_TOS`.
    ///
    /// [`set_tos`]: Socket::set_tos
    #[cfg(not(any(
        target_os = "fuchsia",
        target_os = "redox",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
    )))]
    pub fn tos(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, sys::IP_TOS).map(|tos| tos as u32)
        }
    }

    /// Set the value of the `IP_RECVTOS` option for this socket.
    ///
    /// If enabled, the `IP_TOS` ancillary message is passed with
    /// incoming packets. It contains a byte which specifies the
    /// Type of Service/Precedence field of the packet header.
    #[cfg(not(any(
        target_os = "aix",
        target_os = "dragonfly",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "haiku",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
        target_os = "cygwin",
    )))]
    pub fn set_recv_tos(&self, recv_tos: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IP,
                sys::IP_RECVTOS,
                recv_tos as c_int,
            )
        }
    }

    /// Get the value of the `IP_RECVTOS` option for this socket.
    ///
    /// For more information about this option, see [`set_recv_tos`].
    ///
    /// [`set_recv_tos`]: Socket::set_recv_tos
    #[cfg(not(any(
        target_os = "aix",
        target_os = "dragonfly",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "haiku",
        target_os = "nto",
        target_os = "espidf",
        target_os = "vita",
        target_os = "cygwin",
    )))]
    pub fn recv_tos(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IP, sys::IP_RECVTOS)
                .map(|recv_tos| recv_tos > 0)
        }
    }
}

/// Socket options for IPv6 sockets, get/set using `IPPROTO_IPV6`.
///
/// Additional documentation can be found in documentation of the OS.
/// * Linux: <https://man7.org/linux/man-pages/man7/ipv6.7.html>
/// * Windows: <https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ipv6-socket-options>
impl Socket {
    /// Get the value of the `IP_HDRINCL` option on this socket.
    ///
    /// For more information about this option, see [`set_header_included`].
    ///
    /// [`set_header_included`]: Socket::set_header_included
    #[cfg(all(
        feature = "all",
        not(any(
            target_os = "redox",
            target_os = "espidf",
            target_os = "openbsd",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd"
        ))
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    pub fn header_included_v6(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IP_HDRINCL)
                .map(|included| included != 0)
        }
    }

    /// Set the value of the `IP_HDRINCL` option on this socket.
    ///
    /// If enabled, the user supplies an IP header in front of the user data.
    /// Valid only for [`SOCK_RAW`] sockets; see [raw(7)] for more information.
    /// When this flag is enabled, the values set by `IP_OPTIONS` are ignored.
    ///
    /// [`SOCK_RAW`]: Type::RAW
    /// [raw(7)]: https://man7.org/linux/man-pages/man7/raw.7.html
    #[cfg_attr(
        any(target_os = "fuchsia", target_os = "illumos", target_os = "solaris"),
        allow(rustdoc::broken_intra_doc_links)
    )]
    #[cfg(all(
        feature = "all",
        not(any(
            target_os = "redox",
            target_os = "espidf",
            target_os = "openbsd",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd"
        ))
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "all", not(any(target_os = "redox", target_os = "espidf")))))
    )]
    pub fn set_header_included_v6(&self, included: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IP_HDRINCL,
                included as c_int,
            )
        }
    }

    /// Join a multicast group using `IPV6_ADD_MEMBERSHIP` option on this socket.
    ///
    /// Some OSs use `IPV6_JOIN_GROUP` for this option.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    #[cfg(not(target_os = "nto"))]
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        let mreq = sys::Ipv6Mreq {
            ipv6mr_multiaddr: sys::to_in6_addr(multiaddr),
            // NOTE: some OSs use `c_int`, others use `c_uint`.
            ipv6mr_interface: interface as _,
        };
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_ADD_MEMBERSHIP,
                mreq,
            )
        }
    }

    /// Leave a multicast group using `IPV6_DROP_MEMBERSHIP` option on this socket.
    ///
    /// Some OSs use `IPV6_LEAVE_GROUP` for this option.
    ///
    /// For more information about this option, see [`join_multicast_v6`].
    ///
    /// [`join_multicast_v6`]: Socket::join_multicast_v6
    #[cfg(not(target_os = "nto"))]
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        let mreq = sys::Ipv6Mreq {
            ipv6mr_multiaddr: sys::to_in6_addr(multiaddr),
            // NOTE: some OSs use `c_int`, others use `c_uint`.
            ipv6mr_interface: interface as _,
        };
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_DROP_MEMBERSHIP,
                mreq,
            )
        }
    }

    /// Get the value of the `IPV6_MULTICAST_HOPS` option for this socket
    ///
    /// For more information about this option, see [`set_multicast_hops_v6`].
    ///
    /// [`set_multicast_hops_v6`]: Socket::set_multicast_hops_v6
    pub fn multicast_hops_v6(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_MULTICAST_HOPS)
                .map(|hops| hops as u32)
        }
    }

    /// Set the value of the `IPV6_MULTICAST_HOPS` option for this socket
    ///
    /// Indicates the number of "routers" multicast packets will transit for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    pub fn set_multicast_hops_v6(&self, hops: u32) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_MULTICAST_HOPS,
                hops as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_MULTICAST_ALL` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_all_v6`].
    ///
    /// [`set_multicast_all_v6`]: Socket::set_multicast_all_v6
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub fn multicast_all_v6(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, libc::IPV6_MULTICAST_ALL)
                .map(|all| all != 0)
        }
    }

    /// Set the value of the `IPV6_MULTICAST_ALL` option for this socket.
    ///
    /// This option can be used to modify the delivery policy of
    /// multicast messages.  The argument is a boolean
    /// (defaults to true).  If set to true, the socket will receive
    /// messages from all the groups that have been joined
    /// globally on the whole system.  Otherwise, it will deliver
    /// messages only from the groups that have been explicitly
    /// joined (for example via the `IPV6_ADD_MEMBERSHIP` option) on
    /// this particular socket.
    #[cfg(all(feature = "all", target_os = "linux"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "all", target_os = "linux"))))]
    pub fn set_multicast_all_v6(&self, all: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                libc::IPV6_MULTICAST_ALL,
                all as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_MULTICAST_IF` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_if_v6`].
    ///
    /// [`set_multicast_if_v6`]: Socket::set_multicast_if_v6
    pub fn multicast_if_v6(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_MULTICAST_IF)
                .map(|interface| interface as u32)
        }
    }

    /// Set the value of the `IPV6_MULTICAST_IF` option for this socket.
    ///
    /// Specifies the interface to use for routing multicast packets. Unlike
    /// ipv4, this is generally required in ipv6 contexts where network routing
    /// prefixes may overlap.
    pub fn set_multicast_if_v6(&self, interface: u32) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_MULTICAST_IF,
                interface as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_loop_v6`].
    ///
    /// [`set_multicast_loop_v6`]: Socket::set_multicast_loop_v6
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_MULTICAST_LOOP)
                .map(|loop_v6| loop_v6 != 0)
        }
    }

    /// Set the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this may not have any affect on IPv4 sockets.
    pub fn set_multicast_loop_v6(&self, loop_v6: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_MULTICAST_LOOP,
                loop_v6 as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_UNICAST_HOPS` option for this socket.
    ///
    /// Specifies the hop limit for ipv6 unicast packets
    pub fn unicast_hops_v6(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_UNICAST_HOPS)
                .map(|hops| hops as u32)
        }
    }

    /// Set the value for the `IPV6_UNICAST_HOPS` option on this socket.
    ///
    /// Specifies the hop limit for ipv6 unicast packets
    pub fn set_unicast_hops_v6(&self, hops: u32) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_UNICAST_HOPS,
                hops as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_V6ONLY` option for this socket.
    ///
    /// For more information about this option, see [`set_only_v6`].
    ///
    /// [`set_only_v6`]: Socket::set_only_v6
    pub fn only_v6(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_V6ONLY)
                .map(|only_v6| only_v6 != 0)
        }
    }

    /// Set the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// If this is set to `true` then the socket is restricted to sending and
    /// receiving IPv6 packets only. In this case two IPv4 and IPv6 applications
    /// can bind the same port at the same time.
    ///
    /// If this is set to `false` then the socket can be used to send and
    /// receive packets from an IPv4-mapped IPv6 address.
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_V6ONLY,
                only_v6 as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_RECVTCLASS` option for this socket.
    ///
    /// For more information about this option, see [`set_recv_tclass_v6`].
    ///
    /// [`set_recv_tclass_v6`]: Socket::set_recv_tclass_v6
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "espidf",
        target_os = "vita",
    )))]
    pub fn recv_tclass_v6(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_RECVTCLASS)
                .map(|recv_tclass| recv_tclass > 0)
        }
    }

    /// Set the value of the `IPV6_RECVTCLASS` option for this socket.
    ///
    /// If enabled, the `IPV6_TCLASS` ancillary message is passed with incoming
    /// packets. It contains a byte which specifies the traffic class field of
    /// the packet header.
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "solaris",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "espidf",
        target_os = "vita",
    )))]
    pub fn set_recv_tclass_v6(&self, recv_tclass: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_RECVTCLASS,
                recv_tclass as c_int,
            )
        }
    }

    /// Get the value of the `IPV6_RECVHOPLIMIT` option for this socket.
    ///
    /// For more information about this option, see [`set_recv_hoplimit_v6`].
    ///
    /// [`set_recv_hoplimit_v6`]: Socket::set_recv_hoplimit_v6
    #[cfg(all(
        feature = "all",
        not(any(
            windows,
            target_os = "dragonfly",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "redox",
            target_os = "solaris",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "espidf",
            target_os = "vita",
            target_os = "cygwin",
        ))
    ))]
    pub fn recv_hoplimit_v6(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_IPV6, sys::IPV6_RECVHOPLIMIT)
                .map(|recv_hoplimit| recv_hoplimit > 0)
        }
    }
    /// Set the value of the `IPV6_RECVHOPLIMIT` option for this socket.
    ///
    /// The received hop limit is returned as ancillary data by recvmsg()
    /// only if the application has enabled the IPV6_RECVHOPLIMIT socket
    /// option:
    #[cfg(all(
        feature = "all",
        not(any(
            windows,
            target_os = "dragonfly",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "redox",
            target_os = "solaris",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "espidf",
            target_os = "vita",
            target_os = "cygwin",
        ))
    ))]
    pub fn set_recv_hoplimit_v6(&self, recv_hoplimit: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_IPV6,
                sys::IPV6_RECVHOPLIMIT,
                recv_hoplimit as c_int,
            )
        }
    }
}

/// Socket options for TCP sockets, get/set using `IPPROTO_TCP`.
///
/// Additional documentation can be found in documentation of the OS.
/// * Linux: <https://man7.org/linux/man-pages/man7/tcp.7.html>
/// * Windows: <https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-tcp-socket-options>
impl Socket {
    /// Get the value of the `TCP_KEEPIDLE` option on this socket.
    ///
    /// This returns the value of `TCP_KEEPALIVE` on macOS and iOS and `TCP_KEEPIDLE` on all other
    /// supported Unix operating systems.
    #[cfg(all(
        feature = "all",
        not(any(
            windows,
            target_os = "haiku",
            target_os = "openbsd",
            target_os = "vita"
        ))
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            feature = "all",
            not(any(
                windows,
                target_os = "haiku",
                target_os = "openbsd",
                target_os = "vita"
            ))
        )))
    )]
    pub fn keepalive_time(&self) -> io::Result<Duration> {
        sys::keepalive_time(self.as_raw())
    }

    /// Get the value of the `TCP_KEEPINTVL` option on this socket.
    ///
    /// For more information about this option, see [`set_tcp_keepalive`].
    ///
    /// [`set_tcp_keepalive`]: Socket::set_tcp_keepalive
    #[cfg(all(
        feature = "all",
        any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "ios",
            target_os = "visionos",
            target_os = "linux",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "cygwin",
        )
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            feature = "all",
            any(
                target_os = "android",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "fuchsia",
                target_os = "illumos",
                target_os = "ios",
                target_os = "visionos",
                target_os = "linux",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "tvos",
                target_os = "watchos",
            )
        )))
    )]
    pub fn keepalive_interval(&self) -> io::Result<Duration> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_TCP, sys::TCP_KEEPINTVL)
                .map(|secs| Duration::from_secs(secs as u64))
        }
    }

    /// Get the value of the `TCP_KEEPCNT` option on this socket.
    ///
    /// For more information about this option, see [`set_tcp_keepalive`].
    ///
    /// [`set_tcp_keepalive`]: Socket::set_tcp_keepalive
    #[cfg(all(
        feature = "all",
        any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "ios",
            target_os = "visionos",
            target_os = "linux",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "cygwin",
        )
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            feature = "all",
            any(
                target_os = "android",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "fuchsia",
                target_os = "illumos",
                target_os = "ios",
                target_os = "visionos",
                target_os = "linux",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "tvos",
                target_os = "watchos",
            )
        )))
    )]
    pub fn keepalive_retries(&self) -> io::Result<u32> {
        unsafe {
            getsockopt::<c_int>(self.as_raw(), sys::IPPROTO_TCP, sys::TCP_KEEPCNT)
                .map(|retries| retries as u32)
        }
    }

    /// Set parameters configuring TCP keepalive probes for this socket.
    ///
    /// The supported parameters depend on the operating system, and are
    /// configured using the [`TcpKeepalive`] struct. At a minimum, all systems
    /// support configuring the [keepalive time]: the time after which the OS
    /// will start sending keepalive messages on an idle connection.
    ///
    /// [keepalive time]: TcpKeepalive::with_time
    ///
    /// # Notes
    ///
    /// * This will enable `SO_KEEPALIVE` on this socket, if it is not already
    ///   enabled.
    /// * On some platforms, such as Windows, any keepalive parameters *not*
    ///   configured by the `TcpKeepalive` struct passed to this function may be
    ///   overwritten with their default values. Therefore, this function should
    ///   either only be called once per socket, or the same parameters should
    ///   be passed every time it is called.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use socket2::{Socket, TcpKeepalive, Domain, Type};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    /// let keepalive = TcpKeepalive::new()
    ///     .with_time(Duration::from_secs(4));
    ///     // Depending on the target operating system, we may also be able to
    ///     // configure the keepalive probe interval and/or the number of
    ///     // retries here as well.
    ///
    /// socket.set_tcp_keepalive(&keepalive)?;
    /// # Ok(()) }
    /// ```
    ///
    pub fn set_tcp_keepalive(&self, params: &TcpKeepalive) -> io::Result<()> {
        self.set_keepalive(true)?;
        sys::set_tcp_keepalive(self.as_raw(), params)
    }

    /// Get the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`set_nodelay`].
    ///
    /// [`set_nodelay`]: Socket::set_nodelay
    pub fn nodelay(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<Bool>(self.as_raw(), sys::IPPROTO_TCP, sys::TCP_NODELAY)
                .map(|nodelay| nodelay != 0)
        }
    }

    /// Set the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this option disables the Nagle algorithm. This means that
    /// segments are always sent as soon as possible, even if there is only a
    /// small amount of data. When not set, data is buffered until there is a
    /// sufficient amount to send out, thereby avoiding the frequent sending of
    /// small packets.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw(),
                sys::IPPROTO_TCP,
                sys::TCP_NODELAY,
                nodelay as c_int,
            )
        }
    }

    /// Get the value for the `SO_ORIGINAL_DST` option on this socket.
    #[cfg(all(
        feature = "all",
        any(
            target_os = "android",
            target_os = "fuchsia",
            target_os = "linux",
            target_os = "windows",
        )
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            feature = "all",
            any(
                target_os = "android",
                target_os = "fuchsia",
                target_os = "linux",
                target_os = "windows",
            )
        )))
    )]
    pub fn original_dst(&self) -> io::Result<SockAddr> {
        sys::original_dst(self.as_raw())
    }

    /// Get the value for the `IP6T_SO_ORIGINAL_DST` option on this socket.
    #[cfg(all(
        feature = "all",
        any(target_os = "android", target_os = "linux", target_os = "windows")
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            feature = "all",
            any(target_os = "android", target_os = "linux", target_os = "windows")
        )))
    )]
    pub fn original_dst_ipv6(&self) -> io::Result<SockAddr> {
        sys::original_dst_ipv6(self.as_raw())
    }
}

impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Safety: the `recv` implementation promises not to write uninitialised
        // bytes to the `buf`fer, so this casting is safe.
        let buf = unsafe { &mut *(buf as *mut [u8] as *mut [MaybeUninit<u8>]) };
        self.recv(buf)
    }

    #[cfg(not(target_os = "redox"))]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        // Safety: both `IoSliceMut` and `MaybeUninitSlice` promise to have the
        // same layout, that of `iovec`/`WSABUF`. Furthermore, `recv_vectored`
        // promises to not write unitialised bytes to the `bufs` and pass it
        // directly to the `recvmsg` system call, so this is safe.
        let bufs = unsafe { &mut *(bufs as *mut [IoSliceMut<'_>] as *mut [MaybeUninitSlice<'_>]) };
        self.recv_vectored(bufs).map(|(n, _)| n)
    }
}

impl<'a> Read for &'a Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Safety: see other `Read::read` impl.
        let buf = unsafe { &mut *(buf as *mut [u8] as *mut [MaybeUninit<u8>]) };
        self.recv(buf)
    }

    #[cfg(not(target_os = "redox"))]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        // Safety: see other `Read::read` impl.
        let bufs = unsafe { &mut *(bufs as *mut [IoSliceMut<'_>] as *mut [MaybeUninitSlice<'_>]) };
        self.recv_vectored(bufs).map(|(n, _)| n)
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf)
    }

    #[cfg(not(target_os = "redox"))]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> Write for &'a Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf)
    }

    #[cfg(not(target_os = "redox"))]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.send_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Debug for Socket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Socket")
            .field("raw", &self.as_raw())
            .field("local_addr", &self.local_addr().ok())
            .field("peer_addr", &self.peer_addr().ok())
            .finish()
    }
}

from!(net::TcpStream, Socket);
from!(net::TcpListener, Socket);
from!(net::UdpSocket, Socket);
from!(Socket, net::TcpStream);
from!(Socket, net::TcpListener);
from!(Socket, net::UdpSocket);
