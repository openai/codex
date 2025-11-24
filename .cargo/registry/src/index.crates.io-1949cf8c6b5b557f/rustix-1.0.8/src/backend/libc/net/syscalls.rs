//! libc syscalls supporting `rustix::net`.

use super::read_sockaddr::initialize_family_to_unspec;
use super::send_recv::{RecvFlags, SendFlags};
use crate::backend::c;
#[cfg(target_os = "linux")]
use crate::backend::conv::ret_u32;
use crate::backend::conv::{borrowed_fd, ret, ret_owned_fd, ret_send_recv, send_recv_len};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io;
use crate::net::addr::SocketAddrArg;
#[cfg(target_os = "linux")]
use crate::net::MMsgHdr;
use crate::net::{
    AddressFamily, Protocol, Shutdown, SocketAddrAny, SocketAddrBuf, SocketFlags, SocketType,
};
use crate::utils::as_ptr;
use core::mem::{size_of, MaybeUninit};
use core::ptr::null_mut;
#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
use {
    super::msghdr::{noaddr_msghdr, with_msghdr, with_recv_msghdr},
    super::send_recv::ReturnFlags,
    crate::io::{IoSlice, IoSliceMut},
    crate::net::{RecvAncillaryBuffer, RecvMsg, SendAncillaryBuffer},
};

pub(crate) unsafe fn recv(
    fd: BorrowedFd<'_>,
    buf: (*mut u8, usize),
    flags: RecvFlags,
) -> io::Result<usize> {
    ret_send_recv(c::recv(
        borrowed_fd(fd),
        buf.0.cast(),
        send_recv_len(buf.1),
        bitflags_bits!(flags),
    ))
}

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

pub(crate) unsafe fn recvfrom(
    fd: BorrowedFd<'_>,
    buf: (*mut u8, usize),
    flags: RecvFlags,
) -> io::Result<(usize, Option<SocketAddrAny>)> {
    let mut addr = SocketAddrBuf::new();

    // `recvfrom` does not write to the storage if the socket is
    // connection-oriented sockets, so we initialize the family field to
    // `AF_UNSPEC` so that we can detect this case.
    initialize_family_to_unspec(addr.storage.as_mut_ptr().cast::<c::sockaddr>());

    let nread = ret_send_recv(c::recvfrom(
        borrowed_fd(fd),
        buf.0.cast(),
        send_recv_len(buf.1),
        bitflags_bits!(flags),
        addr.storage.as_mut_ptr().cast::<c::sockaddr>(),
        &mut addr.len,
    ))?;

    Ok((nread, addr.into_any_option()))
}

pub(crate) fn sendto(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &impl SocketAddrArg,
) -> io::Result<usize> {
    unsafe {
        addr.with_sockaddr(|addr_ptr, addr_len| {
            ret_send_recv(c::sendto(
                borrowed_fd(fd),
                buf.as_ptr().cast(),
                send_recv_len(buf.len()),
                bitflags_bits!(flags),
                addr_ptr.cast(),
                bitcast!(addr_len),
            ))
        })
    }
}

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

pub(crate) fn bind(sockfd: BorrowedFd<'_>, addr: &impl SocketAddrArg) -> io::Result<()> {
    unsafe {
        addr.with_sockaddr(|addr_ptr, addr_len| {
            ret(c::bind(
                borrowed_fd(sockfd),
                addr_ptr.cast(),
                bitcast!(addr_len),
            ))
        })
    }
}

pub(crate) fn connect(sockfd: BorrowedFd<'_>, addr: &impl SocketAddrArg) -> io::Result<()> {
    unsafe {
        addr.with_sockaddr(|addr_ptr, addr_len| {
            ret(c::connect(
                borrowed_fd(sockfd),
                addr_ptr.cast(),
                bitcast!(addr_len),
            ))
        })
    }
}

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

pub(crate) fn listen(sockfd: BorrowedFd<'_>, backlog: c::c_int) -> io::Result<()> {
    unsafe { ret(c::listen(borrowed_fd(sockfd), backlog)) }
}

pub(crate) fn accept(sockfd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    unsafe {
        let owned_fd = ret_owned_fd(c::accept(borrowed_fd(sockfd), null_mut(), null_mut()))?;
        Ok(owned_fd)
    }
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub(crate) fn recvmsg(
    sockfd: BorrowedFd<'_>,
    iov: &mut [IoSliceMut<'_>],
    control: &mut RecvAncillaryBuffer<'_>,
    msg_flags: RecvFlags,
) -> io::Result<RecvMsg> {
    let mut addr = SocketAddrBuf::new();

    // SAFETY: This passes the `msghdr` reference to the OS which reads the
    // buffers only within the designated bounds.
    let (bytes, flags) = unsafe {
        with_recv_msghdr(&mut addr, iov, control, |msghdr| {
            let bytes = ret_send_recv(c::recvmsg(
                borrowed_fd(sockfd),
                msghdr,
                bitflags_bits!(msg_flags),
            ))?;
            Ok((bytes, msghdr.msg_flags))
        })?
    };

    Ok(RecvMsg {
        bytes,
        address: unsafe { addr.into_any_option() },
        flags: ReturnFlags::from_bits_retain(bitcast!(flags)),
    })
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub(crate) fn sendmsg(
    sockfd: BorrowedFd<'_>,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    let msghdr = noaddr_msghdr(iov, control);
    unsafe {
        ret_send_recv(c::sendmsg(
            borrowed_fd(sockfd),
            &msghdr,
            bitflags_bits!(msg_flags),
        ))
    }
}

#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub(crate) fn sendmsg_addr(
    sockfd: BorrowedFd<'_>,
    addr: &impl SocketAddrArg,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    // SAFETY: This passes the `msghdr` reference to the OS which reads the
    // buffers only within the designated bounds.
    unsafe {
        with_msghdr(addr, iov, control, |msghdr| {
            ret_send_recv(c::sendmsg(
                borrowed_fd(sockfd),
                msghdr,
                bitflags_bits!(msg_flags),
            ))
        })
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn sendmmsg(
    sockfd: BorrowedFd<'_>,
    msgs: &mut [MMsgHdr<'_>],
    flags: SendFlags,
) -> io::Result<usize> {
    unsafe {
        ret_u32(c::sendmmsg(
            borrowed_fd(sockfd),
            msgs.as_mut_ptr() as _,
            msgs.len().try_into().unwrap_or(c::c_uint::MAX),
            bitflags_bits!(flags),
        ))
        .map(|ret| ret as usize)
    }
}

#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
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

pub(crate) fn acceptfrom(sockfd: BorrowedFd<'_>) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    unsafe {
        let mut addr = SocketAddrBuf::new();
        let owned_fd = ret_owned_fd(c::accept(
            borrowed_fd(sockfd),
            addr.storage.as_mut_ptr().cast::<c::sockaddr>(),
            &mut addr.len,
        ))?;
        Ok((owned_fd, addr.into_any_option()))
    }
}

#[cfg(not(any(
    apple,
    windows,
    target_os = "aix",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
)))]
pub(crate) fn acceptfrom_with(
    sockfd: BorrowedFd<'_>,
    flags: SocketFlags,
) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    unsafe {
        let mut addr = SocketAddrBuf::new();
        let owned_fd = ret_owned_fd(c::accept4(
            borrowed_fd(sockfd),
            addr.storage.as_mut_ptr().cast::<c::sockaddr>(),
            &mut addr.len,
            flags.bits() as c::c_int,
        ))?;
        Ok((owned_fd, addr.into_any_option()))
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
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
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
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
))]
pub(crate) fn acceptfrom_with(
    sockfd: BorrowedFd<'_>,
    _flags: SocketFlags,
) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    acceptfrom(sockfd)
}

pub(crate) fn shutdown(sockfd: BorrowedFd<'_>, how: Shutdown) -> io::Result<()> {
    unsafe { ret(c::shutdown(borrowed_fd(sockfd), how as c::c_int)) }
}

pub(crate) fn getsockname(sockfd: BorrowedFd<'_>) -> io::Result<SocketAddrAny> {
    unsafe {
        let mut addr = SocketAddrBuf::new();
        ret(c::getsockname(
            borrowed_fd(sockfd),
            addr.storage.as_mut_ptr().cast::<c::sockaddr>(),
            &mut addr.len,
        ))?;
        Ok(addr.into_any())
    }
}

pub(crate) fn getpeername(sockfd: BorrowedFd<'_>) -> io::Result<Option<SocketAddrAny>> {
    unsafe {
        let mut addr = SocketAddrBuf::new();
        ret(c::getpeername(
            borrowed_fd(sockfd),
            addr.storage.as_mut_ptr().cast::<c::sockaddr>(),
            &mut addr.len,
        ))?;
        Ok(addr.into_any_option())
    }
}

#[cfg(not(windows))]
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
