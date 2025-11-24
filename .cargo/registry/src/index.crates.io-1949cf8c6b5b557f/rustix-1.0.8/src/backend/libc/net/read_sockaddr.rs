//! The BSD sockets API requires us to read the `sa_family` field before we can
//! interpret the rest of a `sockaddr` produced by the kernel.

#[cfg(unix)]
use super::addr::SocketAddrUnix;
use super::ext::{in6_addr_s6_addr, in_addr_s_addr, sockaddr_in6_sin6_scope_id};
use crate::backend::c;
#[cfg(not(windows))]
use crate::ffi::CStr;
use crate::io::Errno;
use crate::net::addr::SocketAddrLen;
#[cfg(linux_kernel)]
use crate::net::netlink::SocketAddrNetlink;
#[cfg(target_os = "linux")]
use crate::net::xdp::{SocketAddrXdp, SocketAddrXdpFlags};
use crate::net::{AddressFamily, Ipv4Addr, Ipv6Addr, SocketAddrAny, SocketAddrV4, SocketAddrV6};
use core::mem::size_of;

// This must match the header of `sockaddr`.
#[repr(C)]
pub(crate) struct sockaddr_header {
    #[cfg(any(
        bsd,
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "nto",
        target_os = "vita"
    ))]
    sa_len: u8,
    #[cfg(any(
        bsd,
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "nto",
        target_os = "vita"
    ))]
    sa_family: u8,
    #[cfg(not(any(
        bsd,
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "nto",
        target_os = "vita"
    )))]
    sa_family: u16,
}

/// Read the `sa_family` field from a socket address returned from the OS.
///
/// # Safety
///
/// `storage` must point to a valid socket address returned from the OS.
#[inline]
pub(crate) unsafe fn read_sa_family(storage: *const c::sockaddr) -> u16 {
    // Assert that we know the layout of `sockaddr`.
    let _ = c::sockaddr {
        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "vita"
        ))]
        sa_len: 0_u8,
        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "vita"
        ))]
        sa_family: 0_u8,
        #[cfg(not(any(
            bsd,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "vita"
        )))]
        sa_family: 0_u16,
        #[cfg(not(any(target_os = "haiku", target_os = "horizon")))]
        sa_data: [0; 14],
        #[cfg(target_os = "horizon")]
        sa_data: [0; 26],
        #[cfg(target_os = "haiku")]
        sa_data: [0; 30],
    };

    (*storage.cast::<sockaddr_header>()).sa_family.into()
}

/// Read the first byte of the `sun_path` field, assuming we have an `AF_UNIX`
/// socket address.
#[cfg(apple)]
#[inline]
unsafe fn read_sun_path0(storage: *const c::sockaddr) -> u8 {
    // In `read_sa_family` we assert that we know the layout of `sockaddr`.
    storage
        .cast::<u8>()
        .add(super::addr::offsetof_sun_path())
        .read()
}

/// Check if a socket address returned from the OS is considered non-empty.
///
/// # Safety
///
/// `storage` must point to a least an initialized `sockaddr_header`.
#[inline]
pub(crate) unsafe fn sockaddr_nonempty(storage: *const c::sockaddr, len: SocketAddrLen) -> bool {
    if len == 0 {
        return false;
    }

    assert!(len as usize >= size_of::<c::sa_family_t>());
    let family: c::c_int = read_sa_family(storage.cast::<c::sockaddr>()).into();
    if family == c::AF_UNSPEC {
        return false;
    }

    // On macOS, if we get an `AF_UNIX` with an empty path, treat it as an
    // absent address.
    #[cfg(apple)]
    if family == c::AF_UNIX && read_sun_path0(storage) == 0 {
        return false;
    }

    true
}

/// Set the `sa_family` field of a socket address to `AF_UNSPEC`, so that we
/// can test for `AF_UNSPEC` to test whether it was stored to.
///
/// # Safety
///
/// `storage` must point to a least an initialized `sockaddr_header`.
pub(crate) unsafe fn initialize_family_to_unspec(storage: *mut c::sockaddr) {
    (*storage.cast::<sockaddr_header>()).sa_family = c::AF_UNSPEC as _;
}

#[inline]
pub(crate) fn read_sockaddr_v4(addr: &SocketAddrAny) -> Result<SocketAddrV4, Errno> {
    if addr.address_family() != AddressFamily::INET {
        return Err(Errno::AFNOSUPPORT);
    }
    assert!(addr.addr_len() as usize >= size_of::<c::sockaddr_in>());
    let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_in>() };
    Ok(SocketAddrV4::new(
        Ipv4Addr::from(u32::from_be(in_addr_s_addr(decode.sin_addr))),
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
        Ipv6Addr::from(in6_addr_s6_addr(decode.sin6_addr)),
        u16::from_be(decode.sin6_port),
        u32::from_be(decode.sin6_flowinfo),
        sockaddr_in6_sin6_scope_id(decode),
    ))
}

#[cfg(unix)]
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
        #[cfg(linux_kernel)]
        if decode.sun_path[0] == 0 {
            let name = &decode.sun_path[1..len - offsetof_sun_path];
            let name = unsafe { core::mem::transmute::<&[c::c_char], &[u8]>(name) };
            return SocketAddrUnix::new_abstract_name(name);
        }

        // Otherwise we expect a NUL-terminated filesystem path.

        // Trim off unused bytes from the end of `path_bytes`.
        let path_bytes = if cfg!(any(solarish, target_os = "freebsd")) {
            // FreeBSD and illumos sometimes set the length to longer
            // than the length of the NUL-terminated string. Find the
            // NUL and truncate the string accordingly.
            &decode.sun_path[..decode
                .sun_path
                .iter()
                .position(|b| *b == 0)
                .ok_or(Errno::INVAL)?]
        } else {
            // Otherwise, use the provided length.
            let provided_len = len - 1 - offsetof_sun_path;
            if decode.sun_path[provided_len] != 0 {
                return Err(Errno::INVAL);
            }
            debug_assert_eq!(
                unsafe { CStr::from_ptr(decode.sun_path.as_ptr().cast()) }
                    .to_bytes()
                    .len(),
                provided_len
            );
            &decode.sun_path[..provided_len]
        };

        SocketAddrUnix::new(unsafe { core::mem::transmute::<&[c::c_char], &[u8]>(path_bytes) })
    }
}

#[cfg(target_os = "linux")]
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

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn read_sockaddr_netlink(addr: &SocketAddrAny) -> Result<SocketAddrNetlink, Errno> {
    if addr.address_family() != AddressFamily::NETLINK {
        return Err(Errno::AFNOSUPPORT);
    }
    assert!(addr.addr_len() as usize >= size_of::<c::sockaddr_nl>());
    let decode = unsafe { &*addr.as_ptr().cast::<c::sockaddr_nl>() };
    Ok(SocketAddrNetlink::new(decode.nl_pid, decode.nl_groups))
}
