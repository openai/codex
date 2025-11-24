use std::io;
use std::os::fd::FromRawFd;
use std::os::unix::net::{self, SocketAddr};

use crate::sys::unix::net::new_socket;
use crate::sys::unix::uds::unix_addr;

pub(crate) fn connect_addr(address: &SocketAddr) -> io::Result<net::UnixStream> {
    let fd = new_socket(libc::AF_UNIX, libc::SOCK_STREAM)?;
    let socket = unsafe { net::UnixStream::from_raw_fd(fd) };

    let (unix_address, addrlen) = unix_addr(address);
    let sockaddr = &unix_address as *const libc::sockaddr_un as *const libc::sockaddr;
    match syscall!(connect(fd, sockaddr, addrlen)) {
        Ok(_) => {}
        Err(ref err) if err.raw_os_error() == Some(libc::EINPROGRESS) => {}
        Err(e) => return Err(e),
    }

    Ok(socket)
}

pub(crate) fn pair() -> io::Result<(net::UnixStream, net::UnixStream)> {
    super::pair(libc::SOCK_STREAM)
}
