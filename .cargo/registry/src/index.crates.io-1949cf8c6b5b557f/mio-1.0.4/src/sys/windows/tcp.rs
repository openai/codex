use std::io;
use std::net::{self, SocketAddr};
use std::os::windows::io::AsRawSocket;

use windows_sys::Win32::Networking::WinSock::{self, SOCKET, SOCKET_ERROR, SOCK_STREAM};

use crate::sys::windows::net::{new_ip_socket, socket_addr};

pub(crate) fn new_for_addr(address: SocketAddr) -> io::Result<SOCKET> {
    new_ip_socket(address, SOCK_STREAM)
}

pub(crate) fn bind(socket: &net::TcpListener, addr: SocketAddr) -> io::Result<()> {
    use WinSock::bind;

    let (raw_addr, raw_addr_length) = socket_addr(&addr);
    syscall!(
        bind(
            socket.as_raw_socket() as _,
            raw_addr.as_ptr(),
            raw_addr_length
        ),
        PartialEq::eq,
        SOCKET_ERROR
    )?;
    Ok(())
}

pub(crate) fn connect(socket: &net::TcpStream, addr: SocketAddr) -> io::Result<()> {
    use WinSock::connect;

    let (raw_addr, raw_addr_length) = socket_addr(&addr);
    let res = syscall!(
        connect(
            socket.as_raw_socket() as _,
            raw_addr.as_ptr(),
            raw_addr_length
        ),
        PartialEq::eq,
        SOCKET_ERROR
    );

    match res {
        Err(err) if err.kind() != io::ErrorKind::WouldBlock => Err(err),
        _ => Ok(()),
    }
}

pub(crate) fn listen(socket: &net::TcpListener, backlog: u32) -> io::Result<()> {
    use std::convert::TryInto;
    use WinSock::listen;

    let backlog = backlog.try_into().unwrap_or(i32::MAX);
    syscall!(
        listen(socket.as_raw_socket() as _, backlog),
        PartialEq::eq,
        SOCKET_ERROR
    )?;
    Ok(())
}

pub(crate) fn accept(listener: &net::TcpListener) -> io::Result<(net::TcpStream, SocketAddr)> {
    // The non-blocking state of `listener` is inherited. See
    // https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept#remarks.
    listener.accept()
}
