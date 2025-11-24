#![cfg(not(target_os = "wasi"))]
use std::io;
use std::net::{self, SocketAddr};

pub fn bind(_: SocketAddr) -> io::Result<net::UdpSocket> {
    os_required!()
}

pub(crate) fn only_v6(_: &net::UdpSocket) -> io::Result<bool> {
    os_required!()
}
