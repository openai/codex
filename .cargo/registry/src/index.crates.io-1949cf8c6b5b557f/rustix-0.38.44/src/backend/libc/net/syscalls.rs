//! libc syscalls supporting `rustix::net`.

#[cfg(unix)]
use super::addr::SocketAddrUnix;
#[cfg(target_os = "linux")]
use super::msghdr::with_xdp_msghdr;
#[cfg(target_os = "linux")]
use super::write_sockaddr::encode_sockaddr_xdp;
use crate::backend::c;
use crate::backend::conv::{borrowed_fd, ret, ret_owned_fd, ret_send_recv, send_recv_len};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
#[cfg(target_os = "linux")]
use crate::net::xdp::SocketAddrXdp;
use crate::net::{SocketAddrAny, SocketAddrV4, SocketAddrV6};
use crate::utils::as_ptr;
use core::mem::{size_of, MaybeUninit};
#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use {
    super::msghdr::{with_noaddr_msghdr, with_recv_msghdr, with_v4_msghdr, with_v6_msghdr},
    crate::io::{IoSlice, IoSliceMut},
    crate::net::{RecvAncillaryBuffer, RecvMsgReturn, SendAncillaryBuffer},
};
#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
use {
    super::read_sockaddr::{initialize_family_to_unspec, maybe_read_sockaddr_os, read_sockaddr_os},
    super::send_recv::{RecvFlags, SendFlags},
    super::write_sockaddr::{encode_sockaddr_v4, encode_sockaddr_v6},
    crate::net::{AddressFamily, Protocol, Shutdown, SocketFlags, SocketType},
    core::ptr::null_mut,
};

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) unsafe fn recv(
    fd: BorrowedFd<'_>,
    buf: *mut u8,
    len: usize,
    flags: RecvFlags,
) -> io::Result<usize> {
    ret_send_recv(c::recv(
        borrowed_fd(fd),
        buf.cast(),
        send_recv_len(len),
        bitflags_bits!(flags),
    ))
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn send(fd: BorrowedFd<'_>, buf: &[u8], flags: SendFlags) -> io::Result<usize> {
    unsafe {
        ret_send_recv(c::send(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            send_recv_len(buf.len()),
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) unsafe fn recvfrom(
    fd: BorrowedFd<'_>,
    buf: *mut u8,
    buf_len: usize,
    flags: RecvFlags,
) -> io::Result<(usize, Option<SocketAddrAny>)> {
    let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();
    let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;

    // `recvfrom` does not write to the storage if the socket is
    // connection-oriented sockets, so we initialize the family field to
    // `AF_UNSPEC` so that we can detect this case.
    initialize_family_to_unspec(storage.as_mut_ptr());

    ret_send_recv(c::recvfrom(
        borrowed_fd(fd),
        buf.cast(),
        send_recv_len(buf_len),
        bitflags_bits!(flags),
        storage.as_mut_ptr().cast(),
        &mut len,
    ))
    .map(|nread| {
        (
            nread,
            maybe_read_sockaddr_os(storage.as_ptr(), len.try_into().unwrap()),
        )
    })
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn sendto_v4(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrV4,
) -> io::Result<usize> {
    unsafe {
        ret_send_recv(c::sendto(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            send_recv_len(buf.len()),
            bitflags_bits!(flags),
            as_ptr(&encode_sockaddr_v4(addr)).cast::<c::sockaddr>(),
            size_of::<c::sockaddr_in>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn sendto_v6(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrV6,
) -> io::Result<usize> {
    unsafe {
        ret_send_recv(c::sendto(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            send_recv_len(buf.len()),
            bitflags_bits!(flags),
            as_ptr(&encode_sockaddr_v6(addr)).cast::<c::sockaddr>(),
            size_of::<c::sockaddr_in6>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub(crate) fn sendto_unix(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrUnix,
) -> io::Result<usize> {
    unsafe {
        ret_send_recv(c::sendto(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            send_recv_len(buf.len()),
            bitflags_bits!(flags),
            as_ptr(&addr.unix).cast(),
            addr.addr_len(),
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn sendto_xdp(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrXdp,
) -> io::Result<usize> {
    unsafe {
        ret_send_recv(c::sendto(
            borrowed_fd(fd),
            buf.as_ptr().cast(),
            send_recv_len(buf.len()),
            bitflags_bits!(flags),
            as_ptr(&encode_sockaddr_xdp(addr)).cast::<c::sockaddr>(),
            size_of::<c::sockaddr_xdp>() as _,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn socket(
    domain: AddressFamily,
    type_: SocketType,
    protocol: Option<Protocol>,
) -> io::Result<OwnedFd> {
    let raw_protocol = match protocol {
        Some(p) => p.0.get(),
        None => 0,
    };
    unsafe {
        ret_owned_fd(c::socket(
            domain.0 as c::c_int,
            type_.0 as c::c_int,
            raw_protocol as c::c_int,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn socket_with(
    domain: AddressFamily,
    type_: SocketType,
    flags: SocketFlags,
    protocol: Option<Protocol>,
) -> io::Result<OwnedFd> {
    let raw_protocol = match protocol {
        Some(p) => p.0.get(),
        None => 0,
    };
    unsafe {
        ret_owned_fd(c::socket(
            domain.0 as c::c_int,
            (type_.0 | flags.bits()) as c::c_int,
            raw_protocol as c::c_int,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn bind_v4(sockfd: BorrowedFd<'_>, addr: &SocketAddrV4) -> io::Result<()> {
    unsafe {
        ret(c::bind(
            borrowed_fd(sockfd),
            as_ptr(&encode_sockaddr_v4(addr)).cast(),
            size_of::<c::sockaddr_in>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn bind_v6(sockfd: BorrowedFd<'_>, addr: &SocketAddrV6) -> io::Result<()> {
    unsafe {
        ret(c::bind(
            borrowed_fd(sockfd),
            as_ptr(&encode_sockaddr_v6(addr)).cast(),
            size_of::<c::sockaddr_in6>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub(crate) fn bind_unix(sockfd: BorrowedFd<'_>, addr: &SocketAddrUnix) -> io::Result<()> {
    unsafe {
        ret(c::bind(
            borrowed_fd(sockfd),
            as_ptr(&addr.unix).cast(),
            addr.addr_len(),
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn bind_xdp(sockfd: BorrowedFd<'_>, addr: &SocketAddrXdp) -> io::Result<()> {
    unsafe {
        ret(c::bind(
            borrowed_fd(sockfd),
            as_ptr(&encode_sockaddr_xdp(addr)).cast(),
            size_of::<c::sockaddr_xdp>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn connect_v4(sockfd: BorrowedFd<'_>, addr: &SocketAddrV4) -> io::Result<()> {
    unsafe {
        ret(c::connect(
            borrowed_fd(sockfd),
            as_ptr(&encode_sockaddr_v4(addr)).cast(),
            size_of::<c::sockaddr_in>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn connect_v6(sockfd: BorrowedFd<'_>, addr: &SocketAddrV6) -> io::Result<()> {
    unsafe {
        ret(c::connect(
            borrowed_fd(sockfd),
            as_ptr(&encode_sockaddr_v6(addr)).cast(),
            size_of::<c::sockaddr_in6>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub(crate) fn connect_unix(sockfd: BorrowedFd<'_>, addr: &SocketAddrUnix) -> io::Result<()> {
    unsafe {
        ret(c::connect(
            borrowed_fd(sockfd),
            as_ptr(&addr.unix).cast(),
            addr.addr_len(),
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn connect_unspec(sockfd: BorrowedFd<'_>) -> io::Result<()> {
    debug_assert_eq!(c::AF_UNSPEC, 0);
    let addr = MaybeUninit::<c::sockaddr_storage>::zeroed();
    unsafe {
        ret(c::connect(
            borrowed_fd(sockfd),
            as_ptr(&addr).cast(),
            size_of::<c::sockaddr_storage>() as c::socklen_t,
        ))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn listen(sockfd: BorrowedFd<'_>, backlog: c::c_int) -> io::Result<()> {
    unsafe { ret(c::listen(borrowed_fd(sockfd), backlog)) }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn accept(sockfd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    unsafe {
        let owned_fd = ret_owned_fd(c::accept(borrowed_fd(sockfd), null_mut(), null_mut()))?;
        Ok(owned_fd)
    }
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn recvmsg(
    sockfd: BorrowedFd<'_>,
    iov: &mut [IoSliceMut<'_>],
    control: &mut RecvAncillaryBuffer<'_>,
    msg_flags: RecvFlags,
) -> io::Result<RecvMsgReturn> {
    let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();

    with_recv_msghdr(&mut storage, iov, control, |msghdr| {
        let result = unsafe {
            ret_send_recv(c::recvmsg(
                borrowed_fd(sockfd),
                msghdr,
                bitflags_bits!(msg_flags),
            ))
        };

        result.map(|bytes| {
            // Get the address of the sender, if any.
            let addr =
                unsafe { maybe_read_sockaddr_os(msghdr.msg_name as _, msghdr.msg_namelen as _) };

            RecvMsgReturn {
                bytes,
                address: addr,
                flags: RecvFlags::from_bits_retain(bitcast!(msghdr.msg_flags)),
            }
        })
    })
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn sendmsg(
    sockfd: BorrowedFd<'_>,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_noaddr_msghdr(iov, control, |msghdr| unsafe {
        ret_send_recv(c::sendmsg(
            borrowed_fd(sockfd),
            &msghdr,
            bitflags_bits!(msg_flags),
        ))
    })
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn sendmsg_v4(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrV4,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_v4_msghdr(addr, iov, control, |msghdr| unsafe {
        ret_send_recv(c::sendmsg(
            borrowed_fd(sockfd),
            &msghdr,
            bitflags_bits!(msg_flags),
        ))
    })
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn sendmsg_v6(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrV6,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_v6_msghdr(addr, iov, control, |msghdr| unsafe {
        ret_send_recv(c::sendmsg(
            borrowed_fd(sockfd),
            &msghdr,
            bitflags_bits!(msg_flags),
        ))
    })
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "redox", target_os = "vita"))
))]
pub(crate) fn sendmsg_unix(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrUnix,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    super::msghdr::with_unix_msghdr(addr, iov, control, |msghdr| unsafe {
        ret_send_recv(c::sendmsg(
            borrowed_fd(sockfd),
            &msghdr,
            bitflags_bits!(msg_flags),
        ))
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn sendmsg_xdp(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrXdp,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_xdp_msghdr(addr, iov, control, |msghdr| unsafe {
        ret_send_recv(c::sendmsg(
            borrowed_fd(sockfd),
            &msghdr,
            bitflags_bits!(msg_flags),
        ))
    })
}

#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "redox",
    target_os = "nto",
    target_os = "vita",
    target_os = "wasi",
)))]
pub(crate) fn accept_with(sockfd: BorrowedFd<'_>, flags: SocketFlags) -> io::Result<OwnedFd> {
    unsafe {
        let owned_fd = ret_owned_fd(c::accept4(
            borrowed_fd(sockfd),
            null_mut(),
            null_mut(),
            flags.bits() as c::c_int,
        ))?;
        Ok(owned_fd)
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn acceptfrom(sockfd: BorrowedFd<'_>) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    unsafe {
        let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();
        let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;
        let owned_fd = ret_owned_fd(c::accept(
            borrowed_fd(sockfd),
            storage.as_mut_ptr().cast(),
            &mut len,
        ))?;
        Ok((
            owned_fd,
            maybe_read_sockaddr_os(storage.as_ptr(), len.try_into().unwrap()),
        ))
    }
}

#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
pub(crate) fn acceptfrom_with(
    sockfd: BorrowedFd<'_>,
    flags: SocketFlags,
) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    unsafe {
        let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();
        let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;
        let owned_fd = ret_owned_fd(c::accept4(
            borrowed_fd(sockfd),
            storage.as_mut_ptr().cast(),
            &mut len,
            flags.bits() as c::c_int,
        ))?;
        Ok((
            owned_fd,
            maybe_read_sockaddr_os(storage.as_ptr(), len.try_into().unwrap()),
        ))
    }
}

/// Darwin lacks `accept4`, but does have `accept`. We define `SocketFlags` to
/// have no flags, so we can discard it here.
#[cfg(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "nto",
    target_os = "vita",
))]
pub(crate) fn accept_with(sockfd: BorrowedFd<'_>, _flags: SocketFlags) -> io::Result<OwnedFd> {
    accept(sockfd)
}

/// Darwin lacks `accept4`, but does have `accept`. We define `SocketFlags` to
/// have no flags, so we can discard it here.
#[cfg(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "nto",
    target_os = "vita",
))]
pub(crate) fn acceptfrom_with(
    sockfd: BorrowedFd<'_>,
    _flags: SocketFlags,
) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    acceptfrom(sockfd)
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn shutdown(sockfd: BorrowedFd<'_>, how: Shutdown) -> io::Result<()> {
    unsafe { ret(c::shutdown(borrowed_fd(sockfd), how as c::c_int)) }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn getsockname(sockfd: BorrowedFd<'_>) -> io::Result<SocketAddrAny> {
    unsafe {
        let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();
        let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;
        ret(c::getsockname(
            borrowed_fd(sockfd),
            storage.as_mut_ptr().cast(),
            &mut len,
        ))?;
        Ok(read_sockaddr_os(storage.as_ptr(), len.try_into().unwrap()))
    }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn getpeername(sockfd: BorrowedFd<'_>) -> io::Result<Option<SocketAddrAny>> {
    unsafe {
        let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();
        let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;
        ret(c::getpeername(
            borrowed_fd(sockfd),
            storage.as_mut_ptr().cast(),
            &mut len,
        ))?;
        Ok(maybe_read_sockaddr_os(
            storage.as_ptr(),
            len.try_into().unwrap(),
        ))
    }
}

#[cfg(not(any(windows, target_os = "redox", target_os = "wasi")))]
pub(crate) fn socketpair(
    domain: AddressFamily,
    type_: SocketType,
    flags: SocketFlags,
    protocol: Option<Protocol>,
) -> io::Result<(OwnedFd, OwnedFd)> {
    let raw_protocol = match protocol {
        Some(p) => p.0.get(),
        None => 0,
    };
    unsafe {
        let mut fds = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(c::socketpair(
            c::c_int::from(domain.0),
            (type_.0 | flags.bits()) as c::c_int,
            raw_protocol as c::c_int,
            fds.as_mut_ptr().cast::<c::c_int>(),
        ))?;

        let [fd0, fd1] = fds.assume_init();
        Ok((fd0, fd1))
    }
}
