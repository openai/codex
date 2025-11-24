use std::io;
use std::mem::{self, MaybeUninit};
use std::net::{self, SocketAddr};
use std::os::windows::io::{AsRawSocket, FromRawSocket};
use std::os::windows::raw::SOCKET as StdSocket; // windows-sys uses usize, stdlib uses u32/u64.

use crate::sys::windows::net::{new_ip_socket, socket_addr};
use windows_sys::Win32::Networking::WinSock::{
    bind as win_bind, getsockopt, IPPROTO_IPV6, IPV6_V6ONLY, SOCKET_ERROR, SOCK_DGRAM,
};

pub fn bind(addr: SocketAddr) -> io::Result<net::UdpSocket> {
    let raw_socket = new_ip_socket(addr, SOCK_DGRAM)?;
    let socket = unsafe { net::UdpSocket::from_raw_socket(raw_socket as StdSocket) };

    let (raw_addr, raw_addr_length) = socket_addr(&addr);
    syscall!(
        win_bind(raw_socket, raw_addr.as_ptr(), raw_addr_length),
        PartialEq::eq,
        SOCKET_ERROR
    )?;

    Ok(socket)
}

pub(crate) fn only_v6(socket: &net::UdpSocket) -> io::Result<bool> {
    let mut optval: MaybeUninit<i32> = MaybeUninit::uninit();
    let mut optlen = mem::size_of::<i32>() as i32;

    syscall!(
        getsockopt(
            socket.as_raw_socket() as usize,
            IPPROTO_IPV6 as i32,
            IPV6_V6ONLY as i32,
            optval.as_mut_ptr().cast(),
            &mut optlen,
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )?;

    debug_assert_eq!(optlen as usize, mem::size_of::<i32>());
    // Safety: `getsockopt` initialised `optval` for us.
    let optval = unsafe { optval.assume_init() };
    Ok(optval != 0)
}
