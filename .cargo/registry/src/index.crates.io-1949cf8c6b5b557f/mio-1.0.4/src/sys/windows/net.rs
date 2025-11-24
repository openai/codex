use std::io;
use std::mem;
use std::net::SocketAddr;
use std::sync::Once;

use windows_sys::Win32::Networking::WinSock::{
    closesocket, ioctlsocket, socket, AF_INET, AF_INET6, FIONBIO, IN6_ADDR, IN6_ADDR_0,
    INVALID_SOCKET, IN_ADDR, IN_ADDR_0, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_IN6_0,
    SOCKET,
};

/// Initialise the network stack for Windows.
fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Let standard library call `WSAStartup` for us, we can't do it
        // ourselves because otherwise using any type in `std::net` would panic
        // when it tries to call `WSAStartup` a second time.
        drop(std::net::UdpSocket::bind("127.0.0.1:0"));
    });
}

/// Create a new non-blocking socket.
pub(crate) fn new_ip_socket(addr: SocketAddr, socket_type: i32) -> io::Result<SOCKET> {
    let domain = match addr {
        SocketAddr::V4(..) => AF_INET,
        SocketAddr::V6(..) => AF_INET6,
    };

    new_socket(domain.into(), socket_type)
}

pub(crate) fn new_socket(domain: u32, socket_type: i32) -> io::Result<SOCKET> {
    init();

    let socket = syscall!(
        socket(domain as i32, socket_type, 0),
        PartialEq::eq,
        INVALID_SOCKET
    )?;

    if let Err(err) = syscall!(ioctlsocket(socket, FIONBIO, &mut 1), PartialEq::ne, 0) {
        let _ = unsafe { closesocket(socket) };
        return Err(err);
    }

    Ok(socket as SOCKET)
}

/// A type with the same memory layout as `SOCKADDR`. Used in converting Rust level
/// SocketAddr* types into their system representation. The benefit of this specific
/// type over using `SOCKADDR_STORAGE` is that this type is exactly as large as it
/// needs to be and not a lot larger. And it can be initialized cleaner from Rust.
#[repr(C)]
pub(crate) union SocketAddrCRepr {
    v4: SOCKADDR_IN,
    v6: SOCKADDR_IN6,
}

impl SocketAddrCRepr {
    pub(crate) fn as_ptr(&self) -> *const SOCKADDR {
        self as *const _ as *const SOCKADDR
    }
}

pub(crate) fn socket_addr(addr: &SocketAddr) -> (SocketAddrCRepr, i32) {
    match addr {
        SocketAddr::V4(ref addr) => {
            // `s_addr` is stored as BE on all machine and the array is in BE order.
            // So the native endian conversion method is used so that it's never swapped.
            let sin_addr = unsafe {
                let mut s_un = mem::zeroed::<IN_ADDR_0>();
                s_un.S_addr = u32::from_ne_bytes(addr.ip().octets());
                IN_ADDR { S_un: s_un }
            };

            let sockaddr_in = SOCKADDR_IN {
                sin_family: AF_INET as u16, // 1
                sin_port: addr.port().to_be(),
                sin_addr,
                sin_zero: [0; 8],
            };

            let sockaddr = SocketAddrCRepr { v4: sockaddr_in };
            (sockaddr, mem::size_of::<SOCKADDR_IN>() as i32)
        }
        SocketAddr::V6(ref addr) => {
            let sin6_addr = unsafe {
                let mut u = mem::zeroed::<IN6_ADDR_0>();
                u.Byte = addr.ip().octets();
                IN6_ADDR { u }
            };
            let u = unsafe {
                let mut u = mem::zeroed::<SOCKADDR_IN6_0>();
                u.sin6_scope_id = addr.scope_id();
                u
            };

            let sockaddr_in6 = SOCKADDR_IN6 {
                sin6_family: AF_INET6 as u16, // 23
                sin6_port: addr.port().to_be(),
                sin6_addr,
                sin6_flowinfo: addr.flowinfo(),
                Anonymous: u,
            };

            let sockaddr = SocketAddrCRepr { v6: sockaddr_in6 };
            (sockaddr, mem::size_of::<SOCKADDR_IN6>() as i32)
        }
    }
}
