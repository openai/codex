//! The BSD sockets API requires us to read the `sa_family` field before we can
//! interpret the rest of a `sockaddr` produced by the kernel.
#![allow(unsafe_code)]

use crate::backend::c;
use crate::io::Errno;
use crate::net::addr::SocketAddrLen;
use crate::net::netlink::SocketAddrNetlink;
#[cfg(target_os = "linux")]
use crate::net::xdp::{SocketAddrXdp, SocketAddrXdpFlags};
use crate::net::{
    AddressFamily, Ipv4Addr, Ipv6Addr, SocketAddrAny, SocketAddrUnix, SocketAddrV4, SocketAddrV6,
};
use core::mem::size_of;
use core::slice;

// This must match the header of `sockaddr`.
#[repr(C)]
pub(crate) struct sockaddr_header {
    sa_family: u16,
}

/// Read the `sa_family` field from a socket address returned from the OS.
///
/// # Safety
///
/// `storage` must point to a least an initialized `sockaddr_header`.
#[inline]
pub(crate) const unsafe fn read_sa_family(storage: *const c::sockaddr) -> u16 {
    // Assert that we know the layout of `sockaddr`.
    let _ = c::sockaddr {
        __storage: c::sockaddr_storage {
            __bindgen_anon_1: linux_raw_sys::net::__kernel_sockaddr_storage__bindgen_ty_1 {
                __bindgen_anon_1:
                    linux_raw_sys::net::__kernel_sockaddr_storage__bindgen_ty_1__bindgen_ty_1 {
                        ss_family: 0_u16,
                        __data: [0; 126_usize],
                    },
            },
        },
    };

    (*storage.cast::<sockaddr_header>()).sa_family
}

/// Set the `sa_family` field of a socket address to `AF_UNSPEC`, so that we
/// can test for `AF_UNSPEC` to test whether it was stored to.
///
/// # Safety
///
/// `storage` must point to a least an initialized `sockaddr_header`.
#[inline]
pub(crate) unsafe fn initialize_family_to_unspec(storage: *mut c::sockaddr) {
    (*storage.cast::<sockaddr_header>()).sa_family = c::AF_UNSPEC as _;
}

/// Check if a socket address returned from the OS is considered non-empty.
#[inline]
pub(crate) unsafe fn sockaddr_nonempty(_storage: *const c::sockaddr, len: SocketAddrLen) -> bool {
    len != 0
}

#[inline]
pub(crate) fn read_sockaddr_v4(addr: &SocketAddrAny) -> Result<SocketAddrV4, Errno> {
    if addr.address_family() != AddressFamily::INET {
        return Err(Errno::AFNOSUPPORT);
    }
    assert!(addr.addr_len() as usize >= size_of::<c::sockaddr_in>());
    let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_in>() };
    Ok(SocketAddrV4::new(
        Ipv4Addr::from(u32::from_be(decode.sin_addr.s_addr)),
        u16::from_be(decode.sin_port),
    ))
}

#[inline]
pub(crate) fn read_sockaddr_v6(addr: &SocketAddrAny) -> Result<SocketAddrV6, Errno> {
    if addr.address_family() != AddressFamily::INET6 {
        return Err(Errno::AFNOSUPPORT);
    }
    assert!(addr.addr_len() as usize >= size_of::<c::sockaddr_in6>());
    let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_in6>() };
    Ok(SocketAddrV6::new(
        Ipv6Addr::from(unsafe { decode.sin6_addr.in6_u.u6_addr8 }),
        u16::from_be(decode.sin6_port),
        u32::from_be(decode.sin6_flowinfo),
        decode.sin6_scope_id,
    ))
}

#[inline]
pub(crate) fn read_sockaddr_unix(addr: &SocketAddrAny) -> Result<SocketAddrUnix, Errno> {
    if addr.address_family() != AddressFamily::UNIX {
        return Err(Errno::AFNOSUPPORT);
    }
    let offsetof_sun_path = super::addr::offsetof_sun_path();
    let len = addr.addr_len() as usize;

    assert!(len >= offsetof_sun_path);

    if len == offsetof_sun_path {
        SocketAddrUnix::new(&[][..])
    } else {
        let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_un>() };

        // On Linux check for Linux's [abstract namespace].
        //
        // [abstract namespace]: https://man7.org/linux/man-pages/man7/unix.7.html
        if decode.sun_path[0] == 0 {
            let bytes = &decode.sun_path[1..len - offsetof_sun_path];

            // SAFETY: Convert `&[c_char]` to `&[u8]`.
            let bytes = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) };

            return SocketAddrUnix::new_abstract_name(bytes);
        }

        // Otherwise we expect a NUL-terminated filesystem path.
        let bytes = &decode.sun_path[..len - 1 - offsetof_sun_path];

        // SAFETY: Convert `&[c_char]` to `&[u8]`.
        let bytes = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) };

        assert_eq!(decode.sun_path[len - 1 - offsetof_sun_path], 0);
        SocketAddrUnix::new(bytes)
    }
}

#[inline]
pub(crate) fn read_sockaddr_xdp(addr: &SocketAddrAny) -> Result<SocketAddrXdp, Errno> {
    if addr.address_family() != AddressFamily::XDP {
        return Err(Errno::AFNOSUPPORT);
    }
    assert!(addr.addr_len() as usize >= size_of::<c::sockaddr_xdp>());
    let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_xdp>() };

    // This ignores the `sxdp_shared_umem_fd` field, which is only expected to
    // be significant in `bind` calls, and not returned from `acceptfrom` or
    // `recvmsg` or similar.
    Ok(SocketAddrXdp::new(
        SocketAddrXdpFlags::from_bits_retain(decode.sxdp_flags),
        u32::from_be(decode.sxdp_ifindex),
        u32::from_be(decode.sxdp_queue_id),
    ))
}

#[inline]
pub(crate) fn read_sockaddr_netlink(addr: &SocketAddrAny) -> Result<SocketAddrNetlink, Errno> {
    if addr.address_family() != AddressFamily::NETLINK {
        return Err(Errno::AFNOSUPPORT);
    }
    assert!(addr.addr_len() as usize >= size_of::<c::sockaddr_nl>());
    let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_nl>() };
    Ok(SocketAddrNetlink::new(decode.nl_pid, decode.nl_groups))
}
