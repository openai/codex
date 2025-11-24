use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::{self, SocketAddr};

use crate::sys::unix::net::new_socket;
use crate::sys::unix::uds::unix_addr;

pub(crate) fn bind_addr(address: &SocketAddr) -> io::Result<net::UnixDatagram> {
    let socket = unbound()?;

    let (unix_address, addrlen) = unix_addr(address);
    let sockaddr = &unix_address as *const libc::sockaddr_un as *const libc::sockaddr;
    syscall!(bind(socket.as_raw_fd(), sockaddr, addrlen))?;

    Ok(socket)
}

pub(crate) fn unbound() -> io::Result<net::UnixDatagram> {
    let fd = new_socket(libc::AF_UNIX, libc::SOCK_DGRAM)?;
    Ok(unsafe { net::UnixDatagram::from_raw_fd(fd) })
}

pub(crate) fn pair() -> io::Result<(net::UnixDatagram, net::UnixDatagram)> {
    super::pair(libc::SOCK_DGRAM)
}
