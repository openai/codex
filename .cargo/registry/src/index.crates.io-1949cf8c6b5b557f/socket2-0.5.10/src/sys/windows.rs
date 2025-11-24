// Copyright 2015 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::min;
use std::io::{self, IoSlice};
use std::marker::PhantomData;
use std::mem::{self, size_of, MaybeUninit};
use std::net::{self, Ipv4Addr, Ipv6Addr, Shutdown};
use std::os::windows::io::{
    AsRawSocket, AsSocket, BorrowedSocket, FromRawSocket, IntoRawSocket, OwnedSocket, RawSocket,
};
use std::path::Path;
use std::sync::Once;
use std::time::{Duration, Instant};
use std::{process, ptr, slice};

use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
use windows_sys::Win32::Networking::WinSock::{
    self, tcp_keepalive, FIONBIO, IN6_ADDR, IN6_ADDR_0, INVALID_SOCKET, IN_ADDR, IN_ADDR_0,
    POLLERR, POLLHUP, POLLRDNORM, POLLWRNORM, SD_BOTH, SD_RECEIVE, SD_SEND, SIO_KEEPALIVE_VALS,
    SOCKET_ERROR, WSABUF, WSAEMSGSIZE, WSAESHUTDOWN, WSAPOLLFD, WSAPROTOCOL_INFOW,
    WSA_FLAG_NO_HANDLE_INHERIT, WSA_FLAG_OVERLAPPED,
};
#[cfg(feature = "all")]
use windows_sys::Win32::Networking::WinSock::{
    IP6T_SO_ORIGINAL_DST, SOL_IP, SO_ORIGINAL_DST, SO_PROTOCOL_INFOW,
};
use windows_sys::Win32::System::Threading::INFINITE;

use crate::{MsgHdr, RecvFlags, SockAddr, TcpKeepalive, Type};

#[allow(non_camel_case_types)]
pub(crate) type c_int = std::os::raw::c_int;

/// Fake MSG_TRUNC flag for the [`RecvFlags`] struct.
///
/// The flag is enabled when a `WSARecv[From]` call returns `WSAEMSGSIZE`. The
/// value of the flag is defined by us.
pub(crate) const MSG_TRUNC: c_int = 0x01;

// Used in `Domain`.
pub(crate) const AF_INET: c_int = windows_sys::Win32::Networking::WinSock::AF_INET as c_int;
pub(crate) const AF_INET6: c_int = windows_sys::Win32::Networking::WinSock::AF_INET6 as c_int;
pub(crate) const AF_UNIX: c_int = windows_sys::Win32::Networking::WinSock::AF_UNIX as c_int;
pub(crate) const AF_UNSPEC: c_int = windows_sys::Win32::Networking::WinSock::AF_UNSPEC as c_int;
// Used in `Type`.
pub(crate) const SOCK_STREAM: c_int = windows_sys::Win32::Networking::WinSock::SOCK_STREAM as c_int;
pub(crate) const SOCK_DGRAM: c_int = windows_sys::Win32::Networking::WinSock::SOCK_DGRAM as c_int;
pub(crate) const SOCK_RAW: c_int = windows_sys::Win32::Networking::WinSock::SOCK_RAW as c_int;
const SOCK_RDM: c_int = windows_sys::Win32::Networking::WinSock::SOCK_RDM as c_int;
pub(crate) const SOCK_SEQPACKET: c_int =
    windows_sys::Win32::Networking::WinSock::SOCK_SEQPACKET as c_int;
// Used in `Protocol`.
pub(crate) use windows_sys::Win32::Networking::WinSock::{
    IPPROTO_ICMP, IPPROTO_ICMPV6, IPPROTO_TCP, IPPROTO_UDP,
};
// Used in `SockAddr`.
pub(crate) use windows_sys::Win32::Networking::WinSock::{
    SOCKADDR as sockaddr, SOCKADDR_IN as sockaddr_in, SOCKADDR_IN6 as sockaddr_in6,
    SOCKADDR_STORAGE as sockaddr_storage,
};
#[allow(non_camel_case_types)]
pub(crate) type sa_family_t = windows_sys::Win32::Networking::WinSock::ADDRESS_FAMILY;
#[allow(non_camel_case_types)]
pub(crate) type socklen_t = windows_sys::Win32::Networking::WinSock::socklen_t;
// Used in `Socket`.
#[cfg(feature = "all")]
pub(crate) use windows_sys::Win32::Networking::WinSock::IP_HDRINCL;
pub(crate) use windows_sys::Win32::Networking::WinSock::{
    IPPROTO_IPV6, IPV6_ADD_MEMBERSHIP, IPV6_DROP_MEMBERSHIP, IPV6_MREQ as Ipv6Mreq,
    IPV6_MULTICAST_HOPS, IPV6_MULTICAST_IF, IPV6_MULTICAST_LOOP, IPV6_RECVTCLASS,
    IPV6_UNICAST_HOPS, IPV6_V6ONLY, IP_ADD_MEMBERSHIP, IP_ADD_SOURCE_MEMBERSHIP,
    IP_DROP_MEMBERSHIP, IP_DROP_SOURCE_MEMBERSHIP, IP_MREQ as IpMreq,
    IP_MREQ_SOURCE as IpMreqSource, IP_MULTICAST_IF, IP_MULTICAST_LOOP, IP_MULTICAST_TTL,
    IP_RECVTOS, IP_TOS, IP_TTL, LINGER as linger, MSG_OOB, MSG_PEEK, SO_BROADCAST, SO_ERROR,
    SO_KEEPALIVE, SO_LINGER, SO_OOBINLINE, SO_RCVBUF, SO_RCVTIMEO, SO_REUSEADDR, SO_SNDBUF,
    SO_SNDTIMEO, SO_TYPE, TCP_NODELAY,
};
pub(crate) const IPPROTO_IP: c_int = windows_sys::Win32::Networking::WinSock::IPPROTO_IP as c_int;
pub(crate) const SOL_SOCKET: c_int = windows_sys::Win32::Networking::WinSock::SOL_SOCKET as c_int;

/// Type used in set/getsockopt to retrieve the `TCP_NODELAY` option.
///
/// NOTE: <https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-getsockopt>
/// documents that options such as `TCP_NODELAY` and `SO_KEEPALIVE` expect a
/// `BOOL` (alias for `c_int`, 4 bytes), however in practice this turns out to
/// be false (or misleading) as a `BOOLEAN` (`c_uchar`, 1 byte) is returned by
/// `getsockopt`.
pub(crate) type Bool = windows_sys::Win32::Foundation::BOOLEAN;

/// Maximum size of a buffer passed to system call like `recv` and `send`.
const MAX_BUF_LEN: usize = c_int::MAX as usize;

/// Helper macro to execute a system call that returns an `io::Result`.
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ), $err_test: path, $err_value: expr) => {{
        #[allow(unused_unsafe)]
        let res = unsafe { windows_sys::Win32::Networking::WinSock::$fn($($arg, )*) };
        if $err_test(&res, &$err_value) {
            Err(io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

impl_debug!(
    crate::Domain,
    self::AF_INET,
    self::AF_INET6,
    self::AF_UNIX,
    self::AF_UNSPEC,
);

/// Windows only API.
impl Type {
    /// Our custom flag to set `WSA_FLAG_NO_HANDLE_INHERIT` on socket creation.
    /// Trying to mimic `Type::cloexec` on windows.
    const NO_INHERIT: c_int = 1 << ((size_of::<c_int>() * 8) - 1); // Last bit.

    /// Set `WSA_FLAG_NO_HANDLE_INHERIT` on the socket.
    #[cfg(feature = "all")]
    #[cfg_attr(docsrs, doc(cfg(all(windows, feature = "all"))))]
    pub const fn no_inherit(self) -> Type {
        self._no_inherit()
    }

    pub(crate) const fn _no_inherit(self) -> Type {
        Type(self.0 | Type::NO_INHERIT)
    }
}

impl_debug!(
    crate::Type,
    self::SOCK_STREAM,
    self::SOCK_DGRAM,
    self::SOCK_RAW,
    self::SOCK_RDM,
    self::SOCK_SEQPACKET,
);

impl_debug!(
    crate::Protocol,
    WinSock::IPPROTO_ICMP,
    WinSock::IPPROTO_ICMPV6,
    WinSock::IPPROTO_TCP,
    WinSock::IPPROTO_UDP,
);

impl std::fmt::Debug for RecvFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvFlags")
            .field("is_truncated", &self.is_truncated())
            .finish()
    }
}

#[repr(transparent)]
pub struct MaybeUninitSlice<'a> {
    vec: WSABUF,
    _lifetime: PhantomData<&'a mut [MaybeUninit<u8>]>,
}

unsafe impl<'a> Send for MaybeUninitSlice<'a> {}

unsafe impl<'a> Sync for MaybeUninitSlice<'a> {}

impl<'a> MaybeUninitSlice<'a> {
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> MaybeUninitSlice<'a> {
        assert!(buf.len() <= u32::MAX as usize);
        MaybeUninitSlice {
            vec: WSABUF {
                len: buf.len() as u32,
                buf: buf.as_mut_ptr().cast(),
            },
            _lifetime: PhantomData,
        }
    }

    pub fn as_slice(&self) -> &[MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts(self.vec.buf.cast(), self.vec.len as usize) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe { slice::from_raw_parts_mut(self.vec.buf.cast(), self.vec.len as usize) }
    }
}

// Used in `MsgHdr`.
pub(crate) use windows_sys::Win32::Networking::WinSock::WSAMSG as msghdr;

pub(crate) fn set_msghdr_name(msg: &mut msghdr, name: &SockAddr) {
    msg.name = name.as_ptr() as *mut _;
    msg.namelen = name.len();
}

pub(crate) fn set_msghdr_iov(msg: &mut msghdr, ptr: *mut WSABUF, len: usize) {
    msg.lpBuffers = ptr;
    msg.dwBufferCount = min(len, u32::MAX as usize) as u32;
}

pub(crate) fn set_msghdr_control(msg: &mut msghdr, ptr: *mut u8, len: usize) {
    msg.Control.buf = ptr;
    msg.Control.len = len as u32;
}

pub(crate) fn set_msghdr_flags(msg: &mut msghdr, flags: c_int) {
    msg.dwFlags = flags as u32;
}

pub(crate) fn msghdr_flags(msg: &msghdr) -> RecvFlags {
    RecvFlags(msg.dwFlags as c_int)
}

pub(crate) fn msghdr_control_len(msg: &msghdr) -> usize {
    msg.Control.len as _
}

fn init() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        // Initialize winsock through the standard library by just creating a
        // dummy socket. Whether this is successful or not we drop the result as
        // libstd will be sure to have initialized winsock.
        let _ = net::UdpSocket::bind("127.0.0.1:34254");
    });
}

pub(crate) type Socket = windows_sys::Win32::Networking::WinSock::SOCKET;

pub(crate) unsafe fn socket_from_raw(socket: Socket) -> crate::socket::Inner {
    crate::socket::Inner::from_raw_socket(socket as RawSocket)
}

pub(crate) fn socket_as_raw(socket: &crate::socket::Inner) -> Socket {
    socket.as_raw_socket() as Socket
}

pub(crate) fn socket_into_raw(socket: crate::socket::Inner) -> Socket {
    socket.into_raw_socket() as Socket
}

pub(crate) fn socket(family: c_int, mut ty: c_int, protocol: c_int) -> io::Result<Socket> {
    init();

    // Check if we set our custom flag.
    let flags = if ty & Type::NO_INHERIT != 0 {
        ty = ty & !Type::NO_INHERIT;
        WSA_FLAG_NO_HANDLE_INHERIT
    } else {
        0
    };

    syscall!(
        WSASocketW(
            family,
            ty,
            protocol,
            ptr::null_mut(),
            0,
            WSA_FLAG_OVERLAPPED | flags,
        ),
        PartialEq::eq,
        INVALID_SOCKET
    )
}

pub(crate) fn bind(socket: Socket, addr: &SockAddr) -> io::Result<()> {
    syscall!(bind(socket, addr.as_ptr(), addr.len()), PartialEq::ne, 0).map(|_| ())
}

pub(crate) fn connect(socket: Socket, addr: &SockAddr) -> io::Result<()> {
    syscall!(connect(socket, addr.as_ptr(), addr.len()), PartialEq::ne, 0).map(|_| ())
}

pub(crate) fn poll_connect(socket: &crate::Socket, timeout: Duration) -> io::Result<()> {
    let start = Instant::now();

    let mut fd_array = WSAPOLLFD {
        fd: socket.as_raw(),
        events: (POLLRDNORM | POLLWRNORM) as i16,
        revents: 0,
    };

    loop {
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return Err(io::ErrorKind::TimedOut.into());
        }

        let timeout = (timeout - elapsed).as_millis();
        let timeout = clamp(timeout, 1, c_int::MAX as u128) as c_int;

        match syscall!(
            WSAPoll(&mut fd_array, 1, timeout),
            PartialEq::eq,
            SOCKET_ERROR
        ) {
            Ok(0) => return Err(io::ErrorKind::TimedOut.into()),
            Ok(_) => {
                // Error or hang up indicates an error (or failure to connect).
                if (fd_array.revents & POLLERR as i16) != 0
                    || (fd_array.revents & POLLHUP as i16) != 0
                {
                    match socket.take_error() {
                        Ok(Some(err)) => return Err(err),
                        Ok(None) => {
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                "no error set after POLLHUP",
                            ))
                        }
                        Err(err) => return Err(err),
                    }
                }
                return Ok(());
            }
            // Got interrupted, try again.
            Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
}

// TODO: use clamp from std lib, stable since 1.50.
fn clamp<T>(value: T, min: T, max: T) -> T
where
    T: Ord,
{
    if value <= min {
        min
    } else if value >= max {
        max
    } else {
        value
    }
}

pub(crate) fn listen(socket: Socket, backlog: c_int) -> io::Result<()> {
    syscall!(listen(socket, backlog), PartialEq::ne, 0).map(|_| ())
}

pub(crate) fn accept(socket: Socket) -> io::Result<(Socket, SockAddr)> {
    // Safety: `accept` initialises the `SockAddr` for us.
    unsafe {
        SockAddr::try_init(|storage, len| {
            syscall!(
                accept(socket, storage.cast(), len),
                PartialEq::eq,
                INVALID_SOCKET
            )
        })
    }
}

pub(crate) fn getsockname(socket: Socket) -> io::Result<SockAddr> {
    // Safety: `getsockname` initialises the `SockAddr` for us.
    unsafe {
        SockAddr::try_init(|storage, len| {
            syscall!(
                getsockname(socket, storage.cast(), len),
                PartialEq::eq,
                SOCKET_ERROR
            )
        })
    }
    .map(|(_, addr)| addr)
}

pub(crate) fn getpeername(socket: Socket) -> io::Result<SockAddr> {
    // Safety: `getpeername` initialises the `SockAddr` for us.
    unsafe {
        SockAddr::try_init(|storage, len| {
            syscall!(
                getpeername(socket, storage.cast(), len),
                PartialEq::eq,
                SOCKET_ERROR
            )
        })
    }
    .map(|(_, addr)| addr)
}

pub(crate) fn try_clone(socket: Socket) -> io::Result<Socket> {
    let mut info: MaybeUninit<WSAPROTOCOL_INFOW> = MaybeUninit::uninit();
    syscall!(
        // NOTE: `process.id` is the same as `GetCurrentProcessId`.
        WSADuplicateSocketW(socket, process::id(), info.as_mut_ptr()),
        PartialEq::eq,
        SOCKET_ERROR
    )?;
    // Safety: `WSADuplicateSocketW` intialised `info` for us.
    let mut info = unsafe { info.assume_init() };

    syscall!(
        WSASocketW(
            info.iAddressFamily,
            info.iSocketType,
            info.iProtocol,
            &mut info,
            0,
            WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT,
        ),
        PartialEq::eq,
        INVALID_SOCKET
    )
}

pub(crate) fn set_nonblocking(socket: Socket, nonblocking: bool) -> io::Result<()> {
    let mut nonblocking = if nonblocking { 1 } else { 0 };
    ioctlsocket(socket, FIONBIO, &mut nonblocking)
}

pub(crate) fn shutdown(socket: Socket, how: Shutdown) -> io::Result<()> {
    let how = match how {
        Shutdown::Write => SD_SEND,
        Shutdown::Read => SD_RECEIVE,
        Shutdown::Both => SD_BOTH,
    } as i32;
    syscall!(shutdown(socket, how), PartialEq::eq, SOCKET_ERROR).map(|_| ())
}

pub(crate) fn recv(socket: Socket, buf: &mut [MaybeUninit<u8>], flags: c_int) -> io::Result<usize> {
    let res = syscall!(
        recv(
            socket,
            buf.as_mut_ptr().cast(),
            min(buf.len(), MAX_BUF_LEN) as c_int,
            flags,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    );
    match res {
        Ok(n) => Ok(n as usize),
        Err(ref err) if err.raw_os_error() == Some(WSAESHUTDOWN as i32) => Ok(0),
        Err(err) => Err(err),
    }
}

pub(crate) fn recv_vectored(
    socket: Socket,
    bufs: &mut [crate::MaybeUninitSlice<'_>],
    flags: c_int,
) -> io::Result<(usize, RecvFlags)> {
    let mut nread = 0;
    let mut flags = flags as u32;
    let res = syscall!(
        WSARecv(
            socket,
            bufs.as_mut_ptr().cast(),
            min(bufs.len(), u32::MAX as usize) as u32,
            &mut nread,
            &mut flags,
            ptr::null_mut(),
            None,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    );
    match res {
        Ok(_) => Ok((nread as usize, RecvFlags(0))),
        Err(ref err) if err.raw_os_error() == Some(WSAESHUTDOWN as i32) => Ok((0, RecvFlags(0))),
        Err(ref err) if err.raw_os_error() == Some(WSAEMSGSIZE as i32) => {
            Ok((nread as usize, RecvFlags(MSG_TRUNC)))
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn recv_from(
    socket: Socket,
    buf: &mut [MaybeUninit<u8>],
    flags: c_int,
) -> io::Result<(usize, SockAddr)> {
    // Safety: `recvfrom` initialises the `SockAddr` for us.
    unsafe {
        SockAddr::try_init(|storage, addrlen| {
            let res = syscall!(
                recvfrom(
                    socket,
                    buf.as_mut_ptr().cast(),
                    min(buf.len(), MAX_BUF_LEN) as c_int,
                    flags,
                    storage.cast(),
                    addrlen,
                ),
                PartialEq::eq,
                SOCKET_ERROR
            );
            match res {
                Ok(n) => Ok(n as usize),
                Err(ref err) if err.raw_os_error() == Some(WSAESHUTDOWN as i32) => Ok(0),
                Err(err) => Err(err),
            }
        })
    }
}

pub(crate) fn peek_sender(socket: Socket) -> io::Result<SockAddr> {
    // Safety: `recvfrom` initialises the `SockAddr` for us.
    let ((), sender) = unsafe {
        SockAddr::try_init(|storage, addrlen| {
            let res = syscall!(
                recvfrom(
                    socket,
                    // Windows *appears* not to care if you pass a null pointer.
                    ptr::null_mut(),
                    0,
                    MSG_PEEK,
                    storage.cast(),
                    addrlen,
                ),
                PartialEq::eq,
                SOCKET_ERROR
            );
            match res {
                Ok(_n) => Ok(()),
                Err(e) => match e.raw_os_error() {
                    Some(code) if code == (WSAESHUTDOWN as i32) || code == (WSAEMSGSIZE as i32) => {
                        Ok(())
                    }
                    _ => Err(e),
                },
            }
        })
    }?;

    Ok(sender)
}

pub(crate) fn recv_from_vectored(
    socket: Socket,
    bufs: &mut [crate::MaybeUninitSlice<'_>],
    flags: c_int,
) -> io::Result<(usize, RecvFlags, SockAddr)> {
    // Safety: `recvfrom` initialises the `SockAddr` for us.
    unsafe {
        SockAddr::try_init(|storage, addrlen| {
            let mut nread = 0;
            let mut flags = flags as u32;
            let res = syscall!(
                WSARecvFrom(
                    socket,
                    bufs.as_mut_ptr().cast(),
                    min(bufs.len(), u32::MAX as usize) as u32,
                    &mut nread,
                    &mut flags,
                    storage.cast(),
                    addrlen,
                    ptr::null_mut(),
                    None,
                ),
                PartialEq::eq,
                SOCKET_ERROR
            );
            match res {
                Ok(_) => Ok((nread as usize, RecvFlags(0))),
                Err(ref err) if err.raw_os_error() == Some(WSAESHUTDOWN as i32) => {
                    Ok((nread as usize, RecvFlags(0)))
                }
                Err(ref err) if err.raw_os_error() == Some(WSAEMSGSIZE as i32) => {
                    Ok((nread as usize, RecvFlags(MSG_TRUNC)))
                }
                Err(err) => Err(err),
            }
        })
    }
    .map(|((n, recv_flags), addr)| (n, recv_flags, addr))
}

pub(crate) fn send(socket: Socket, buf: &[u8], flags: c_int) -> io::Result<usize> {
    syscall!(
        send(
            socket,
            buf.as_ptr().cast(),
            min(buf.len(), MAX_BUF_LEN) as c_int,
            flags,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|n| n as usize)
}

pub(crate) fn send_vectored(
    socket: Socket,
    bufs: &[IoSlice<'_>],
    flags: c_int,
) -> io::Result<usize> {
    let mut nsent = 0;
    syscall!(
        WSASend(
            socket,
            // FIXME: From the `WSASend` docs [1]:
            // > For a Winsock application, once the WSASend function is called,
            // > the system owns these buffers and the application may not
            // > access them.
            //
            // So what we're doing is actually UB as `bufs` needs to be `&mut
            // [IoSlice<'_>]`.
            //
            // Tracking issue: https://github.com/rust-lang/socket2-rs/issues/129.
            //
            // NOTE: `send_to_vectored` has the same problem.
            //
            // [1] https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsasend
            bufs.as_ptr() as *mut _,
            min(bufs.len(), u32::MAX as usize) as u32,
            &mut nsent,
            flags as u32,
            std::ptr::null_mut(),
            None,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| nsent as usize)
}

pub(crate) fn send_to(
    socket: Socket,
    buf: &[u8],
    addr: &SockAddr,
    flags: c_int,
) -> io::Result<usize> {
    syscall!(
        sendto(
            socket,
            buf.as_ptr().cast(),
            min(buf.len(), MAX_BUF_LEN) as c_int,
            flags,
            addr.as_ptr(),
            addr.len(),
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|n| n as usize)
}

pub(crate) fn send_to_vectored(
    socket: Socket,
    bufs: &[IoSlice<'_>],
    addr: &SockAddr,
    flags: c_int,
) -> io::Result<usize> {
    let mut nsent = 0;
    syscall!(
        WSASendTo(
            socket,
            // FIXME: Same problem as in `send_vectored`.
            bufs.as_ptr() as *mut _,
            bufs.len().min(u32::MAX as usize) as u32,
            &mut nsent,
            flags as u32,
            addr.as_ptr(),
            addr.len(),
            ptr::null_mut(),
            None,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| nsent as usize)
}

pub(crate) fn sendmsg(socket: Socket, msg: &MsgHdr<'_, '_, '_>, flags: c_int) -> io::Result<usize> {
    let mut nsent = 0;
    syscall!(
        WSASendMsg(
            socket,
            &msg.inner,
            flags as u32,
            &mut nsent,
            ptr::null_mut(),
            None,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| nsent as usize)
}

/// Wrapper around `getsockopt` to deal with platform specific timeouts.
pub(crate) fn timeout_opt(fd: Socket, lvl: c_int, name: i32) -> io::Result<Option<Duration>> {
    unsafe { getsockopt(fd, lvl, name).map(from_ms) }
}

fn from_ms(duration: u32) -> Option<Duration> {
    if duration == 0 {
        None
    } else {
        let secs = duration / 1000;
        let nsec = (duration % 1000) * 1000000;
        Some(Duration::new(secs as u64, nsec as u32))
    }
}

/// Wrapper around `setsockopt` to deal with platform specific timeouts.
pub(crate) fn set_timeout_opt(
    socket: Socket,
    level: c_int,
    optname: i32,
    duration: Option<Duration>,
) -> io::Result<()> {
    let duration = into_ms(duration);
    unsafe { setsockopt(socket, level, optname, duration) }
}

fn into_ms(duration: Option<Duration>) -> u32 {
    // Note that a duration is a (u64, u32) (seconds, nanoseconds) pair, and the
    // timeouts in windows APIs are typically u32 milliseconds. To translate, we
    // have two pieces to take care of:
    //
    // * Nanosecond precision is rounded up
    // * Greater than u32::MAX milliseconds (50 days) is rounded up to
    //   INFINITE (never time out).
    duration.map_or(0, |duration| {
        min(duration.as_millis(), INFINITE as u128) as u32
    })
}

pub(crate) fn set_tcp_keepalive(socket: Socket, keepalive: &TcpKeepalive) -> io::Result<()> {
    let mut keepalive = tcp_keepalive {
        onoff: 1,
        keepalivetime: into_ms(keepalive.time),
        keepaliveinterval: into_ms(keepalive.interval),
    };
    let mut out = 0;
    syscall!(
        WSAIoctl(
            socket,
            SIO_KEEPALIVE_VALS,
            &mut keepalive as *mut _ as *mut _,
            size_of::<tcp_keepalive>() as _,
            ptr::null_mut(),
            0,
            &mut out,
            ptr::null_mut(),
            None,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| ())
}

/// Caller must ensure `T` is the correct type for `level` and `optname`.
// NOTE: `optname` is actually `i32`, but all constants are `u32`.
pub(crate) unsafe fn getsockopt<T>(socket: Socket, level: c_int, optname: i32) -> io::Result<T> {
    let mut optval: MaybeUninit<T> = MaybeUninit::uninit();
    let mut optlen = mem::size_of::<T>() as c_int;
    syscall!(
        getsockopt(
            socket,
            level as i32,
            optname,
            optval.as_mut_ptr().cast(),
            &mut optlen,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| {
        debug_assert_eq!(optlen as usize, mem::size_of::<T>());
        // Safety: `getsockopt` initialised `optval` for us.
        optval.assume_init()
    })
}

/// Caller must ensure `T` is the correct type for `level` and `optname`.
// NOTE: `optname` is actually `i32`, but all constants are `u32`.
pub(crate) unsafe fn setsockopt<T>(
    socket: Socket,
    level: c_int,
    optname: i32,
    optval: T,
) -> io::Result<()> {
    syscall!(
        setsockopt(
            socket,
            level as i32,
            optname,
            (&optval as *const T).cast(),
            mem::size_of::<T>() as c_int,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| ())
}

fn ioctlsocket(socket: Socket, cmd: i32, payload: &mut u32) -> io::Result<()> {
    syscall!(
        ioctlsocket(socket, cmd, payload),
        PartialEq::eq,
        SOCKET_ERROR
    )
    .map(|_| ())
}

pub(crate) fn to_in_addr(addr: &Ipv4Addr) -> IN_ADDR {
    IN_ADDR {
        S_un: IN_ADDR_0 {
            // `S_un` is stored as BE on all machines, and the array is in BE
            // order. So the native endian conversion method is used so that
            // it's never swapped.
            S_addr: u32::from_ne_bytes(addr.octets()),
        },
    }
}

pub(crate) fn from_in_addr(in_addr: IN_ADDR) -> Ipv4Addr {
    Ipv4Addr::from(unsafe { in_addr.S_un.S_addr }.to_ne_bytes())
}

pub(crate) fn to_in6_addr(addr: &Ipv6Addr) -> IN6_ADDR {
    IN6_ADDR {
        u: IN6_ADDR_0 {
            Byte: addr.octets(),
        },
    }
}

pub(crate) fn from_in6_addr(addr: IN6_ADDR) -> Ipv6Addr {
    Ipv6Addr::from(unsafe { addr.u.Byte })
}

pub(crate) fn to_mreqn(
    multiaddr: &Ipv4Addr,
    interface: &crate::socket::InterfaceIndexOrAddress,
) -> IpMreq {
    IpMreq {
        imr_multiaddr: to_in_addr(multiaddr),
        // Per https://docs.microsoft.com/en-us/windows/win32/api/ws2ipdef/ns-ws2ipdef-ip_mreq#members:
        //
        // imr_interface
        //
        // The local IPv4 address of the interface or the interface index on
        // which the multicast group should be joined or dropped. This value is
        // in network byte order. If this member specifies an IPv4 address of
        // 0.0.0.0, the default IPv4 multicast interface is used.
        //
        // To use an interface index of 1 would be the same as an IP address of
        // 0.0.0.1.
        imr_interface: match interface {
            crate::socket::InterfaceIndexOrAddress::Index(interface) => {
                to_in_addr(&(*interface).into())
            }
            crate::socket::InterfaceIndexOrAddress::Address(interface) => to_in_addr(interface),
        },
    }
}

#[cfg(feature = "all")]
pub(crate) fn original_dst(socket: Socket) -> io::Result<SockAddr> {
    unsafe {
        SockAddr::try_init(|storage, len| {
            syscall!(
                getsockopt(
                    socket,
                    SOL_IP as i32,
                    SO_ORIGINAL_DST as i32,
                    storage.cast(),
                    len,
                ),
                PartialEq::eq,
                SOCKET_ERROR
            )
        })
    }
    .map(|(_, addr)| addr)
}

#[cfg(feature = "all")]
pub(crate) fn original_dst_ipv6(socket: Socket) -> io::Result<SockAddr> {
    unsafe {
        SockAddr::try_init(|storage, len| {
            syscall!(
                getsockopt(
                    socket,
                    SOL_IP as i32,
                    IP6T_SO_ORIGINAL_DST as i32,
                    storage.cast(),
                    len,
                ),
                PartialEq::eq,
                SOCKET_ERROR
            )
        })
    }
    .map(|(_, addr)| addr)
}

#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) fn unix_sockaddr(path: &Path) -> io::Result<SockAddr> {
    // SAFETY: a `sockaddr_storage` of all zeros is valid.
    let mut storage = unsafe { mem::zeroed::<sockaddr_storage>() };
    let len = {
        let storage: &mut windows_sys::Win32::Networking::WinSock::SOCKADDR_UN =
            unsafe { &mut *(&mut storage as *mut sockaddr_storage).cast() };

        // Windows expects a UTF-8 path here even though Windows paths are
        // usually UCS-2 encoded. If Rust exposed OsStr's Wtf8 encoded
        // buffer, this could be used directly, relying on Windows to
        // validate the path, but Rust hides this implementation detail.
        //
        // See <https://github.com/rust-lang/rust/pull/95290>.
        let bytes = path
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path must be valid UTF-8"))?
            .as_bytes();

        // Windows appears to allow non-null-terminated paths, but this is
        // not documented, so do not rely on it yet.
        //
        // See <https://github.com/rust-lang/socket2/issues/331>.
        if bytes.len() >= storage.sun_path.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must be shorter than SUN_LEN",
            ));
        }

        storage.sun_family = crate::sys::AF_UNIX as sa_family_t;
        // `storage` was initialized to zero above, so the path is
        // already null terminated.
        storage.sun_path[..bytes.len()].copy_from_slice(bytes);

        let base = storage as *const _ as usize;
        let path = &storage.sun_path as *const _ as usize;
        let sun_path_offset = path - base;
        sun_path_offset + bytes.len() + 1
    };
    Ok(unsafe { SockAddr::new(storage, len as socklen_t) })
}

/// Windows only API.
impl crate::Socket {
    /// Sets `HANDLE_FLAG_INHERIT` using `SetHandleInformation`.
    #[cfg(feature = "all")]
    #[cfg_attr(docsrs, doc(cfg(all(windows, feature = "all"))))]
    pub fn set_no_inherit(&self, no_inherit: bool) -> io::Result<()> {
        self._set_no_inherit(no_inherit)
    }

    pub(crate) fn _set_no_inherit(&self, no_inherit: bool) -> io::Result<()> {
        // NOTE: can't use `syscall!` because it expects the function in the
        // `windows_sys::Win32::Networking::WinSock::` path.
        let res = unsafe {
            SetHandleInformation(
                self.as_raw() as HANDLE,
                HANDLE_FLAG_INHERIT,
                !no_inherit as _,
            )
        };
        if res == 0 {
            // Zero means error.
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Returns the [`Protocol`] of this socket by checking the `SO_PROTOCOL_INFOW`
    /// option on this socket.
    ///
    /// [`Protocol`]: crate::Protocol
    #[cfg(feature = "all")]
    pub fn protocol(&self) -> io::Result<Option<crate::Protocol>> {
        let info = unsafe {
            getsockopt::<WSAPROTOCOL_INFOW>(self.as_raw(), SOL_SOCKET, SO_PROTOCOL_INFOW)?
        };
        match info.iProtocol {
            0 => Ok(None),
            p => Ok(Some(crate::Protocol::from(p))),
        }
    }
}

#[cfg_attr(docsrs, doc(cfg(windows)))]
impl AsSocket for crate::Socket {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        // SAFETY: lifetime is bound by self.
        unsafe { BorrowedSocket::borrow_raw(self.as_raw() as RawSocket) }
    }
}

#[cfg_attr(docsrs, doc(cfg(windows)))]
impl AsRawSocket for crate::Socket {
    fn as_raw_socket(&self) -> RawSocket {
        self.as_raw() as RawSocket
    }
}

#[cfg_attr(docsrs, doc(cfg(windows)))]
impl From<crate::Socket> for OwnedSocket {
    fn from(sock: crate::Socket) -> OwnedSocket {
        // SAFETY: sock.into_raw() always returns a valid fd.
        unsafe { OwnedSocket::from_raw_socket(sock.into_raw() as RawSocket) }
    }
}

#[cfg_attr(docsrs, doc(cfg(windows)))]
impl IntoRawSocket for crate::Socket {
    fn into_raw_socket(self) -> RawSocket {
        self.into_raw() as RawSocket
    }
}

#[cfg_attr(docsrs, doc(cfg(windows)))]
impl From<OwnedSocket> for crate::Socket {
    fn from(fd: OwnedSocket) -> crate::Socket {
        // SAFETY: `OwnedFd` ensures the fd is valid.
        unsafe { crate::Socket::from_raw_socket(fd.into_raw_socket()) }
    }
}

#[cfg_attr(docsrs, doc(cfg(windows)))]
impl FromRawSocket for crate::Socket {
    unsafe fn from_raw_socket(socket: RawSocket) -> crate::Socket {
        crate::Socket::from_raw(socket as Socket)
    }
}

#[test]
fn in_addr_convertion() {
    let ip = Ipv4Addr::new(127, 0, 0, 1);
    let raw = to_in_addr(&ip);
    assert_eq!(unsafe { raw.S_un.S_addr }, 127 << 0 | 1 << 24);
    assert_eq!(from_in_addr(raw), ip);

    let ip = Ipv4Addr::new(127, 34, 4, 12);
    let raw = to_in_addr(&ip);
    assert_eq!(
        unsafe { raw.S_un.S_addr },
        127 << 0 | 34 << 8 | 4 << 16 | 12 << 24
    );
    assert_eq!(from_in_addr(raw), ip);
}

#[test]
fn in6_addr_convertion() {
    let ip = Ipv6Addr::new(0x2000, 1, 2, 3, 4, 5, 6, 7);
    let raw = to_in6_addr(&ip);
    let want = [
        0x2000u16.to_be(),
        1u16.to_be(),
        2u16.to_be(),
        3u16.to_be(),
        4u16.to_be(),
        5u16.to_be(),
        6u16.to_be(),
        7u16.to_be(),
    ];
    assert_eq!(unsafe { raw.u.Word }, want);
    assert_eq!(from_in6_addr(raw), ip);
}
