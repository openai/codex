#![cfg(test)]

//! A denied managed CONNECT never yields a stream for the TCP stack.

use super::*;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;

#[test]
fn denied_socks_connect_never_returns_an_upstream() {
    let proxy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let proxy_addr = proxy.local_addr().unwrap();
    let native_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let target = native_listener.local_addr().unwrap();
    let native = TcpStream::connect(target).unwrap();
    let source = native.local_addr().unwrap();
    let task =
        thread::spawn(move || connect(source, target, proxy_addr, &Mutex::new(Names::default())));
    rustix::net::sockopt::set_socket_timeout(
        &proxy,
        rustix::net::sockopt::Timeout::Recv,
        Some(TIMEOUT),
    )
    .unwrap();
    let (mut upstream, _) = proxy.accept().unwrap();
    upstream.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut greeting = [0; 3];
    upstream.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [5, 1, 0]);
    upstream.write_all(&[5, 0]).unwrap();
    let request = CommandRequest::read_from(&mut upstream).unwrap();
    assert!(matches!(request.address, Address::SocketAddr(address) if address == target));
    upstream.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(upstream.read(&mut [0]).unwrap(), 0);
    assert!(task.join().unwrap().is_err());
}
