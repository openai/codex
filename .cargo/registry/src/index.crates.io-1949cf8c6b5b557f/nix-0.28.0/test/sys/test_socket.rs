#[cfg(linux_android)]
use crate::*;
use libc::c_char;
use nix::sys::socket::{getsockname, AddressFamily, UnixAddr};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{SocketAddrV4, SocketAddrV6};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::slice;
use std::str::FromStr;

#[cfg(target_os = "linux")]
#[cfg_attr(qemu, ignore)]
#[test]
pub fn test_timestamping() {
    use nix::sys::socket::{
        recvmsg, sendmsg, setsockopt, socket, sockopt::Timestamping,
        ControlMessageOwned, MsgFlags, SockFlag, SockType, SockaddrIn,
        TimestampingFlag,
    };
    use std::io::{IoSlice, IoSliceMut};

    let sock_addr = SockaddrIn::from_str("127.0.0.1:6797").unwrap();

    let ssock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("send socket failed");

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    nix::sys::socket::bind(rsock.as_raw_fd(), &sock_addr).unwrap();

    setsockopt(&rsock, Timestamping, &TimestampingFlag::all()).unwrap();

    let sbuf = [0u8; 2048];
    let mut rbuf = [0u8; 2048];
    let flags = MsgFlags::empty();
    let iov1 = [IoSlice::new(&sbuf)];
    let mut iov2 = [IoSliceMut::new(&mut rbuf)];

    let mut cmsg = cmsg_space!(nix::sys::socket::Timestamps);
    sendmsg(ssock.as_raw_fd(), &iov1, &[], flags, Some(&sock_addr)).unwrap();
    let recv =
        recvmsg::<()>(rsock.as_raw_fd(), &mut iov2, Some(&mut cmsg), flags)
            .unwrap();

    let mut ts = None;
    for c in recv.cmsgs() {
        if let ControlMessageOwned::ScmTimestampsns(timestamps) = c {
            ts = Some(timestamps.system);
        }
    }
    let ts = ts.expect("ScmTimestampns is present");
    let sys_time =
        ::nix::time::clock_gettime(::nix::time::ClockId::CLOCK_REALTIME)
            .unwrap();
    let diff = if ts > sys_time {
        ts - sys_time
    } else {
        sys_time - ts
    };
    assert!(std::time::Duration::from(diff).as_secs() < 60);
}

#[cfg(target_os = "freebsd")]
#[test]
pub fn test_timestamping_realtime() {
    use nix::sys::socket::{
        recvmsg, sendmsg, setsockopt, socket, sockopt::ReceiveTimestamp,
        sockopt::TsClock, ControlMessageOwned, MsgFlags, SockFlag, SockType,
        SockaddrIn, SocketTimestamp,
    };
    use std::io::{IoSlice, IoSliceMut};

    let sock_addr = SockaddrIn::from_str("127.0.0.1:6792").unwrap();

    let ssock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("send socket failed");

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    nix::sys::socket::bind(rsock.as_raw_fd(), &sock_addr).unwrap();

    setsockopt(&rsock, ReceiveTimestamp, &true).unwrap();
    setsockopt(&rsock, TsClock, &SocketTimestamp::SO_TS_REALTIME).unwrap();

    let sbuf = [0u8; 2048];
    let mut rbuf = [0u8; 2048];
    let flags = MsgFlags::empty();
    let iov1 = [IoSlice::new(&sbuf)];
    let mut iov2 = [IoSliceMut::new(&mut rbuf)];

    let mut cmsg = cmsg_space!(nix::sys::time::TimeVal);
    sendmsg(ssock.as_raw_fd(), &iov1, &[], flags, Some(&sock_addr)).unwrap();
    let recv =
        recvmsg::<()>(rsock.as_raw_fd(), &mut iov2, Some(&mut cmsg), flags)
            .unwrap();

    let mut ts = None;
    for c in recv.cmsgs() {
        if let ControlMessageOwned::ScmRealtime(timeval) = c {
            ts = Some(timeval);
        }
    }
    let ts = ts.expect("ScmRealtime is present");
    let sys_time =
        ::nix::time::clock_gettime(::nix::time::ClockId::CLOCK_REALTIME)
            .unwrap();
    let diff = if ts > sys_time {
        ts - sys_time
    } else {
        sys_time - ts
    };
    assert!(std::time::Duration::from(diff).as_secs() < 60);
}

#[cfg(target_os = "freebsd")]
#[test]
pub fn test_timestamping_monotonic() {
    use nix::sys::socket::{
        recvmsg, sendmsg, setsockopt, socket, sockopt::ReceiveTimestamp,
        sockopt::TsClock, ControlMessageOwned, MsgFlags, SockFlag, SockType,
        SockaddrIn, SocketTimestamp,
    };
    use std::io::{IoSlice, IoSliceMut};

    let sock_addr = SockaddrIn::from_str("127.0.0.1:6803").unwrap();

    let ssock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("send socket failed");

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    nix::sys::socket::bind(rsock.as_raw_fd(), &sock_addr).unwrap();

    setsockopt(&rsock, ReceiveTimestamp, &true).unwrap();
    setsockopt(&rsock, TsClock, &SocketTimestamp::SO_TS_MONOTONIC).unwrap();

    let sbuf = [0u8; 2048];
    let mut rbuf = [0u8; 2048];
    let flags = MsgFlags::empty();
    let iov1 = [IoSlice::new(&sbuf)];
    let mut iov2 = [IoSliceMut::new(&mut rbuf)];

    let mut cmsg = cmsg_space!(nix::sys::time::TimeVal);
    sendmsg(ssock.as_raw_fd(), &iov1, &[], flags, Some(&sock_addr)).unwrap();
    let recv =
        recvmsg::<()>(rsock.as_raw_fd(), &mut iov2, Some(&mut cmsg), flags)
            .unwrap();

    let mut ts = None;
    for c in recv.cmsgs() {
        if let ControlMessageOwned::ScmMonotonic(timeval) = c {
            ts = Some(timeval);
        }
    }
    let ts = ts.expect("ScmMonotonic is present");
    let sys_time =
        ::nix::time::clock_gettime(::nix::time::ClockId::CLOCK_MONOTONIC)
            .unwrap();
    let diff = sys_time - ts; // Monotonic clock sys_time must be greater
    assert!(std::time::Duration::from(diff).as_secs() < 60);
}

#[test]
pub fn test_path_to_sock_addr() {
    let path = "/foo/bar";
    let actual = Path::new(path);
    let addr = UnixAddr::new(actual).unwrap();

    let expect: &[c_char] =
        unsafe { slice::from_raw_parts(path.as_ptr().cast(), path.len()) };
    assert_eq!(unsafe { &(*addr.as_ptr()).sun_path[..8] }, expect);

    assert_eq!(addr.path(), Some(actual));
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[test]
pub fn test_addr_equality_path() {
    let path = "/foo/bar";
    let actual = Path::new(path);
    let addr1 = UnixAddr::new(actual).unwrap();
    let mut addr2 = addr1;

    unsafe { (*addr2.as_mut_ptr()).sun_path[10] = 127 };

    assert_eq!(addr1, addr2);
    assert_eq!(calculate_hash(&addr1), calculate_hash(&addr2));
}

#[cfg(linux_android)]
#[test]
pub fn test_abstract_sun_path_too_long() {
    let name = String::from("nix\0abstract\0tesnix\0abstract\0tesnix\0abstract\0tesnix\0abstract\0tesnix\0abstract\0testttttnix\0abstract\0test\0make\0sure\0this\0is\0long\0enough");
    let addr = UnixAddr::new_abstract(name.as_bytes());
    addr.expect_err("assertion failed");
}

#[cfg(linux_android)]
#[test]
pub fn test_addr_equality_abstract() {
    let name = String::from("nix\0abstract\0test");
    let addr1 = UnixAddr::new_abstract(name.as_bytes()).unwrap();
    let mut addr2 = addr1;

    assert_eq!(addr1, addr2);
    assert_eq!(calculate_hash(&addr1), calculate_hash(&addr2));

    unsafe { (*addr2.as_mut_ptr()).sun_path[17] = 127 };
    assert_ne!(addr1, addr2);
    assert_ne!(calculate_hash(&addr1), calculate_hash(&addr2));
}

// Test getting/setting abstract addresses (without unix socket creation)
#[cfg(linux_android)]
#[test]
pub fn test_abstract_uds_addr() {
    let empty = String::new();
    let addr = UnixAddr::new_abstract(empty.as_bytes()).unwrap();
    let sun_path: [u8; 0] = [];
    assert_eq!(addr.as_abstract(), Some(&sun_path[..]));

    let name = String::from("nix\0abstract\0test");
    let addr = UnixAddr::new_abstract(name.as_bytes()).unwrap();
    let sun_path = [
        110u8, 105, 120, 0, 97, 98, 115, 116, 114, 97, 99, 116, 0, 116, 101,
        115, 116,
    ];
    assert_eq!(addr.as_abstract(), Some(&sun_path[..]));
    assert_eq!(addr.path(), None);

    // Internally, name is null-prefixed (abstract namespace)
    assert_eq!(unsafe { (*addr.as_ptr()).sun_path[0] }, 0);
}

// Test getting an unnamed address (without unix socket creation)
#[cfg(linux_android)]
#[test]
pub fn test_unnamed_uds_addr() {
    use crate::nix::sys::socket::SockaddrLike;

    let addr = UnixAddr::new_unnamed();

    assert!(addr.is_unnamed());
    assert_eq!(addr.len(), 2);
    assert!(addr.path().is_none());
    assert_eq!(addr.path_len(), 0);

    assert!(addr.as_abstract().is_none());
}

#[test]
pub fn test_getsockname() {
    use nix::sys::socket::bind;
    use nix::sys::socket::{socket, AddressFamily, SockFlag, SockType};

    let tempdir = tempfile::tempdir().unwrap();
    let sockname = tempdir.path().join("sock");
    let sock = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");
    let sockaddr = UnixAddr::new(&sockname).unwrap();
    bind(sock.as_raw_fd(), &sockaddr).expect("bind failed");
    assert_eq!(
        sockaddr,
        getsockname(sock.as_raw_fd()).expect("getsockname failed")
    );
}

#[test]
pub fn test_socketpair() {
    use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
    use nix::unistd::{read, write};

    let (fd1, fd2) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    write(&fd1, b"hello").unwrap();
    let mut buf = [0; 5];
    read(fd2.as_raw_fd(), &mut buf).unwrap();

    assert_eq!(&buf[..], b"hello");
}

#[test]
pub fn test_recvmsg_sockaddr_un() {
    use nix::sys::socket::{
        self, bind, socket, AddressFamily, MsgFlags, SockFlag, SockType,
    };

    let tempdir = tempfile::tempdir().unwrap();
    let sockname = tempdir.path().join("sock");
    let sock = socket(
        AddressFamily::Unix,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");
    let sockaddr = UnixAddr::new(&sockname).unwrap();
    bind(sock.as_raw_fd(), &sockaddr).expect("bind failed");

    // Send a message
    let send_buffer = "hello".as_bytes();
    if let Err(e) = socket::sendmsg(
        sock.as_raw_fd(),
        &[std::io::IoSlice::new(send_buffer)],
        &[],
        MsgFlags::empty(),
        Some(&sockaddr),
    ) {
        crate::skip!("Couldn't send ({e:?}), so skipping test");
    }

    // Receive the message
    let mut recv_buffer = [0u8; 32];
    let mut iov = [std::io::IoSliceMut::new(&mut recv_buffer)];
    let received =
        socket::recvmsg(sock.as_raw_fd(), &mut iov, None, MsgFlags::empty())
            .unwrap();
    // Check the address in the received message
    assert_eq!(sockaddr, received.address.unwrap());
}

#[test]
pub fn test_std_conversions() {
    use nix::sys::socket::*;

    let std_sa = SocketAddrV4::from_str("127.0.0.1:6789").unwrap();
    let sock_addr = SockaddrIn::from(std_sa);
    assert_eq!(std_sa, sock_addr.into());

    let std_sa = SocketAddrV6::from_str("[::1]:6000").unwrap();
    let sock_addr: SockaddrIn6 = SockaddrIn6::from(std_sa);
    assert_eq!(std_sa, sock_addr.into());
}

mod recvfrom {
    use super::*;
    use nix::sys::socket::*;
    use nix::{errno::Errno, Result};
    use std::thread;

    const MSG: &[u8] = b"Hello, World!";

    fn sendrecv<Fs, Fr>(
        rsock: RawFd,
        ssock: RawFd,
        f_send: Fs,
        mut f_recv: Fr,
    ) -> Option<SockaddrStorage>
    where
        Fs: Fn(RawFd, &[u8], MsgFlags) -> Result<usize> + Send + 'static,
        Fr: FnMut(usize, Option<SockaddrStorage>),
    {
        let mut buf: [u8; 13] = [0u8; 13];
        let mut l = 0;
        let mut from = None;

        let send_thread = thread::spawn(move || {
            let mut l = 0;
            while l < std::mem::size_of_val(MSG) {
                l += f_send(ssock, &MSG[l..], MsgFlags::empty()).unwrap();
            }
        });

        while l < std::mem::size_of_val(MSG) {
            let (len, from_) = recvfrom(rsock, &mut buf[l..]).unwrap();
            f_recv(len, from_);
            from = from_;
            l += len;
        }
        assert_eq!(&buf, MSG);
        send_thread.join().unwrap();
        from
    }

    #[test]
    pub fn stream() {
        let (fd2, fd1) = socketpair(
            AddressFamily::Unix,
            SockType::Stream,
            None,
            SockFlag::empty(),
        )
        .unwrap();
        // Ignore from for stream sockets
        let _ = sendrecv(fd1.as_raw_fd(), fd2.as_raw_fd(), send, |_, _| {});
    }

    #[test]
    pub fn udp() {
        let std_sa = SocketAddrV4::from_str("127.0.0.1:6795").unwrap();
        let sock_addr = SockaddrIn::from(std_sa);
        let rsock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .unwrap();
        bind(rsock.as_raw_fd(), &sock_addr).unwrap();
        let ssock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        let from = sendrecv(
            rsock.as_raw_fd(),
            ssock.as_raw_fd(),
            move |s, m, flags| sendto(s.as_raw_fd(), m, &sock_addr, flags),
            |_, _| {},
        );
        // UDP sockets should set the from address
        assert_eq!(AddressFamily::Inet, from.unwrap().family().unwrap());
    }

    #[cfg(target_os = "linux")]
    mod udp_offload {
        use super::*;
        use nix::sys::socket::sockopt::{UdpGroSegment, UdpGsoSegment};
        use std::io::IoSlice;

        #[test]
        // Disable the test under emulation because it fails in Cirrus-CI.  Lack
        // of QEMU support is suspected.
        #[cfg_attr(qemu, ignore)]
        pub fn gso() {
            require_kernel_version!(udp_offload::gso, ">= 4.18");

            // In this test, we send the data and provide a GSO segment size.
            // Since we are sending the buffer of size 13, six UDP packets
            // with size 2 and two UDP packet with size 1 will be sent.
            let segment_size: u16 = 2;

            let sock_addr = SockaddrIn::new(127, 0, 0, 1, 6791);
            let rsock = socket(
                AddressFamily::Inet,
                SockType::Datagram,
                SockFlag::empty(),
                None,
            )
            .unwrap();

            setsockopt(&rsock, UdpGsoSegment, &(segment_size as _))
                .expect("setsockopt UDP_SEGMENT failed");

            bind(rsock.as_raw_fd(), &sock_addr).unwrap();
            let ssock = socket(
                AddressFamily::Inet,
                SockType::Datagram,
                SockFlag::empty(),
                None,
            )
            .expect("send socket failed");

            let mut num_packets_received: i32 = 0;

            sendrecv(
                rsock.as_raw_fd(),
                ssock.as_raw_fd(),
                move |s, m, flags| {
                    let iov = [IoSlice::new(m)];
                    let cmsg = ControlMessage::UdpGsoSegments(&segment_size);
                    sendmsg(
                        s.as_raw_fd(),
                        &iov,
                        &[cmsg],
                        flags,
                        Some(&sock_addr),
                    )
                },
                {
                    let num_packets_received_ref = &mut num_packets_received;

                    move |len, _| {
                        // check that we receive UDP packets with payload size
                        // less or equal to segment size
                        assert!(len <= segment_size as usize);
                        *num_packets_received_ref += 1;
                    }
                },
            );

            // Buffer size is 13, we will receive six packets of size 2,
            // and one packet of size 1.
            assert_eq!(7, num_packets_received);
        }

        #[test]
        // Disable the test on emulated platforms because it fails in Cirrus-CI.
        // Lack of QEMU support is suspected.
        #[cfg_attr(qemu, ignore)]
        pub fn gro() {
            require_kernel_version!(udp_offload::gro, ">= 5.3");

            // It's hard to guarantee receiving GRO packets. Just checking
            // that `setsockopt` doesn't fail with error

            let rsock = socket(
                AddressFamily::Inet,
                SockType::Datagram,
                SockFlag::empty(),
                None,
            )
            .unwrap();

            setsockopt(&rsock, UdpGroSegment, &true)
                .expect("setsockopt UDP_GRO failed");
        }
    }

    #[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
    #[test]
    pub fn udp_sendmmsg() {
        use std::io::IoSlice;

        let std_sa = SocketAddrV4::from_str("127.0.0.1:6793").unwrap();
        let std_sa2 = SocketAddrV4::from_str("127.0.0.1:6794").unwrap();
        let sock_addr = SockaddrIn::from(std_sa);
        let sock_addr2 = SockaddrIn::from(std_sa2);

        let rsock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .unwrap();
        bind(rsock.as_raw_fd(), &sock_addr).unwrap();
        let ssock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");

        let from = sendrecv(
            rsock.as_raw_fd(),
            ssock.as_raw_fd(),
            move |s, m, flags| {
                let batch_size = 15;
                let mut iovs = Vec::with_capacity(1 + batch_size);
                let mut addrs = Vec::with_capacity(1 + batch_size);
                let mut data = MultiHeaders::preallocate(1 + batch_size, None);
                let iov = IoSlice::new(m);
                // first chunk:
                iovs.push([iov]);
                addrs.push(Some(sock_addr));

                for _ in 0..batch_size {
                    iovs.push([iov]);
                    addrs.push(Some(sock_addr2));
                }

                let res = sendmmsg(s, &mut data, &iovs, addrs, [], flags)?;
                let mut sent_messages = 0;
                let mut sent_bytes = 0;
                for item in res {
                    sent_messages += 1;
                    sent_bytes += item.bytes;
                }
                //
                assert_eq!(sent_messages, iovs.len());
                assert_eq!(sent_bytes, sent_messages * m.len());
                Ok(sent_messages)
            },
            |_, _| {},
        );
        // UDP sockets should set the from address
        assert_eq!(AddressFamily::Inet, from.unwrap().family().unwrap());
    }

    #[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
    #[test]
    pub fn udp_recvmmsg() {
        use nix::sys::socket::{recvmmsg, MsgFlags};
        use std::io::IoSliceMut;

        const NUM_MESSAGES_SENT: usize = 2;
        const DATA: [u8; 2] = [1, 2];

        let inet_addr = SocketAddrV4::from_str("127.0.0.1:6798").unwrap();
        let sock_addr = SockaddrIn::from(inet_addr);

        let rsock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .unwrap();
        bind(rsock.as_raw_fd(), &sock_addr).unwrap();
        let ssock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");

        let send_thread = thread::spawn(move || {
            for _ in 0..NUM_MESSAGES_SENT {
                sendto(
                    ssock.as_raw_fd(),
                    &DATA[..],
                    &sock_addr,
                    MsgFlags::empty(),
                )
                .unwrap();
            }
        });

        let mut msgs = std::collections::LinkedList::new();

        // Buffers to receive exactly `NUM_MESSAGES_SENT` messages
        let mut receive_buffers = [[0u8; 32]; NUM_MESSAGES_SENT];
        msgs.extend(
            receive_buffers
                .iter_mut()
                .map(|buf| [IoSliceMut::new(&mut buf[..])]),
        );

        let mut data =
            MultiHeaders::<SockaddrIn>::preallocate(msgs.len(), None);

        let res: Vec<RecvMsg<SockaddrIn>> = recvmmsg(
            rsock.as_raw_fd(),
            &mut data,
            msgs.iter_mut(),
            MsgFlags::empty(),
            None,
        )
        .expect("recvmmsg")
        .collect();
        assert_eq!(res.len(), DATA.len());

        for RecvMsg { address, bytes, .. } in res.into_iter() {
            assert_eq!(AddressFamily::Inet, address.unwrap().family().unwrap());
            assert_eq!(DATA.len(), bytes);
        }

        for buf in &receive_buffers {
            assert_eq!(&buf[..DATA.len()], DATA);
        }

        send_thread.join().unwrap();
    }

    #[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
    #[test]
    pub fn udp_recvmmsg_dontwait_short_read() {
        use nix::sys::socket::{recvmmsg, MsgFlags};
        use std::io::IoSliceMut;

        const NUM_MESSAGES_SENT: usize = 2;
        const DATA: [u8; 4] = [1, 2, 3, 4];

        let inet_addr = SocketAddrV4::from_str("127.0.0.1:6799").unwrap();
        let sock_addr = SockaddrIn::from(inet_addr);

        let rsock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .unwrap();
        bind(rsock.as_raw_fd(), &sock_addr).unwrap();
        let ssock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");

        let send_thread = thread::spawn(move || {
            for _ in 0..NUM_MESSAGES_SENT {
                sendto(
                    ssock.as_raw_fd(),
                    &DATA[..],
                    &sock_addr,
                    MsgFlags::empty(),
                )
                .unwrap();
            }
        });
        // Ensure we've sent all the messages before continuing so `recvmmsg`
        // will return right away
        send_thread.join().unwrap();

        let mut msgs = std::collections::LinkedList::new();

        // Buffers to receive >`NUM_MESSAGES_SENT` messages to ensure `recvmmsg`
        // will return when there are fewer than requested messages in the
        // kernel buffers when using `MSG_DONTWAIT`.
        let mut receive_buffers = [[0u8; 32]; NUM_MESSAGES_SENT + 2];
        msgs.extend(
            receive_buffers
                .iter_mut()
                .map(|buf| [IoSliceMut::new(&mut buf[..])]),
        );

        let mut data = MultiHeaders::<SockaddrIn>::preallocate(
            NUM_MESSAGES_SENT + 2,
            None,
        );

        let res: Vec<RecvMsg<SockaddrIn>> = recvmmsg(
            rsock.as_raw_fd(),
            &mut data,
            msgs.iter_mut(),
            MsgFlags::MSG_DONTWAIT,
            None,
        )
        .expect("recvmmsg")
        .collect();
        assert_eq!(res.len(), NUM_MESSAGES_SENT);

        for RecvMsg { address, bytes, .. } in res.into_iter() {
            assert_eq!(AddressFamily::Inet, address.unwrap().family().unwrap());
            assert_eq!(DATA.len(), bytes);
        }

        for buf in &receive_buffers[..NUM_MESSAGES_SENT] {
            assert_eq!(&buf[..DATA.len()], DATA);
        }
    }

    #[test]
    pub fn udp_inet6() {
        let addr = std::net::Ipv6Addr::from_str("::1").unwrap();
        let rport = 6796;
        let rstd_sa = SocketAddrV6::new(addr, rport, 0, 0);
        let raddr = SockaddrIn6::from(rstd_sa);
        let sport = 6798;
        let sstd_sa = SocketAddrV6::new(addr, sport, 0, 0);
        let saddr = SockaddrIn6::from(sstd_sa);
        let rsock = socket(
            AddressFamily::Inet6,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("receive socket failed");
        match bind(rsock.as_raw_fd(), &raddr) {
            Err(Errno::EADDRNOTAVAIL) => {
                println!("IPv6 not available, skipping test.");
                return;
            }
            Err(e) => panic!("bind: {e}"),
            Ok(()) => (),
        }
        let ssock = socket(
            AddressFamily::Inet6,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        bind(ssock.as_raw_fd(), &saddr).unwrap();
        let from = sendrecv(
            rsock.as_raw_fd(),
            ssock.as_raw_fd(),
            move |s, m, flags| sendto(s.as_raw_fd(), m, &raddr, flags),
            |_, _| {},
        );
        assert_eq!(AddressFamily::Inet6, from.unwrap().family().unwrap());
        let osent_addr = from.unwrap();
        let sent_addr = osent_addr.as_sockaddr_in6().unwrap();
        assert_eq!(sent_addr.ip(), addr);
        assert_eq!(sent_addr.port(), sport);
    }
}

// Test error handling of our recvmsg wrapper
#[test]
pub fn test_recvmsg_ebadf() {
    use nix::errno::Errno;
    use nix::sys::socket::{recvmsg, MsgFlags};
    use std::io::IoSliceMut;

    let mut buf = [0u8; 5];
    let mut iov = [IoSliceMut::new(&mut buf[..])];

    let fd = -1; // Bad file descriptor
    let r = recvmsg::<()>(fd.as_raw_fd(), &mut iov, None, MsgFlags::empty());

    assert_eq!(r.err().unwrap(), Errno::EBADF);
}

// Disable the test on emulated platforms due to a bug in QEMU versions <
// 2.12.0.  https://bugs.launchpad.net/qemu/+bug/1701808
#[cfg_attr(qemu, ignore)]
#[test]
pub fn test_scm_rights() {
    use nix::sys::socket::{
        recvmsg, sendmsg, socketpair, AddressFamily, ControlMessage,
        ControlMessageOwned, MsgFlags, SockFlag, SockType,
    };
    use nix::unistd::{close, pipe, read, write};
    use std::io::{IoSlice, IoSliceMut};

    let (fd1, fd2) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    let (r, w) = pipe().unwrap();
    let mut received_r: Option<RawFd> = None;

    {
        let iov = [IoSlice::new(b"hello")];
        let fds = [r.as_raw_fd()];
        let cmsg = ControlMessage::ScmRights(&fds);
        assert_eq!(
            sendmsg::<()>(
                fd1.as_raw_fd(),
                &iov,
                &[cmsg],
                MsgFlags::empty(),
                None
            )
            .unwrap(),
            5
        );
    }

    {
        let mut buf = [0u8; 5];

        let mut iov = [IoSliceMut::new(&mut buf[..])];
        let mut cmsgspace = cmsg_space!([RawFd; 1]);
        let msg = recvmsg::<()>(
            fd2.as_raw_fd(),
            &mut iov,
            Some(&mut cmsgspace),
            MsgFlags::empty(),
        )
        .unwrap();

        for cmsg in msg.cmsgs() {
            if let ControlMessageOwned::ScmRights(fd) = cmsg {
                assert_eq!(received_r, None);
                assert_eq!(fd.len(), 1);
                received_r = Some(fd[0]);
            } else {
                panic!("unexpected cmsg");
            }
        }
        assert_eq!(msg.bytes, 5);
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
    }

    let received_r = received_r.expect("Did not receive passed fd");
    // Ensure that the received file descriptor works
    write(&w, b"world").unwrap();
    let mut buf = [0u8; 5];
    read(received_r.as_raw_fd(), &mut buf).unwrap();
    assert_eq!(&buf[..], b"world");
    close(received_r).unwrap();
}

// Disable the test on emulated platforms due to not enabled support of AF_ALG in QEMU from rust cross
#[cfg(linux_android)]
#[cfg_attr(qemu, ignore)]
#[test]
pub fn test_af_alg_cipher() {
    use nix::sys::socket::sockopt::AlgSetKey;
    use nix::sys::socket::{
        accept, bind, sendmsg, setsockopt, socket, AddressFamily, AlgAddr,
        ControlMessage, MsgFlags, SockFlag, SockType,
    };
    use nix::unistd::read;
    use std::io::IoSlice;

    skip_if_cirrus!("Fails for an unknown reason Cirrus CI.  Bug #1352");
    // Travis's seccomp profile blocks AF_ALG
    // https://docs.docker.com/engine/security/seccomp/
    skip_if_seccomp!(test_af_alg_cipher);

    let alg_type = "skcipher";
    let alg_name = "ctr-aes-aesni";
    // 256-bits secret key
    let key = vec![0u8; 32];
    // 16-bytes IV
    let iv_len = 16;
    let iv = vec![1u8; iv_len];
    // 256-bytes plain payload
    let payload_len = 256;
    let payload = vec![2u8; payload_len];

    let sock = socket(
        AddressFamily::Alg,
        SockType::SeqPacket,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");

    let sockaddr = AlgAddr::new(alg_type, alg_name);
    bind(sock.as_raw_fd(), &sockaddr).expect("bind failed");

    assert_eq!(sockaddr.alg_name().to_string_lossy(), alg_name);
    assert_eq!(sockaddr.alg_type().to_string_lossy(), alg_type);

    setsockopt(&sock, AlgSetKey::default(), &key).expect("setsockopt");
    let session_socket = accept(sock.as_raw_fd()).expect("accept failed");

    let msgs = [
        ControlMessage::AlgSetOp(&libc::ALG_OP_ENCRYPT),
        ControlMessage::AlgSetIv(iv.as_slice()),
    ];
    let iov = IoSlice::new(&payload);
    sendmsg::<()>(
        session_socket.as_raw_fd(),
        &[iov],
        &msgs,
        MsgFlags::empty(),
        None,
    )
    .expect("sendmsg encrypt");

    // allocate buffer for encrypted data
    let mut encrypted = vec![0u8; payload_len];
    let num_bytes =
        read(session_socket.as_raw_fd(), &mut encrypted).expect("read encrypt");
    assert_eq!(num_bytes, payload_len);

    let iov = IoSlice::new(&encrypted);

    let iv = vec![1u8; iv_len];

    let msgs = [
        ControlMessage::AlgSetOp(&libc::ALG_OP_DECRYPT),
        ControlMessage::AlgSetIv(iv.as_slice()),
    ];
    sendmsg::<()>(
        session_socket.as_raw_fd(),
        &[iov],
        &msgs,
        MsgFlags::empty(),
        None,
    )
    .expect("sendmsg decrypt");

    // allocate buffer for decrypted data
    let mut decrypted = vec![0u8; payload_len];
    let num_bytes =
        read(session_socket.as_raw_fd(), &mut decrypted).expect("read decrypt");

    assert_eq!(num_bytes, payload_len);
    assert_eq!(decrypted, payload);
}

// Disable the test on emulated platforms due to not enabled support of AF_ALG
// in QEMU from rust cross
#[cfg(linux_android)]
#[cfg_attr(qemu, ignore)]
#[test]
pub fn test_af_alg_aead() {
    use libc::{ALG_OP_DECRYPT, ALG_OP_ENCRYPT};
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use nix::sys::socket::sockopt::{AlgSetAeadAuthSize, AlgSetKey};
    use nix::sys::socket::{
        accept, bind, sendmsg, setsockopt, socket, AddressFamily, AlgAddr,
        ControlMessage, MsgFlags, SockFlag, SockType,
    };
    use nix::unistd::read;
    use std::io::IoSlice;

    skip_if_cirrus!("Fails for an unknown reason Cirrus CI.  Bug #1352");
    // Travis's seccomp profile blocks AF_ALG
    // https://docs.docker.com/engine/security/seccomp/
    skip_if_seccomp!(test_af_alg_aead);

    let auth_size = 4usize;
    let assoc_size = 16u32;

    let alg_type = "aead";
    let alg_name = "gcm(aes)";
    // 256-bits secret key
    let key = vec![0u8; 32];
    // 12-bytes IV
    let iv_len = 12;
    let iv = vec![1u8; iv_len];
    // 256-bytes plain payload
    let payload_len = 256;
    let mut payload =
        vec![2u8; payload_len + (assoc_size as usize) + auth_size];

    for i in 0..assoc_size {
        payload[i as usize] = 10;
    }

    let len = payload.len();

    for i in 0..auth_size {
        payload[len - 1 - i] = 0;
    }

    let sock = socket(
        AddressFamily::Alg,
        SockType::SeqPacket,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");

    let sockaddr = AlgAddr::new(alg_type, alg_name);
    bind(sock.as_raw_fd(), &sockaddr).expect("bind failed");

    setsockopt(&sock, AlgSetAeadAuthSize, &auth_size)
        .expect("setsockopt AlgSetAeadAuthSize");
    setsockopt(&sock, AlgSetKey::default(), &key)
        .expect("setsockopt AlgSetKey");
    let session_socket = accept(sock.as_raw_fd()).expect("accept failed");

    let msgs = [
        ControlMessage::AlgSetOp(&ALG_OP_ENCRYPT),
        ControlMessage::AlgSetIv(iv.as_slice()),
        ControlMessage::AlgSetAeadAssoclen(&assoc_size),
    ];

    let iov = IoSlice::new(&payload);
    sendmsg::<()>(
        session_socket.as_raw_fd(),
        &[iov],
        &msgs,
        MsgFlags::empty(),
        None,
    )
    .expect("sendmsg encrypt");

    // allocate buffer for encrypted data
    let mut encrypted =
        vec![0u8; (assoc_size as usize) + payload_len + auth_size];
    let num_bytes =
        read(session_socket.as_raw_fd(), &mut encrypted).expect("read encrypt");
    assert_eq!(num_bytes, payload_len + auth_size + (assoc_size as usize));

    for i in 0..assoc_size {
        encrypted[i as usize] = 10;
    }

    let iov = IoSlice::new(&encrypted);

    let iv = vec![1u8; iv_len];

    let session_socket = accept(sock.as_raw_fd()).expect("accept failed");

    let msgs = [
        ControlMessage::AlgSetOp(&ALG_OP_DECRYPT),
        ControlMessage::AlgSetIv(iv.as_slice()),
        ControlMessage::AlgSetAeadAssoclen(&assoc_size),
    ];
    sendmsg::<()>(
        session_socket.as_raw_fd(),
        &[iov],
        &msgs,
        MsgFlags::empty(),
        None,
    )
    .expect("sendmsg decrypt");

    // allocate buffer for decrypted data
    let mut decrypted =
        vec![0u8; payload_len + (assoc_size as usize) + auth_size];
    // Starting with kernel 4.9, the interface changed slightly such that the
    // authentication tag memory is only needed in the output buffer for encryption
    // and in the input buffer for decryption.
    // Do not block on read, as we may have fewer bytes than buffer size
    fcntl(session_socket, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
        .expect("fcntl non_blocking");
    let num_bytes =
        read(session_socket.as_raw_fd(), &mut decrypted).expect("read decrypt");

    assert!(num_bytes >= payload_len + (assoc_size as usize));
    assert_eq!(
        decrypted[(assoc_size as usize)..(payload_len + (assoc_size as usize))],
        payload[(assoc_size as usize)..payload_len + (assoc_size as usize)]
    );
}

// Verify `ControlMessage::Ipv4PacketInfo` for `sendmsg`.
// This creates a (udp) socket bound to localhost, then sends a message to
// itself but uses Ipv4PacketInfo to force the source address to be localhost.
//
// This would be a more interesting test if we could assume that the test host
// has more than one IP address (since we could select a different address to
// test from).
#[cfg(any(target_os = "linux", apple_targets, target_os = "netbsd"))]
#[test]
pub fn test_sendmsg_ipv4packetinfo() {
    use cfg_if::cfg_if;
    use nix::sys::socket::{
        bind, sendmsg, socket, AddressFamily, ControlMessage, MsgFlags,
        SockFlag, SockType, SockaddrIn,
    };
    use std::io::IoSlice;

    let sock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");

    let sock_addr = SockaddrIn::new(127, 0, 0, 1, 4000);

    bind(sock.as_raw_fd(), &sock_addr).expect("bind failed");

    let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let iov = [IoSlice::new(&slice)];

    cfg_if! {
        if #[cfg(target_os = "netbsd")] {
            let pi = libc::in_pktinfo {
                ipi_ifindex: 0, /* Unspecified interface */
                ipi_addr: libc::in_addr { s_addr: 0 },
            };
        } else {
            let pi = libc::in_pktinfo {
                ipi_ifindex: 0, /* Unspecified interface */
                ipi_addr: libc::in_addr { s_addr: 0 },
                ipi_spec_dst: sock_addr.as_ref().sin_addr,
            };
        }
    }

    let cmsg = [ControlMessage::Ipv4PacketInfo(&pi)];

    sendmsg(
        sock.as_raw_fd(),
        &iov,
        &cmsg,
        MsgFlags::empty(),
        Some(&sock_addr),
    )
    .expect("sendmsg");
}

// Verify `ControlMessage::Ipv6PacketInfo` for `sendmsg`.
// This creates a (udp) socket bound to ip6-localhost, then sends a message to
// itself but uses Ipv6PacketInfo to force the source address to be
// ip6-localhost.
//
// This would be a more interesting test if we could assume that the test host
// has more than one IP address (since we could select a different address to
// test from).
#[cfg(any(
    target_os = "linux",
    apple_targets,
    target_os = "netbsd",
    target_os = "freebsd"
))]
#[test]
pub fn test_sendmsg_ipv6packetinfo() {
    use nix::errno::Errno;
    use nix::sys::socket::{
        bind, sendmsg, socket, AddressFamily, ControlMessage, MsgFlags,
        SockFlag, SockType, SockaddrIn6,
    };
    use std::io::IoSlice;

    let sock = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");

    let std_sa = SocketAddrV6::from_str("[::1]:6000").unwrap();
    let sock_addr: SockaddrIn6 = SockaddrIn6::from(std_sa);

    if let Err(Errno::EADDRNOTAVAIL) = bind(sock.as_raw_fd(), &sock_addr) {
        println!("IPv6 not available, skipping test.");
        return;
    }

    let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let iov = [IoSlice::new(&slice)];

    let pi = libc::in6_pktinfo {
        ipi6_ifindex: 0, /* Unspecified interface */
        ipi6_addr: sock_addr.as_ref().sin6_addr,
    };

    let cmsg = [ControlMessage::Ipv6PacketInfo(&pi)];

    sendmsg::<SockaddrIn6>(
        sock.as_raw_fd(),
        &iov,
        &cmsg,
        MsgFlags::empty(),
        Some(&sock_addr),
    )
    .expect("sendmsg");
}

// Verify that ControlMessage::Ipv4SendSrcAddr works for sendmsg. This
// creates a UDP socket bound to all local interfaces (0.0.0.0). It then
// sends message to itself at 127.0.0.1 while explicitly specifying
// 127.0.0.1 as the source address through an Ipv4SendSrcAddr
// (IP_SENDSRCADDR) control message.
//
// Note that binding to 0.0.0.0 is *required* on FreeBSD; sendmsg
// returns EINVAL otherwise. (See FreeBSD's ip(4) man page.)
#[cfg(any(freebsdlike, netbsdlike))]
#[test]
pub fn test_sendmsg_ipv4sendsrcaddr() {
    use nix::sys::socket::{
        bind, sendmsg, socket, AddressFamily, ControlMessage, MsgFlags,
        SockFlag, SockType, SockaddrIn,
    };
    use std::io::IoSlice;

    let sock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");

    let unspec_sock_addr = SockaddrIn::new(0, 0, 0, 0, 0);
    bind(sock.as_raw_fd(), &unspec_sock_addr).expect("bind failed");
    let bound_sock_addr: SockaddrIn = getsockname(sock.as_raw_fd()).unwrap();
    let localhost_sock_addr: SockaddrIn =
        SockaddrIn::new(127, 0, 0, 1, bound_sock_addr.port());

    let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let iov = [IoSlice::new(&slice)];
    let cmsg = [ControlMessage::Ipv4SendSrcAddr(
        &localhost_sock_addr.as_ref().sin_addr,
    )];

    sendmsg(
        sock.as_raw_fd(),
        &iov,
        &cmsg,
        MsgFlags::empty(),
        Some(&localhost_sock_addr),
    )
    .expect("sendmsg");
}

/// Tests that passing multiple fds using a single `ControlMessage` works.
// Disable the test on emulated platforms due to a bug in QEMU versions <
// 2.12.0.  https://bugs.launchpad.net/qemu/+bug/1701808
#[cfg_attr(qemu, ignore)]
#[test]
fn test_scm_rights_single_cmsg_multiple_fds() {
    use nix::sys::socket::{
        recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags,
    };
    use std::io::{IoSlice, IoSliceMut};
    use std::os::unix::io::{AsRawFd, RawFd};
    use std::os::unix::net::UnixDatagram;
    use std::thread;

    let (send, receive) = UnixDatagram::pair().unwrap();
    let thread = thread::spawn(move || {
        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];

        let mut space = cmsg_space!([RawFd; 2]);
        let msg = recvmsg::<()>(
            receive.as_raw_fd(),
            &mut iovec,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .unwrap();
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));

        let mut cmsgs = msg.cmsgs();
        match cmsgs.next() {
            Some(ControlMessageOwned::ScmRights(fds)) => {
                assert_eq!(
                    fds.len(),
                    2,
                    "unexpected fd count (expected 2 fds, got {})",
                    fds.len()
                );
            }
            _ => panic!(),
        }
        assert!(cmsgs.next().is_none(), "unexpected control msg");

        assert_eq!(msg.bytes, 8);
        assert_eq!(*iovec[0], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    });

    let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let iov = [IoSlice::new(&slice)];
    let fds = [libc::STDIN_FILENO, libc::STDOUT_FILENO]; // pass stdin and stdout
    let cmsg = [ControlMessage::ScmRights(&fds)];
    sendmsg::<()>(send.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None)
        .unwrap();
    thread.join().unwrap();
}

// Verify `sendmsg` builds a valid `msghdr` when passing an empty
// `cmsgs` argument.  This should result in a msghdr with a nullptr
// msg_control field and a msg_controllen of 0 when calling into the
// raw `sendmsg`.
#[test]
pub fn test_sendmsg_empty_cmsgs() {
    use nix::sys::socket::{
        recvmsg, sendmsg, socketpair, AddressFamily, MsgFlags, SockFlag,
        SockType,
    };
    use std::io::{IoSlice, IoSliceMut};

    let (fd1, fd2) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();

    {
        let iov = [IoSlice::new(b"hello")];
        assert_eq!(
            sendmsg::<()>(fd1.as_raw_fd(), &iov, &[], MsgFlags::empty(), None)
                .unwrap(),
            5
        );
    }

    {
        let mut buf = [0u8; 5];
        let mut iov = [IoSliceMut::new(&mut buf[..])];

        let mut cmsgspace = cmsg_space!([RawFd; 1]);
        let msg = recvmsg::<()>(
            fd2.as_raw_fd(),
            &mut iov,
            Some(&mut cmsgspace),
            MsgFlags::empty(),
        )
        .unwrap();

        if msg.cmsgs().next().is_some() {
            panic!("unexpected cmsg");
        }
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        assert_eq!(msg.bytes, 5);
    }
}

#[cfg(any(linux_android, freebsdlike))]
#[test]
fn test_scm_credentials() {
    use nix::sys::socket::{
        recvmsg, sendmsg, socketpair, AddressFamily, ControlMessage,
        ControlMessageOwned, MsgFlags, SockFlag, SockType, UnixCredentials,
    };
    #[cfg(linux_android)]
    use nix::sys::socket::{setsockopt, sockopt::PassCred};
    use nix::unistd::{getgid, getpid, getuid};
    use std::io::{IoSlice, IoSliceMut};

    let (send, recv) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    #[cfg(linux_android)]
    setsockopt(&recv, PassCred, &true).unwrap();

    {
        let iov = [IoSlice::new(b"hello")];
        #[cfg(linux_android)]
        let cred = UnixCredentials::new();
        #[cfg(linux_android)]
        let cmsg = ControlMessage::ScmCredentials(&cred);
        #[cfg(freebsdlike)]
        let cmsg = ControlMessage::ScmCreds;
        assert_eq!(
            sendmsg::<()>(
                send.as_raw_fd(),
                &iov,
                &[cmsg],
                MsgFlags::empty(),
                None
            )
            .unwrap(),
            5
        );
    }

    {
        let mut buf = [0u8; 5];
        let mut iov = [IoSliceMut::new(&mut buf[..])];

        let mut cmsgspace = cmsg_space!(UnixCredentials);
        let msg = recvmsg::<()>(
            recv.as_raw_fd(),
            &mut iov,
            Some(&mut cmsgspace),
            MsgFlags::empty(),
        )
        .unwrap();
        let mut received_cred = None;

        for cmsg in msg.cmsgs() {
            let cred = match cmsg {
                #[cfg(linux_android)]
                ControlMessageOwned::ScmCredentials(cred) => cred,
                #[cfg(freebsdlike)]
                ControlMessageOwned::ScmCreds(cred) => cred,
                other => panic!("unexpected cmsg {other:?}"),
            };
            assert!(received_cred.is_none());
            assert_eq!(cred.pid(), getpid().as_raw());
            assert_eq!(cred.uid(), getuid().as_raw());
            assert_eq!(cred.gid(), getgid().as_raw());
            received_cred = Some(cred);
        }
        received_cred.expect("no creds received");
        assert_eq!(msg.bytes, 5);
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
    }
}

/// Ensure that we can send `SCM_CREDENTIALS` and `SCM_RIGHTS` with a single
/// `sendmsg` call.
#[cfg(linux_android)]
// qemu's handling of multiple cmsgs is bugged, ignore tests under emulation
// see https://bugs.launchpad.net/qemu/+bug/1781280
#[cfg_attr(qemu, ignore)]
#[test]
fn test_scm_credentials_and_rights() {
    let space = cmsg_space!(libc::ucred, RawFd);
    test_impl_scm_credentials_and_rights(space);
}

/// Ensure that passing a an oversized control message buffer to recvmsg
/// still works.
#[cfg(linux_android)]
// qemu's handling of multiple cmsgs is bugged, ignore tests under emulation
// see https://bugs.launchpad.net/qemu/+bug/1781280
#[cfg_attr(qemu, ignore)]
#[test]
fn test_too_large_cmsgspace() {
    let space = vec![0u8; 1024];
    test_impl_scm_credentials_and_rights(space);
}

#[cfg(linux_android)]
fn test_impl_scm_credentials_and_rights(mut space: Vec<u8>) {
    use libc::ucred;
    use nix::sys::socket::sockopt::PassCred;
    use nix::sys::socket::{
        recvmsg, sendmsg, setsockopt, socketpair, ControlMessage,
        ControlMessageOwned, MsgFlags, SockFlag, SockType,
    };
    use nix::unistd::{close, getgid, getpid, getuid, pipe, write};
    use std::io::{IoSlice, IoSliceMut};

    let (send, recv) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .unwrap();
    setsockopt(&recv, PassCred, &true).unwrap();

    let (r, w) = pipe().unwrap();
    let mut received_r: Option<RawFd> = None;

    {
        let iov = [IoSlice::new(b"hello")];
        let cred = ucred {
            pid: getpid().as_raw(),
            uid: getuid().as_raw(),
            gid: getgid().as_raw(),
        }
        .into();
        let fds = [r.as_raw_fd()];
        let cmsgs = [
            ControlMessage::ScmCredentials(&cred),
            ControlMessage::ScmRights(&fds),
        ];
        assert_eq!(
            sendmsg::<()>(
                send.as_raw_fd(),
                &iov,
                &cmsgs,
                MsgFlags::empty(),
                None
            )
            .unwrap(),
            5
        );
    }

    {
        let mut buf = [0u8; 5];
        let mut iov = [IoSliceMut::new(&mut buf[..])];
        let msg = recvmsg::<()>(
            recv.as_raw_fd(),
            &mut iov,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .unwrap();
        let mut received_cred = None;

        assert_eq!(msg.cmsgs().count(), 2, "expected 2 cmsgs");

        for cmsg in msg.cmsgs() {
            match cmsg {
                ControlMessageOwned::ScmRights(fds) => {
                    assert_eq!(received_r, None, "already received fd");
                    assert_eq!(fds.len(), 1);
                    received_r = Some(fds[0]);
                }
                ControlMessageOwned::ScmCredentials(cred) => {
                    assert!(received_cred.is_none());
                    assert_eq!(cred.pid(), getpid().as_raw());
                    assert_eq!(cred.uid(), getuid().as_raw());
                    assert_eq!(cred.gid(), getgid().as_raw());
                    received_cred = Some(cred);
                }
                _ => panic!("unexpected cmsg"),
            }
        }
        received_cred.expect("no creds received");
        assert_eq!(msg.bytes, 5);
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
    }

    let received_r = received_r.expect("Did not receive passed fd");
    // Ensure that the received file descriptor works
    write(&w, b"world").unwrap();
    let mut buf = [0u8; 5];
    read(received_r.as_raw_fd(), &mut buf).unwrap();
    assert_eq!(&buf[..], b"world");
    close(received_r).unwrap();
}

// Test creating and using named unix domain sockets
#[test]
pub fn test_named_unixdomain() {
    use nix::sys::socket::{
        accept, bind, connect, listen, socket, Backlog, UnixAddr,
    };
    use nix::sys::socket::{SockFlag, SockType};
    use nix::unistd::{read, write};
    use std::thread;

    let tempdir = tempfile::tempdir().unwrap();
    let sockname = tempdir.path().join("sock");
    let s1 = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");
    let sockaddr = UnixAddr::new(&sockname).unwrap();
    bind(s1.as_raw_fd(), &sockaddr).expect("bind failed");
    listen(&s1, Backlog::new(10).unwrap()).expect("listen failed");

    let thr = thread::spawn(move || {
        let s2 = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::empty(),
            None,
        )
        .expect("socket failed");
        connect(s2.as_raw_fd(), &sockaddr).expect("connect failed");
        write(&s2, b"hello").expect("write failed");
    });

    let s3 = accept(s1.as_raw_fd()).expect("accept failed");

    let mut buf = [0; 5];
    read(s3.as_raw_fd(), &mut buf).unwrap();
    thr.join().unwrap();

    assert_eq!(&buf[..], b"hello");
}

#[test]
pub fn test_listen_wrongbacklog() {
    use nix::sys::socket::Backlog;

    assert!(Backlog::new(libc::SOMAXCONN + 1).is_err());
    assert!(Backlog::new(-2).is_err());
}

// Test using unnamed unix domain addresses
#[cfg(linux_android)]
#[test]
pub fn test_unnamed_unixdomain() {
    use nix::sys::socket::{getsockname, socketpair};
    use nix::sys::socket::{SockFlag, SockType};

    let (fd_1, _fd_2) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .expect("socketpair failed");

    let addr_1: UnixAddr =
        getsockname(fd_1.as_raw_fd()).expect("getsockname failed");
    assert!(addr_1.is_unnamed());
}

// Test creating and using unnamed unix domain addresses for autobinding sockets
#[cfg(linux_android)]
#[test]
pub fn test_unnamed_unixdomain_autobind() {
    use nix::sys::socket::{bind, getsockname, socket};
    use nix::sys::socket::{SockFlag, SockType};

    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .expect("socket failed");

    // unix(7): "If a bind(2) call specifies addrlen as `sizeof(sa_family_t)`, or [...], then the
    // socket is autobound to an abstract address"
    bind(fd.as_raw_fd(), &UnixAddr::new_unnamed()).expect("bind failed");

    let addr: UnixAddr =
        getsockname(fd.as_raw_fd()).expect("getsockname failed");
    let addr = addr.as_abstract().unwrap();

    // changed from 8 to 5 bytes in Linux 2.3.15, and rust's minimum supported Linux version is 3.2
    // (as of 2022-11)
    assert_eq!(addr.len(), 5);
}

// Test creating and using named system control sockets
#[cfg(apple_targets)]
#[test]
pub fn test_syscontrol() {
    use nix::errno::Errno;
    use nix::sys::socket::{
        socket, SockFlag, SockProtocol, SockType, SysControlAddr,
    };

    let fd = socket(
        AddressFamily::System,
        SockType::Datagram,
        SockFlag::empty(),
        SockProtocol::KextControl,
    )
    .expect("socket failed");
    SysControlAddr::from_name(fd.as_raw_fd(), "com.apple.net.utun_control", 0)
        .expect("resolving sys_control name failed");
    assert_eq!(
        SysControlAddr::from_name(fd.as_raw_fd(), "foo.bar.lol", 0).err(),
        Some(Errno::ENOENT)
    );

    // requires root privileges
    // connect(fd.as_raw_fd(), &sockaddr).expect("connect failed");
}

#[cfg(any(bsd, linux_android))]
fn loopback_address(
    family: AddressFamily,
) -> Option<nix::ifaddrs::InterfaceAddress> {
    use nix::ifaddrs::getifaddrs;
    use nix::net::if_::*;
    use nix::sys::socket::SockaddrLike;
    use std::io;
    use std::io::Write;

    let mut addrs = match getifaddrs() {
        Ok(iter) => iter,
        Err(e) => {
            let stdioerr = io::stderr();
            let mut handle = stdioerr.lock();
            writeln!(handle, "getifaddrs: {e:?}").unwrap();
            return None;
        }
    };
    // return first address matching family
    addrs.find(|ifaddr| {
        ifaddr.flags.contains(InterfaceFlags::IFF_LOOPBACK)
            && ifaddr.address.as_ref().and_then(SockaddrLike::family)
                == Some(family)
    })
}

#[cfg(any(linux_android, apple_targets, target_os = "netbsd"))]
// qemu doesn't seem to be emulating this correctly in these architectures
#[cfg_attr(
    all(
        qemu,
        any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64",
        )
    ),
    ignore
)]
#[test]
pub fn test_recv_ipv4pktinfo() {
    use nix::net::if_::*;
    use nix::sys::socket::sockopt::Ipv4PacketInfo;
    use nix::sys::socket::{bind, SockFlag, SockType, SockaddrIn};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use std::io::{IoSlice, IoSliceMut};

    let lo_ifaddr = loopback_address(AddressFamily::Inet);
    let (lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (
            ifaddr.interface_name,
            ifaddr.address.expect("Expect IPv4 address on interface"),
        ),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("receive socket failed");
    bind(receive.as_raw_fd(), &lo).expect("bind failed");
    let sa: SockaddrIn =
        getsockname(receive.as_raw_fd()).expect("getsockname failed");
    setsockopt(&receive, Ipv4PacketInfo, &true).expect("setsockopt failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoSlice::new(&slice)];

        let send = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        sendmsg(send.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&sa))
            .expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];

        let mut space = cmsg_space!(libc::in_pktinfo);
        let msg = recvmsg::<()>(
            receive.as_raw_fd(),
            &mut iovec,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .expect("recvmsg failed");
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));

        let mut cmsgs = msg.cmsgs();
        if let Some(ControlMessageOwned::Ipv4PacketInfo(pktinfo)) = cmsgs.next()
        {
            let i = if_nametoindex(lo_name.as_bytes()).expect("if_nametoindex");
            assert_eq!(
                pktinfo.ipi_ifindex as libc::c_uint, i,
                "unexpected ifindex (expected {}, got {})",
                i, pktinfo.ipi_ifindex
            );
        }
        assert!(cmsgs.next().is_none(), "unexpected additional control msg");
        assert_eq!(msg.bytes, 8);
        assert_eq!(*iovec[0], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }
}

#[cfg(bsd)]
// qemu doesn't seem to be emulating this correctly in these architectures
#[cfg_attr(
    all(
        qemu,
        any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64",
        )
    ),
    ignore
)]
#[test]
pub fn test_recvif() {
    use nix::net::if_::*;
    use nix::sys::socket::sockopt::{Ipv4RecvDstAddr, Ipv4RecvIf};
    use nix::sys::socket::{bind, SockFlag, SockType, SockaddrIn};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use std::io::{IoSlice, IoSliceMut};

    let lo_ifaddr = loopback_address(AddressFamily::Inet);
    let (lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (
            ifaddr.interface_name,
            ifaddr.address.expect("Expect IPv4 address on interface"),
        ),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("receive socket failed");
    bind(receive.as_raw_fd(), &lo).expect("bind failed");
    let sa: SockaddrIn =
        getsockname(receive.as_raw_fd()).expect("getsockname failed");
    setsockopt(&receive, Ipv4RecvIf, &true)
        .expect("setsockopt IP_RECVIF failed");
    setsockopt(&receive, Ipv4RecvDstAddr, &true)
        .expect("setsockopt IP_RECVDSTADDR failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoSlice::new(&slice)];

        let send = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        sendmsg(send.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&sa))
            .expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];
        let mut space = cmsg_space!(libc::sockaddr_dl, libc::in_addr);
        let msg = recvmsg::<()>(
            receive.as_raw_fd(),
            &mut iovec,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .expect("recvmsg failed");
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        assert_eq!(msg.cmsgs().count(), 2, "expected 2 cmsgs");

        let mut rx_recvif = false;
        let mut rx_recvdstaddr = false;
        for cmsg in msg.cmsgs() {
            match cmsg {
                ControlMessageOwned::Ipv4RecvIf(dl) => {
                    rx_recvif = true;
                    let i = if_nametoindex(lo_name.as_bytes())
                        .expect("if_nametoindex");
                    assert_eq!(
                        dl.sdl_index as libc::c_uint, i,
                        "unexpected ifindex (expected {}, got {})",
                        i, dl.sdl_index
                    );
                }
                ControlMessageOwned::Ipv4RecvDstAddr(addr) => {
                    rx_recvdstaddr = true;
                    if let Some(sin) = lo.as_sockaddr_in() {
                        assert_eq!(sin.as_ref().sin_addr.s_addr,
                                   addr.s_addr,
                                   "unexpected destination address (expected {}, got {})",
                                   sin.as_ref().sin_addr.s_addr,
                                   addr.s_addr);
                    } else {
                        panic!("unexpected Sockaddr");
                    }
                }
                _ => panic!("unexpected additional control msg"),
            }
        }
        assert!(rx_recvif);
        assert!(rx_recvdstaddr);
        assert_eq!(msg.bytes, 8);
        assert_eq!(*iovec[0], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }
}

#[cfg(any(linux_android, target_os = "freebsd"))]
#[cfg_attr(qemu, ignore)]
#[test]
pub fn test_recvif_ipv4() {
    use nix::sys::socket::sockopt::Ipv4OrigDstAddr;
    use nix::sys::socket::{bind, SockFlag, SockType, SockaddrIn};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use std::io::{IoSlice, IoSliceMut};

    let lo_ifaddr = loopback_address(AddressFamily::Inet);
    let (_lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (
            ifaddr.interface_name,
            ifaddr.address.expect("Expect IPv4 address on interface"),
        ),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("receive socket failed");
    bind(receive.as_raw_fd(), &lo).expect("bind failed");
    let sa: SockaddrIn =
        getsockname(receive.as_raw_fd()).expect("getsockname failed");
    setsockopt(&receive, Ipv4OrigDstAddr, &true)
        .expect("setsockopt IP_ORIGDSTADDR failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoSlice::new(&slice)];

        let send = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        sendmsg(send.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&sa))
            .expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];
        let mut space = cmsg_space!(libc::sockaddr_in);
        let msg = recvmsg::<()>(
            receive.as_raw_fd(),
            &mut iovec,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .expect("recvmsg failed");
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        assert_eq!(msg.cmsgs().count(), 1, "expected 1 cmsgs");

        let mut rx_recvorigdstaddr = false;
        for cmsg in msg.cmsgs() {
            match cmsg {
                ControlMessageOwned::Ipv4OrigDstAddr(addr) => {
                    rx_recvorigdstaddr = true;
                    if let Some(sin) = lo.as_sockaddr_in() {
                        assert_eq!(sin.as_ref().sin_addr.s_addr,
                                   addr.sin_addr.s_addr,
                                   "unexpected destination address (expected {}, got {})",
                                   sin.as_ref().sin_addr.s_addr,
                                   addr.sin_addr.s_addr);
                    } else {
                        panic!("unexpected Sockaddr");
                    }
                }
                _ => panic!("unexpected additional control msg"),
            }
        }
        assert!(rx_recvorigdstaddr);
        assert_eq!(msg.bytes, 8);
        assert_eq!(*iovec[0], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }
}

#[cfg(any(linux_android, target_os = "freebsd"))]
#[cfg_attr(qemu, ignore)]
#[test]
pub fn test_recvif_ipv6() {
    use nix::sys::socket::sockopt::Ipv6OrigDstAddr;
    use nix::sys::socket::{bind, SockFlag, SockType, SockaddrIn6};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use std::io::{IoSlice, IoSliceMut};

    let lo_ifaddr = loopback_address(AddressFamily::Inet6);
    let (_lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (
            ifaddr.interface_name,
            ifaddr.address.expect("Expect IPv6 address on interface"),
        ),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("receive socket failed");
    bind(receive.as_raw_fd(), &lo).expect("bind failed");
    let sa: SockaddrIn6 =
        getsockname(receive.as_raw_fd()).expect("getsockname failed");
    setsockopt(&receive, Ipv6OrigDstAddr, &true)
        .expect("setsockopt IP_ORIGDSTADDR failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoSlice::new(&slice)];

        let send = socket(
            AddressFamily::Inet6,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        sendmsg(send.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&sa))
            .expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];
        let mut space = cmsg_space!(libc::sockaddr_in6);
        let msg = recvmsg::<()>(
            receive.as_raw_fd(),
            &mut iovec,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .expect("recvmsg failed");
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        assert_eq!(msg.cmsgs().count(), 1, "expected 1 cmsgs");

        let mut rx_recvorigdstaddr = false;
        for cmsg in msg.cmsgs() {
            match cmsg {
                ControlMessageOwned::Ipv6OrigDstAddr(addr) => {
                    rx_recvorigdstaddr = true;
                    if let Some(sin) = lo.as_sockaddr_in6() {
                        assert_eq!(sin.as_ref().sin6_addr.s6_addr,
                                   addr.sin6_addr.s6_addr,
                                   "unexpected destination address (expected {:?}, got {:?})",
                                   sin.as_ref().sin6_addr.s6_addr,
                                   addr.sin6_addr.s6_addr);
                    } else {
                        panic!("unexpected Sockaddr");
                    }
                }
                _ => panic!("unexpected additional control msg"),
            }
        }
        assert!(rx_recvorigdstaddr);
        assert_eq!(msg.bytes, 8);
        assert_eq!(*iovec[0], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }
}

#[cfg(any(linux_android, target_os = "freebsd", apple_targets, netbsdlike))]
// qemu doesn't seem to be emulating this correctly in these architectures
#[cfg_attr(
    all(
        qemu,
        any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64",
        )
    ),
    ignore
)]
#[test]
pub fn test_recv_ipv6pktinfo() {
    use nix::net::if_::*;
    use nix::sys::socket::sockopt::Ipv6RecvPacketInfo;
    use nix::sys::socket::{bind, SockFlag, SockType, SockaddrIn6};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use std::io::{IoSlice, IoSliceMut};

    let lo_ifaddr = loopback_address(AddressFamily::Inet6);
    let (lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (
            ifaddr.interface_name,
            ifaddr.address.expect("Expect IPv6 address on interface"),
        ),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("receive socket failed");
    bind(receive.as_raw_fd(), &lo).expect("bind failed");
    let sa: SockaddrIn6 =
        getsockname(receive.as_raw_fd()).expect("getsockname failed");
    setsockopt(&receive, Ipv6RecvPacketInfo, &true).expect("setsockopt failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoSlice::new(&slice)];

        let send = socket(
            AddressFamily::Inet6,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        )
        .expect("send socket failed");
        sendmsg(send.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&sa))
            .expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];

        let mut space = cmsg_space!(libc::in6_pktinfo);
        let msg = recvmsg::<()>(
            receive.as_raw_fd(),
            &mut iovec,
            Some(&mut space),
            MsgFlags::empty(),
        )
        .expect("recvmsg failed");
        assert!(!msg
            .flags
            .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));

        let mut cmsgs = msg.cmsgs();
        if let Some(ControlMessageOwned::Ipv6PacketInfo(pktinfo)) = cmsgs.next()
        {
            let i = if_nametoindex(lo_name.as_bytes()).expect("if_nametoindex");
            assert_eq!(
                pktinfo.ipi6_ifindex as libc::c_uint, i,
                "unexpected ifindex (expected {}, got {})",
                i, pktinfo.ipi6_ifindex
            );
        }
        assert!(cmsgs.next().is_none(), "unexpected additional control msg");
        assert_eq!(msg.bytes, 8);
        assert_eq!(*iovec[0], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }
}

#[cfg(linux_android)]
#[test]
pub fn test_vsock() {
    use nix::sys::socket::SockaddrLike;
    use nix::sys::socket::{AddressFamily, VsockAddr};
    use std::mem;

    let port: u32 = 3000;

    let addr_local = VsockAddr::new(libc::VMADDR_CID_LOCAL, port);
    assert_eq!(addr_local.cid(), libc::VMADDR_CID_LOCAL);
    assert_eq!(addr_local.port(), port);

    let addr_any = VsockAddr::new(libc::VMADDR_CID_ANY, libc::VMADDR_PORT_ANY);
    assert_eq!(addr_any.cid(), libc::VMADDR_CID_ANY);
    assert_eq!(addr_any.port(), libc::VMADDR_PORT_ANY);

    assert_ne!(addr_local, addr_any);
    assert_ne!(calculate_hash(&addr_local), calculate_hash(&addr_any));

    let addr1 = VsockAddr::new(libc::VMADDR_CID_HOST, port);
    let addr2 = VsockAddr::new(libc::VMADDR_CID_HOST, port);
    assert_eq!(addr1, addr2);
    assert_eq!(calculate_hash(&addr1), calculate_hash(&addr2));

    let addr3 = unsafe {
        VsockAddr::from_raw(
            addr2.as_ref() as *const libc::sockaddr_vm as *const libc::sockaddr,
            Some(mem::size_of::<libc::sockaddr_vm>().try_into().unwrap()),
        )
    }
    .unwrap();
    assert_eq!(
        addr3.as_ref().svm_family,
        AddressFamily::Vsock as libc::sa_family_t
    );
    assert_eq!(addr3.as_ref().svm_cid, addr1.cid());
    assert_eq!(addr3.as_ref().svm_port, addr1.port());
}

#[cfg(apple_targets)]
#[test]
pub fn test_vsock() {
    use nix::sys::socket::SockaddrLike;
    use nix::sys::socket::{AddressFamily, VsockAddr};
    use std::mem;

    let port: u32 = 3000;

    // macOS doesn't have a VMADDR_CID_LOCAL, so test with host again
    let addr_host = VsockAddr::new(libc::VMADDR_CID_HOST, port);
    assert_eq!(addr_host.cid(), libc::VMADDR_CID_HOST);
    assert_eq!(addr_host.port(), port);

    let addr_any = VsockAddr::new(libc::VMADDR_CID_ANY, libc::VMADDR_PORT_ANY);
    assert_eq!(addr_any.cid(), libc::VMADDR_CID_ANY);
    assert_eq!(addr_any.port(), libc::VMADDR_PORT_ANY);

    assert_ne!(addr_host, addr_any);
    assert_ne!(calculate_hash(&addr_host), calculate_hash(&addr_any));

    let addr1 = VsockAddr::new(libc::VMADDR_CID_HOST, port);
    let addr2 = VsockAddr::new(libc::VMADDR_CID_HOST, port);
    assert_eq!(addr1, addr2);
    assert_eq!(calculate_hash(&addr1), calculate_hash(&addr2));

    let addr3 = unsafe {
        VsockAddr::from_raw(
            addr2.as_ref() as *const libc::sockaddr_vm as *const libc::sockaddr,
            Some(mem::size_of::<libc::sockaddr_vm>().try_into().unwrap()),
        )
    }
    .unwrap();
    assert_eq!(
        addr3.as_ref().svm_family,
        AddressFamily::Vsock as libc::sa_family_t
    );
    let cid = addr3.as_ref().svm_cid;
    let port = addr3.as_ref().svm_port;
    assert_eq!(cid, addr1.cid());
    assert_eq!(port, addr1.port());
}

// Disable the test on emulated platforms because it fails in Cirrus-CI.  Lack
// of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
#[cfg(target_os = "linux")]
#[test]
fn test_recvmsg_timestampns() {
    use nix::sys::socket::*;
    use nix::sys::time::*;
    use std::io::{IoSlice, IoSliceMut};
    use std::time::*;

    // Set up
    let message = "Ohayō!".as_bytes();
    let in_socket = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&in_socket, sockopt::ReceiveTimestampns, &true).unwrap();
    let localhost = SockaddrIn::new(127, 0, 0, 1, 0);
    bind(in_socket.as_raw_fd(), &localhost).unwrap();
    let address: SockaddrIn = getsockname(in_socket.as_raw_fd()).unwrap();
    // Get initial time
    let time0 = SystemTime::now();
    // Send the message
    let iov = [IoSlice::new(message)];
    let flags = MsgFlags::empty();
    let l = sendmsg(in_socket.as_raw_fd(), &iov, &[], flags, Some(&address))
        .unwrap();
    assert_eq!(message.len(), l);
    // Receive the message
    let mut buffer = vec![0u8; message.len()];
    let mut cmsgspace = nix::cmsg_space!(TimeSpec);

    let mut iov = [IoSliceMut::new(&mut buffer)];
    let r = recvmsg::<()>(
        in_socket.as_raw_fd(),
        &mut iov,
        Some(&mut cmsgspace),
        flags,
    )
    .unwrap();
    let rtime = match r.cmsgs().next() {
        Some(ControlMessageOwned::ScmTimestampns(rtime)) => rtime,
        Some(_) => panic!("Unexpected control message"),
        None => panic!("No control message"),
    };
    // Check the final time
    let time1 = SystemTime::now();
    // the packet's received timestamp should lie in-between the two system
    // times, unless the system clock was adjusted in the meantime.
    let rduration =
        Duration::new(rtime.tv_sec() as u64, rtime.tv_nsec() as u32);
    assert!(time0.duration_since(UNIX_EPOCH).unwrap() <= rduration);
    assert!(rduration <= time1.duration_since(UNIX_EPOCH).unwrap());
}

// Disable the test on emulated platforms because it fails in Cirrus-CI.  Lack
// of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
#[cfg(target_os = "linux")]
#[test]
fn test_recvmmsg_timestampns() {
    use nix::sys::socket::*;
    use nix::sys::time::*;
    use std::io::{IoSlice, IoSliceMut};
    use std::time::*;

    // Set up
    let message = "Ohayō!".as_bytes();
    let in_socket = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    setsockopt(&in_socket, sockopt::ReceiveTimestampns, &true).unwrap();
    let localhost = SockaddrIn::from_str("127.0.0.1:0").unwrap();
    bind(in_socket.as_raw_fd(), &localhost).unwrap();
    let address: SockaddrIn = getsockname(in_socket.as_raw_fd()).unwrap();
    // Get initial time
    let time0 = SystemTime::now();
    // Send the message
    let iov = [IoSlice::new(message)];
    let flags = MsgFlags::empty();
    let l = sendmsg(in_socket.as_raw_fd(), &iov, &[], flags, Some(&address))
        .unwrap();
    assert_eq!(message.len(), l);
    // Receive the message
    let mut buffer = vec![0u8; message.len()];
    let cmsgspace = nix::cmsg_space!(TimeSpec);
    let mut iov = vec![[IoSliceMut::new(&mut buffer)]];
    let mut data = MultiHeaders::preallocate(1, Some(cmsgspace));
    let r: Vec<RecvMsg<()>> = recvmmsg(
        in_socket.as_raw_fd(),
        &mut data,
        iov.iter_mut(),
        flags,
        None,
    )
    .unwrap()
    .collect();
    let rtime = match r[0].cmsgs().next() {
        Some(ControlMessageOwned::ScmTimestampns(rtime)) => rtime,
        Some(_) => panic!("Unexpected control message"),
        None => panic!("No control message"),
    };
    // Check the final time
    let time1 = SystemTime::now();
    // the packet's received timestamp should lie in-between the two system
    // times, unless the system clock was adjusted in the meantime.
    let rduration =
        Duration::new(rtime.tv_sec() as u64, rtime.tv_nsec() as u32);
    assert!(time0.duration_since(UNIX_EPOCH).unwrap() <= rduration);
    assert!(rduration <= time1.duration_since(UNIX_EPOCH).unwrap());
}

// Disable the test on emulated platforms because it fails in Cirrus-CI.  Lack
// of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
#[cfg(any(linux_android, target_os = "fuchsia"))]
#[test]
fn test_recvmsg_rxq_ovfl() {
    use nix::sys::socket::sockopt::{RcvBuf, RxqOvfl};
    use nix::sys::socket::*;
    use nix::Error;
    use std::io::{IoSlice, IoSliceMut};

    let message = [0u8; 2048];
    let bufsize = message.len() * 2;

    let in_socket = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    let out_socket = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    let localhost = SockaddrIn::from_str("127.0.0.1:0").unwrap();
    bind(in_socket.as_raw_fd(), &localhost).unwrap();

    let address: SockaddrIn = getsockname(in_socket.as_raw_fd()).unwrap();
    connect(out_socket.as_raw_fd(), &address).unwrap();

    // Set SO_RXQ_OVFL flag.
    setsockopt(&in_socket, RxqOvfl, &1).unwrap();

    // Set the receiver buffer size to hold only 2 messages.
    setsockopt(&in_socket, RcvBuf, &bufsize).unwrap();

    let mut drop_counter = 0;

    for _ in 0..2 {
        let iov = [IoSlice::new(&message)];
        let flags = MsgFlags::empty();

        // Send the 3 messages (the receiver buffer can only hold 2 messages)
        // to create an overflow.
        for _ in 0..3 {
            let l = sendmsg(
                out_socket.as_raw_fd(),
                &iov,
                &[],
                flags,
                Some(&address),
            )
            .unwrap();
            assert_eq!(message.len(), l);
        }

        // Receive the message and check the drop counter if any.
        loop {
            let mut buffer = vec![0u8; message.len()];
            let mut cmsgspace = nix::cmsg_space!(u32);

            let mut iov = [IoSliceMut::new(&mut buffer)];

            match recvmsg::<()>(
                in_socket.as_raw_fd(),
                &mut iov,
                Some(&mut cmsgspace),
                MsgFlags::MSG_DONTWAIT,
            ) {
                Ok(r) => {
                    drop_counter = match r.cmsgs().next() {
                        Some(ControlMessageOwned::RxqOvfl(drop_counter)) => {
                            drop_counter
                        }
                        Some(_) => panic!("Unexpected control message"),
                        None => 0,
                    };
                }
                Err(Error::EAGAIN) => {
                    break;
                }
                _ => {
                    panic!("unknown recvmsg() error");
                }
            }
        }
    }

    // One packet lost.
    assert_eq!(drop_counter, 1);
}

#[cfg(linux_android)]
mod linux_errqueue {
    use super::FromStr;
    use nix::sys::socket::*;
    use std::os::unix::io::AsRawFd;

    // Send a UDP datagram to a bogus destination address and observe an ICMP error (v4).
    //
    // Disable the test on QEMU because QEMU emulation of IP_RECVERR is broken (as documented on PR
    // #1514).
    #[cfg_attr(qemu, ignore)]
    #[test]
    fn test_recverr_v4() {
        #[repr(u8)]
        enum IcmpTypes {
            DestUnreach = 3, // ICMP_DEST_UNREACH
        }
        #[repr(u8)]
        enum IcmpUnreachCodes {
            PortUnreach = 3, // ICMP_PORT_UNREACH
        }

        test_recverr_impl::<sockaddr_in, _, _>(
            "127.0.0.1:6800",
            AddressFamily::Inet,
            sockopt::Ipv4RecvErr,
            libc::SO_EE_ORIGIN_ICMP,
            IcmpTypes::DestUnreach as u8,
            IcmpUnreachCodes::PortUnreach as u8,
            // Closure handles protocol-specific testing and returns generic sock_extended_err for
            // protocol-independent test impl.
            |cmsg| {
                if let ControlMessageOwned::Ipv4RecvErr(ext_err, err_addr) =
                    cmsg
                {
                    if let Some(origin) = err_addr {
                        // Validate that our network error originated from 127.0.0.1:0.
                        assert_eq!(origin.sin_family, AddressFamily::Inet as _);
                        assert_eq!(
                            origin.sin_addr.s_addr,
                            u32::from_be(0x7f000001)
                        );
                        assert_eq!(origin.sin_port, 0);
                    } else {
                        panic!("Expected some error origin");
                    }
                    *ext_err
                } else {
                    panic!("Unexpected control message {cmsg:?}");
                }
            },
        )
    }

    // Essentially the same test as v4.
    //
    // Disable the test on QEMU because QEMU emulation of IPV6_RECVERR is broken (as documented on
    // PR #1514).
    #[cfg_attr(qemu, ignore)]
    #[test]
    fn test_recverr_v6() {
        #[repr(u8)]
        enum IcmpV6Types {
            DestUnreach = 1, // ICMPV6_DEST_UNREACH
        }
        #[repr(u8)]
        enum IcmpV6UnreachCodes {
            PortUnreach = 4, // ICMPV6_PORT_UNREACH
        }

        test_recverr_impl::<sockaddr_in6, _, _>(
            "[::1]:6801",
            AddressFamily::Inet6,
            sockopt::Ipv6RecvErr,
            libc::SO_EE_ORIGIN_ICMP6,
            IcmpV6Types::DestUnreach as u8,
            IcmpV6UnreachCodes::PortUnreach as u8,
            // Closure handles protocol-specific testing and returns generic sock_extended_err for
            // protocol-independent test impl.
            |cmsg| {
                if let ControlMessageOwned::Ipv6RecvErr(ext_err, err_addr) =
                    cmsg
                {
                    if let Some(origin) = err_addr {
                        // Validate that our network error originated from localhost:0.
                        assert_eq!(
                            origin.sin6_family,
                            AddressFamily::Inet6 as _
                        );
                        assert_eq!(
                            origin.sin6_addr.s6_addr,
                            std::net::Ipv6Addr::LOCALHOST.octets()
                        );
                        assert_eq!(origin.sin6_port, 0);
                    } else {
                        panic!("Expected some error origin");
                    }
                    *ext_err
                } else {
                    panic!("Unexpected control message {cmsg:?}");
                }
            },
        )
    }

    fn test_recverr_impl<SA, OPT, TESTF>(
        sa: &str,
        af: AddressFamily,
        opt: OPT,
        ee_origin: u8,
        ee_type: u8,
        ee_code: u8,
        testf: TESTF,
    ) where
        OPT: SetSockOpt<Val = bool>,
        TESTF: FnOnce(&ControlMessageOwned) -> libc::sock_extended_err,
    {
        use nix::errno::Errno;
        use std::io::IoSliceMut;

        const MESSAGE_CONTENTS: &str = "ABCDEF";
        let std_sa = std::net::SocketAddr::from_str(sa).unwrap();
        let sock_addr = SockaddrStorage::from(std_sa);
        let sock = socket(af, SockType::Datagram, SockFlag::SOCK_CLOEXEC, None)
            .unwrap();
        setsockopt(&sock, opt, &true).unwrap();
        if let Err(e) = sendto(
            sock.as_raw_fd(),
            MESSAGE_CONTENTS.as_bytes(),
            &sock_addr,
            MsgFlags::empty(),
        ) {
            assert_eq!(e, Errno::EADDRNOTAVAIL);
            println!("{af:?} not available, skipping test.");
            return;
        }

        let mut buf = [0u8; 8];
        let mut iovec = [IoSliceMut::new(&mut buf)];
        let mut cspace = cmsg_space!(libc::sock_extended_err, SA);

        let msg = recvmsg(
            sock.as_raw_fd(),
            &mut iovec,
            Some(&mut cspace),
            MsgFlags::MSG_ERRQUEUE,
        )
        .unwrap();
        // The sent message / destination associated with the error is returned:
        assert_eq!(msg.bytes, MESSAGE_CONTENTS.as_bytes().len());
        // recvmsg(2): "The original destination address of the datagram that caused the error is
        // supplied via msg_name;" however, this is not literally true.  E.g., an earlier version
        // of this test used 0.0.0.0 (::0) as the destination address, which was mutated into
        // 127.0.0.1 (::1).
        assert_eq!(msg.address, Some(sock_addr));

        // Check for expected control message.
        let ext_err = match msg.cmsgs().next() {
            Some(cmsg) => testf(&cmsg),
            None => panic!("No control message"),
        };

        assert_eq!(ext_err.ee_errno, libc::ECONNREFUSED as u32);
        assert_eq!(ext_err.ee_origin, ee_origin);
        // ip(7): ee_type and ee_code are set from the type and code fields of the ICMP (ICMPv6)
        // header.
        assert_eq!(ext_err.ee_type, ee_type);
        assert_eq!(ext_err.ee_code, ee_code);
        // ip(7): ee_info contains the discovered MTU for EMSGSIZE errors.
        assert_eq!(ext_err.ee_info, 0);

        let bytes = msg.bytes;
        assert_eq!(&buf[..bytes], MESSAGE_CONTENTS.as_bytes());
    }
}

// Disable the test on emulated platforms because it fails in Cirrus-CI.  Lack
// of QEMU support is suspected.
#[cfg_attr(qemu, ignore)]
#[cfg(target_os = "linux")]
#[test]
pub fn test_txtime() {
    use nix::sys::socket::{
        bind, recvmsg, sendmsg, setsockopt, socket, sockopt, ControlMessage,
        MsgFlags, SockFlag, SockType, SockaddrIn,
    };
    use nix::sys::time::TimeValLike;
    use nix::time::{clock_gettime, ClockId};

    require_kernel_version!(test_txtime, ">= 5.8");

    let sock_addr = SockaddrIn::from_str("127.0.0.1:6802").unwrap();

    let ssock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .expect("send socket failed");

    let txtime_cfg = libc::sock_txtime {
        clockid: libc::CLOCK_MONOTONIC,
        flags: 0,
    };
    setsockopt(&ssock, sockopt::TxTime, &txtime_cfg).unwrap();

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .unwrap();
    bind(rsock.as_raw_fd(), &sock_addr).unwrap();

    let sbuf = [0u8; 2048];
    let iov1 = [std::io::IoSlice::new(&sbuf)];

    let now = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    let delay = std::time::Duration::from_secs(1).into();
    let txtime = (now + delay).num_nanoseconds() as u64;

    let cmsg = ControlMessage::TxTime(&txtime);
    sendmsg(
        ssock.as_raw_fd(),
        &iov1,
        &[cmsg],
        MsgFlags::empty(),
        Some(&sock_addr),
    )
    .unwrap();

    let mut rbuf = [0u8; 2048];
    let mut iov2 = [std::io::IoSliceMut::new(&mut rbuf)];
    recvmsg::<()>(rsock.as_raw_fd(), &mut iov2, None, MsgFlags::empty())
        .unwrap();
}

// cfg needed for capability check.
#[cfg(linux_android)]
#[test]
fn test_icmp_protocol() {
    use nix::sys::socket::{
        sendto, socket, AddressFamily, MsgFlags, SockFlag, SockProtocol,
        SockType, SockaddrIn,
    };

    require_capability!("test_icmp_protocol", CAP_NET_RAW);

    let owned_fd = socket(
        AddressFamily::Inet,
        SockType::Raw,
        SockFlag::empty(),
        SockProtocol::Icmp,
    )
    .unwrap();

    // Send a minimal ICMP packet with no payload.
    let packet = [
        0x08, // Type
        0x00, // Code
        0x84, 0x85, // Checksum
        0x73, 0x8a, // ID
        0x00, 0x00, // Sequence Number
    ];

    let dest_addr = SockaddrIn::new(127, 0, 0, 1, 0);
    sendto(owned_fd.as_raw_fd(), &packet, &dest_addr, MsgFlags::empty())
        .unwrap();
}

// test contains both recvmmsg and timestaping which is linux only
// there are existing tests for recvmmsg only in tests/
#[cfg_attr(qemu, ignore)]
#[cfg(target_os = "linux")]
#[test]
fn test_recvmm2() -> nix::Result<()> {
    use nix::sys::{
        socket::{
            bind, recvmmsg, sendmsg, setsockopt, socket, sockopt::Timestamping,
            AddressFamily, ControlMessageOwned, MsgFlags, MultiHeaders,
            SockFlag, SockType, SockaddrIn, TimestampingFlag, Timestamps,
        },
        time::TimeSpec,
    };
    use std::io::{IoSlice, IoSliceMut};
    use std::os::unix::io::AsRawFd;
    use std::str::FromStr;

    let sock_addr = SockaddrIn::from_str("127.0.0.1:6790").unwrap();

    let ssock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )?;

    let rsock = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::SOCK_NONBLOCK,
        None,
    )?;

    bind(rsock.as_raw_fd(), &sock_addr)?;

    setsockopt(&rsock, Timestamping, &TimestampingFlag::all())?;

    let sbuf = (0..400).map(|i| i as u8).collect::<Vec<_>>();

    let mut recv_buf = vec![0; 1024];

    let mut recv_iovs = Vec::new();
    let mut pkt_iovs = Vec::new();

    for (ix, chunk) in recv_buf.chunks_mut(256).enumerate() {
        pkt_iovs.push(IoSliceMut::new(chunk));
        if ix % 2 == 1 {
            recv_iovs.push(pkt_iovs);
            pkt_iovs = Vec::new();
        }
    }
    drop(pkt_iovs);

    let flags = MsgFlags::empty();
    let iov1 = [IoSlice::new(&sbuf)];

    let cmsg = cmsg_space!(Timestamps);
    sendmsg(ssock.as_raw_fd(), &iov1, &[], flags, Some(&sock_addr)).unwrap();

    let mut data = MultiHeaders::<()>::preallocate(recv_iovs.len(), Some(cmsg));

    let t = TimeSpec::from_duration(std::time::Duration::from_secs(10));

    let recv = recvmmsg(
        rsock.as_raw_fd(),
        &mut data,
        recv_iovs.iter_mut(),
        flags,
        Some(t),
    )?;

    for rmsg in recv {
        #[cfg(not(any(qemu, target_arch = "aarch64")))]
        let mut saw_time = false;
        let mut recvd = 0;
        for cmsg in rmsg.cmsgs() {
            if let ControlMessageOwned::ScmTimestampsns(timestamps) = cmsg {
                let ts = timestamps.system;

                let sys_time = nix::time::clock_gettime(
                    nix::time::ClockId::CLOCK_REALTIME,
                )?;
                let diff = if ts > sys_time {
                    ts - sys_time
                } else {
                    sys_time - ts
                };
                assert!(std::time::Duration::from(diff).as_secs() < 60);
                #[cfg(not(any(qemu, target_arch = "aarch64")))]
                {
                    saw_time = true;
                }
            }
        }

        #[cfg(not(any(qemu, target_arch = "aarch64")))]
        assert!(saw_time);

        for iov in rmsg.iovs() {
            recvd += iov.len();
        }
        assert_eq!(recvd, 400);
    }

    Ok(())
}

#[cfg(not(target_os = "redox"))]
#[test]
fn can_use_cmsg_space() {
    let _ = cmsg_space!(u8);
}

#[cfg(not(any(linux_android, target_os = "redox", target_os = "haiku")))]
#[test]
fn can_open_routing_socket() {
    use nix::sys::socket::{socket, AddressFamily, SockFlag, SockType};

    let _ =
        socket(AddressFamily::Route, SockType::Raw, SockFlag::empty(), None)
            .expect("Failed to open routing socket");
}
