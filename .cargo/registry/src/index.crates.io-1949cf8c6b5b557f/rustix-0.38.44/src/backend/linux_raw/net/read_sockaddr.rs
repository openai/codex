//! The BSD sockets API requires us to read the `ss_family` field before we can
//! interpret the rest of a `sockaddr` produced by the kernel.
#![allow(unsafe_code)]

use crate::backend::c;
use crate::io;
#[cfg(target_os = "linux")]
use crate::net::xdp::{SockaddrXdpFlags, SocketAddrXdp};
use crate::net::{Ipv4Addr, Ipv6Addr, SocketAddrAny, SocketAddrUnix, SocketAddrV4, SocketAddrV6};
use core::mem::size_of;
use core::slice;

// This must match the header of `sockaddr`.
#[repr(C)]
struct sockaddr_header {
    ss_family: u16,
}

/// Read the `ss_family` field from a socket address returned from the OS.
///
/// # Safety
///
/// `storage` must point to a valid socket address returned from the OS.
#[inline]
unsafe fn read_ss_family(storage: *const c::sockaddr) -> u16 {
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

    (*storage.cast::<sockaddr_header>()).ss_family
}

/// Set the `ss_family` field of a socket address to `AF_UNSPEC`, so that we
/// can test for `AF_UNSPEC` to test whether it was stored to.
#[inline]
pub(crate) unsafe fn initialize_family_to_unspec(storage: *mut c::sockaddr) {
    (*storage.cast::<sockaddr_header>()).ss_family = c::AF_UNSPEC as _;
}

/// Read a socket address encoded in a platform-specific format.
///
/// # Safety
///
/// `storage` must point to valid socket address storage.
pub(crate) unsafe fn read_sockaddr(
    storage: *const c::sockaddr,
    len: usize,
) -> io::Result<SocketAddrAny> {
    let offsetof_sun_path = super::addr::offsetof_sun_path();

    if len < size_of::<c::sa_family_t>() {
        return Err(io::Errno::INVAL);
    }
    match read_ss_family(storage).into() {
        c::AF_INET => {
            if len < size_of::<c::sockaddr_in>() {
                return Err(io::Errno::INVAL);
            }
            let decode = &*storage.cast::<c::sockaddr_in>();
            Ok(SocketAddrAny::V4(SocketAddrV4::new(
                Ipv4Addr::from(u32::from_be(decode.sin_addr.s_addr)),
                u16::from_be(decode.sin_port),
            )))
        }
        c::AF_INET6 => {
            if len < size_of::<c::sockaddr_in6>() {
                return Err(io::Errno::INVAL);
            }
            let decode = &*storage.cast::<c::sockaddr_in6>();
            Ok(SocketAddrAny::V6(SocketAddrV6::new(
                Ipv6Addr::from(decode.sin6_addr.in6_u.u6_addr8),
                u16::from_be(decode.sin6_port),
                u32::from_be(decode.sin6_flowinfo),
                decode.sin6_scope_id,
            )))
        }
        c::AF_UNIX => {
            if len < offsetof_sun_path {
                return Err(io::Errno::INVAL);
            }
            if len == offsetof_sun_path {
                Ok(SocketAddrAny::Unix(SocketAddrUnix::new(&[][..])?))
            } else {
                let decode = &*storage.cast::<c::sockaddr_un>();

                // On Linux check for Linux's [abstract namespace].
                //
                // [abstract namespace]: https://man7.org/linux/man-pages/man7/unix.7.html
                if decode.sun_path[0] == 0 {
                    let bytes = &decode.sun_path[1..len - offsetof_sun_path];

                    // SAFETY: Convert `&[c_char]` to `&[u8]`.
                    let bytes = slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len());

                    return SocketAddrUnix::new_abstract_name(bytes).map(SocketAddrAny::Unix);
                }

                // Otherwise we expect a NUL-terminated filesystem path.
                let bytes = &decode.sun_path[..len - 1 - offsetof_sun_path];

                // SAFETY: Convert `&[c_char]` to `&[u8]`.
                let bytes = slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len());

                assert_eq!(decode.sun_path[len - 1 - offsetof_sun_path], 0);
                Ok(SocketAddrAny::Unix(SocketAddrUnix::new(bytes)?))
            }
        }
        #[cfg(target_os = "linux")]
        c::AF_XDP => {
            if len < size_of::<c::sockaddr_xdp>() {
                return Err(io::Errno::INVAL);
            }
            let decode = &*storage.cast::<c::sockaddr_xdp>();
            Ok(SocketAddrAny::Xdp(SocketAddrXdp::new(
                SockaddrXdpFlags::from_bits_retain(decode.sxdp_flags),
                u32::from_be(decode.sxdp_ifindex),
                u32::from_be(decode.sxdp_queue_id),
                u32::from_be(decode.sxdp_shared_umem_fd),
            )))
        }
        _ => Err(io::Errno::NOTSUP),
    }
}

/// Read an optional socket address returned from the OS.
///
/// # Safety
///
/// `storage` must point to a valid socket address returned from the OS.
pub(crate) unsafe fn maybe_read_sockaddr_os(
    storage: *const c::sockaddr,
    len: usize,
) -> Option<SocketAddrAny> {
    if len == 0 {
        None
    } else {
        Some(read_sockaddr_os(storage, len))
    }
}

/// Read a socket address returned from the OS.
///
/// # Safety
///
/// `storage` must point to a valid socket address returned from the OS.
pub(crate) unsafe fn read_sockaddr_os(storage: *const c::sockaddr, len: usize) -> SocketAddrAny {
    let offsetof_sun_path = super::addr::offsetof_sun_path();

    assert!(len >= size_of::<c::sa_family_t>());
    match read_ss_family(storage).into() {
        c::AF_INET => {
            assert!(len >= size_of::<c::sockaddr_in>());
            let decode = &*storage.cast::<c::sockaddr_in>();
            SocketAddrAny::V4(SocketAddrV4::new(
                Ipv4Addr::from(u32::from_be(decode.sin_addr.s_addr)),
                u16::from_be(decode.sin_port),
            ))
        }
        c::AF_INET6 => {
            assert!(len >= size_of::<c::sockaddr_in6>());
            let decode = &*storage.cast::<c::sockaddr_in6>();
            SocketAddrAny::V6(SocketAddrV6::new(
                Ipv6Addr::from(decode.sin6_addr.in6_u.u6_addr8),
                u16::from_be(decode.sin6_port),
                u32::from_be(decode.sin6_flowinfo),
                decode.sin6_scope_id,
            ))
        }
        c::AF_UNIX => {
            assert!(len >= offsetof_sun_path);
            if len == offsetof_sun_path {
                SocketAddrAny::Unix(SocketAddrUnix::new(&[][..]).unwrap())
            } else {
                let decode = &*storage.cast::<c::sockaddr_un>();

                // On Linux check for Linux's [abstract namespace].
                //
                // [abstract namespace]: https://man7.org/linux/man-pages/man7/unix.7.html
                if decode.sun_path[0] == 0 {
                    let bytes = &decode.sun_path[1..len - offsetof_sun_path];

                    // SAFETY: Convert `&[c_char]` to `&[u8]`.
                    let bytes = slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len());

                    return SocketAddrAny::Unix(SocketAddrUnix::new_abstract_name(bytes).unwrap());
                }

                // Otherwise we expect a NUL-terminated filesystem path.
                assert_eq!(decode.sun_path[len - 1 - offsetof_sun_path], 0);

                let bytes = &decode.sun_path[..len - 1 - offsetof_sun_path];

                // SAFETY: Convert `&[c_char]` to `&[u8]`.
                let bytes = slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len());

                SocketAddrAny::Unix(SocketAddrUnix::new(bytes).unwrap())
            }
        }
        #[cfg(target_os = "linux")]
        c::AF_XDP => {
            assert!(len >= size_of::<c::sockaddr_xdp>());
            let decode = &*storage.cast::<c::sockaddr_xdp>();
            SocketAddrAny::Xdp(SocketAddrXdp::new(
                SockaddrXdpFlags::from_bits_retain(decode.sxdp_flags),
                u32::from_be(decode.sxdp_ifindex),
                u32::from_be(decode.sxdp_queue_id),
                u32::from_be(decode.sxdp_shared_umem_fd),
            ))
        }
        other => unimplemented!("{:?}", other),
    }
}
