use std::io;
use std::mem::size_of;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

pub(crate) fn new_ip_socket(addr: SocketAddr, socket_type: libc::c_int) -> io::Result<libc::c_int> {
    let domain = match addr {
        SocketAddr::V4(..) => libc::AF_INET,
        SocketAddr::V6(..) => libc::AF_INET6,
    };

    new_socket(domain, socket_type)
}

/// Create a new non-blocking socket.
pub(crate) fn new_socket(domain: libc::c_int, socket_type: libc::c_int) -> io::Result<libc::c_int> {
    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "hermit",
    ))]
    let socket_type = socket_type | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
    #[cfg(target_os = "nto")]
    let socket_type = socket_type | libc::SOCK_CLOEXEC;

    let socket = syscall!(socket(domain, socket_type, 0))?;

    // Mimic `libstd` and set `SO_NOSIGPIPE` on apple systems.
    #[cfg(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    if let Err(err) = syscall!(setsockopt(
        socket,
        libc::SOL_SOCKET,
        libc::SO_NOSIGPIPE,
        &1 as *const libc::c_int as *const libc::c_void,
        size_of::<libc::c_int>() as libc::socklen_t
    )) {
        let _ = syscall!(close(socket));
        return Err(err);
    }

    // Darwin (and others) doesn't have SOCK_NONBLOCK or SOCK_CLOEXEC.
    #[cfg(any(
        target_os = "aix",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "vita",
        target_os = "nto",
    ))]
    {
        if let Err(err) = syscall!(fcntl(socket, libc::F_SETFL, libc::O_NONBLOCK)) {
            let _ = syscall!(close(socket));
            return Err(err);
        }
        #[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "nto")))]
        if let Err(err) = syscall!(fcntl(socket, libc::F_SETFD, libc::FD_CLOEXEC)) {
            let _ = syscall!(close(socket));
            return Err(err);
        }
    }

    Ok(socket)
}

/// A type with the same memory layout as `libc::sockaddr`. Used in converting Rust level
/// SocketAddr* types into their system representation. The benefit of this specific
/// type over using `libc::sockaddr_storage` is that this type is exactly as large as it
/// needs to be and not a lot larger. And it can be initialized cleaner from Rust.
#[repr(C)]
pub(crate) union SocketAddrCRepr {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SocketAddrCRepr {
    pub(crate) fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }
}

/// Converts a Rust `SocketAddr` into the system representation.
pub(crate) fn socket_addr(addr: &SocketAddr) -> (SocketAddrCRepr, libc::socklen_t) {
    match addr {
        SocketAddr::V4(ref addr) => {
            // `s_addr` is stored as BE on all machine and the array is in BE order.
            // So the native endian conversion method is used so that it's never swapped.
            let sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            };

            let sockaddr_in = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: addr.port().to_be(),
                sin_addr,
                #[cfg(not(any(target_os = "haiku", target_os = "vita")))]
                sin_zero: [0; 8],
                #[cfg(target_os = "haiku")]
                sin_zero: [0; 24],
                #[cfg(target_os = "vita")]
                sin_zero: [0; 6],
                #[cfg(any(
                    target_os = "aix",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "haiku",
                    target_os = "hurd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "tvos",
                    target_os = "visionos",
                    target_os = "watchos",
                    target_os = "espidf",
                    target_os = "vita",
                    target_os = "hermit",
                    target_os = "nto",
                ))]
                sin_len: 0,
                #[cfg(target_os = "vita")]
                sin_vport: addr.port().to_be(),
            };

            let sockaddr = SocketAddrCRepr { v4: sockaddr_in };
            let socklen = size_of::<libc::sockaddr_in>() as libc::socklen_t;
            (sockaddr, socklen)
        }
        SocketAddr::V6(ref addr) => {
            let sockaddr_in6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: addr.port().to_be(),
                sin6_addr: libc::in6_addr {
                    s6_addr: addr.ip().octets(),
                },
                sin6_flowinfo: addr.flowinfo(),
                sin6_scope_id: addr.scope_id(),
                #[cfg(any(
                    target_os = "aix",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "haiku",
                    target_os = "hurd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "tvos",
                    target_os = "visionos",
                    target_os = "watchos",
                    target_os = "espidf",
                    target_os = "vita",
                    target_os = "nto",
                    target_os = "hermit",
                ))]
                sin6_len: 0,
                #[cfg(target_os = "vita")]
                sin6_vport: addr.port().to_be(),
                #[cfg(any(target_os = "illumos", target_os = "solaris"))]
                __sin6_src_id: 0,
            };

            let sockaddr = SocketAddrCRepr { v6: sockaddr_in6 };
            let socklen = size_of::<libc::sockaddr_in6>() as libc::socklen_t;
            (sockaddr, socklen)
        }
    }
}

/// Converts a `libc::sockaddr` compatible struct into a native Rust `SocketAddr`.
///
/// # Safety
///
/// `storage` must have the `ss_family` field correctly initialized.
/// `storage` must be initialised to a `sockaddr_in` or `sockaddr_in6`.
pub(crate) unsafe fn to_socket_addr(
    storage: *const libc::sockaddr_storage,
) -> io::Result<SocketAddr> {
    match (*storage).ss_family as libc::c_int {
        libc::AF_INET => {
            // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
            let addr: &libc::sockaddr_in = &*(storage as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
            let port = u16::from_be(addr.sin_port);
            Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        }
        libc::AF_INET6 => {
            // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
            let addr: &libc::sockaddr_in6 = &*(storage as *const libc::sockaddr_in6);
            let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
            let port = u16::from_be(addr.sin6_port);
            Ok(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )))
        }
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}
