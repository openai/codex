//! linux_raw syscalls supporting `rustix::net`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

#[cfg(target_os = "linux")]
use super::msghdr::with_xdp_msghdr;
use super::msghdr::{
    with_noaddr_msghdr, with_recv_msghdr, with_unix_msghdr, with_v4_msghdr, with_v6_msghdr,
};
use super::read_sockaddr::{initialize_family_to_unspec, maybe_read_sockaddr_os, read_sockaddr_os};
use super::send_recv::{RecvFlags, SendFlags};
#[cfg(target_os = "linux")]
use super::write_sockaddr::encode_sockaddr_xdp;
use super::write_sockaddr::{encode_sockaddr_v4, encode_sockaddr_v6};
use crate::backend::c;
use crate::backend::conv::{
    by_mut, by_ref, c_int, c_uint, pass_usize, ret, ret_owned_fd, ret_usize, size_of, slice,
    socklen_t, zero,
};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::io::{self, IoSlice, IoSliceMut};
#[cfg(target_os = "linux")]
use crate::net::xdp::SocketAddrXdp;
use crate::net::{
    AddressFamily, Protocol, RecvAncillaryBuffer, RecvMsgReturn, SendAncillaryBuffer, Shutdown,
    SocketAddrAny, SocketAddrUnix, SocketAddrV4, SocketAddrV6, SocketFlags, SocketType,
};
use c::{sockaddr, sockaddr_in, sockaddr_in6, socklen_t};
use core::mem::MaybeUninit;
#[cfg(target_arch = "x86")]
use {
    crate::backend::conv::{slice_just_addr, x86_sys},
    crate::backend::reg::{ArgReg, SocketArg},
    linux_raw_sys::net::{
        SYS_ACCEPT, SYS_ACCEPT4, SYS_BIND, SYS_CONNECT, SYS_GETPEERNAME, SYS_GETSOCKNAME,
        SYS_LISTEN, SYS_RECV, SYS_RECVFROM, SYS_RECVMSG, SYS_SEND, SYS_SENDMSG, SYS_SENDTO,
        SYS_SHUTDOWN, SYS_SOCKET, SYS_SOCKETPAIR,
    },
};

#[inline]
pub(crate) fn socket(
    family: AddressFamily,
    type_: SocketType,
    protocol: Option<Protocol>,
) -> io::Result<OwnedFd> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret_owned_fd(syscall_readonly!(__NR_socket, family, type_, protocol))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SOCKET),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                family.into(),
                type_.into(),
                protocol.into(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn socket_with(
    family: AddressFamily,
    type_: SocketType,
    flags: SocketFlags,
    protocol: Option<Protocol>,
) -> io::Result<OwnedFd> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_socket,
            family,
            (type_, flags),
            protocol
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_owned_fd(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SOCKET),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                family.into(),
                (type_, flags).into(),
                protocol.into(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn socketpair(
    family: AddressFamily,
    type_: SocketType,
    flags: SocketFlags,
    protocol: Option<Protocol>,
) -> io::Result<(OwnedFd, OwnedFd)> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(syscall!(
            __NR_socketpair,
            family,
            (type_, flags),
            protocol,
            &mut result
        ))?;
        let [fd0, fd1] = result.assume_init();
        Ok((fd0, fd1))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(syscall!(
            __NR_socketcall,
            x86_sys(SYS_SOCKETPAIR),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                family.into(),
                (type_, flags).into(),
                protocol.into(),
                (&mut result).into(),
            ])
        ))?;
        let [fd0, fd1] = result.assume_init();
        Ok((fd0, fd1))
    }
}

#[inline]
pub(crate) fn accept(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    #[cfg(not(any(target_arch = "x86", target_arch = "s390x")))]
    unsafe {
        let fd = ret_owned_fd(syscall_readonly!(__NR_accept, fd, zero(), zero()))?;
        Ok(fd)
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let fd = ret_owned_fd(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_ACCEPT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[fd.into(), zero(), zero()])
        ))?;
        Ok(fd)
    }
    #[cfg(target_arch = "s390x")]
    {
        // accept is not available on s390x
        accept_with(fd, SocketFlags::empty())
    }
}

#[inline]
pub(crate) fn accept_with(fd: BorrowedFd<'_>, flags: SocketFlags) -> io::Result<OwnedFd> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        let fd = ret_owned_fd(syscall_readonly!(__NR_accept4, fd, zero(), zero(), flags))?;
        Ok(fd)
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let fd = ret_owned_fd(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_ACCEPT4),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[fd.into(), zero(), zero(), flags.into()])
        ))?;
        Ok(fd)
    }
}

#[inline]
pub(crate) fn acceptfrom(fd: BorrowedFd<'_>) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    #[cfg(not(any(target_arch = "x86", target_arch = "s390x")))]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        let fd = ret_owned_fd(syscall!(
            __NR_accept,
            fd,
            &mut storage,
            by_mut(&mut addrlen)
        ))?;
        Ok((
            fd,
            maybe_read_sockaddr_os(&storage.assume_init(), addrlen.try_into().unwrap()),
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        let fd = ret_owned_fd(syscall!(
            __NR_socketcall,
            x86_sys(SYS_ACCEPT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                (&mut storage).into(),
                by_mut(&mut addrlen),
            ])
        ))?;
        Ok((
            fd,
            maybe_read_sockaddr_os(&storage.assume_init(), addrlen.try_into().unwrap()),
        ))
    }
    #[cfg(target_arch = "s390x")]
    {
        // accept is not available on s390x
        acceptfrom_with(fd, SocketFlags::empty())
    }
}

#[inline]
pub(crate) fn acceptfrom_with(
    fd: BorrowedFd<'_>,
    flags: SocketFlags,
) -> io::Result<(OwnedFd, Option<SocketAddrAny>)> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        let fd = ret_owned_fd(syscall!(
            __NR_accept4,
            fd,
            &mut storage,
            by_mut(&mut addrlen),
            flags
        ))?;
        Ok((
            fd,
            maybe_read_sockaddr_os(&storage.assume_init(), addrlen.try_into().unwrap()),
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        let fd = ret_owned_fd(syscall!(
            __NR_socketcall,
            x86_sys(SYS_ACCEPT4),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                (&mut storage).into(),
                by_mut(&mut addrlen),
                flags.into(),
            ])
        ))?;
        Ok((
            fd,
            maybe_read_sockaddr_os(&storage.assume_init(), addrlen.try_into().unwrap()),
        ))
    }
}

#[inline]
pub(crate) fn recvmsg(
    sockfd: BorrowedFd<'_>,
    iov: &mut [IoSliceMut<'_>],
    control: &mut RecvAncillaryBuffer<'_>,
    msg_flags: RecvFlags,
) -> io::Result<RecvMsgReturn> {
    let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();

    with_recv_msghdr(&mut storage, iov, control, |msghdr| {
        #[cfg(not(target_arch = "x86"))]
        let result =
            unsafe { ret_usize(syscall!(__NR_recvmsg, sockfd, by_mut(msghdr), msg_flags)) };

        #[cfg(target_arch = "x86")]
        let result = unsafe {
            ret_usize(syscall!(
                __NR_socketcall,
                x86_sys(SYS_RECVMSG),
                slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                    sockfd.into(),
                    by_mut(msghdr),
                    msg_flags.into(),
                ])
            ))
        };

        result.map(|bytes| {
            // Get the address of the sender, if any.
            let addr =
                unsafe { maybe_read_sockaddr_os(msghdr.msg_name as _, msghdr.msg_namelen as _) };

            RecvMsgReturn {
                bytes,
                address: addr,
                flags: RecvFlags::from_bits_retain(msghdr.msg_flags),
            }
        })
    })
}

#[inline]
pub(crate) fn sendmsg(
    sockfd: BorrowedFd<'_>,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_noaddr_msghdr(iov, control, |msghdr| {
        #[cfg(not(target_arch = "x86"))]
        let result =
            unsafe { ret_usize(syscall!(__NR_sendmsg, sockfd, by_ref(&msghdr), msg_flags)) };

        #[cfg(target_arch = "x86")]
        let result = unsafe {
            ret_usize(syscall!(
                __NR_socketcall,
                x86_sys(SYS_SENDMSG),
                slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                    sockfd.into(),
                    by_ref(&msghdr),
                    msg_flags.into()
                ])
            ))
        };

        result
    })
}

#[inline]
pub(crate) fn sendmsg_v4(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrV4,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_v4_msghdr(addr, iov, control, |msghdr| {
        #[cfg(not(target_arch = "x86"))]
        let result =
            unsafe { ret_usize(syscall!(__NR_sendmsg, sockfd, by_ref(&msghdr), msg_flags)) };

        #[cfg(target_arch = "x86")]
        let result = unsafe {
            ret_usize(syscall!(
                __NR_socketcall,
                x86_sys(SYS_SENDMSG),
                slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                    sockfd.into(),
                    by_ref(&msghdr),
                    msg_flags.into(),
                ])
            ))
        };

        result
    })
}

#[inline]
pub(crate) fn sendmsg_v6(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrV6,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_v6_msghdr(addr, iov, control, |msghdr| {
        #[cfg(not(target_arch = "x86"))]
        let result =
            unsafe { ret_usize(syscall!(__NR_sendmsg, sockfd, by_ref(&msghdr), msg_flags)) };

        #[cfg(target_arch = "x86")]
        let result = unsafe {
            ret_usize(syscall!(
                __NR_socketcall,
                x86_sys(SYS_SENDMSG),
                slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                    sockfd.into(),
                    by_ref(&msghdr),
                    msg_flags.into()
                ])
            ))
        };

        result
    })
}

#[inline]
pub(crate) fn sendmsg_unix(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrUnix,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_unix_msghdr(addr, iov, control, |msghdr| {
        #[cfg(not(target_arch = "x86"))]
        let result =
            unsafe { ret_usize(syscall!(__NR_sendmsg, sockfd, by_ref(&msghdr), msg_flags)) };

        #[cfg(target_arch = "x86")]
        let result = unsafe {
            ret_usize(syscall!(
                __NR_socketcall,
                x86_sys(SYS_SENDMSG),
                slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                    sockfd.into(),
                    by_ref(&msghdr),
                    msg_flags.into()
                ])
            ))
        };

        result
    })
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn sendmsg_xdp(
    sockfd: BorrowedFd<'_>,
    addr: &SocketAddrXdp,
    iov: &[IoSlice<'_>],
    control: &mut SendAncillaryBuffer<'_, '_, '_>,
    msg_flags: SendFlags,
) -> io::Result<usize> {
    with_xdp_msghdr(addr, iov, control, |msghdr| {
        #[cfg(not(target_arch = "x86"))]
        let result =
            unsafe { ret_usize(syscall!(__NR_sendmsg, sockfd, by_ref(&msghdr), msg_flags)) };

        #[cfg(target_arch = "x86")]
        let result = unsafe {
            ret_usize(syscall!(
                __NR_socketcall,
                x86_sys(SYS_SENDMSG),
                slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                    sockfd.into(),
                    by_ref(&msghdr),
                    msg_flags.into()
                ])
            ))
        };

        result
    })
}

#[inline]
pub(crate) fn shutdown(fd: BorrowedFd<'_>, how: Shutdown) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_shutdown,
            fd,
            c_uint(how as c::c_uint)
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SHUTDOWN),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[fd.into(), c_uint(how as c::c_uint)])
        ))
    }
}

#[inline]
pub(crate) fn send(fd: BorrowedFd<'_>, buf: &[u8], flags: SendFlags) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64",
    )))]
    unsafe {
        ret_usize(syscall_readonly!(__NR_send, fd, buf_addr, buf_len, flags))
    }
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
        target_arch = "s390x",
        target_arch = "x86_64",
    ))]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_sendto,
            fd,
            buf_addr,
            buf_len,
            flags,
            zero(),
            zero()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SEND),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                buf_addr,
                buf_len,
                flags.into()
            ])
        ))
    }
}

#[inline]
pub(crate) fn sendto_v4(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrV4,
) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_sendto,
            fd,
            buf_addr,
            buf_len,
            flags,
            by_ref(&encode_sockaddr_v4(addr)),
            size_of::<sockaddr_in, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SENDTO),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                buf_addr,
                buf_len,
                flags.into(),
                by_ref(&encode_sockaddr_v4(addr)),
                size_of::<sockaddr_in, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn sendto_v6(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrV6,
) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_sendto,
            fd,
            buf_addr,
            buf_len,
            flags,
            by_ref(&encode_sockaddr_v6(addr)),
            size_of::<sockaddr_in6, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SENDTO),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                buf_addr,
                buf_len,
                flags.into(),
                by_ref(&encode_sockaddr_v6(addr)),
                size_of::<sockaddr_in6, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn sendto_unix(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrUnix,
) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_sendto,
            fd,
            buf_addr,
            buf_len,
            flags,
            by_ref(&addr.unix),
            socklen_t(addr.addr_len())
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SENDTO),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                buf_addr,
                buf_len,
                flags.into(),
                by_ref(&addr.unix),
                socklen_t(addr.addr_len()),
            ])
        ))
    }
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn sendto_xdp(
    fd: BorrowedFd<'_>,
    buf: &[u8],
    flags: SendFlags,
    addr: &SocketAddrXdp,
) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_sendto,
            fd,
            buf_addr,
            buf_len,
            flags,
            by_ref(&encode_sockaddr_xdp(addr)),
            size_of::<c::sockaddr_xdp, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret_usize(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_SENDTO),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                buf_addr,
                buf_len,
                flags.into(),
                by_ref(&encode_sockaddr_xdp(addr)),
                size_of::<c::sockaddr_xdp, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) unsafe fn recv(
    fd: BorrowedFd<'_>,
    buf: *mut u8,
    len: usize,
    flags: RecvFlags,
) -> io::Result<usize> {
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
        target_arch = "s390x",
        target_arch = "x86",
        target_arch = "x86_64",
    )))]
    {
        ret_usize(syscall!(__NR_recv, fd, buf, pass_usize(len), flags))
    }
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
        target_arch = "s390x",
        target_arch = "x86_64",
    ))]
    {
        ret_usize(syscall!(
            __NR_recvfrom,
            fd,
            buf,
            pass_usize(len),
            flags,
            zero(),
            zero()
        ))
    }
    #[cfg(target_arch = "x86")]
    {
        ret_usize(syscall!(
            __NR_socketcall,
            x86_sys(SYS_RECV),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                buf.into(),
                pass_usize(len),
                flags.into(),
            ])
        ))
    }
}

#[inline]
pub(crate) unsafe fn recvfrom(
    fd: BorrowedFd<'_>,
    buf: *mut u8,
    len: usize,
    flags: RecvFlags,
) -> io::Result<(usize, Option<SocketAddrAny>)> {
    let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
    let mut storage = MaybeUninit::<sockaddr>::uninit();

    // `recvfrom` does not write to the storage if the socket is
    // connection-oriented sockets, so we initialize the family field to
    // `AF_UNSPEC` so that we can detect this case.
    initialize_family_to_unspec(storage.as_mut_ptr());

    #[cfg(not(target_arch = "x86"))]
    let nread = ret_usize(syscall!(
        __NR_recvfrom,
        fd,
        buf,
        pass_usize(len),
        flags,
        &mut storage,
        by_mut(&mut addrlen)
    ))?;
    #[cfg(target_arch = "x86")]
    let nread = ret_usize(syscall!(
        __NR_socketcall,
        x86_sys(SYS_RECVFROM),
        slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
            fd.into(),
            buf.into(),
            pass_usize(len),
            flags.into(),
            (&mut storage).into(),
            by_mut(&mut addrlen),
        ])
    ))?;

    Ok((
        nread,
        maybe_read_sockaddr_os(&storage.assume_init(), addrlen.try_into().unwrap()),
    ))
}

#[inline]
pub(crate) fn getpeername(fd: BorrowedFd<'_>) -> io::Result<Option<SocketAddrAny>> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        ret(syscall!(
            __NR_getpeername,
            fd,
            &mut storage,
            by_mut(&mut addrlen)
        ))?;
        Ok(maybe_read_sockaddr_os(
            &storage.assume_init(),
            addrlen.try_into().unwrap(),
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        ret(syscall!(
            __NR_socketcall,
            x86_sys(SYS_GETPEERNAME),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                (&mut storage).into(),
                by_mut(&mut addrlen),
            ])
        ))?;
        Ok(maybe_read_sockaddr_os(
            &storage.assume_init(),
            addrlen.try_into().unwrap(),
        ))
    }
}

#[inline]
pub(crate) fn getsockname(fd: BorrowedFd<'_>) -> io::Result<SocketAddrAny> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        ret(syscall!(
            __NR_getsockname,
            fd,
            &mut storage,
            by_mut(&mut addrlen)
        ))?;
        Ok(read_sockaddr_os(
            &storage.assume_init(),
            addrlen.try_into().unwrap(),
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        let mut addrlen = core::mem::size_of::<sockaddr>() as socklen_t;
        let mut storage = MaybeUninit::<sockaddr>::uninit();
        ret(syscall!(
            __NR_socketcall,
            x86_sys(SYS_GETSOCKNAME),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                (&mut storage).into(),
                by_mut(&mut addrlen),
            ])
        ))?;
        Ok(read_sockaddr_os(
            &storage.assume_init(),
            addrlen.try_into().unwrap(),
        ))
    }
}

#[inline]
pub(crate) fn bind_v4(fd: BorrowedFd<'_>, addr: &SocketAddrV4) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_bind,
            fd,
            by_ref(&encode_sockaddr_v4(addr)),
            size_of::<sockaddr_in, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_BIND),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&encode_sockaddr_v4(addr)),
                size_of::<sockaddr_in, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn bind_v6(fd: BorrowedFd<'_>, addr: &SocketAddrV6) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_bind,
            fd,
            by_ref(&encode_sockaddr_v6(addr)),
            size_of::<sockaddr_in6, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_BIND),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&encode_sockaddr_v6(addr)),
                size_of::<sockaddr_in6, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn bind_unix(fd: BorrowedFd<'_>, addr: &SocketAddrUnix) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_bind,
            fd,
            by_ref(&addr.unix),
            socklen_t(addr.addr_len())
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_BIND),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&addr.unix),
                socklen_t(addr.addr_len()),
            ])
        ))
    }
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn bind_xdp(fd: BorrowedFd<'_>, addr: &SocketAddrXdp) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_bind,
            fd,
            by_ref(&encode_sockaddr_xdp(addr)),
            size_of::<c::sockaddr_xdp, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_BIND),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&encode_sockaddr_xdp(addr)),
                size_of::<c::sockaddr_xdp, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn connect_v4(fd: BorrowedFd<'_>, addr: &SocketAddrV4) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_connect,
            fd,
            by_ref(&encode_sockaddr_v4(addr)),
            size_of::<sockaddr_in, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_CONNECT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&encode_sockaddr_v4(addr)),
                size_of::<sockaddr_in, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn connect_v6(fd: BorrowedFd<'_>, addr: &SocketAddrV6) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_connect,
            fd,
            by_ref(&encode_sockaddr_v6(addr)),
            size_of::<sockaddr_in6, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_CONNECT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&encode_sockaddr_v6(addr)),
                size_of::<sockaddr_in6, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn connect_unix(fd: BorrowedFd<'_>, addr: &SocketAddrUnix) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_connect,
            fd,
            by_ref(&addr.unix),
            socklen_t(addr.addr_len())
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_CONNECT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&addr.unix),
                socklen_t(addr.addr_len()),
            ])
        ))
    }
}

#[inline]
pub(crate) fn connect_unspec(fd: BorrowedFd<'_>) -> io::Result<()> {
    debug_assert_eq!(c::AF_UNSPEC, 0);
    let addr = MaybeUninit::<c::sockaddr_storage>::zeroed();

    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(
            __NR_connect,
            fd,
            by_ref(&addr),
            size_of::<c::sockaddr_storage, _>()
        ))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_CONNECT),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[
                fd.into(),
                by_ref(&addr),
                size_of::<c::sockaddr_storage, _>(),
            ])
        ))
    }
}

#[inline]
pub(crate) fn listen(fd: BorrowedFd<'_>, backlog: c::c_int) -> io::Result<()> {
    #[cfg(not(target_arch = "x86"))]
    unsafe {
        ret(syscall_readonly!(__NR_listen, fd, c_int(backlog)))
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        ret(syscall_readonly!(
            __NR_socketcall,
            x86_sys(SYS_LISTEN),
            slice_just_addr::<ArgReg<'_, SocketArg>, _>(&[fd.into(), c_int(backlog)])
        ))
    }
}
