use std::ffi::OsStr;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{self, SocketAddr};
use std::path::Path;
use std::{io, mem};

use crate::net::UnixStream;
use crate::sys::unix::net::new_socket;
use crate::sys::unix::uds::{path_offset, unix_addr};

pub(crate) fn bind_addr(address: &SocketAddr) -> io::Result<net::UnixListener> {
    let fd = new_socket(libc::AF_UNIX, libc::SOCK_STREAM)?;
    let socket = unsafe { net::UnixListener::from_raw_fd(fd) };

    let (unix_address, addrlen) = unix_addr(address);
    let sockaddr = &unix_address as *const libc::sockaddr_un as *const libc::sockaddr;
    syscall!(bind(fd, sockaddr, addrlen))?;
    syscall!(listen(fd, 1024))?;

    Ok(socket)
}

pub(crate) fn accept(listener: &net::UnixListener) -> io::Result<(UnixStream, SocketAddr)> {
    // SAFETY: `libc::sockaddr_un` zero filled is properly initialized.
    //
    // `0` is a valid value for `sockaddr_un::sun_family`; it is
    // `libc::AF_UNSPEC`.
    //
    // `[0; 108]` is a valid value for `sockaddr_un::sun_path`; it begins an
    // abstract path.
    let mut sockaddr = unsafe { mem::zeroed::<libc::sockaddr_un>() };

    let mut socklen = mem::size_of_val(&sockaddr) as libc::socklen_t;

    #[cfg(not(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "redox",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "vita",
        target_os = "nto",
        // Android x86's seccomp profile forbids calls to `accept4(2)`
        // See https://github.com/tokio-rs/mio/issues/1445 for details
        all(target_arch = "x86", target_os = "android"),
    )))]
    let socket = {
        let flags = libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
        syscall!(accept4(
            listener.as_raw_fd(),
            &mut sockaddr as *mut libc::sockaddr_un as *mut libc::sockaddr,
            &mut socklen,
            flags
        ))
        .map(|socket| unsafe { net::UnixStream::from_raw_fd(socket) })
    };

    #[cfg(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "redox",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "vita",
        target_os = "nto",
        all(target_arch = "x86", target_os = "android")
    ))]
    let socket = syscall!(accept(
        listener.as_raw_fd(),
        &mut sockaddr as *mut libc::sockaddr_un as *mut libc::sockaddr,
        &mut socklen,
    ))
    .and_then(|socket| {
        // Ensure the socket is closed if either of the `fcntl` calls
        // error below.
        let s = unsafe { net::UnixStream::from_raw_fd(socket) };
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        syscall!(fcntl(socket, libc::F_SETFD, libc::FD_CLOEXEC))?;

        // See https://github.com/tokio-rs/mio/issues/1450
        #[cfg(any(
            all(target_arch = "x86", target_os = "android"),
            target_os = "espidf",
            target_os = "vita",
            target_os = "nto",
        ))]
        syscall!(fcntl(socket, libc::F_SETFL, libc::O_NONBLOCK))?;

        Ok(s)
    });

    let socket = socket.map(UnixStream::from_std)?;

    #[allow(unused_mut)] // See below.
    let mut path_len = socklen as usize - path_offset(&sockaddr);
    // On FreeBSD and Darwin, it returns a length of 14/16, but an unnamed (all
    // zero) address. Map that to a length of 0 to match other OS.
    if sockaddr.sun_path[0] == 0 {
        path_len = 0;
    }
    // SAFETY: going from i8 to u8 is fine in this context.
    let mut path =
        unsafe { &*(&sockaddr.sun_path[..path_len] as *const [libc::c_char] as *const [u8]) };
    // Remove last null as `SocketAddr::from_pathname` doesn't accept it.
    if let Some(0) = path.last() {
        path = &path[..path.len() - 1];
    }
    let address = SocketAddr::from_pathname(Path::new(OsStr::from_bytes(path)))?;
    Ok((socket, address))
}
