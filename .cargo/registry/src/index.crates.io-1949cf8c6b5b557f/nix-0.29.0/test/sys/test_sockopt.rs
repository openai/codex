#[cfg(linux_android)]
use crate::*;
use nix::sys::socket::{
    getsockopt, setsockopt, socket, sockopt, AddressFamily, SockFlag,
    SockProtocol, SockType,
};
use rand::{thread_rng, Rng};
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

// NB: FreeBSD supports LOCAL_PEERCRED for SOCK_SEQPACKET, but OSX does not.
#[cfg(freebsdlike)]
#[test]
pub fn test_local_peercred_seqpacket() {
    use nix::{
        sys::socket::socketpair,
        unistd::{Gid, Uid},
    };

    let (fd1, _fd2) = socketpair(
        AddressFamily::Unix,
        SockType::SeqPacket,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    let xucred = getsockopt(&fd1, sockopt::LocalPeerCred).unwrap();
    assert_eq!(xucred.version(), 0);
    assert_eq!(Uid::from_raw(xucred.uid()), Uid::current());
    assert_eq!(Gid::from_raw(xucred.groups()[0]), Gid::current());
}

#[cfg(any(freebsdlike, apple_targets))]
#[test]
pub fn test_local_peercred_stream() {
    use nix::{
        sys::socket::socketpair,
        unistd::{Gid, Uid},
    };

    let (fd1, _fd2) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    let xucred = getsockopt(&fd1, sockopt::LocalPeerCred).unwrap();
    assert_eq!(xucred.version(), 0);
    assert_eq!(Uid::from_raw(xucred.uid()), Uid::current());
    assert_eq!(Gid::from_raw(xucred.groups()[0]), Gid::current());
}

#[cfg(apple_targets)]
#[test]
pub fn test_local_peer_pid() {
    use nix::sys::socket::socketpair;

    let (fd1, _fd2) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    let pid = getsockopt(&fd1, sockopt::LocalPeerPid).unwrap();
    assert_eq!(pid, std::process::id() as _);
}

#[cfg(target_os = "linux")]
#[test]
fn is_so_mark_functional() {
    use nix::sys::socket::sockopt;

    require_capability!("is_so_mark_functional", CAP_NET_ADMIN);

    let s = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&s, sockopt::Mark, &1337).unwrap();
    let mark = getsockopt(&s, sockopt::Mark).unwrap();
    assert_eq!(mark, 1337);
}

#[test]
fn test_so_buf() {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        SockProtocol::Udp,
    )
    .unwrap();
    let bufsize: usize = thread_rng().gen_range(4096..131_072);
    setsockopt(&fd, sockopt::SndBuf, &bufsize).unwrap();
    let actual = getsockopt(&fd, sockopt::SndBuf).unwrap();
    assert!(actual >= bufsize);
    setsockopt(&fd, sockopt::RcvBuf, &bufsize).unwrap();
    let actual = getsockopt(&fd, sockopt::RcvBuf).unwrap();
    assert!(actual >= bufsize);
}

#[cfg(target_os = "freebsd")]
#[test]
fn test_so_listen_q_limit() {
    use nix::sys::socket::{bind, listen, Backlog, SockaddrIn};
    use std::net::SocketAddrV4;
    use std::str::FromStr;

    let std_sa = SocketAddrV4::from_str("127.0.0.1:4004").unwrap();
    let sock_addr = SockaddrIn::from(std_sa);

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    bind(rsock.as_raw_fd(), &sock_addr).unwrap();
    let pre_limit = getsockopt(&rsock, sockopt::ListenQLimit).unwrap();
    assert_eq!(pre_limit, 0);
    listen(&rsock, Backlog::new(42).unwrap()).unwrap();
    let post_limit = getsockopt(&rsock, sockopt::ListenQLimit).unwrap();
    assert_eq!(post_limit, 42);
}

#[test]
fn test_so_tcp_maxseg() {
    use nix::sys::socket::{
        accept, bind, connect, getsockname, listen, Backlog, SockaddrIn,
    };
    use nix::unistd::write;
    use std::net::SocketAddrV4;
    use std::str::FromStr;

    let std_sa = SocketAddrV4::from_str("127.0.0.1:0").unwrap();
    let mut sock_addr = SockaddrIn::from(std_sa);

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    bind(rsock.as_raw_fd(), &sock_addr).unwrap();
    sock_addr = getsockname(rsock.as_raw_fd()).unwrap();
    listen(&rsock, Backlog::new(10).unwrap()).unwrap();
    let initial = getsockopt(&rsock, sockopt::TcpMaxSeg).unwrap();
    // Initial MSS is expected to be 536 (https://tools.ietf.org/html/rfc879#section-1) but some
    // platforms keep it even lower. This might fail if you've tuned your initial MSS to be larger
    // than 700
    cfg_if! {
        if #[cfg(linux_android)] {
            let segsize: u32 = 873;
            assert!(initial < segsize);
            setsockopt(&rsock, sockopt::TcpMaxSeg, &segsize).unwrap();
        } else {
            assert!(initial < 700);
        }
    }

    // Connect and check the MSS that was advertised
    let ssock = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    connect(ssock.as_raw_fd(), &sock_addr).unwrap();
    let rsess = accept(rsock.as_raw_fd()).unwrap();
    let rsess = unsafe { OwnedFd::from_raw_fd(rsess) };
    write(&rsess, b"hello").unwrap();
    let actual = getsockopt(&ssock, sockopt::TcpMaxSeg).unwrap();
    // Actual max segment size takes header lengths into account, max IPv4 options (60 bytes) + max
    // TCP options (40 bytes) are subtracted from the requested maximum as a lower boundary.
    cfg_if! {
        if #[cfg(linux_android)] {
            assert!((segsize - 100) <= actual);
            assert!(actual <= segsize);
        } else {
            assert!(initial < actual);
            assert!(536 < actual);
        }
    }
}

#[test]
fn test_so_type() {
    let sockfd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    assert_eq!(Ok(SockType::Stream), getsockopt(&sockfd, sockopt::SockType));
}

/// getsockopt(_, sockopt::SockType) should gracefully handle unknown socket
/// types.  Regression test for https://github.com/nix-rust/nix/issues/1819
#[cfg(linux_android)]
#[test]
fn test_so_type_unknown() {
    use nix::errno::Errno;

    require_capability!("test_so_type", CAP_NET_RAW);
    let raw_fd = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_PACKET, 0) };
    assert!(raw_fd >= 0, "Error opening socket: {}", nix::Error::last());
    let sockfd = unsafe { OwnedFd::from_raw_fd(raw_fd) };

    assert_eq!(Err(Errno::EINVAL), getsockopt(&sockfd, sockopt::SockType));
}

// The CI doesn't supported getsockopt and setsockopt on emulated processors.
// It's believed that a QEMU issue, the tests run ok on a fully emulated system.
// Current CI just run the binary with QEMU but the Kernel remains the same as the host.
// So the syscall doesn't work properly unless the kernel is also emulated.
#[test]
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    any(target_os = "freebsd", target_os = "linux")
))]
fn test_tcp_congestion() {
    use std::ffi::OsString;

    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    let val = getsockopt(&fd, sockopt::TcpCongestion).unwrap();
    setsockopt(&fd, sockopt::TcpCongestion, &val).unwrap();

    setsockopt(
        &fd,
        sockopt::TcpCongestion,
        &OsString::from("tcp_congestion_does_not_exist"),
    )
    .unwrap_err();

    assert_eq!(getsockopt(&fd, sockopt::TcpCongestion).unwrap(), val);
}

#[test]
#[cfg(linux_android)]
fn test_bindtodevice() {
    skip_if_not_root!("test_bindtodevice");

    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    let val = getsockopt(&fd, sockopt::BindToDevice).unwrap();
    setsockopt(&fd, sockopt::BindToDevice, &val).unwrap();

    assert_eq!(getsockopt(&fd, sockopt::BindToDevice).unwrap(), val);
}

#[test]
fn test_so_tcp_keepalive() {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    setsockopt(&fd, sockopt::KeepAlive, &true).unwrap();
    assert!(getsockopt(&fd, sockopt::KeepAlive).unwrap());

    #[cfg(any(linux_android, freebsdlike))]
    {
        let x = getsockopt(&fd, sockopt::TcpKeepIdle).unwrap();
        setsockopt(&fd, sockopt::TcpKeepIdle, &(x + 1)).unwrap();
        assert_eq!(getsockopt(&fd, sockopt::TcpKeepIdle).unwrap(), x + 1);

        let x = getsockopt(&fd, sockopt::TcpKeepCount).unwrap();
        setsockopt(&fd, sockopt::TcpKeepCount, &(x + 1)).unwrap();
        assert_eq!(getsockopt(&fd, sockopt::TcpKeepCount).unwrap(), x + 1);

        let x = getsockopt(&fd, sockopt::TcpKeepInterval).unwrap();
        setsockopt(&fd, sockopt::TcpKeepInterval, &(x + 1)).unwrap();
        assert_eq!(getsockopt(&fd, sockopt::TcpKeepInterval).unwrap(), x + 1);
    }
}

#[test]
#[cfg(linux_android)]
#[cfg_attr(qemu, ignore)]
fn test_get_mtu() {
    use nix::sys::socket::{bind, connect, SockaddrIn};
    use std::net::SocketAddrV4;
    use std::str::FromStr;

    let std_sa = SocketAddrV4::from_str("127.0.0.1:0").unwrap();
    let std_sb = SocketAddrV4::from_str("127.0.0.1:4002").unwrap();

    let usock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        SockProtocol::Udp,
    )
    .unwrap();

    // Bind and initiate connection
    bind(usock.as_raw_fd(), &SockaddrIn::from(std_sa)).unwrap();
    connect(usock.as_raw_fd(), &SockaddrIn::from(std_sb)).unwrap();

    // Loopback connections have 2^16 - the maximum - MTU
    assert_eq!(getsockopt(&usock, sockopt::IpMtu), Ok(u16::MAX as i32))
}

#[test]
#[cfg(any(linux_android, target_os = "freebsd"))]
fn test_ttl_opts() {
    let fd4 = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd4, sockopt::Ipv4Ttl, &1)
        .expect("setting ipv4ttl on an inet socket should succeed");
    let fd6 = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd6, sockopt::Ipv6Ttl, &1)
        .expect("setting ipv6ttl on an inet6 socket should succeed");
}

#[test]
#[cfg(any(linux_android, target_os = "freebsd"))]
fn test_multicast_ttl_opts_ipv4() {
    let fd4 = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd4, sockopt::IpMulticastTtl, &2)
        .expect("setting ipmulticastttl on an inet socket should succeed");
}

#[test]
#[cfg(linux_android)]
fn test_multicast_ttl_opts_ipv6() {
    let fd6 = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd6, sockopt::IpMulticastTtl, &2)
        .expect("setting ipmulticastttl on an inet6 socket should succeed");
}

#[test]
fn test_ipv6_multicast_hops() {
    let fd6 = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd6, sockopt::Ipv6MulticastHops, &7)
        .expect("setting ipv6multicasthops on an inet6 socket should succeed");
}

#[test]
#[cfg(apple_targets)]
fn test_dontfrag_opts() {
    let fd4 = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    setsockopt(&fd4, sockopt::IpDontFrag, &true)
        .expect("setting IP_DONTFRAG on an inet stream socket should succeed");
    setsockopt(&fd4, sockopt::IpDontFrag, &false).expect(
        "unsetting IP_DONTFRAG on an inet stream socket should succeed",
    );
    let fd4d = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd4d, sockopt::IpDontFrag, &true).expect(
        "setting IP_DONTFRAG on an inet datagram socket should succeed",
    );
    setsockopt(&fd4d, sockopt::IpDontFrag, &false).expect(
        "unsetting IP_DONTFRAG on an inet datagram socket should succeed",
    );
}

#[test]
#[cfg(any(linux_android, apple_targets))]
// Disable the test under emulation because it fails in Cirrus-CI.  Lack
// of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
fn test_v6dontfrag_opts() {
    let fd6 = socket(
        AddressFamily::Inet6,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    setsockopt(&fd6, sockopt::Ipv6DontFrag, &true).expect(
        "setting IPV6_DONTFRAG on an inet6 stream socket should succeed",
    );
    setsockopt(&fd6, sockopt::Ipv6DontFrag, &false).expect(
        "unsetting IPV6_DONTFRAG on an inet6 stream socket should succeed",
    );
    let fd6d = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd6d, sockopt::Ipv6DontFrag, &true).expect(
        "setting IPV6_DONTFRAG on an inet6 datagram socket should succeed",
    );
    setsockopt(&fd6d, sockopt::Ipv6DontFrag, &false).expect(
        "unsetting IPV6_DONTFRAG on an inet6 datagram socket should succeed",
    );
}

#[test]
#[cfg(target_os = "linux")]
fn test_so_priority() {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    let priority = 3;
    setsockopt(&fd, sockopt::Priority, &priority).unwrap();
    assert_eq!(getsockopt(&fd, sockopt::Priority).unwrap(), priority);
}

#[test]
#[cfg(target_os = "linux")]
fn test_ip_tos() {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    let tos = 0x80; // CS4
    setsockopt(&fd, sockopt::IpTos, &tos).unwrap();
    assert_eq!(getsockopt(&fd, sockopt::IpTos).unwrap(), tos);
}

#[test]
#[cfg(target_os = "linux")]
// Disable the test under emulation because it fails in Cirrus-CI.  Lack
// of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
fn test_ipv6_tclass() {
    let fd = socket(
        AddressFamily::Inet6,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    let class = 0x80; // CS4
    setsockopt(&fd, sockopt::Ipv6TClass, &class).unwrap();
    assert_eq!(getsockopt(&fd, sockopt::Ipv6TClass).unwrap(), class);
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_receive_timestamp() {
    let fd = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd, sockopt::ReceiveTimestamp, &true).unwrap();
    assert!(getsockopt(&fd, sockopt::ReceiveTimestamp).unwrap());
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_ts_clock_realtime_micro() {
    use nix::sys::socket::SocketTimestamp;

    let fd = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    // FreeBSD setsockopt docs say to set SO_TS_CLOCK after setting SO_TIMESTAMP.
    setsockopt(&fd, sockopt::ReceiveTimestamp, &true).unwrap();

    setsockopt(
        &fd,
        sockopt::TsClock,
        &SocketTimestamp::SO_TS_REALTIME_MICRO,
    )
    .unwrap();
    assert_eq!(
        getsockopt(&fd, sockopt::TsClock).unwrap(),
        SocketTimestamp::SO_TS_REALTIME_MICRO
    );
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_ts_clock_bintime() {
    use nix::sys::socket::SocketTimestamp;

    let fd = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    // FreeBSD setsockopt docs say to set SO_TS_CLOCK after setting SO_TIMESTAMP.
    setsockopt(&fd, sockopt::ReceiveTimestamp, &true).unwrap();

    setsockopt(&fd, sockopt::TsClock, &SocketTimestamp::SO_TS_BINTIME).unwrap();
    assert_eq!(
        getsockopt(&fd, sockopt::TsClock).unwrap(),
        SocketTimestamp::SO_TS_BINTIME
    );
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_ts_clock_realtime() {
    use nix::sys::socket::SocketTimestamp;

    let fd = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    // FreeBSD setsockopt docs say to set SO_TS_CLOCK after setting SO_TIMESTAMP.
    setsockopt(&fd, sockopt::ReceiveTimestamp, &true).unwrap();

    setsockopt(&fd, sockopt::TsClock, &SocketTimestamp::SO_TS_REALTIME)
        .unwrap();
    assert_eq!(
        getsockopt(&fd, sockopt::TsClock).unwrap(),
        SocketTimestamp::SO_TS_REALTIME
    );
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_ts_clock_monotonic() {
    use nix::sys::socket::SocketTimestamp;

    let fd = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    // FreeBSD setsockopt docs say to set SO_TS_CLOCK after setting SO_TIMESTAMP.
    setsockopt(&fd, sockopt::ReceiveTimestamp, &true).unwrap();

    setsockopt(&fd, sockopt::TsClock, &SocketTimestamp::SO_TS_MONOTONIC)
        .unwrap();
    assert_eq!(
        getsockopt(&fd, sockopt::TsClock).unwrap(),
        SocketTimestamp::SO_TS_MONOTONIC
    );
}

#[test]
#[cfg(linux_android)]
// Disable the test under emulation because it fails with ENOPROTOOPT in CI
// on cross target. Lack of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
fn test_ip_bind_address_no_port() {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    setsockopt(&fd, sockopt::IpBindAddressNoPort, &true).expect(
        "setting IP_BIND_ADDRESS_NO_PORT on an inet stream socket should succeed",
    );
    assert!(getsockopt(&fd, sockopt::IpBindAddressNoPort).expect(
        "getting IP_BIND_ADDRESS_NO_PORT on an inet stream socket should succeed",
    ));
    setsockopt(&fd, sockopt::IpBindAddressNoPort, &false).expect(
        "unsetting IP_BIND_ADDRESS_NO_PORT on an inet stream socket should succeed",
    );
    assert!(!getsockopt(&fd, sockopt::IpBindAddressNoPort).expect(
        "getting IP_BIND_ADDRESS_NO_PORT on an inet stream socket should succeed",
    ));
}

#[test]
#[cfg(linux_android)]
fn test_tcp_fast_open_connect() {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    setsockopt(&fd, sockopt::TcpFastOpenConnect, &true).expect(
        "setting TCP_FASTOPEN_CONNECT on an inet stream socket should succeed",
    );
    assert!(getsockopt(&fd, sockopt::TcpFastOpenConnect).expect(
        "getting TCP_FASTOPEN_CONNECT on an inet stream socket should succeed",
    ));
    setsockopt(&fd, sockopt::TcpFastOpenConnect, &false).expect(
        "unsetting TCP_FASTOPEN_CONNECT on an inet stream socket should succeed",
    );
    assert!(!getsockopt(&fd, sockopt::TcpFastOpenConnect).expect(
        "getting TCP_FASTOPEN_CONNECT on an inet stream socket should succeed",
    ));
}

#[cfg(linux_android)]
#[test]
fn can_get_peercred_on_unix_socket() {
    use nix::sys::socket::{socketpair, sockopt, SockFlag, SockType};

    let (a, b) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    let a_cred = getsockopt(&a, sockopt::PeerCredentials).unwrap();
    let b_cred = getsockopt(&b, sockopt::PeerCredentials).unwrap();
    assert_eq!(a_cred, b_cred);
    assert_ne!(a_cred.pid(), 0);
}

#[test]
fn is_socket_type_unix() {
    use nix::sys::socket::{socketpair, sockopt, SockFlag, SockType};

    let (a, _b) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    let a_type = getsockopt(&a, sockopt::SockType).unwrap();
    assert_eq!(a_type, SockType::Stream);
}

#[test]
fn is_socket_type_dgram() {
    use nix::sys::socket::{
        getsockopt, sockopt, AddressFamily, SockFlag, SockType,
    };

    let s = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    let s_type = getsockopt(&s, sockopt::SockType).unwrap();
    assert_eq!(s_type, SockType::Datagram);
}

#[cfg(any(target_os = "freebsd", target_os = "linux"))]
#[test]
fn can_get_listen_on_tcp_socket() {
    use nix::sys::socket::{
        getsockopt, listen, socket, sockopt, AddressFamily, Backlog, SockFlag,
        SockType,
    };

    let s = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    let s_listening = getsockopt(&s, sockopt::AcceptConn).unwrap();
    assert!(!s_listening);
    listen(&s, Backlog::new(10).unwrap()).unwrap();
    let s_listening2 = getsockopt(&s, sockopt::AcceptConn).unwrap();
    assert!(s_listening2);
}

#[cfg(target_os = "linux")]
// Some architectures running under cross don't support `setsockopt(SOL_TCP, TCP_ULP)`
// because the cross image is based on Ubuntu 16.04 which predates TCP ULP support
// (it was added in kernel v4.13 released in 2017). For these architectures,
// the `setsockopt(SOL_TCP, TCP_ULP, "tls", sizeof("tls"))` call succeeds
// but the subsequent `setsockopt(SOL_TLS, TLS_TX, ...)` call fails with `ENOPROTOOPT`.
// It's as if the first `setsockopt` call enabled some other option, not `TCP_ULP`.
// For example, `strace` says:
//
//     [pid   813] setsockopt(4, SOL_TCP, 0x1f /* TCP_??? */, [7564404], 4) = 0
//
// It's not clear why `setsockopt(SOL_TCP, TCP_ULP)` succeeds if the container image libc doesn't support it,
// but in any case we can't run the test on such an architecture, so skip it.
#[cfg_attr(qemu, ignore)]
#[test]
fn test_ktls() {
    use nix::sys::socket::{
        accept, bind, connect, getsockname, listen, Backlog, SockaddrIn,
    };
    use std::net::SocketAddrV4;
    use std::str::FromStr;

    let std_sa = SocketAddrV4::from_str("127.0.0.1:0").unwrap();
    let mut sock_addr = SockaddrIn::from(std_sa);

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    bind(rsock.as_raw_fd(), &sock_addr).unwrap();
    sock_addr = getsockname(rsock.as_raw_fd()).unwrap();
    listen(&rsock, Backlog::new(10).unwrap()).unwrap();

    let ssock = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        SockProtocol::Tcp,
    )
    .unwrap();
    connect(ssock.as_raw_fd(), &sock_addr).unwrap();

    let _rsess = accept(rsock.as_raw_fd()).unwrap();

    match setsockopt(&ssock, sockopt::TcpUlp::default(), b"tls") {
        Ok(()) => (),

        // TLS ULP is not enabled, so we can't test kTLS.
        Err(nix::Error::ENOENT) => skip!("TLS ULP is not enabled"),

        Err(err) => panic!("{err:?}"),
    }

    // In real life we would do a TLS handshake and extract the protocol version and secrets.
    // For this test we just make some up.

    let tx = sockopt::TlsCryptoInfo::Aes128Gcm(libc::tls12_crypto_info_aes_gcm_128 {
        info: libc::tls_crypto_info {
            version: libc::TLS_1_2_VERSION,
            cipher_type: libc::TLS_CIPHER_AES_GCM_128,
        },
        iv: *b"\x04\x05\x06\x07\x08\x09\x0a\x0b",
        key: *b"\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f",
        salt: *b"\x00\x01\x02\x03",
        rec_seq: *b"\x00\x00\x00\x00\x00\x00\x00\x00",
    });
    setsockopt(&ssock, sockopt::TcpTlsTx, &tx)
        .expect("setting TLS_TX after enabling TLS ULP should succeed");

    let rx = sockopt::TlsCryptoInfo::Aes128Gcm(libc::tls12_crypto_info_aes_gcm_128 {
        info: libc::tls_crypto_info {
            version: libc::TLS_1_2_VERSION,
            cipher_type: libc::TLS_CIPHER_AES_GCM_128,
        },
        iv: *b"\xf4\xf5\xf6\xf7\xf8\xf9\xfa\xfb",
        key: *b"\xe0\xe1\xe2\xe3\xe4\xe5\xe6\xe7\xe8\xe9\xea\xeb\xec\xed\xee\xef",
        salt: *b"\xf0\xf1\xf2\xf3",
        rec_seq: *b"\x00\x00\x00\x00\x00\x00\x00\x00",
    });
    match setsockopt(&ssock, sockopt::TcpTlsRx, &rx) {
        Ok(()) => (),
        Err(nix::Error::ENOPROTOOPT) => {
            // TLS_TX was added in v4.13 and TLS_RX in v4.17, so we appear to be between that range.
            // It's good enough that TLS_TX worked, so let the test succeed.
        }
        Err(err) => panic!("{err:?}"),
    }
}

#[test]
#[cfg(apple_targets)]
fn test_utun_ifname() {
    skip_if_not_root!("test_utun_ifname");

    use nix::sys::socket::connect;
    use nix::sys::socket::SysControlAddr;

    let fd = socket(
        AddressFamily::System,
        SockType::Datagram,
        SockFlag::empty(),
        SockProtocol::KextControl,
    )
    .unwrap();

    let unit = 123;
    let addr = SysControlAddr::from_name(
        fd.as_raw_fd(),
        "com.apple.net.utun_control",
        unit,
    )
    .unwrap();

    connect(fd.as_raw_fd(), &addr).unwrap();

    let name = getsockopt(&fd, sockopt::UtunIfname)
        .expect("getting UTUN_OPT_IFNAME on a utun interface should succeed");

    let expected_name = format!("utun{}", unit - 1);
    assert_eq!(name.into_string(), Ok(expected_name));
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_reuseport_lb() {
    let fd = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&fd, sockopt::ReusePortLb, &false).unwrap();
    assert!(!getsockopt(&fd, sockopt::ReusePortLb).unwrap());
    setsockopt(&fd, sockopt::ReusePortLb, &true).unwrap();
    assert!(getsockopt(&fd, sockopt::ReusePortLb).unwrap());
}
