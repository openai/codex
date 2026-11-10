#![cfg(test)]

//! Verify rejected TUN requests cannot open an upstream connection.

use super::*;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::thread;

#[test]
fn invalid_tun_requests_never_open_upstream() {
    let proxy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = proxy.local_addr().unwrap();
    for request in [
        [5, 3, 0, 1, 127, 0, 0, 1, 0, 53],
        [5, 2, 0, 1, 127, 0, 0, 1, 0, 80],
        [5, 1, 1, 1, 127, 0, 0, 1, 0, 80],
        [5, 1, 0, 3, 3, b'd', b'b', b'x', 0, 80],
        [5, 1, 0, 1, 198, 18, 0, 99, 1, 187],
    ] {
        assert!(connect(&mut request.as_slice(), addr, &Mutex::new(Names::default())).is_err());
    }
    proxy.set_nonblocking(true).unwrap();
    assert_eq!(
        proxy.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn denied_socks_connect_never_forwards_pipelined_payload() {
    let proxy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    rustix::net::sockopt::set_socket_timeout(
        &proxy,
        rustix::net::sockopt::Timeout::Recv,
        Some(TIMEOUT),
    )
    .unwrap();
    let proxy_addr = proxy.local_addr().unwrap();
    let incoming = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut client = TcpStream::connect(incoming.local_addr().unwrap()).unwrap();
    let (server, _) = incoming.accept().unwrap();
    let task = thread::spawn(move || relay(server, proxy_addr, &Mutex::new(Names::default())));

    // A malicious client can send application data before either handshake completes.
    let connect = [5, 1, 0, 1, 192, 0, 2, 7, 0x15, 0x38];
    client.write_all(&[5, 2, 2, 0]).unwrap();
    client.write_all(&connect).unwrap();
    client.write_all(b"private marker").unwrap();
    client.shutdown(std::net::Shutdown::Write).unwrap();

    let (mut upstream, _) = proxy.accept().unwrap();
    upstream.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut greeting = [0; 3];
    upstream.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [5, 1, 0]);
    upstream.write_all(&[5, 0]).unwrap();
    let mut request = [0; 10];
    upstream.read_exact(&mut request).unwrap();
    assert_eq!(request, connect);
    upstream.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();

    let mut response = [0; 12]; // greeting and refusal sent to the client
    client.read_exact(&mut response).unwrap();
    assert_eq!(&response[..4], &[5, 0, 5, 2]);
    let mut byte = [0];
    assert_eq!(upstream.read(&mut byte).unwrap(), 0);
    assert!(task.join().unwrap().is_err());
}
