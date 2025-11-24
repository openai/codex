//! Socket interface functions
//!
//! [Further reading](https://man7.org/linux/man-pages/man7/socket.7.html)
#[cfg(any(target_os = "freebsd", linux_android))]
#[cfg(feature = "uio")]
use crate::sys::time::TimeSpec;
#[cfg(not(target_os = "redox"))]
#[cfg(feature = "uio")]
use crate::sys::time::TimeVal;
use crate::{errno::Errno, Result};
use cfg_if::cfg_if;
use libc::{self, c_int, size_t, socklen_t};
#[cfg(all(feature = "uio", not(target_os = "redox")))]
use libc::{
    c_void, iovec, CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, CMSG_NXTHDR, CMSG_SPACE,
};
#[cfg(not(target_os = "redox"))]
use std::io::{IoSlice, IoSliceMut};
#[cfg(feature = "net")]
use std::net;
use std::os::unix::io::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::{mem, ptr};

#[deny(missing_docs)]
mod addr;
#[deny(missing_docs)]
pub mod sockopt;

/*
 *
 * ===== Re-exports =====
 *
 */

pub use self::addr::{SockaddrLike, SockaddrStorage};

#[cfg(solarish)]
pub use self::addr::{AddressFamily, UnixAddr};
#[cfg(not(solarish))]
pub use self::addr::{AddressFamily, UnixAddr};
#[cfg(not(any(solarish, target_os = "haiku", target_os = "hurd", target_os = "redox")))]
#[cfg(feature = "net")]
pub use self::addr::{LinkAddr, SockaddrIn, SockaddrIn6};
#[cfg(any(solarish, target_os = "haiku", target_os = "hurd", target_os = "redox"))]
#[cfg(feature = "net")]
pub use self::addr::{SockaddrIn, SockaddrIn6};

#[cfg(linux_android)]
pub use crate::sys::socket::addr::alg::AlgAddr;
#[cfg(linux_android)]
pub use crate::sys::socket::addr::netlink::NetlinkAddr;
#[cfg(apple_targets)]
#[cfg(feature = "ioctl")]
pub use crate::sys::socket::addr::sys_control::SysControlAddr;
#[cfg(any(linux_android, apple_targets))]
pub use crate::sys::socket::addr::vsock::VsockAddr;

#[cfg(all(feature = "uio", not(target_os = "redox")))]
pub use libc::{cmsghdr, msghdr};
pub use libc::{sa_family_t, sockaddr, sockaddr_storage, sockaddr_un};
#[cfg(feature = "net")]
pub use libc::{sockaddr_in, sockaddr_in6};

#[cfg(feature = "net")]
use crate::sys::socket::addr::{ipv4addr_to_libc, ipv6addr_to_libc};

/// These constants are used to specify the communication semantics
/// when creating a socket with [`socket()`](fn.socket.html)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
#[non_exhaustive]
pub enum SockType {
    /// Provides sequenced, reliable, two-way, connection-
    /// based byte streams.  An out-of-band data transmission
    /// mechanism may be supported.
    Stream = libc::SOCK_STREAM,
    /// Supports datagrams (connectionless, unreliable
    /// messages of a fixed maximum length).
    Datagram = libc::SOCK_DGRAM,
    /// Provides a sequenced, reliable, two-way connection-
    /// based data transmission path for datagrams of fixed
    /// maximum length; a consumer is required to read an
    /// entire packet with each input system call.
    SeqPacket = libc::SOCK_SEQPACKET,
    /// Provides raw network protocol access.
    #[cfg(not(target_os = "redox"))]
    Raw = libc::SOCK_RAW,
    /// Provides a reliable datagram layer that does not
    /// guarantee ordering.
    #[cfg(not(any(target_os = "haiku", target_os = "redox")))]
    Rdm = libc::SOCK_RDM,
}
// The TryFrom impl could've been derived using libc_enum!.  But for
// backwards-compatibility with Nix-0.25.0 we manually implement it, so as to
// keep the old variant names.
impl TryFrom<i32> for SockType {
    type Error = crate::Error;

    fn try_from(x: i32) -> Result<Self> {
        match x {
            libc::SOCK_STREAM => Ok(Self::Stream),
            libc::SOCK_DGRAM => Ok(Self::Datagram),
            libc::SOCK_SEQPACKET => Ok(Self::SeqPacket),
            #[cfg(not(target_os = "redox"))]
            libc::SOCK_RAW => Ok(Self::Raw),
            #[cfg(not(any(target_os = "haiku", target_os = "redox")))]
            libc::SOCK_RDM => Ok(Self::Rdm),
            _ => Err(Errno::EINVAL),
        }
    }
}

/// Constants used in [`socket`](fn.socket.html) and [`socketpair`](fn.socketpair.html)
/// to specify the protocol to use.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum SockProtocol {
    /// TCP protocol ([ip(7)](https://man7.org/linux/man-pages/man7/ip.7.html))
    Tcp = libc::IPPROTO_TCP,
    /// UDP protocol ([ip(7)](https://man7.org/linux/man-pages/man7/ip.7.html))
    Udp = libc::IPPROTO_UDP,
    /// Raw sockets ([raw(7)](https://man7.org/linux/man-pages/man7/raw.7.html))
    Raw = libc::IPPROTO_RAW,
    /// Allows applications to configure and control a KEXT
    /// ([ref](https://developer.apple.com/library/content/documentation/Darwin/Conceptual/NKEConceptual/control/control.html))
    #[cfg(apple_targets)]
    KextControl = libc::SYSPROTO_CONTROL,
    /// Receives routing and link updates and may be used to modify the routing tables (both IPv4 and IPv6), IP addresses, link
    // parameters, neighbor setups, queueing disciplines, traffic classes and packet classifiers
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkRoute = libc::NETLINK_ROUTE,
    /// Reserved for user-mode socket protocols
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkUserSock = libc::NETLINK_USERSOCK,
    /// Query information about sockets of various protocol families from the kernel
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkSockDiag = libc::NETLINK_SOCK_DIAG,
    /// Netfilter/iptables ULOG.
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkNFLOG = libc::NETLINK_NFLOG,
    /// SELinux event notifications.
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkSELinux = libc::NETLINK_SELINUX,
    /// Open-iSCSI
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkISCSI = libc::NETLINK_ISCSI,
    /// Auditing
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkAudit = libc::NETLINK_AUDIT,
    /// Access to FIB lookup from user space
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkFIBLookup = libc::NETLINK_FIB_LOOKUP,
    /// Netfilter subsystem
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkNetFilter = libc::NETLINK_NETFILTER,
    /// SCSI Transports
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkSCSITransport = libc::NETLINK_SCSITRANSPORT,
    /// Infiniband RDMA
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkRDMA = libc::NETLINK_RDMA,
    /// Transport IPv6 packets from netfilter to user space.  Used by ip6_queue kernel module.
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkIPv6Firewall = libc::NETLINK_IP6_FW,
    /// DECnet routing messages
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkDECNetRoutingMessage = libc::NETLINK_DNRTMSG,
    /// Kernel messages to user space
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkKObjectUEvent = libc::NETLINK_KOBJECT_UEVENT,
    /// Generic netlink family for simplified netlink usage.
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkGeneric = libc::NETLINK_GENERIC,
    /// Netlink interface to request information about ciphers registered with the kernel crypto API as well as allow
    /// configuration of the kernel crypto API.
    /// ([ref](https://www.man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(linux_android)]
    NetlinkCrypto = libc::NETLINK_CRYPTO,
    /// Non-DIX type protocol number defined for the Ethernet IEEE 802.3 interface that allows packets of all protocols
    /// defined in the interface to be received.
    /// ([ref](https://man7.org/linux/man-pages/man7/packet.7.html))
    // The protocol number is fed into the socket syscall in network byte order.
    #[cfg(linux_android)]
    EthAll = (libc::ETH_P_ALL as u16).to_be() as i32,
    /// ICMP protocol ([icmp(7)](https://man7.org/linux/man-pages/man7/icmp.7.html))
    Icmp = libc::IPPROTO_ICMP,
    /// ICMPv6 protocol (ICMP over IPv6)
    IcmpV6 = libc::IPPROTO_ICMPV6,
}

impl SockProtocol {
    /// The Controller Area Network raw socket protocol
    /// ([ref](https://docs.kernel.org/networking/can.html#how-to-use-socketcan))
    #[cfg(target_os = "linux")]
    #[allow(non_upper_case_globals)]
    pub const CanRaw: SockProtocol = SockProtocol::Icmp; // Matches libc::CAN_RAW

    /// The Controller Area Network broadcast manager protocol
    /// ([ref](https://docs.kernel.org/networking/can.html#how-to-use-socketcan))
    #[cfg(target_os = "linux")]
    #[allow(non_upper_case_globals)]
    pub const CanBcm: SockProtocol = SockProtocol::NetlinkUserSock; // Matches libc::CAN_BCM

    /// Allows applications and other KEXTs to be notified when certain kernel events occur
    /// ([ref](https://developer.apple.com/library/content/documentation/Darwin/Conceptual/NKEConceptual/control/control.html))
    #[cfg(apple_targets)]
    #[allow(non_upper_case_globals)]
    pub const KextEvent: SockProtocol = SockProtocol::Icmp; // Matches libc::SYSPROTO_EVENT
}
#[cfg(linux_android)]
libc_bitflags! {
    /// Configuration flags for `SO_TIMESTAMPING` interface
    ///
    /// For use with [`Timestamping`][sockopt::Timestamping].
    /// [Further reading](https://www.kernel.org/doc/html/latest/networking/timestamping.html)
    pub struct TimestampingFlag: libc::c_uint {
        /// Report any software timestamps when available.
        SOF_TIMESTAMPING_SOFTWARE;
        /// Report hardware timestamps as generated by SOF_TIMESTAMPING_TX_HARDWARE when available.
        SOF_TIMESTAMPING_RAW_HARDWARE;
        /// Collect transmitting timestamps as reported by hardware
        SOF_TIMESTAMPING_TX_HARDWARE;
        /// Collect transmitting timestamps as reported by software
        SOF_TIMESTAMPING_TX_SOFTWARE;
        /// Collect receiving timestamps as reported by hardware
        SOF_TIMESTAMPING_RX_HARDWARE;
        /// Collect receiving timestamps as reported by software
        SOF_TIMESTAMPING_RX_SOFTWARE;
        /// Generate a unique identifier along with each transmitted packet
        SOF_TIMESTAMPING_OPT_ID;
        /// Return transmit timestamps alongside an empty packet instead of the original packet
        SOF_TIMESTAMPING_OPT_TSONLY;
    }
}

libc_bitflags! {
    /// Additional socket options
    pub struct SockFlag: c_int {
        /// Set non-blocking mode on the new socket
        #[cfg(any(linux_android,
                  freebsdlike,
                  netbsdlike,
                  solarish))]
        SOCK_NONBLOCK;
        /// Set close-on-exec on the new descriptor
        #[cfg(any(linux_android,
                  freebsdlike,
                  netbsdlike,
                  solarish))]
        SOCK_CLOEXEC;
        /// Return `EPIPE` instead of raising `SIGPIPE`
        #[cfg(target_os = "netbsd")]
        SOCK_NOSIGPIPE;
        /// For domains `AF_INET(6)`, only allow `connect(2)`, `sendto(2)`, or `sendmsg(2)`
        /// to the DNS port (typically 53)
        #[cfg(target_os = "openbsd")]
        SOCK_DNS;
    }
}

libc_bitflags! {
    /// Flags for send/recv and their relatives
    pub struct MsgFlags: c_int {
        /// Sends or requests out-of-band data on sockets that support this notion
        /// (e.g., of type [`Stream`](enum.SockType.html)); the underlying protocol must also
        /// support out-of-band data.
        MSG_OOB;
        /// Peeks at an incoming message. The data is treated as unread and the next
        /// [`recv()`](fn.recv.html)
        /// or similar function shall still return this data.
        MSG_PEEK;
        /// Receive operation blocks until the full amount of data can be
        /// returned. The function may return smaller amount of data if a signal
        /// is caught, an error or disconnect occurs.
        MSG_WAITALL;
        /// Enables nonblocking operation; if the operation would block,
        /// `EAGAIN` or `EWOULDBLOCK` is returned.  This provides similar
        /// behavior to setting the `O_NONBLOCK` flag
        /// (via the [`fcntl`](../../fcntl/fn.fcntl.html)
        /// `F_SETFL` operation), but differs in that `MSG_DONTWAIT` is a per-
        /// call option, whereas `O_NONBLOCK` is a setting on the open file
        /// description (see [open(2)](https://man7.org/linux/man-pages/man2/open.2.html)),
        /// which will affect all threads in
        /// the calling process and as well as other processes that hold
        /// file descriptors referring to the same open file description.
        #[cfg(not(target_os = "aix"))]
        MSG_DONTWAIT;
        /// Receive flags: Control Data was discarded (buffer too small)
        MSG_CTRUNC;
        /// For raw ([`Packet`](addr/enum.AddressFamily.html)), Internet datagram
        /// (since Linux 2.4.27/2.6.8),
        /// netlink (since Linux 2.6.22) and UNIX datagram (since Linux 3.4)
        /// sockets: return the real length of the packet or datagram, even
        /// when it was longer than the passed buffer. Not implemented for UNIX
        /// domain ([unix(7)](https://linux.die.net/man/7/unix)) sockets.
        ///
        /// For use with Internet stream sockets, see [tcp(7)](https://linux.die.net/man/7/tcp).
        MSG_TRUNC;
        /// Terminates a record (when this notion is supported, as for
        /// sockets of type [`SeqPacket`](enum.SockType.html)).
        MSG_EOR;
        /// This flag specifies that queued errors should be received from
        /// the socket error queue. (For more details, see
        /// [recvfrom(2)](https://linux.die.net/man/2/recvfrom))
        #[cfg(linux_android)]
        MSG_ERRQUEUE;
        /// Set the `close-on-exec` flag for the file descriptor received via a UNIX domain
        /// file descriptor using the `SCM_RIGHTS` operation (described in
        /// [unix(7)](https://linux.die.net/man/7/unix)).
        /// This flag is useful for the same reasons as the `O_CLOEXEC` flag of
        /// [open(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/open.html).
        ///
        /// Only used in [`recvmsg`](fn.recvmsg.html) function.
        #[cfg(any(linux_android, freebsdlike, netbsdlike))]
        MSG_CMSG_CLOEXEC;
        /// Requests not to send `SIGPIPE` errors when the other end breaks the connection.
        /// (For more details, see [send(2)](https://linux.die.net/man/2/send)).
        #[cfg(any(linux_android,
                  freebsdlike,
                  solarish,
                  netbsdlike,
                  target_os = "fuchsia",
                  target_os = "haiku"))]
        MSG_NOSIGNAL;
        /// Turns on [`MSG_DONTWAIT`] after the first message has been received (only for
        /// `recvmmsg()`).
        #[cfg(any(linux_android,
                  netbsdlike,
                  target_os = "fuchsia",
                  target_os = "freebsd"))]
        MSG_WAITFORONE;
    }
}

#[cfg(target_os = "freebsd")]
libc_enum! {
    /// A selector for which clock to use when generating packet timestamps.
    /// Used when setting [`TsClock`](crate::sys::socket::sockopt::TsClock) on a socket.
    /// (For more details, see [setsockopt(2)](https://man.freebsd.org/cgi/man.cgi?setsockopt)).
    #[repr(i32)]
    #[non_exhaustive]
    pub enum SocketTimestamp {
        /// Microsecond resolution, realtime. This is the default.
        SO_TS_REALTIME_MICRO,
        /// Sub-nanosecond resolution, realtime.
        SO_TS_BINTIME,
        /// Nanosecond resolution, realtime.
        SO_TS_REALTIME,
        /// Nanosecond resolution, monotonic.
        SO_TS_MONOTONIC,
    }
}

cfg_if! {
    if #[cfg(linux_android)] {
        /// Unix credentials of the sending process.
        ///
        /// This struct is used with the `SO_PEERCRED` ancillary message
        /// and the `SCM_CREDENTIALS` control message for UNIX sockets.
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct UnixCredentials(libc::ucred);

        impl UnixCredentials {
            /// Creates a new instance with the credentials of the current process
            pub fn new() -> Self {
                // Safe because these FFI functions are inherently safe
                unsafe {
                    UnixCredentials(libc::ucred {
                        pid: libc::getpid(),
                        uid: libc::getuid(),
                        gid: libc::getgid()
                    })
                }
            }

            /// Returns the process identifier
            pub fn pid(&self) -> libc::pid_t {
                self.0.pid
            }

            /// Returns the user identifier
            pub fn uid(&self) -> libc::uid_t {
                self.0.uid
            }

            /// Returns the group identifier
            pub fn gid(&self) -> libc::gid_t {
                self.0.gid
            }
        }

        impl Default for UnixCredentials {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<libc::ucred> for UnixCredentials {
            fn from(cred: libc::ucred) -> Self {
                UnixCredentials(cred)
            }
        }

        impl From<UnixCredentials> for libc::ucred {
            fn from(uc: UnixCredentials) -> Self {
                uc.0
            }
        }
    } else if #[cfg(freebsdlike)] {
        /// Unix credentials of the sending process.
        ///
        /// This struct is used with the `SCM_CREDS` ancillary message for UNIX sockets.
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct UnixCredentials(libc::cmsgcred);

        impl UnixCredentials {
            /// Returns the process identifier
            pub fn pid(&self) -> libc::pid_t {
                self.0.cmcred_pid
            }

            /// Returns the real user identifier
            pub fn uid(&self) -> libc::uid_t {
                self.0.cmcred_uid
            }

            /// Returns the effective user identifier
            pub fn euid(&self) -> libc::uid_t {
                self.0.cmcred_euid
            }

            /// Returns the real group identifier
            pub fn gid(&self) -> libc::gid_t {
                self.0.cmcred_gid
            }

            /// Returns a list group identifiers (the first one being the effective GID)
            pub fn groups(&self) -> &[libc::gid_t] {
                unsafe {
                    std::slice::from_raw_parts(
                        self.0.cmcred_groups.as_ptr(),
                        self.0.cmcred_ngroups as _
                    )
                }
            }
        }

        impl From<libc::cmsgcred> for UnixCredentials {
            fn from(cred: libc::cmsgcred) -> Self {
                UnixCredentials(cred)
            }
        }
    }
}

cfg_if! {
    if #[cfg(any(freebsdlike, apple_targets))] {
        /// Return type of [`LocalPeerCred`](crate::sys::socket::sockopt::LocalPeerCred)
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct XuCred(libc::xucred);

        impl XuCred {
            /// Structure layout version
            pub fn version(&self) -> u32 {
                self.0.cr_version
            }

            /// Effective user ID
            pub fn uid(&self) -> libc::uid_t {
                self.0.cr_uid
            }

            /// Returns a list of group identifiers (the first one being the
            /// effective GID)
            pub fn groups(&self) -> &[libc::gid_t] {
                &self.0.cr_groups
            }
        }
    }
}

feature! {
#![feature = "net"]
/// Request for multicast socket operations
///
/// This is a wrapper type around `ip_mreq`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpMembershipRequest(libc::ip_mreq);

impl IpMembershipRequest {
    /// Instantiate a new `IpMembershipRequest`
    ///
    /// If `interface` is `None`, then `Ipv4Addr::any()` will be used for the interface.
    pub fn new(group: net::Ipv4Addr, interface: Option<net::Ipv4Addr>)
        -> Self
    {
        let imr_addr = match interface {
            None => net::Ipv4Addr::UNSPECIFIED,
            Some(addr) => addr
        };
        IpMembershipRequest(libc::ip_mreq {
            imr_multiaddr: ipv4addr_to_libc(group),
            imr_interface: ipv4addr_to_libc(imr_addr)
        })
    }
}

/// Request for ipv6 multicast socket operations
///
/// This is a wrapper type around `ipv6_mreq`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ipv6MembershipRequest(libc::ipv6_mreq);

impl Ipv6MembershipRequest {
    /// Instantiate a new `Ipv6MembershipRequest`
    pub const fn new(group: net::Ipv6Addr) -> Self {
        Ipv6MembershipRequest(libc::ipv6_mreq {
            ipv6mr_multiaddr: ipv6addr_to_libc(&group),
            ipv6mr_interface: 0,
        })
    }
}
}

#[cfg(not(target_os = "redox"))]
feature! {
#![feature = "uio"]

/// Create a buffer large enough for storing some control messages as returned
/// by [`recvmsg`](fn.recvmsg.html).
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate nix;
/// # use nix::sys::time::TimeVal;
/// # use std::os::unix::io::RawFd;
/// # fn main() {
/// // Create a buffer for a `ControlMessageOwned::ScmTimestamp` message
/// let _ = cmsg_space!(TimeVal);
/// // Create a buffer big enough for a `ControlMessageOwned::ScmRights` message
/// // with two file descriptors
/// let _ = cmsg_space!([RawFd; 2]);
/// // Create a buffer big enough for a `ControlMessageOwned::ScmRights` message
/// // and a `ControlMessageOwned::ScmTimestamp` message
/// let _ = cmsg_space!(RawFd, TimeVal);
/// # }
/// ```
#[macro_export]
macro_rules! cmsg_space {
    ( $( $x:ty ),* ) => {
        {
            let space = 0 $(+ $crate::sys::socket::cmsg_space::<$x>())*;
            Vec::<u8>::with_capacity(space)
        }
    }
}

#[inline]
#[doc(hidden)]
pub const fn cmsg_space<T>() -> usize {
    // SAFETY: CMSG_SPACE is always safe
    unsafe { libc::CMSG_SPACE(mem::size_of::<T>() as libc::c_uint) as usize }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Contains outcome of sending or receiving a message
///
/// Use [`cmsgs`][RecvMsg::cmsgs] to access all the control messages present, and
/// [`iovs`][RecvMsg::iovs`] to access underlying io slices.
pub struct RecvMsg<'a, 's, S> {
    pub bytes: usize,
    cmsghdr: Option<&'a cmsghdr>,
    pub address: Option<S>,
    pub flags: MsgFlags,
    iobufs: std::marker::PhantomData<& 's()>,
    mhdr: msghdr,
}

impl<'a, S> RecvMsg<'a, '_, S> {
    /// Iterate over the valid control messages pointed to by this
    /// msghdr.
    pub fn cmsgs(&self) -> CmsgIterator {
        CmsgIterator {
            cmsghdr: self.cmsghdr,
            mhdr: &self.mhdr
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CmsgIterator<'a> {
    /// Control message buffer to decode from. Must adhere to cmsg alignment.
    cmsghdr: Option<&'a cmsghdr>,
    mhdr: &'a msghdr
}

impl<'a> Iterator for CmsgIterator<'a> {
    type Item = ControlMessageOwned;

    fn next(&mut self) -> Option<ControlMessageOwned> {
        match self.cmsghdr {
            None => None,   // No more messages
            Some(hdr) => {
                // Get the data.
                // Safe if cmsghdr points to valid data returned by recvmsg(2)
                let cm = unsafe { Some(ControlMessageOwned::decode_from(hdr))};
                // Advance the internal pointer.  Safe if mhdr and cmsghdr point
                // to valid data returned by recvmsg(2)
                self.cmsghdr = unsafe {
                    let p = CMSG_NXTHDR(self.mhdr as *const _, hdr as *const _);
                    p.as_ref()
                };
                cm
            }
        }
    }
}

/// A type-safe wrapper around a single control message, as used with
/// [`recvmsg`].
///
/// [Further reading](https://man7.org/linux/man-pages/man3/cmsg.3.html)
//  Nix version 0.13.0 and earlier used ControlMessage for both recvmsg and
//  sendmsg.  However, on some platforms the messages returned by recvmsg may be
//  unaligned.  ControlMessageOwned takes those messages by copy, obviating any
//  alignment issues.
//
//  See https://github.com/nix-rust/nix/issues/999
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ControlMessageOwned {
    /// Received version of [`ControlMessage::ScmRights`]
    ScmRights(Vec<RawFd>),
    /// Received version of [`ControlMessage::ScmCredentials`]
    #[cfg(linux_android)]
    ScmCredentials(UnixCredentials),
    /// Received version of [`ControlMessage::ScmCreds`]
    #[cfg(freebsdlike)]
    ScmCreds(UnixCredentials),
    /// A message of type `SCM_TIMESTAMP`, containing the time the
    /// packet was received by the kernel.
    ///
    /// See the kernel's explanation in "SO_TIMESTAMP" of
    /// [networking/timestamping](https://www.kernel.org/doc/Documentation/networking/timestamping.txt).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate nix;
    /// # use nix::sys::socket::*;
    /// # use nix::sys::time::*;
    /// # use std::io::{IoSlice, IoSliceMut};
    /// # use std::time::*;
    /// # use std::str::FromStr;
    /// # use std::os::unix::io::AsRawFd;
    /// # fn main() {
    /// // Set up
    /// let message = "Ohayō!".as_bytes();
    /// let in_socket = socket(
    ///     AddressFamily::Inet,
    ///     SockType::Datagram,
    ///     SockFlag::empty(),
    ///     None).unwrap();
    /// setsockopt(&in_socket, sockopt::ReceiveTimestamp, &true).unwrap();
    /// let localhost = SockaddrIn::from_str("127.0.0.1:0").unwrap();
    /// bind(in_socket.as_raw_fd(), &localhost).unwrap();
    /// let address: SockaddrIn = getsockname(in_socket.as_raw_fd()).unwrap();
    /// // Get initial time
    /// let time0 = SystemTime::now();
    /// // Send the message
    /// let iov = [IoSlice::new(message)];
    /// let flags = MsgFlags::empty();
    /// let l = sendmsg(in_socket.as_raw_fd(), &iov, &[], flags, Some(&address)).unwrap();
    /// assert_eq!(message.len(), l);
    /// // Receive the message
    /// let mut buffer = vec![0u8; message.len()];
    /// let mut cmsgspace = cmsg_space!(TimeVal);
    /// let mut iov = [IoSliceMut::new(&mut buffer)];
    /// let r = recvmsg::<SockaddrIn>(in_socket.as_raw_fd(), &mut iov, Some(&mut cmsgspace), flags)
    ///     .unwrap();
    /// let rtime = match r.cmsgs().next() {
    ///     Some(ControlMessageOwned::ScmTimestamp(rtime)) => rtime,
    ///     Some(_) => panic!("Unexpected control message"),
    ///     None => panic!("No control message")
    /// };
    /// // Check the final time
    /// let time1 = SystemTime::now();
    /// // the packet's received timestamp should lie in-between the two system
    /// // times, unless the system clock was adjusted in the meantime.
    /// let rduration = Duration::new(rtime.tv_sec() as u64,
    ///                               rtime.tv_usec() as u32 * 1000);
    /// assert!(time0.duration_since(UNIX_EPOCH).unwrap() <= rduration);
    /// assert!(rduration <= time1.duration_since(UNIX_EPOCH).unwrap());
    /// // Close socket
    /// # }
    /// ```
    ScmTimestamp(TimeVal),
    /// A set of nanosecond resolution timestamps
    ///
    /// [Further reading](https://www.kernel.org/doc/html/latest/networking/timestamping.html)
    #[cfg(linux_android)]
    ScmTimestampsns(Timestamps),
    /// Nanoseconds resolution timestamp
    ///
    /// [Further reading](https://www.kernel.org/doc/html/latest/networking/timestamping.html)
    #[cfg(linux_android)]
    ScmTimestampns(TimeSpec),
    /// Realtime clock timestamp
    ///
    /// [Further reading](https://man.freebsd.org/cgi/man.cgi?setsockopt)
    #[cfg(target_os = "freebsd")]
    ScmRealtime(TimeSpec),
    /// Monotonic clock timestamp
    ///
    /// [Further reading](https://man.freebsd.org/cgi/man.cgi?setsockopt)
    #[cfg(target_os = "freebsd")]
    ScmMonotonic(TimeSpec),
    #[cfg(any(linux_android, apple_targets, target_os = "netbsd"))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4PacketInfo(libc::in_pktinfo),
    #[cfg(any(linux_android, bsd))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv6PacketInfo(libc::in6_pktinfo),
    #[cfg(bsd)]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4RecvIf(libc::sockaddr_dl),
    #[cfg(bsd)]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4RecvDstAddr(libc::in_addr),
    #[cfg(any(linux_android, target_os = "freebsd"))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4OrigDstAddr(libc::sockaddr_in),
    #[cfg(any(linux_android, target_os = "freebsd"))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv6OrigDstAddr(libc::sockaddr_in6),

    /// UDP Generic Receive Offload (GRO) allows receiving multiple UDP
    /// packets from a single sender.
    /// Fixed-size payloads are following one by one in a receive buffer.
    /// This Control Message indicates the size of all smaller packets,
    /// except, maybe, the last one.
    ///
    /// `UdpGroSegment` socket option should be enabled on a socket
    /// to allow receiving GRO packets.
    #[cfg(target_os = "linux")]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    UdpGroSegments(u16),

    /// SO_RXQ_OVFL indicates that an unsigned 32 bit value
    /// ancilliary msg (cmsg) should be attached to recieved
    /// skbs indicating the number of packets dropped by the
    /// socket between the last recieved packet and this
    /// received packet.
    ///
    /// `RxqOvfl` socket option should be enabled on a socket
    /// to allow receiving the drop counter.
    #[cfg(any(linux_android, target_os = "fuchsia"))]
    RxqOvfl(u32),

    /// Socket error queue control messages read with the `MSG_ERRQUEUE` flag.
    #[cfg(linux_android)]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4RecvErr(libc::sock_extended_err, Option<sockaddr_in>),
    /// Socket error queue control messages read with the `MSG_ERRQUEUE` flag.
    #[cfg(linux_android)]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv6RecvErr(libc::sock_extended_err, Option<sockaddr_in6>),

    /// `SOL_TLS` messages of type `TLS_GET_RECORD_TYPE`
    #[cfg(any(target_os = "linux"))]
    TlsGetRecordType(TlsGetRecordType),

    /// Catch-all variant for unimplemented cmsg types.
    #[doc(hidden)]
    Unknown(UnknownCmsg),
}

/// For representing packet timestamps via `SO_TIMESTAMPING` interface
#[cfg(linux_android)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Timestamps {
    /// software based timestamp, usually one containing data
    pub system: TimeSpec,
    /// legacy timestamp, usually empty
    pub hw_trans: TimeSpec,
    /// hardware based timestamp
    pub hw_raw: TimeSpec,
}

/// These constants correspond to TLS 1.2 message types, as defined in
/// RFC 5246, Appendix A.1
#[cfg(any(target_os = "linux"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum TlsGetRecordType {
    ChangeCipherSpec ,
    Alert,
    Handshake,
    ApplicationData,
    Unknown(u8),
}

#[cfg(any(target_os = "linux"))]
impl From<u8> for TlsGetRecordType {
    fn from(x: u8) -> Self {
        match x {
            20 => TlsGetRecordType::ChangeCipherSpec,
            21 => TlsGetRecordType::Alert,
            22 => TlsGetRecordType::Handshake,
            23 => TlsGetRecordType::ApplicationData,
            _ => TlsGetRecordType::Unknown(x),
        }
    }
}

impl ControlMessageOwned {
    /// Decodes a `ControlMessageOwned` from raw bytes.
    ///
    /// This is only safe to call if the data is correct for the message type
    /// specified in the header. Normally, the kernel ensures that this is the
    /// case. "Correct" in this case includes correct length, alignment and
    /// actual content.
    // Clippy complains about the pointer alignment of `p`, not understanding
    // that it's being fed to a function that can handle that.
    #[allow(clippy::cast_ptr_alignment)]
    unsafe fn decode_from(header: &cmsghdr) -> ControlMessageOwned
    {
        let p = unsafe { CMSG_DATA(header) };
        // The cast is not unnecessary on all platforms.
        #[allow(clippy::unnecessary_cast)]
        let len = header as *const _ as usize + header.cmsg_len as usize
            - p as usize;
        match (header.cmsg_level, header.cmsg_type) {
            (libc::SOL_SOCKET, libc::SCM_RIGHTS) => {
                let n = len / mem::size_of::<RawFd>();
                let mut fds = Vec::with_capacity(n);
                for i in 0..n {
                    unsafe {
                        let fdp = (p as *const RawFd).add(i);
                        fds.push(ptr::read_unaligned(fdp));
                    }
                }
                ControlMessageOwned::ScmRights(fds)
            },
            #[cfg(linux_android)]
            (libc::SOL_SOCKET, libc::SCM_CREDENTIALS) => {
                let cred: libc::ucred = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::ScmCredentials(cred.into())
            }
            #[cfg(freebsdlike)]
            (libc::SOL_SOCKET, libc::SCM_CREDS) => {
                let cred: libc::cmsgcred = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::ScmCreds(cred.into())
            }
            #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
            (libc::SOL_SOCKET, libc::SCM_TIMESTAMP) => {
                let tv: libc::timeval = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::ScmTimestamp(TimeVal::from(tv))
            },
            #[cfg(linux_android)]
            (libc::SOL_SOCKET, libc::SCM_TIMESTAMPNS) => {
                let ts: libc::timespec = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::ScmTimestampns(TimeSpec::from(ts))
            }
            #[cfg(target_os = "freebsd")]
            (libc::SOL_SOCKET, libc::SCM_REALTIME) => {
                let ts: libc::timespec = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::ScmRealtime(TimeSpec::from(ts))
            }
            #[cfg(target_os = "freebsd")]
            (libc::SOL_SOCKET, libc::SCM_MONOTONIC) => {
                let ts: libc::timespec = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::ScmMonotonic(TimeSpec::from(ts))
            }
            #[cfg(linux_android)]
            (libc::SOL_SOCKET, libc::SCM_TIMESTAMPING) => {
                let tp = p as *const libc::timespec;
                let ts: libc::timespec = unsafe { ptr::read_unaligned(tp) };
                let system = TimeSpec::from(ts);
                let ts: libc::timespec = unsafe { ptr::read_unaligned(tp.add(1)) };
                let hw_trans = TimeSpec::from(ts);
                let ts: libc::timespec = unsafe { ptr::read_unaligned(tp.add(2)) };
                let hw_raw = TimeSpec::from(ts);
                let timestamping = Timestamps { system, hw_trans, hw_raw };
                ControlMessageOwned::ScmTimestampsns(timestamping)
            }
            #[cfg(any(target_os = "freebsd", linux_android, apple_targets))]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IPV6, libc::IPV6_PKTINFO) => {
                let info = unsafe { ptr::read_unaligned(p as *const libc::in6_pktinfo) };
                ControlMessageOwned::Ipv6PacketInfo(info)
            }
            #[cfg(any(linux_android, apple_targets, target_os = "netbsd"))]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IP, libc::IP_PKTINFO) => {
                let info = unsafe { ptr::read_unaligned(p as *const libc::in_pktinfo) };
                ControlMessageOwned::Ipv4PacketInfo(info)
            }
            #[cfg(bsd)]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IP, libc::IP_RECVIF) => {
                let dl = unsafe { ptr::read_unaligned(p as *const libc::sockaddr_dl) };
                ControlMessageOwned::Ipv4RecvIf(dl)
            },
            #[cfg(bsd)]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IP, libc::IP_RECVDSTADDR) => {
                let dl = unsafe { ptr::read_unaligned(p as *const libc::in_addr) };
                ControlMessageOwned::Ipv4RecvDstAddr(dl)
            },
            #[cfg(any(linux_android, target_os = "freebsd"))]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IP, libc::IP_ORIGDSTADDR) => {
                let dl = unsafe { ptr::read_unaligned(p as *const libc::sockaddr_in) };
                ControlMessageOwned::Ipv4OrigDstAddr(dl)
            },
            #[cfg(target_os = "linux")]
            #[cfg(feature = "net")]
            (libc::SOL_UDP, libc::UDP_GRO) => {
                let gso_size: u16 = unsafe { ptr::read_unaligned(p as *const _) };
                ControlMessageOwned::UdpGroSegments(gso_size)
            },
            #[cfg(any(linux_android, target_os = "fuchsia"))]
            (libc::SOL_SOCKET, libc::SO_RXQ_OVFL) => {
                let drop_counter = unsafe { ptr::read_unaligned(p as *const u32) };
                ControlMessageOwned::RxqOvfl(drop_counter)
            },
            #[cfg(linux_android)]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IP, libc::IP_RECVERR) => {
                let (err, addr) = unsafe { Self::recv_err_helper::<sockaddr_in>(p, len) };
                ControlMessageOwned::Ipv4RecvErr(err, addr)
            },
            #[cfg(linux_android)]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IPV6, libc::IPV6_RECVERR) => {
                let (err, addr) = unsafe { Self::recv_err_helper::<sockaddr_in6>(p, len) };
                ControlMessageOwned::Ipv6RecvErr(err, addr)
            },
            #[cfg(any(linux_android, target_os = "freebsd"))]
            #[cfg(feature = "net")]
            (libc::IPPROTO_IPV6, libc::IPV6_ORIGDSTADDR) => {
                let dl = unsafe { ptr::read_unaligned(p as *const libc::sockaddr_in6) };
                ControlMessageOwned::Ipv6OrigDstAddr(dl)
            },
            #[cfg(any(target_os = "linux"))]
            (libc::SOL_TLS, libc::TLS_GET_RECORD_TYPE) => {
                let content_type = unsafe { ptr::read_unaligned(p as *const u8) };
                ControlMessageOwned::TlsGetRecordType(content_type.into())
            },
            (_, _) => {
                let sl = unsafe { std::slice::from_raw_parts(p, len) };
                let ucmsg = UnknownCmsg(*header, Vec::<u8>::from(sl));
                ControlMessageOwned::Unknown(ucmsg)
            }
        }
    }

    #[cfg(linux_android)]
    #[cfg(feature = "net")]
    #[allow(clippy::cast_ptr_alignment)]    // False positive
    unsafe fn recv_err_helper<T>(p: *mut libc::c_uchar, len: usize) -> (libc::sock_extended_err, Option<T>) {
        let ee = p as *const libc::sock_extended_err;
        let err = unsafe { ptr::read_unaligned(ee) };

        // For errors originating on the network, SO_EE_OFFENDER(ee) points inside the p[..len]
        // CMSG_DATA buffer.  For local errors, there is no address included in the control
        // message, and SO_EE_OFFENDER(ee) points beyond the end of the buffer.  So, we need to
        // validate that the address object is in-bounds before we attempt to copy it.
        let addrp = unsafe { libc::SO_EE_OFFENDER(ee) as *const T };

        if unsafe { addrp.offset(1) } as usize - (p as usize) > len {
            (err, None)
        } else {
            (err, Some(unsafe { ptr::read_unaligned(addrp) }))
        }
    }
}

/// A type-safe zero-copy wrapper around a single control message, as used with
/// [`sendmsg`].  More types may be added to this enum; do not exhaustively
/// pattern-match it.
///
/// [Further reading](https://man7.org/linux/man-pages/man3/cmsg.3.html)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ControlMessage<'a> {
    /// A message of type `SCM_RIGHTS`, containing an array of file
    /// descriptors passed between processes.
    ///
    /// See the description in the "Ancillary messages" section of the
    /// [unix(7) man page](https://man7.org/linux/man-pages/man7/unix.7.html).
    ///
    /// Using multiple `ScmRights` messages for a single `sendmsg` call isn't
    /// recommended since it causes platform-dependent behaviour: It might
    /// swallow all but the first `ScmRights` message or fail with `EINVAL`.
    /// Instead, you can put all fds to be passed into a single `ScmRights`
    /// message.
    ScmRights(&'a [RawFd]),
    /// A message of type `SCM_CREDENTIALS`, containing the pid, uid and gid of
    /// a process connected to the socket.
    ///
    /// This is similar to the socket option `SO_PEERCRED`, but requires a
    /// process to explicitly send its credentials. A process running as root is
    /// allowed to specify any credentials, while credentials sent by other
    /// processes are verified by the kernel.
    ///
    /// For further information, please refer to the
    /// [`unix(7)`](https://man7.org/linux/man-pages/man7/unix.7.html) man page.
    #[cfg(linux_android)]
    ScmCredentials(&'a UnixCredentials),
    /// A message of type `SCM_CREDS`, containing the pid, uid, euid, gid and groups of
    /// a process connected to the socket.
    ///
    /// This is similar to the socket options `LOCAL_CREDS` and `LOCAL_PEERCRED`, but
    /// requires a process to explicitly send its credentials.
    ///
    /// Credentials are always overwritten by the kernel, so this variant does have
    /// any data, unlike the receive-side
    /// [`ControlMessageOwned::ScmCreds`].
    ///
    /// For further information, please refer to the
    /// [`unix(4)`](https://www.freebsd.org/cgi/man.cgi?query=unix) man page.
    #[cfg(freebsdlike)]
    ScmCreds,

    /// Set IV for `AF_ALG` crypto API.
    ///
    /// For further information, please refer to the
    /// [`documentation`](https://kernel.readthedocs.io/en/sphinx-samples/crypto-API.html)
    #[cfg(linux_android)]
    AlgSetIv(&'a [u8]),
    /// Set crypto operation for `AF_ALG` crypto API. It may be one of
    /// `ALG_OP_ENCRYPT` or `ALG_OP_DECRYPT`
    ///
    /// For further information, please refer to the
    /// [`documentation`](https://kernel.readthedocs.io/en/sphinx-samples/crypto-API.html)
    #[cfg(linux_android)]
    AlgSetOp(&'a libc::c_int),
    /// Set the length of associated authentication data (AAD) (applicable only to AEAD algorithms)
    /// for `AF_ALG` crypto API.
    ///
    /// For further information, please refer to the
    /// [`documentation`](https://kernel.readthedocs.io/en/sphinx-samples/crypto-API.html)
    #[cfg(linux_android)]
    AlgSetAeadAssoclen(&'a u32),

    /// UDP GSO makes it possible for applications to generate network packets
    /// for a virtual MTU much greater than the real one.
    /// The length of the send data no longer matches the expected length on
    /// the wire.
    /// The size of the datagram payload as it should appear on the wire may be
    /// passed through this control message.
    /// Send buffer should consist of multiple fixed-size wire payloads
    /// following one by one, and the last, possibly smaller one.
    #[cfg(target_os = "linux")]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    UdpGsoSegments(&'a u16),

    /// Configure the sending addressing and interface for v4.
    ///
    /// For further information, please refer to the
    /// [`ip(7)`](https://man7.org/linux/man-pages/man7/ip.7.html) man page.
    #[cfg(any(linux_android, target_os = "netbsd", apple_targets))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4PacketInfo(&'a libc::in_pktinfo),

    /// Configure the sending addressing and interface for v6.
    ///
    /// For further information, please refer to the
    /// [`ipv6(7)`](https://man7.org/linux/man-pages/man7/ipv6.7.html) man page.
    #[cfg(any(linux_android,
              target_os = "netbsd",
              target_os = "freebsd",
              apple_targets))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv6PacketInfo(&'a libc::in6_pktinfo),

    /// Configure the IPv4 source address with `IP_SENDSRCADDR`.
    #[cfg(any(freebsdlike, netbsdlike))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv4SendSrcAddr(&'a libc::in_addr),

    /// Configure the hop limit for v6 multicast traffic.
    ///
    /// Set the IPv6 hop limit for this message. The argument is an integer
    /// between 0 and 255. A value of -1 will set the hop limit to the route
    /// default if possible on the interface. Without this cmsg,  packets sent
    /// with sendmsg have a hop limit of 1 and will not leave the local network.
    /// For further information, please refer to the
    /// [`ipv6(7)`](https://man7.org/linux/man-pages/man7/ipv6.7.html) man page.
    #[cfg(any(linux_android, freebsdlike, apple_targets, target_os = "haiku"))]
    #[cfg(feature = "net")]
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    Ipv6HopLimit(&'a libc::c_int),

    /// SO_RXQ_OVFL indicates that an unsigned 32 bit value
    /// ancilliary msg (cmsg) should be attached to recieved
    /// skbs indicating the number of packets dropped by the
    /// socket between the last recieved packet and this
    /// received packet.
    #[cfg(any(linux_android, target_os = "fuchsia"))]
    RxqOvfl(&'a u32),

    /// Configure the transmission time of packets.
    ///
    /// For further information, please refer to the
    /// [`tc-etf(8)`](https://man7.org/linux/man-pages/man8/tc-etf.8.html) man
    /// page.
    #[cfg(target_os = "linux")]
    TxTime(&'a u64),
}

// An opaque structure used to prevent cmsghdr from being a public type
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownCmsg(cmsghdr, Vec<u8>);

impl<'a> ControlMessage<'a> {
    /// The value of CMSG_SPACE on this message.
    /// Safe because CMSG_SPACE is always safe
    fn space(&self) -> usize {
        unsafe{CMSG_SPACE(self.len() as libc::c_uint) as usize}
    }

    /// The value of CMSG_LEN on this message.
    /// Safe because CMSG_LEN is always safe
    #[cfg(any(target_os = "android",
              all(target_os = "linux", not(target_env = "musl"))))]
    fn cmsg_len(&self) -> usize {
        unsafe{CMSG_LEN(self.len() as libc::c_uint) as usize}
    }

    #[cfg(not(any(target_os = "android",
                  all(target_os = "linux", not(target_env = "musl")))))]
    fn cmsg_len(&self) -> libc::c_uint {
        unsafe{CMSG_LEN(self.len() as libc::c_uint)}
    }

    /// Return a reference to the payload data as a byte pointer
    fn copy_to_cmsg_data(&self, cmsg_data: *mut u8) {
        let data_ptr = match *self {
            ControlMessage::ScmRights(fds) => {
                fds as *const _ as *const u8
            },
            #[cfg(linux_android)]
            ControlMessage::ScmCredentials(creds) => {
                &creds.0 as *const libc::ucred as *const u8
            }
            #[cfg(freebsdlike)]
            ControlMessage::ScmCreds => {
                // The kernel overwrites the data, we just zero it
                // to make sure it's not uninitialized memory
                unsafe { ptr::write_bytes(cmsg_data, 0, self.len()) };
                return
            }
            #[cfg(linux_android)]
            ControlMessage::AlgSetIv(iv) => {
                #[allow(deprecated)] // https://github.com/rust-lang/libc/issues/1501
                let af_alg_iv = libc::af_alg_iv {
                    ivlen: iv.len() as u32,
                    iv: [0u8; 0],
                };

                let size = mem::size_of_val(&af_alg_iv);

                unsafe {
                    ptr::copy_nonoverlapping(
                        &af_alg_iv as *const _ as *const u8,
                        cmsg_data,
                        size,
                    );
                    ptr::copy_nonoverlapping(
                        iv.as_ptr(),
                        cmsg_data.add(size),
                        iv.len()
                    );
                };

                return
            },
            #[cfg(linux_android)]
            ControlMessage::AlgSetOp(op) => {
                op as *const _ as *const u8
            },
            #[cfg(linux_android)]
            ControlMessage::AlgSetAeadAssoclen(len) => {
                len as *const _ as *const u8
            },
            #[cfg(target_os = "linux")]
            #[cfg(feature = "net")]
            ControlMessage::UdpGsoSegments(gso_size) => {
                gso_size as *const _ as *const u8
            },
            #[cfg(any(linux_android, target_os = "netbsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4PacketInfo(info) => info as *const _ as *const u8,
            #[cfg(any(linux_android, target_os = "netbsd",
                      target_os = "freebsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6PacketInfo(info) => info as *const _ as *const u8,
            #[cfg(any(freebsdlike, netbsdlike))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4SendSrcAddr(addr) => addr as *const _ as *const u8,
            #[cfg(any(linux_android, freebsdlike, apple_targets, target_os = "haiku"))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6HopLimit(limit) => limit as *const _ as *const u8,
            #[cfg(any(linux_android, target_os = "fuchsia"))]
            ControlMessage::RxqOvfl(drop_count) => {
                drop_count as *const _ as *const u8
            },
            #[cfg(target_os = "linux")]
            ControlMessage::TxTime(tx_time) => {
                tx_time as *const _ as *const u8
            },
        };
        unsafe {
            ptr::copy_nonoverlapping(
                data_ptr,
                cmsg_data,
                self.len()
            )
        };
    }

    /// The size of the payload, excluding its cmsghdr
    fn len(&self) -> usize {
        match *self {
            ControlMessage::ScmRights(fds) => {
                mem::size_of_val(fds)
            },
            #[cfg(linux_android)]
            ControlMessage::ScmCredentials(creds) => {
                mem::size_of_val(creds)
            }
            #[cfg(freebsdlike)]
            ControlMessage::ScmCreds => {
                mem::size_of::<libc::cmsgcred>()
            }
            #[cfg(linux_android)]
            ControlMessage::AlgSetIv(iv) => {
                mem::size_of::<&[u8]>() + iv.len()
            },
            #[cfg(linux_android)]
            ControlMessage::AlgSetOp(op) => {
                mem::size_of_val(op)
            },
            #[cfg(linux_android)]
            ControlMessage::AlgSetAeadAssoclen(len) => {
                mem::size_of_val(len)
            },
            #[cfg(target_os = "linux")]
            #[cfg(feature = "net")]
            ControlMessage::UdpGsoSegments(gso_size) => {
                mem::size_of_val(gso_size)
            },
            #[cfg(any(linux_android, target_os = "netbsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4PacketInfo(info) => mem::size_of_val(info),
            #[cfg(any(linux_android, target_os = "netbsd",
                      target_os = "freebsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6PacketInfo(info) => mem::size_of_val(info),
            #[cfg(any(freebsdlike, netbsdlike))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4SendSrcAddr(addr) => mem::size_of_val(addr),
            #[cfg(any(linux_android, freebsdlike, apple_targets, target_os = "haiku"))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6HopLimit(limit) => {
                mem::size_of_val(limit)
            },
            #[cfg(any(linux_android, target_os = "fuchsia"))]
            ControlMessage::RxqOvfl(drop_count) => {
                mem::size_of_val(drop_count)
            },
            #[cfg(target_os = "linux")]
            ControlMessage::TxTime(tx_time) => {
                mem::size_of_val(tx_time)
            },
        }
    }

    /// Returns the value to put into the `cmsg_level` field of the header.
    fn cmsg_level(&self) -> libc::c_int {
        match *self {
            ControlMessage::ScmRights(_) => libc::SOL_SOCKET,
            #[cfg(linux_android)]
            ControlMessage::ScmCredentials(_) => libc::SOL_SOCKET,
            #[cfg(freebsdlike)]
            ControlMessage::ScmCreds => libc::SOL_SOCKET,
            #[cfg(linux_android)]
            ControlMessage::AlgSetIv(_) | ControlMessage::AlgSetOp(_) |
                ControlMessage::AlgSetAeadAssoclen(_) => libc::SOL_ALG,
            #[cfg(target_os = "linux")]
            #[cfg(feature = "net")]
            ControlMessage::UdpGsoSegments(_) => libc::SOL_UDP,
            #[cfg(any(linux_android, target_os = "netbsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4PacketInfo(_) => libc::IPPROTO_IP,
            #[cfg(any(linux_android, target_os = "netbsd",
                      target_os = "freebsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6PacketInfo(_) => libc::IPPROTO_IPV6,
            #[cfg(any(freebsdlike, netbsdlike))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4SendSrcAddr(_) => libc::IPPROTO_IP,
            #[cfg(any(linux_android, freebsdlike, apple_targets, target_os = "haiku"))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6HopLimit(_) => libc::IPPROTO_IPV6,
            #[cfg(any(linux_android, target_os = "fuchsia"))]
            ControlMessage::RxqOvfl(_) => libc::SOL_SOCKET,
            #[cfg(target_os = "linux")]
            ControlMessage::TxTime(_) => libc::SOL_SOCKET,
        }
    }

    /// Returns the value to put into the `cmsg_type` field of the header.
    fn cmsg_type(&self) -> libc::c_int {
        match *self {
            ControlMessage::ScmRights(_) => libc::SCM_RIGHTS,
            #[cfg(linux_android)]
            ControlMessage::ScmCredentials(_) => libc::SCM_CREDENTIALS,
            #[cfg(freebsdlike)]
            ControlMessage::ScmCreds => libc::SCM_CREDS,
            #[cfg(linux_android)]
            ControlMessage::AlgSetIv(_) => {
                libc::ALG_SET_IV
            },
            #[cfg(linux_android)]
            ControlMessage::AlgSetOp(_) => {
                libc::ALG_SET_OP
            },
            #[cfg(linux_android)]
            ControlMessage::AlgSetAeadAssoclen(_) => {
                libc::ALG_SET_AEAD_ASSOCLEN
            },
            #[cfg(target_os = "linux")]
            #[cfg(feature = "net")]
            ControlMessage::UdpGsoSegments(_) => {
                libc::UDP_SEGMENT
            },
            #[cfg(any(linux_android, target_os = "netbsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4PacketInfo(_) => libc::IP_PKTINFO,
            #[cfg(any(linux_android, target_os = "netbsd",
                      target_os = "freebsd", apple_targets))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6PacketInfo(_) => libc::IPV6_PKTINFO,
            #[cfg(any(freebsdlike, netbsdlike))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv4SendSrcAddr(_) => libc::IP_SENDSRCADDR,
            #[cfg(any(linux_android, freebsdlike, apple_targets, target_os = "haiku"))]
            #[cfg(feature = "net")]
            ControlMessage::Ipv6HopLimit(_) => libc::IPV6_HOPLIMIT,
            #[cfg(any(linux_android, target_os = "fuchsia"))]
            ControlMessage::RxqOvfl(_) => {
                libc::SO_RXQ_OVFL
            },
            #[cfg(target_os = "linux")]
            ControlMessage::TxTime(_) => {
                libc::SCM_TXTIME
            },
        }
    }

    // Unsafe: cmsg must point to a valid cmsghdr with enough space to
    // encode self.
    unsafe fn encode_into(&self, cmsg: *mut cmsghdr) {
        unsafe {
            (*cmsg).cmsg_level = self.cmsg_level();
            (*cmsg).cmsg_type = self.cmsg_type();
            (*cmsg).cmsg_len = self.cmsg_len();
            self.copy_to_cmsg_data( CMSG_DATA(cmsg) );
        }
    }
}


/// Send data in scatter-gather vectors to a socket, possibly accompanied
/// by ancillary data. Optionally direct the message at the given address,
/// as with sendto.
///
/// Allocates if cmsgs is nonempty.
///
/// # Examples
/// When not directing to any specific address, use `()` for the generic type
/// ```
/// # use nix::sys::socket::*;
/// # use nix::unistd::pipe;
/// # use std::io::IoSlice;
/// # use std::os::unix::io::AsRawFd;
/// let (fd1, fd2) = socketpair(AddressFamily::Unix, SockType::Stream, None,
///     SockFlag::empty())
///     .unwrap();
/// let (r, w) = pipe().unwrap();
///
/// let iov = [IoSlice::new(b"hello")];
/// let fds = [r.as_raw_fd()];
/// let cmsg = ControlMessage::ScmRights(&fds);
/// sendmsg::<()>(fd1.as_raw_fd(), &iov, &[cmsg], MsgFlags::empty(), None).unwrap();
/// ```
/// When directing to a specific address, the generic type will be inferred.
/// ```
/// # use nix::sys::socket::*;
/// # use nix::unistd::pipe;
/// # use std::io::IoSlice;
/// # use std::str::FromStr;
/// # use std::os::unix::io::AsRawFd;
/// let localhost = SockaddrIn::from_str("1.2.3.4:8080").unwrap();
/// let fd = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(),
///     None).unwrap();
/// let (r, w) = pipe().unwrap();
///
/// let iov = [IoSlice::new(b"hello")];
/// let fds = [r.as_raw_fd()];
/// let cmsg = ControlMessage::ScmRights(&fds);
/// sendmsg(fd.as_raw_fd(), &iov, &[cmsg], MsgFlags::empty(), Some(&localhost)).unwrap();
/// ```
pub fn sendmsg<S>(fd: RawFd, iov: &[IoSlice<'_>], cmsgs: &[ControlMessage],
               flags: MsgFlags, addr: Option<&S>) -> Result<usize>
    where S: SockaddrLike
{
    let capacity = cmsgs.iter().map(|c| c.space()).sum();

    // First size the buffer needed to hold the cmsgs.  It must be zeroed,
    // because subsequent code will not clear the padding bytes.
    let mut cmsg_buffer = vec![0u8; capacity];

    let mhdr = pack_mhdr_to_send(&mut cmsg_buffer[..], iov, cmsgs, addr);

    let ret = unsafe { libc::sendmsg(fd, &mhdr, flags.bits()) };

    Errno::result(ret).map(|r| r as usize)
}


/// An extension of `sendmsg` that allows the caller to transmit multiple
/// messages on a socket using a single system call. This has performance
/// benefits for some applications.
///
/// Allocations are performed for cmsgs and to build `msghdr` buffer
///
/// # Arguments
///
/// * `fd`:             Socket file descriptor
/// * `data`:           Struct that implements `IntoIterator` with `SendMmsgData` items
/// * `flags`:          Optional flags passed directly to the operating system.
///
/// # Returns
/// `Vec` with numbers of sent bytes on each sent message.
///
/// # References
/// [`sendmsg`](fn.sendmsg.html)
#[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
pub fn sendmmsg<'a, XS, AS, C, I, S>(
    fd: RawFd,
    data: &'a mut MultiHeaders<S>,
    slices: XS,
    // one address per group of slices
    addrs: AS,
    // shared across all the messages
    cmsgs: C,
    flags: MsgFlags
) -> crate::Result<MultiResults<'a, S>>
    where
        XS: IntoIterator<Item = &'a I>,
        AS: AsRef<[Option<S>]>,
        I: AsRef<[IoSlice<'a>]> + 'a,
        C: AsRef<[ControlMessage<'a>]> + 'a,
        S: SockaddrLike + 'a,
{

    let mut count = 0;


    for (i, ((slice, addr), mmsghdr)) in slices.into_iter().zip(addrs.as_ref()).zip(data.items.iter_mut() ).enumerate() {
        let p = &mut mmsghdr.msg_hdr;
        p.msg_iov = slice.as_ref().as_ptr().cast_mut().cast();
        p.msg_iovlen = slice.as_ref().len() as _;

        p.msg_namelen = addr.as_ref().map_or(0, S::len);
        p.msg_name = addr.as_ref().map_or(ptr::null(), S::as_ptr).cast_mut().cast();

        // Encode each cmsg.  This must happen after initializing the header because
        // CMSG_NEXT_HDR and friends read the msg_control and msg_controllen fields.
        // CMSG_FIRSTHDR is always safe
        let mut pmhdr: *mut cmsghdr = unsafe { CMSG_FIRSTHDR(p) };
        for cmsg in cmsgs.as_ref() {
            assert_ne!(pmhdr, ptr::null_mut());
            // Safe because we know that pmhdr is valid, and we initialized it with
            // sufficient space
            unsafe { cmsg.encode_into(pmhdr) };
            // Safe because mhdr is valid
            pmhdr = unsafe { CMSG_NXTHDR(p, pmhdr) };
        }

        // Doing an unchecked addition is alright here, as the only way to obtain an instance of `MultiHeaders`
        // is through the `preallocate` function, which takes an `usize` as an argument to define its size,
        // which also provides an upper bound for the size of this zipped iterator. Thus, `i < usize::MAX` or in
        // other words: `count` doesn't overflow
        count = i + 1;
    }

    // SAFETY: all pointers are guaranteed to be valid for the scope of this function. `count` does represent the
    // maximum number of messages that can be sent safely (i.e. `count` is the minimum of the sizes of `slices`,
    // `data.items` and `addrs`)
    let sent = Errno::result(unsafe {
        libc::sendmmsg(
            fd,
            data.items.as_mut_ptr(),
            count as _,
            flags.bits() as _
        )
    })? as usize;

    Ok(MultiResults {
        rmm: data,
        current_index: 0,
        received: sent
    })

}


#[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
#[derive(Debug)]
/// Preallocated structures needed for [`recvmmsg`] and [`sendmmsg`] functions
pub struct MultiHeaders<S> {
    // preallocated boxed slice of mmsghdr
    items: Box<[libc::mmsghdr]>,
    addresses: Box<[mem::MaybeUninit<S>]>,
    // while we are not using it directly - this is used to store control messages
    // and we retain pointers to them inside items array
    _cmsg_buffers: Option<Box<[u8]>>,
    msg_controllen: usize,
}

#[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
impl<S> MultiHeaders<S> {
    /// Preallocate structure used by [`recvmmsg`] and [`sendmmsg`] takes number of headers to preallocate
    ///
    /// `cmsg_buffer` should be created with [`cmsg_space!`] if needed
    pub fn preallocate(num_slices: usize, cmsg_buffer: Option<Vec<u8>>) -> Self
    where
        S: Copy + SockaddrLike,
    {
        // we will be storing pointers to addresses inside mhdr - convert it into boxed
        // slice so it can'be changed later by pushing anything into self.addresses
        let mut addresses = vec![std::mem::MaybeUninit::<S>::uninit(); num_slices].into_boxed_slice();

        let msg_controllen = cmsg_buffer.as_ref().map_or(0, |v| v.capacity());

        // we'll need a cmsg_buffer for each slice, we preallocate a vector and split
        // it into "slices" parts
        let mut cmsg_buffers =
            cmsg_buffer.map(|v| vec![0u8; v.capacity() * num_slices].into_boxed_slice());

        let items = addresses
            .iter_mut()
            .enumerate()
            .map(|(ix, address)| {
                let (ptr, cap) = match &mut cmsg_buffers {
                    Some(v) => (&mut v[ix * msg_controllen] as *mut u8, msg_controllen),
                    None => (std::ptr::null_mut(), 0),
                };
                let msg_hdr = unsafe { pack_mhdr_to_receive(std::ptr::null_mut(), 0, ptr, cap, address.as_mut_ptr()) };
                libc::mmsghdr {
                    msg_hdr,
                    msg_len: 0,
                }
            })
            .collect::<Vec<_>>();

        Self {
            items: items.into_boxed_slice(),
            addresses,
            _cmsg_buffers: cmsg_buffers,
            msg_controllen,
        }
    }
}

/// An extension of recvmsg that allows the caller to receive multiple messages from a socket using a single system call.
///
/// This has performance benefits for some applications.
///
/// This method performs no allocations.
///
/// Returns an iterator producing [`RecvMsg`], one per received messages. Each `RecvMsg` can produce
/// iterators over [`IoSlice`] with [`iovs`][RecvMsg::iovs`] and
/// `ControlMessageOwned` with [`cmsgs`][RecvMsg::cmsgs].
///
/// # Bugs (in underlying implementation, at least in Linux)
/// The timeout argument does not work as intended. The timeout is checked only after the receipt
/// of each datagram, so that if up to `vlen`-1 datagrams are received before the timeout expires,
/// but then no further datagrams are received, the call will block forever.
///
/// If an error occurs after at least one message has been received, the call succeeds, and returns
/// the number of messages received. The error code is expected to be returned on a subsequent
/// call to recvmmsg(). In the current implementation, however, the error code can be
/// overwritten in the meantime by an unrelated network event on a socket, for example an
/// incoming ICMP packet.

// On aarch64 linux using recvmmsg and trying to get hardware/kernel timestamps might not
// always produce the desired results - see https://github.com/nix-rust/nix/pull/1744 for more
// details

#[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
pub fn recvmmsg<'a, XS, S, I>(
    fd: RawFd,
    data: &'a mut MultiHeaders<S>,
    slices: XS,
    flags: MsgFlags,
    mut timeout: Option<crate::sys::time::TimeSpec>,
) -> crate::Result<MultiResults<'a, S>>
where
    XS: IntoIterator<Item = &'a mut I>,
    I: AsMut<[IoSliceMut<'a>]> + 'a,
{
    let mut count = 0;
    for (i, (slice, mmsghdr)) in slices.into_iter().zip(data.items.iter_mut()).enumerate() {
        let p = &mut mmsghdr.msg_hdr;
        p.msg_iov = slice.as_mut().as_mut_ptr().cast();
        p.msg_iovlen = slice.as_mut().len() as _;

        // Doing an unchecked addition is alright here, as the only way to obtain an instance of `MultiHeaders`
        // is through the `preallocate` function, which takes an `usize` as an argument to define its size,
        // which also provides an upper bound for the size of this zipped iterator. Thus, `i < usize::MAX` or in
        // other words: `count` doesn't overflow
        count = i + 1;
    }

    let timeout_ptr = timeout
        .as_mut()
        .map_or_else(std::ptr::null_mut, |t| t as *mut _ as *mut libc::timespec);

    // SAFETY: all pointers are guaranteed to be valid for the scope of this function. `count` does represent the
    // maximum number of messages that can be received safely (i.e. `count` is the minimum of the sizes of `slices` and `data.items`)
    let received = Errno::result(unsafe {
        libc::recvmmsg(
            fd,
            data.items.as_mut_ptr(),
            count as _,
            flags.bits() as _,
            timeout_ptr,
        )
    })? as usize;

    Ok(MultiResults {
        rmm: data,
        current_index: 0,
        received,
    })
}

/// Iterator over results of [`recvmmsg`]/[`sendmmsg`]
#[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
#[derive(Debug)]
pub struct MultiResults<'a, S> {
    // preallocated structures
    rmm: &'a MultiHeaders<S>,
    current_index: usize,
    received: usize,
}

#[cfg(any(linux_android, target_os = "freebsd", target_os = "netbsd"))]
impl<'a, S> Iterator for MultiResults<'a, S>
where
    S: Copy + SockaddrLike,
{
    type Item = RecvMsg<'a, 'a, S>;

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.received {
            return None;
        }
        let mmsghdr = self.rmm.items[self.current_index];

        // as long as we are not reading past the index writen by recvmmsg - address
        // will be initialized
        let address = unsafe { self.rmm.addresses[self.current_index].assume_init() };

        self.current_index += 1;
        Some(unsafe {
            read_mhdr(
                mmsghdr.msg_hdr,
                mmsghdr.msg_len as isize,
                self.rmm.msg_controllen,
                address,
            )
        })
    }
}

impl<'a, S> RecvMsg<'_, 'a, S> {
    /// Iterate over the filled io slices pointed by this msghdr
    pub fn iovs(&self) -> IoSliceIterator<'a> {
        IoSliceIterator {
            index: 0,
            remaining: self.bytes,
            slices: unsafe {
                // safe for as long as mgdr is properly initialized and references are valid.
                // for multi messages API we initialize it with an empty
                // slice and replace with a concrete buffer
                // for single message API we hold a lifetime reference to ioslices
                std::slice::from_raw_parts(self.mhdr.msg_iov as *const _, self.mhdr.msg_iovlen as _)
            },
        }
    }
}

#[derive(Debug)]
pub struct IoSliceIterator<'a> {
    index: usize,
    remaining: usize,
    slices: &'a [IoSlice<'a>],
}

impl<'a> Iterator for IoSliceIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.slices.len() {
            return None;
        }
        let slice = &self.slices[self.index][..self.remaining.min(self.slices[self.index].len())];
        self.remaining -= slice.len();
        self.index += 1;
        if slice.is_empty() {
            return None;
        }

        Some(slice)
    }
}

unsafe fn read_mhdr<'a, 'i, S>(
    mhdr: msghdr,
    r: isize,
    msg_controllen: usize,
    mut address: S,
) -> RecvMsg<'a, 'i, S>
    where S: SockaddrLike
{
    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    let cmsghdr = {
        let ptr = if mhdr.msg_controllen > 0 {
            debug_assert!(!mhdr.msg_control.is_null());
            debug_assert!(msg_controllen >= mhdr.msg_controllen as usize);
            unsafe { CMSG_FIRSTHDR(&mhdr as *const msghdr) }
        } else {
            ptr::null()
        };

        unsafe {
            ptr.as_ref()
        }
    };

    // Ignore errors if this socket address has statically-known length
    //
    // This is to ensure that unix socket addresses have their length set appropriately.
    let _ = unsafe { address.set_length(mhdr.msg_namelen as usize) };

    RecvMsg {
        bytes: r as usize,
        cmsghdr,
        address: Some(address),
        flags: MsgFlags::from_bits_truncate(mhdr.msg_flags),
        mhdr,
        iobufs: std::marker::PhantomData,
    }
}

/// Pack pointers to various structures into into msghdr
///
/// # Safety
/// `iov_buffer` and `iov_buffer_len` must point to a slice
/// of `IoSliceMut` and number of available elements or be a null pointer and 0
///
/// `cmsg_buffer` and `cmsg_capacity` must point to a byte buffer used
/// to store control headers later or be a null pointer and 0 if control
/// headers are not used
///
/// Buffers must remain valid for the whole lifetime of msghdr
unsafe fn pack_mhdr_to_receive<S>(
    iov_buffer: *mut IoSliceMut,
    iov_buffer_len: usize,
    cmsg_buffer: *mut u8,
    cmsg_capacity: usize,
    address: *mut S,
) -> msghdr
    where
        S: SockaddrLike
{
    // Musl's msghdr has private fields, so this is the only way to
    // initialize it.
    let mut mhdr = mem::MaybeUninit::<msghdr>::zeroed();
    let p = mhdr.as_mut_ptr();
    unsafe {
        (*p).msg_name = address as *mut c_void;
        (*p).msg_namelen = S::size();
        (*p).msg_iov = iov_buffer as *mut iovec;
        (*p).msg_iovlen = iov_buffer_len as _;
        (*p).msg_control = cmsg_buffer as *mut c_void;
        (*p).msg_controllen = cmsg_capacity as _;
        (*p).msg_flags = 0;
        mhdr.assume_init()
    }
}

fn pack_mhdr_to_send<'a, I, C, S>(
    cmsg_buffer: &mut [u8],
    iov: I,
    cmsgs: C,
    addr: Option<&S>
) -> msghdr
    where
        I: AsRef<[IoSlice<'a>]>,
        C: AsRef<[ControlMessage<'a>]>,
        S: SockaddrLike + 'a
{
    let capacity = cmsg_buffer.len();

    // The message header must be initialized before the individual cmsgs.
    let cmsg_ptr = if capacity > 0 {
        cmsg_buffer.as_mut_ptr().cast()
    } else {
        ptr::null_mut()
    };

    let mhdr = unsafe {
        // Musl's msghdr has private fields, so this is the only way to
        // initialize it.
        let mut mhdr = mem::MaybeUninit::<msghdr>::zeroed();
        let p = mhdr.as_mut_ptr();
        (*p).msg_name = addr.map(S::as_ptr).unwrap_or(ptr::null()).cast_mut().cast();
        (*p).msg_namelen = addr.map(S::len).unwrap_or(0);
        // transmute iov into a mutable pointer.  sendmsg doesn't really mutate
        // the buffer, but the standard says that it takes a mutable pointer
        (*p).msg_iov = iov.as_ref().as_ptr().cast_mut().cast();
        (*p).msg_iovlen = iov.as_ref().len() as _;
        (*p).msg_control = cmsg_ptr;
        (*p).msg_controllen = capacity as _;
        (*p).msg_flags = 0;
        mhdr.assume_init()
    };

    // Encode each cmsg.  This must happen after initializing the header because
    // CMSG_NEXT_HDR and friends read the msg_control and msg_controllen fields.
    // CMSG_FIRSTHDR is always safe
    let mut pmhdr: *mut cmsghdr = unsafe { CMSG_FIRSTHDR(&mhdr as *const msghdr) };
    for cmsg in cmsgs.as_ref() {
        assert_ne!(pmhdr, ptr::null_mut());
        // Safe because we know that pmhdr is valid, and we initialized it with
        // sufficient space
        unsafe { cmsg.encode_into(pmhdr) };
        // Safe because mhdr is valid
        pmhdr = unsafe { CMSG_NXTHDR(&mhdr as *const msghdr, pmhdr) };
    }

    mhdr
}

/// Receive message in scatter-gather vectors from a socket, and
/// optionally receive ancillary data into the provided buffer.
/// If no ancillary data is desired, use () as the type parameter.
///
/// # Arguments
///
/// * `fd`:             Socket file descriptor
/// * `iov`:            Scatter-gather list of buffers to receive the message
/// * `cmsg_buffer`:    Space to receive ancillary data.  Should be created by
///                     [`cmsg_space!`](../../macro.cmsg_space.html)
/// * `flags`:          Optional flags passed directly to the operating system.
///
/// # References
/// [recvmsg(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/recvmsg.html)
pub fn recvmsg<'a, 'outer, 'inner, S>(fd: RawFd, iov: &'outer mut [IoSliceMut<'inner>],
                   mut cmsg_buffer: Option<&'a mut Vec<u8>>,
                   flags: MsgFlags) -> Result<RecvMsg<'a, 'outer, S>>
    where S: SockaddrLike + 'a,
    'inner: 'outer
{
    let mut address = mem::MaybeUninit::uninit();

    let (msg_control, msg_controllen) = cmsg_buffer.as_mut()
        .map(|v| (v.as_mut_ptr(), v.capacity()))
        .unwrap_or((ptr::null_mut(), 0));
    let mut mhdr = unsafe {
        pack_mhdr_to_receive(iov.as_mut().as_mut_ptr(), iov.len(), msg_control, msg_controllen, address.as_mut_ptr())
    };

    let ret = unsafe { libc::recvmsg(fd, &mut mhdr, flags.bits()) };

    let r = Errno::result(ret)?;

    Ok(unsafe { read_mhdr(mhdr, r, msg_controllen, address.assume_init()) })
}
}

/// Create an endpoint for communication
///
/// The `protocol` specifies a particular protocol to be used with the
/// socket.  Normally only a single protocol exists to support a
/// particular socket type within a given protocol family, in which case
/// protocol can be specified as `None`.  However, it is possible that many
/// protocols may exist, in which case a particular protocol must be
/// specified in this manner.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/socket.html)
pub fn socket<T: Into<Option<SockProtocol>>>(
    domain: AddressFamily,
    ty: SockType,
    flags: SockFlag,
    protocol: T,
) -> Result<OwnedFd> {
    let protocol = match protocol.into() {
        None => 0,
        Some(p) => p as c_int,
    };

    // SockFlags are usually embedded into `ty`, but we don't do that in `nix` because it's a
    // little easier to understand by separating it out. So we have to merge these bitfields
    // here.
    let mut ty = ty as c_int;
    ty |= flags.bits();

    let res = unsafe { libc::socket(domain as c_int, ty, protocol) };

    match res {
        -1 => Err(Errno::last()),
        fd => {
            // Safe because libc::socket returned success
            unsafe { Ok(OwnedFd::from_raw_fd(fd)) }
        }
    }
}

/// Create a pair of connected sockets
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/socketpair.html)
pub fn socketpair<T: Into<Option<SockProtocol>>>(
    domain: AddressFamily,
    ty: SockType,
    protocol: T,
    flags: SockFlag,
) -> Result<(OwnedFd, OwnedFd)> {
    let protocol = match protocol.into() {
        None => 0,
        Some(p) => p as c_int,
    };

    // SockFlags are usually embedded into `ty`, but we don't do that in `nix` because it's a
    // little easier to understand by separating it out. So we have to merge these bitfields
    // here.
    let mut ty = ty as c_int;
    ty |= flags.bits();

    let mut fds = [-1, -1];

    let res = unsafe {
        libc::socketpair(domain as c_int, ty, protocol, fds.as_mut_ptr())
    };
    Errno::result(res)?;

    // Safe because socketpair returned success.
    unsafe { Ok((OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1]))) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backlog(i32);

impl Backlog {
    /// Sets the listen queue size to system `SOMAXCONN` value
    pub const MAXCONN: Self = Self(libc::SOMAXCONN);
    /// Sets the listen queue size to -1 for system supporting it
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub const MAXALLOWABLE: Self = Self(-1);

    /// Create a `Backlog`, an `EINVAL` will be returned if `val` is invalid.
    pub fn new<I: Into<i32>>(val: I) -> Result<Self> {
        cfg_if! {
            if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
                const MIN: i32 = -1;
            } else {
                const MIN: i32 = 0;
            }
        }

        let val = val.into();

        if !(MIN..Self::MAXCONN.0).contains(&val) {
            return Err(Errno::EINVAL);
        }

        Ok(Self(val))
    }
}

impl From<Backlog> for i32 {
    fn from(backlog: Backlog) -> Self {
        backlog.0
    }
}

/// Listen for connections on a socket
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/listen.html)
pub fn listen<F: AsFd>(sock: &F, backlog: Backlog) -> Result<()> {
    let fd = sock.as_fd().as_raw_fd();
    let res = unsafe { libc::listen(fd, backlog.into()) };

    Errno::result(res).map(drop)
}

/// Bind a name to a socket
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/bind.html)
pub fn bind(fd: RawFd, addr: &dyn SockaddrLike) -> Result<()> {
    let res = unsafe { libc::bind(fd, addr.as_ptr(), addr.len()) };

    Errno::result(res).map(drop)
}

/// Accept a connection on a socket
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/accept.html)
pub fn accept(sockfd: RawFd) -> Result<RawFd> {
    let res = unsafe { libc::accept(sockfd, ptr::null_mut(), ptr::null_mut()) };

    Errno::result(res)
}

/// Accept a connection on a socket
///
/// [Further reading](https://man7.org/linux/man-pages/man2/accept.2.html)
#[cfg(any(
    all(
        target_os = "android",
        any(
            target_arch = "aarch64",
            target_arch = "x86",
            target_arch = "x86_64"
        )
    ),
    freebsdlike,
    netbsdlike,
    target_os = "emscripten",
    target_os = "fuchsia",
    solarish,
    target_os = "linux",
))]
pub fn accept4(sockfd: RawFd, flags: SockFlag) -> Result<RawFd> {
    let res = unsafe {
        libc::accept4(sockfd, ptr::null_mut(), ptr::null_mut(), flags.bits())
    };

    Errno::result(res)
}

/// Initiate a connection on a socket
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/connect.html)
pub fn connect(fd: RawFd, addr: &dyn SockaddrLike) -> Result<()> {
    let res = unsafe { libc::connect(fd, addr.as_ptr(), addr.len()) };

    Errno::result(res).map(drop)
}

/// Receive data from a connection-oriented socket. Returns the number of
/// bytes read
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/recv.html)
pub fn recv(sockfd: RawFd, buf: &mut [u8], flags: MsgFlags) -> Result<usize> {
    unsafe {
        let ret = libc::recv(
            sockfd,
            buf.as_mut_ptr().cast(),
            buf.len() as size_t,
            flags.bits(),
        );

        Errno::result(ret).map(|r| r as usize)
    }
}

/// Receive data from a connectionless or connection-oriented socket. Returns
/// the number of bytes read and, for connectionless sockets,  the socket
/// address of the sender.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/recvfrom.html)
pub fn recvfrom<T: SockaddrLike>(
    sockfd: RawFd,
    buf: &mut [u8],
) -> Result<(usize, Option<T>)> {
    unsafe {
        let mut addr = mem::MaybeUninit::<T>::uninit();
        let mut len = mem::size_of_val(&addr) as socklen_t;

        let ret = Errno::result(libc::recvfrom(
            sockfd,
            buf.as_mut_ptr().cast(),
            buf.len() as size_t,
            0,
            addr.as_mut_ptr().cast(),
            &mut len as *mut socklen_t,
        ))? as usize;

        Ok((ret, T::from_raw(addr.assume_init().as_ptr(), Some(len))))
    }
}

/// Send a message to a socket
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/sendto.html)
pub fn sendto(
    fd: RawFd,
    buf: &[u8],
    addr: &dyn SockaddrLike,
    flags: MsgFlags,
) -> Result<usize> {
    let ret = unsafe {
        libc::sendto(
            fd,
            buf.as_ptr().cast(),
            buf.len() as size_t,
            flags.bits(),
            addr.as_ptr(),
            addr.len(),
        )
    };

    Errno::result(ret).map(|r| r as usize)
}

/// Send data to a connection-oriented socket. Returns the number of bytes read
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/send.html)
pub fn send(fd: RawFd, buf: &[u8], flags: MsgFlags) -> Result<usize> {
    let ret = unsafe {
        libc::send(fd, buf.as_ptr().cast(), buf.len() as size_t, flags.bits())
    };

    Errno::result(ret).map(|r| r as usize)
}

/*
 *
 * ===== Socket Options =====
 *
 */

/// Represents a socket option that can be retrieved.
pub trait GetSockOpt: Copy {
    type Val;

    /// Look up the value of this socket option on the given socket.
    fn get<F: AsFd>(&self, fd: &F) -> Result<Self::Val>;
}

/// Represents a socket option that can be set.
pub trait SetSockOpt: Clone {
    type Val;

    /// Set the value of this socket option on the given socket.
    fn set<F: AsFd>(&self, fd: &F, val: &Self::Val) -> Result<()>;
}

/// Get the current value for the requested socket option
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/getsockopt.html)
pub fn getsockopt<F: AsFd, O: GetSockOpt>(fd: &F, opt: O) -> Result<O::Val> {
    opt.get(fd)
}

/// Sets the value for the requested socket option
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/setsockopt.html)
///
/// # Examples
///
/// ```
/// use nix::sys::socket::setsockopt;
/// use nix::sys::socket::sockopt::KeepAlive;
/// use std::net::TcpListener;
///
/// let listener = TcpListener::bind("0.0.0.0:0").unwrap();
/// let fd = listener;
/// let res = setsockopt(&fd, KeepAlive, &true);
/// assert!(res.is_ok());
/// ```
pub fn setsockopt<F: AsFd, O: SetSockOpt>(
    fd: &F,
    opt: O,
    val: &O::Val,
) -> Result<()> {
    opt.set(fd, val)
}

/// Get the address of the peer connected to the socket `fd`.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/getpeername.html)
pub fn getpeername<T: SockaddrLike>(fd: RawFd) -> Result<T> {
    unsafe {
        let mut addr = mem::MaybeUninit::<T>::uninit();
        let mut len = T::size();

        let ret = libc::getpeername(fd, addr.as_mut_ptr().cast(), &mut len);

        Errno::result(ret)?;

        T::from_raw(addr.assume_init().as_ptr(), Some(len)).ok_or(Errno::EINVAL)
    }
}

/// Get the current address to which the socket `fd` is bound.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/getsockname.html)
pub fn getsockname<T: SockaddrLike>(fd: RawFd) -> Result<T> {
    unsafe {
        let mut addr = mem::MaybeUninit::<T>::uninit();
        let mut len = T::size();

        let ret = libc::getsockname(fd, addr.as_mut_ptr().cast(), &mut len);

        Errno::result(ret)?;

        T::from_raw(addr.assume_init().as_ptr(), Some(len)).ok_or(Errno::EINVAL)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Shutdown {
    /// Further receptions will be disallowed.
    Read,
    /// Further  transmissions will be disallowed.
    Write,
    /// Further receptions and transmissions will be disallowed.
    Both,
}

/// Shut down part of a full-duplex connection.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/shutdown.html)
pub fn shutdown(df: RawFd, how: Shutdown) -> Result<()> {
    unsafe {
        use libc::shutdown;

        let how = match how {
            Shutdown::Read => libc::SHUT_RD,
            Shutdown::Write => libc::SHUT_WR,
            Shutdown::Both => libc::SHUT_RDWR,
        };

        Errno::result(shutdown(df, how)).map(drop)
    }
}

