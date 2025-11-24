#[cfg(not(any(target_os = "openbsd", target_os = "netbsd", solarish)))]
use std::net::{SocketAddr, SocketAddrV6};
use std::{
    io::IoSliceMut,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4, UdpSocket},
    slice,
};

use quinn_udp::{EcnCodepoint, RecvMeta, Transmit, UdpSocketState};
use socket2::Socket;

#[test]
fn basic() {
    let send = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    let recv = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    let dst_addr = recv.local_addr().unwrap();
    test_send_recv(
        &send.into(),
        &recv.into(),
        Transmit {
            destination: dst_addr,
            ecn: None,
            contents: b"hello",
            segment_size: None,
            src_ip: None,
        },
    );
}

#[test]
fn basic_src_ip() {
    let send = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    let recv = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    let src_ip = send.local_addr().unwrap().ip();
    let dst_addr = recv.local_addr().unwrap();
    test_send_recv(
        &send.into(),
        &recv.into(),
        Transmit {
            destination: dst_addr,
            ecn: None,
            contents: b"hello",
            segment_size: None,
            src_ip: Some(src_ip),
        },
    );
}

#[test]
fn ecn_v6() {
    let send = Socket::from(UdpSocket::bind((Ipv6Addr::LOCALHOST, 0)).unwrap());
    let recv = Socket::from(UdpSocket::bind((Ipv6Addr::LOCALHOST, 0)).unwrap());
    for codepoint in [EcnCodepoint::Ect0, EcnCodepoint::Ect1] {
        test_send_recv(
            &send,
            &recv,
            Transmit {
                destination: recv.local_addr().unwrap().as_socket().unwrap(),
                ecn: Some(codepoint),
                contents: b"hello",
                segment_size: None,
                src_ip: None,
            },
        );
    }
}

#[test]
#[cfg(not(any(target_os = "openbsd", target_os = "netbsd", solarish)))]
fn ecn_v4() {
    let send = Socket::from(UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap());
    let recv = Socket::from(UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap());
    for codepoint in [EcnCodepoint::Ect0, EcnCodepoint::Ect1] {
        test_send_recv(
            &send,
            &recv,
            Transmit {
                destination: recv.local_addr().unwrap().as_socket().unwrap(),
                ecn: Some(codepoint),
                contents: b"hello",
                segment_size: None,
                src_ip: None,
            },
        );
    }
}

#[test]
#[cfg(not(any(target_os = "openbsd", target_os = "netbsd", solarish)))]
fn ecn_v6_dualstack() {
    let recv = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .unwrap();
    recv.set_only_v6(false).unwrap();
    // We must use the unspecified address here, rather than a local address, to support dual-stack
    // mode
    recv.bind(&socket2::SockAddr::from(
        "[::]:0".parse::<SocketAddr>().unwrap(),
    ))
    .unwrap();
    let recv_v6 = SocketAddr::V6(SocketAddrV6::new(
        Ipv6Addr::LOCALHOST,
        recv.local_addr().unwrap().as_socket().unwrap().port(),
        0,
        0,
    ));
    let recv_v4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, recv_v6.port()));
    for (src, dst) in [
        (SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0), recv_v6),
        (SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0), recv_v4),
    ] {
        dbg!(src, dst);
        let send = UdpSocket::bind(src).unwrap();
        let send = Socket::from(send);
        for codepoint in [EcnCodepoint::Ect0, EcnCodepoint::Ect1] {
            test_send_recv(
                &send,
                &recv,
                Transmit {
                    destination: dst,
                    ecn: Some(codepoint),
                    contents: b"hello",
                    segment_size: None,
                    src_ip: None,
                },
            );
        }
    }
}

#[test]
#[cfg(not(any(target_os = "openbsd", target_os = "netbsd", solarish)))]
fn ecn_v4_mapped_v6() {
    let send = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .unwrap();
    send.set_only_v6(false).unwrap();
    send.bind(&socket2::SockAddr::from(
        "[::]:0".parse::<SocketAddr>().unwrap(),
    ))
    .unwrap();

    let recv = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let recv = Socket::from(recv);
    let recv_v4_mapped_v6 = SocketAddr::V6(SocketAddrV6::new(
        Ipv4Addr::LOCALHOST.to_ipv6_mapped(),
        recv.local_addr().unwrap().as_socket().unwrap().port(),
        0,
        0,
    ));

    for codepoint in [EcnCodepoint::Ect0, EcnCodepoint::Ect1] {
        test_send_recv(
            &send,
            &recv,
            Transmit {
                destination: recv_v4_mapped_v6,
                ecn: Some(codepoint),
                contents: b"hello",
                segment_size: None,
                src_ip: None,
            },
        );
    }
}

#[test]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "windows", target_os = "android")),
    ignore
)]
fn gso() {
    let send = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    let recv = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0))
        .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
        .unwrap();
    let max_segments = UdpSocketState::new((&send).into())
        .unwrap()
        .max_gso_segments();
    let dst_addr = recv.local_addr().unwrap();
    const SEGMENT_SIZE: usize = 128;
    let msg = vec![0xAB; SEGMENT_SIZE * max_segments];
    test_send_recv(
        &send.into(),
        &recv.into(),
        Transmit {
            destination: dst_addr,
            ecn: None,
            contents: &msg,
            segment_size: Some(SEGMENT_SIZE),
            src_ip: None,
        },
    );
}

#[test]
fn socket_buffers() {
    const BUFFER_SIZE: usize = 123456;
    const FACTOR: usize = if cfg!(any(target_os = "linux", target_os = "android")) {
        2 // Linux and Android set the buffer to double the requested size
    } else {
        1 // Everyone else is sane.
    };

    let send = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .unwrap();
    let recv = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .unwrap();
    for sock in [&send, &recv] {
        sock.bind(&socket2::SockAddr::from(SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            0,
        )))
        .unwrap();

        let socket_state = UdpSocketState::new(sock.into()).expect("created socket state");

        // Change the send buffer size.
        let buffer_before = socket_state.send_buffer_size(sock.into()).unwrap();
        assert_ne!(
            buffer_before,
            BUFFER_SIZE * FACTOR,
            "make sure buffer is not already desired size"
        );
        socket_state
            .set_send_buffer_size(sock.into(), BUFFER_SIZE)
            .expect("set send buffer size {buffer_before} -> {BUFFER_SIZE}");
        let buffer_after = socket_state.send_buffer_size(sock.into()).unwrap();
        assert_eq!(
            buffer_after,
            BUFFER_SIZE * FACTOR,
            "setting send buffer size to {BUFFER_SIZE} resulted in {buffer_before} -> {buffer_after}",
        );

        // Change the receive buffer size.
        let buffer_before = socket_state.recv_buffer_size(sock.into()).unwrap();
        socket_state
            .set_recv_buffer_size(sock.into(), BUFFER_SIZE)
            .expect("set recv buffer size {buffer_before} -> {BUFFER_SIZE}");
        let buffer_after = socket_state.recv_buffer_size(sock.into()).unwrap();
        assert_eq!(
            buffer_after,
            BUFFER_SIZE * FACTOR,
            "setting recv buffer size to {BUFFER_SIZE} resulted in {buffer_before} -> {buffer_after}",
        );
    }

    test_send_recv(
        &send,
        &recv,
        Transmit {
            destination: recv.local_addr().unwrap().as_socket().unwrap(),
            ecn: None,
            contents: b"hello",
            segment_size: None,
            src_ip: None,
        },
    );
}

fn test_send_recv(send: &Socket, recv: &Socket, transmit: Transmit) {
    let send_state = UdpSocketState::new(send.into()).unwrap();
    let recv_state = UdpSocketState::new(recv.into()).unwrap();

    // Reverse non-blocking flag set by `UdpSocketState` to make the test non-racy
    recv.set_nonblocking(false).unwrap();

    send_state.try_send(send.into(), &transmit).unwrap();

    let mut buf = [0; u16::MAX as usize];
    let mut meta = RecvMeta::default();
    let segment_size = transmit.segment_size.unwrap_or(transmit.contents.len());
    let expected_datagrams = transmit.contents.len() / segment_size;
    let mut datagrams = 0;
    while datagrams < expected_datagrams {
        let n = recv_state
            .recv(
                recv.into(),
                &mut [IoSliceMut::new(&mut buf)],
                slice::from_mut(&mut meta),
            )
            .unwrap();
        assert_eq!(n, 1);
        let segments = meta.len / meta.stride;
        for i in 0..segments {
            assert_eq!(
                &buf[(i * meta.stride)..((i + 1) * meta.stride)],
                &transmit.contents
                    [(datagrams + i) * segment_size..(datagrams + i + 1) * segment_size]
            );
        }
        datagrams += segments;

        assert_eq!(
            meta.addr.port(),
            send.local_addr().unwrap().as_socket().unwrap().port()
        );
        let send_v6 = send.local_addr().unwrap().as_socket().unwrap().is_ipv6();
        let recv_v6 = recv.local_addr().unwrap().as_socket().unwrap().is_ipv6();
        let mut addresses = vec![meta.addr.ip()];
        // Not populated on every OS. See `RecvMeta::dst_ip` for details.
        if let Some(addr) = meta.dst_ip {
            addresses.push(addr);
        }
        for addr in addresses {
            match (send_v6, recv_v6) {
                (_, false) => assert_eq!(addr, Ipv4Addr::LOCALHOST),
                // Windows gives us real IPv4 addrs, whereas *nix use IPv6-mapped IPv4
                // addrs. Canonicalize to IPv6-mapped for robustness.
                (false, true) => {
                    assert_eq!(ip_to_v6_mapped(addr), Ipv4Addr::LOCALHOST.to_ipv6_mapped())
                }
                (true, true) => assert!(
                    addr == Ipv6Addr::LOCALHOST || addr == Ipv4Addr::LOCALHOST.to_ipv6_mapped()
                ),
            }
        }

        let ipv4_or_ipv4_mapped_ipv6 = match transmit.destination.ip() {
            IpAddr::V4(_) => true,
            IpAddr::V6(a) => a.to_ipv4_mapped().is_some(),
        };

        // On Android API level <= 25 the IPv4 `IP_TOS` control message is
        // not supported and thus ECN bits can not be received.
        if ipv4_or_ipv4_mapped_ipv6
            && cfg!(target_os = "android")
            && std::env::var("API_LEVEL")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .expect("API_LEVEL environment variable to be set on Android")
                <= 25
        {
            assert_eq!(meta.ecn, None);
        } else {
            assert_eq!(meta.ecn, transmit.ecn);
        }
    }
    assert_eq!(datagrams, expected_datagrams);
}

fn ip_to_v6_mapped(x: IpAddr) -> IpAddr {
    match x {
        IpAddr::V4(x) => IpAddr::V6(x.to_ipv6_mapped()),
        IpAddr::V6(_) => x,
    }
}
