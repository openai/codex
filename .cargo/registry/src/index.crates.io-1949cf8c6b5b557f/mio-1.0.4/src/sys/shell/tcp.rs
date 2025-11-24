use std::io;
use std::net::{self, SocketAddr};

#[cfg(not(target_os = "wasi"))]
pub(crate) fn new_for_addr(_: SocketAddr) -> io::Result<i32> {
    os_required!();
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn bind(_: &net::TcpListener, _: SocketAddr) -> io::Result<()> {
    os_required!();
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn connect(_: &net::TcpStream, _: SocketAddr) -> io::Result<()> {
    os_required!();
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn listen(_: &net::TcpListener, _: u32) -> io::Result<()> {
    os_required!();
}

#[cfg(any(unix, target_os = "hermit"))]
pub(crate) fn set_reuseaddr(_: &net::TcpListener, _: bool) -> io::Result<()> {
    os_required!();
}

pub(crate) fn accept(_: &net::TcpListener) -> io::Result<(net::TcpStream, SocketAddr)> {
    os_required!();
}
