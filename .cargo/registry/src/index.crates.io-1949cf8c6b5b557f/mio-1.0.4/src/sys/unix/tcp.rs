use std::convert::TryInto;
use std::io;
use std::mem::{size_of, MaybeUninit};
use std::net::{self, SocketAddr};
#[cfg(not(target_os = "hermit"))]
use std::os::fd::{AsRawFd, FromRawFd};
// TODO: once <https://github.com/rust-lang/rust/issues/126198> is fixed this
// can use `std::os::fd` and be merged with the above.
#[cfg(target_os = "hermit")]
use std::os::hermit::io::{AsRawFd, FromRawFd};

use crate::sys::unix::net::{new_socket, socket_addr, to_socket_addr};

pub(crate) fn new_for_addr(address: SocketAddr) -> io::Result<libc::c_int> {
    let domain = match address {
        SocketAddr::V4(_) => libc::AF_INET,
        SocketAddr::V6(_) => libc::AF_INET6,
    };
    new_socket(domain, libc::SOCK_STREAM)
}

pub(crate) fn bind(socket: &net::TcpListener, addr: SocketAddr) -> io::Result<()> {
    let (raw_addr, raw_addr_length) = socket_addr(&addr);
    syscall!(bind(socket.as_raw_fd(), raw_addr.as_ptr(), raw_addr_length))?;
    Ok(())
}

pub(crate) fn connect(socket: &net::TcpStream, addr: SocketAddr) -> io::Result<()> {
    let (raw_addr, raw_addr_length) = socket_addr(&addr);

    match syscall!(connect(
        socket.as_raw_fd(),
        raw_addr.as_ptr(),
        raw_addr_length
    )) {
        Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => Err(err),
        _ => Ok(()),
    }
}

pub(crate) fn listen(socket: &net::TcpListener, backlog: u32) -> io::Result<()> {
    let backlog = backlog.try_into().unwrap_or(i32::MAX);
    syscall!(listen(socket.as_raw_fd(), backlog))?;
    Ok(())
}

pub(crate) fn set_reuseaddr(socket: &net::TcpListener, reuseaddr: bool) -> io::Result<()> {
    let val: libc::c_int = i32::from(reuseaddr);
    syscall!(setsockopt(
        socket.as_raw_fd(),
        libc::SOL_SOCKET,
        libc::SO_REUSEADDR,
        &val as *const libc::c_int as *const libc::c_void,
        size_of::<libc::c_int>() as libc::socklen_t,
    ))?;
    Ok(())
}

pub(crate) fn accept(listener: &net::TcpListener) -> io::Result<(net::TcpStream, SocketAddr)> {
    let mut addr: MaybeUninit<libc::sockaddr_storage> = MaybeUninit::uninit();
    let mut length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;

    // On platforms that support it we can use `accept4(2)` to set `NONBLOCK`
    // and `CLOEXEC` in the call to accept the connection.
    #[cfg(any(
        // Android x86's seccomp profile forbids calls to `accept4(2)`
        // See https://github.com/tokio-rs/mio/issues/1445 for details
        all(not(target_arch="x86"), target_os = "android"),
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    let stream = {
        syscall!(accept4(
            listener.as_raw_fd(),
            addr.as_mut_ptr() as *mut _,
            &mut length,
            libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
        ))
        .map(|socket| unsafe { net::TcpStream::from_raw_fd(socket) })
    }?;

    // But not all platforms have the `accept4(2)` call. Luckily BSD (derived)
    // OSs inherit the non-blocking flag from the listener, so we just have to
    // set `CLOEXEC`.
    #[cfg(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "ios",
        target_os = "macos",
        target_os = "redox",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "vita",
        target_os = "hermit",
        target_os = "nto",
        all(target_arch = "x86", target_os = "android"),
    ))]
    let stream = {
        syscall!(accept(
            listener.as_raw_fd(),
            addr.as_mut_ptr() as *mut _,
            &mut length
        ))
        .map(|socket| unsafe { net::TcpStream::from_raw_fd(socket) })
        .and_then(|s| {
            #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
            syscall!(fcntl(s.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC))?;

            // See https://github.com/tokio-rs/mio/issues/1450
            #[cfg(any(
                all(target_arch = "x86", target_os = "android"),
                target_os = "aix",
                target_os = "espidf",
                target_os = "vita",
                target_os = "hermit",
                target_os = "nto",
            ))]
            syscall!(fcntl(s.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK))?;

            Ok(s)
        })
    }?;

    // This is safe because `accept` calls above ensures the address
    // initialised.
    unsafe { to_socket_addr(addr.as_ptr()) }.map(|addr| (stream, addr))
}
