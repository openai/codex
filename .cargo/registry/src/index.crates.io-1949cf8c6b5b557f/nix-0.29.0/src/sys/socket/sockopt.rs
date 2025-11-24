//! Socket options as used by `setsockopt` and `getsockopt`.
use super::{GetSockOpt, SetSockOpt};
use crate::errno::Errno;
use crate::sys::time::TimeVal;
use crate::Result;
use cfg_if::cfg_if;
use libc::{self, c_int, c_void, socklen_t};
use std::ffi::{CStr, CString, OsStr, OsString};
use std::mem::{self, MaybeUninit};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsFd, AsRawFd};

// Constants
// TCP_CA_NAME_MAX isn't defined in user space include files
#[cfg(any(target_os = "freebsd", target_os = "linux"))]
#[cfg(feature = "net")]
const TCP_CA_NAME_MAX: usize = 16;

/// Helper for implementing `SetSockOpt` for a given socket option. See
/// [`::sys::socket::SetSockOpt`](sys/socket/trait.SetSockOpt.html).
///
/// This macro aims to help implementing `SetSockOpt` for different socket options that accept
/// different kinds of data to be used with `setsockopt`.
///
/// Instead of using this macro directly consider using `sockopt_impl!`, especially if the option
/// you are implementing represents a simple type.
///
/// # Arguments
///
/// * `$name:ident`: name of the type you want to implement `SetSockOpt` for.
/// * `$level:expr` : socket layer, or a `protocol level`: could be *raw sockets*
///    (`libc::SOL_SOCKET`), *ip protocol* (libc::IPPROTO_IP), *tcp protocol* (`libc::IPPROTO_TCP`),
///    and more. Please refer to your system manual for more options. Will be passed as the second
///    argument (`level`) to the `setsockopt` call.
/// * `$flag:path`: a flag name to set. Some examples: `libc::SO_REUSEADDR`, `libc::TCP_NODELAY`,
///    `libc::IP_ADD_MEMBERSHIP` and others. Will be passed as the third argument (`option_name`)
///    to the `setsockopt` call.
/// * Type of the value that you are going to set.
/// * Type that implements the `Set` trait for the type from the previous item (like `SetBool` for
///    `bool`, `SetUsize` for `usize`, etc.).
macro_rules! setsockopt_impl {
    ($name:ident, $level:expr, $flag:path, $ty:ty, $setter:ty) => {
        impl SetSockOpt for $name {
            type Val = $ty;

            fn set<F: AsFd>(&self, fd: &F, val: &$ty) -> Result<()> {
                unsafe {
                    let setter: $setter = Set::new(val);

                    let res = libc::setsockopt(
                        fd.as_fd().as_raw_fd(),
                        $level,
                        $flag,
                        setter.ffi_ptr(),
                        setter.ffi_len(),
                    );
                    Errno::result(res).map(drop)
                }
            }
        }
    };
}

/// Helper for implementing `GetSockOpt` for a given socket option. See
/// [`::sys::socket::GetSockOpt`](sys/socket/trait.GetSockOpt.html).
///
/// This macro aims to help implementing `GetSockOpt` for different socket options that accept
/// different kinds of data to be use with `getsockopt`.
///
/// Instead of using this macro directly consider using `sockopt_impl!`, especially if the option
/// you are implementing represents a simple type.
///
/// # Arguments
///
/// * Name of the type you want to implement `GetSockOpt` for.
/// * Socket layer, or a `protocol level`: could be *raw sockets* (`lic::SOL_SOCKET`),  *ip
///    protocol* (libc::IPPROTO_IP), *tcp protocol* (`libc::IPPROTO_TCP`),  and more. Please refer
///    to your system manual for more options. Will be passed as the second argument (`level`) to
///    the `getsockopt` call.
/// * A flag to set. Some examples: `libc::SO_REUSEADDR`, `libc::TCP_NODELAY`,
///    `libc::SO_ORIGINAL_DST` and others. Will be passed as the third argument (`option_name`) to
///    the `getsockopt` call.
/// * Type of the value that you are going to get.
/// * Type that implements the `Get` trait for the type from the previous item (`GetBool` for
///    `bool`, `GetUsize` for `usize`, etc.).
macro_rules! getsockopt_impl {
    ($name:ident, $level:expr, $flag:path, $ty:ty, $getter:ty) => {
        impl GetSockOpt for $name {
            type Val = $ty;

            fn get<F: AsFd>(&self, fd: &F) -> Result<$ty> {
                unsafe {
                    let mut getter: $getter = Get::uninit();

                    let res = libc::getsockopt(
                        fd.as_fd().as_raw_fd(),
                        $level,
                        $flag,
                        getter.ffi_ptr(),
                        getter.ffi_len(),
                    );
                    Errno::result(res)?;

                    match <$ty>::try_from(getter.assume_init()) {
                        Err(_) => Err(Errno::EINVAL),
                        Ok(r) => Ok(r),
                    }
                }
            }
        }
    };
}

/// Helper to generate the sockopt accessors. See
/// [`::sys::socket::GetSockOpt`](sys/socket/trait.GetSockOpt.html) and
/// [`::sys::socket::SetSockOpt`](sys/socket/trait.SetSockOpt.html).
///
/// This macro aims to help implementing `GetSockOpt` and `SetSockOpt` for different socket options
/// that accept different kinds of data to be use with `getsockopt` and `setsockopt` respectively.
///
/// Basically this macro wraps up the [`getsockopt_impl!`](macro.getsockopt_impl.html) and
/// [`setsockopt_impl!`](macro.setsockopt_impl.html) macros.
///
/// # Arguments
///
/// * `GetOnly`, `SetOnly` or `Both`: whether you want to implement only getter, only setter or
///    both of them.
/// * `$name:ident`: name of type `GetSockOpt`/`SetSockOpt` will be implemented for.
/// * `$level:expr` : socket layer, or a `protocol level`: could be *raw sockets*
///    (`libc::SOL_SOCKET`), *ip protocol* (libc::IPPROTO_IP), *tcp protocol* (`libc::IPPROTO_TCP`),
///    and more. Please refer to your system manual for more options. Will be passed as the second
///    argument (`level`) to the `getsockopt`/`setsockopt` call.
/// * `$flag:path`: a flag name to set. Some examples: `libc::SO_REUSEADDR`, `libc::TCP_NODELAY`,
///    `libc::IP_ADD_MEMBERSHIP` and others. Will be passed as the third argument (`option_name`)
///    to the `setsockopt`/`getsockopt` call.
/// * `$ty:ty`: type of the value that will be get/set.
/// * `$getter:ty`: `Get` implementation; optional; only for `GetOnly` and `Both`.
/// * `$setter:ty`: `Set` implementation; optional; only for `SetOnly` and `Both`.
// Some targets don't use all rules.
#[allow(unused_macro_rules)]
macro_rules! sockopt_impl {
    ($(#[$attr:meta])* $name:ident, GetOnly, $level:expr, $flag:path, bool) => {
        sockopt_impl!($(#[$attr])*
                      $name, GetOnly, $level, $flag, bool, GetBool);
    };

    ($(#[$attr:meta])* $name:ident, GetOnly, $level:expr, $flag:path, u8) => {
        sockopt_impl!($(#[$attr])* $name, GetOnly, $level, $flag, u8, GetU8);
    };

    ($(#[$attr:meta])* $name:ident, GetOnly, $level:expr, $flag:path, usize) =>
    {
        sockopt_impl!($(#[$attr])*
                      $name, GetOnly, $level, $flag, usize, GetUsize);
    };

    ($(#[$attr:meta])* $name:ident, SetOnly, $level:expr, $flag:path, bool) => {
        sockopt_impl!($(#[$attr])*
                      $name, SetOnly, $level, $flag, bool, SetBool);
    };

    ($(#[$attr:meta])* $name:ident, SetOnly, $level:expr, $flag:path, u8) => {
        sockopt_impl!($(#[$attr])* $name, SetOnly, $level, $flag, u8, SetU8);
    };

    ($(#[$attr:meta])* $name:ident, SetOnly, $level:expr, $flag:path, usize) =>
    {
        sockopt_impl!($(#[$attr])*
                      $name, SetOnly, $level, $flag, usize, SetUsize);
    };

    ($(#[$attr:meta])* $name:ident, Both, $level:expr, $flag:path, bool) => {
        sockopt_impl!($(#[$attr])*
                      $name, Both, $level, $flag, bool, GetBool, SetBool);
    };

    ($(#[$attr:meta])* $name:ident, Both, $level:expr, $flag:path, u8) => {
        sockopt_impl!($(#[$attr])*
                      $name, Both, $level, $flag, u8, GetU8, SetU8);
    };

    ($(#[$attr:meta])* $name:ident, Both, $level:expr, $flag:path, usize) => {
        sockopt_impl!($(#[$attr])*
                      $name, Both, $level, $flag, usize, GetUsize, SetUsize);
    };

    ($(#[$attr:meta])* $name:ident, Both, $level:expr, $flag:path,
     OsString<$array:ty>) =>
    {
        sockopt_impl!($(#[$attr])*
                      $name, Both, $level, $flag, OsString, GetOsString<$array>,
                      SetOsString);
    };

    /*
     * Matchers with generic getter types must be placed at the end, so
     * they'll only match _after_ specialized matchers fail
     */
    ($(#[$attr:meta])* $name:ident, GetOnly, $level:expr, $flag:path, $ty:ty) =>
    {
        sockopt_impl!($(#[$attr])*
                      $name, GetOnly, $level, $flag, $ty, GetStruct<$ty>);
    };

    ($(#[$attr:meta])* $name:ident, GetOnly, $level:expr, $flag:path, $ty:ty,
     $getter:ty) =>
    {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name;

        getsockopt_impl!($name, $level, $flag, $ty, $getter);
    };

    ($(#[$attr:meta])* $name:ident, SetOnly, $level:expr, $flag:path, $ty:ty) =>
    {
        sockopt_impl!($(#[$attr])*
                      $name, SetOnly, $level, $flag, $ty, SetStruct<$ty>);
    };

    ($(#[$attr:meta])* $name:ident, SetOnly, $level:expr, $flag:path, $ty:ty,
     $setter:ty) =>
    {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name;

        setsockopt_impl!($name, $level, $flag, $ty, $setter);
    };

    ($(#[$attr:meta])* $name:ident, Both, $level:expr, $flag:path, $ty:ty,
     $getter:ty, $setter:ty) =>
    {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name;

        setsockopt_impl!($name, $level, $flag, $ty, $setter);
        getsockopt_impl!($name, $level, $flag, $ty, $getter);
    };

    ($(#[$attr:meta])* $name:ident, Both, $level:expr, $flag:path, $ty:ty) => {
        sockopt_impl!($(#[$attr])*
                      $name, Both, $level, $flag, $ty, GetStruct<$ty>,
                      SetStruct<$ty>);
    };
}

/*
 *
 * ===== Define sockopts =====
 *
 */

sockopt_impl!(
    /// Enables local address reuse
    ReuseAddr,
    Both,
    libc::SOL_SOCKET,
    libc::SO_REUSEADDR,
    bool
);
#[cfg(not(solarish))]
sockopt_impl!(
    /// Permits multiple AF_INET or AF_INET6 sockets to be bound to an
    /// identical socket address.
    ReusePort,
    Both,
    libc::SOL_SOCKET,
    libc::SO_REUSEPORT,
    bool
);
#[cfg(target_os = "freebsd")]
sockopt_impl!(
    /// Enables incoming connections to be distributed among N sockets (up to 256)
    /// via a Load-Balancing hash based algorithm.
    ReusePortLb,
    Both,
    libc::SOL_SOCKET,
    libc::SO_REUSEPORT_LB,
    bool
);
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Under most circumstances, TCP sends data when it is presented; when
    /// outstanding data has not yet been acknowledged, it gathers small amounts
    /// of output to be sent in a single packet once an acknowledgement is
    /// received.  For a small number of clients, such as window systems that
    /// send a stream of mouse events which receive no replies, this
    /// packetization may cause significant delays.  The boolean option
    /// TCP_NODELAY defeats this algorithm.
    TcpNoDelay,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_NODELAY,
    bool
);
sockopt_impl!(
    /// When enabled,  a close(2) or shutdown(2) will not return until all
    /// queued messages for the socket have been successfully sent or the
    /// linger timeout has been reached.
    Linger,
    Both,
    libc::SOL_SOCKET,
    libc::SO_LINGER,
    libc::linger
);
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Join a multicast group
    IpAddMembership,
    SetOnly,
    libc::IPPROTO_IP,
    libc::IP_ADD_MEMBERSHIP,
    super::IpMembershipRequest
);
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Leave a multicast group.
    IpDropMembership,
    SetOnly,
    libc::IPPROTO_IP,
    libc::IP_DROP_MEMBERSHIP,
    super::IpMembershipRequest
);
cfg_if! {
    if #[cfg(linux_android)] {
        #[cfg(feature = "net")]
        sockopt_impl!(
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            /// Join an IPv6 multicast group.
            Ipv6AddMembership, SetOnly, libc::IPPROTO_IPV6, libc::IPV6_ADD_MEMBERSHIP, super::Ipv6MembershipRequest);
        #[cfg(feature = "net")]
        sockopt_impl!(
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            /// Leave an IPv6 multicast group.
            Ipv6DropMembership, SetOnly, libc::IPPROTO_IPV6, libc::IPV6_DROP_MEMBERSHIP, super::Ipv6MembershipRequest);
    } else if #[cfg(any(bsd, solarish))] {
        #[cfg(feature = "net")]
        sockopt_impl!(
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            /// Join an IPv6 multicast group.
            Ipv6AddMembership, SetOnly, libc::IPPROTO_IPV6,
            libc::IPV6_JOIN_GROUP, super::Ipv6MembershipRequest);
        #[cfg(feature = "net")]
        sockopt_impl!(
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            /// Leave an IPv6 multicast group.
            Ipv6DropMembership, SetOnly, libc::IPPROTO_IPV6,
            libc::IPV6_LEAVE_GROUP, super::Ipv6MembershipRequest);
    }
}
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Set or read the time-to-live value of outgoing multicast packets for
    /// this socket.
    IpMulticastTtl,
    Both,
    libc::IPPROTO_IP,
    libc::IP_MULTICAST_TTL,
    u8
);
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Set or read the hop limit value of outgoing IPv6 multicast packets for
    /// this socket.
    Ipv6MulticastHops,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_MULTICAST_HOPS,
    libc::c_int
);
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Set or read a boolean integer argument that determines whether sent
    /// multicast packets should be looped back to the local sockets.
    IpMulticastLoop,
    Both,
    libc::IPPROTO_IP,
    libc::IP_MULTICAST_LOOP,
    bool
);
#[cfg(target_os = "linux")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Set the protocol-defined priority for all packets to be
    /// sent on this socket
    Priority,
    Both,
    libc::SOL_SOCKET,
    libc::SO_PRIORITY,
    libc::c_int
);
#[cfg(target_os = "linux")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Set or receive the Type-Of-Service (TOS) field that is
    /// sent with every IP packet originating from this socket
    IpTos,
    Both,
    libc::IPPROTO_IP,
    libc::IP_TOS,
    libc::c_int
);
#[cfg(target_os = "linux")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Traffic class associated with outgoing packets
    Ipv6TClass,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_TCLASS,
    libc::c_int
);
#[cfg(any(linux_android, target_os = "fuchsia"))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// If enabled, this boolean option allows binding to an IP address that
    /// is nonlocal or does not (yet) exist.
    IpFreebind,
    Both,
    libc::IPPROTO_IP,
    libc::IP_FREEBIND,
    bool
);
#[cfg(linux_android)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// If enabled, the kernel will not reserve an ephemeral port when binding
    /// socket with a port number of 0. The port will later be automatically
    /// chosen at connect time, in a way that allows sharing a source port as
    /// long as the 4-tuple is unique.
    IpBindAddressNoPort,
    Both,
    libc::IPPROTO_IP,
    libc::IP_BIND_ADDRESS_NO_PORT,
    bool
);
sockopt_impl!(
    /// Specify the receiving timeout until reporting an error.
    ReceiveTimeout,
    Both,
    libc::SOL_SOCKET,
    libc::SO_RCVTIMEO,
    TimeVal
);
sockopt_impl!(
    /// Specify the sending timeout until reporting an error.
    SendTimeout,
    Both,
    libc::SOL_SOCKET,
    libc::SO_SNDTIMEO,
    TimeVal
);
sockopt_impl!(
    /// Set or get the broadcast flag.
    Broadcast,
    Both,
    libc::SOL_SOCKET,
    libc::SO_BROADCAST,
    bool
);
sockopt_impl!(
    /// If this option is enabled, out-of-band data is directly placed into
    /// the receive data stream.
    OobInline,
    Both,
    libc::SOL_SOCKET,
    libc::SO_OOBINLINE,
    bool
);
sockopt_impl!(
    /// Get and clear the pending socket error.
    SocketError,
    GetOnly,
    libc::SOL_SOCKET,
    libc::SO_ERROR,
    i32
);
sockopt_impl!(
    /// Set or get the don't route flag.
    DontRoute,
    Both,
    libc::SOL_SOCKET,
    libc::SO_DONTROUTE,
    bool
);
sockopt_impl!(
    /// Enable sending of keep-alive messages on connection-oriented sockets.
    KeepAlive,
    Both,
    libc::SOL_SOCKET,
    libc::SO_KEEPALIVE,
    bool
);
#[cfg(any(freebsdlike, apple_targets))]
sockopt_impl!(
    /// Get the credentials of the peer process of a connected unix domain
    /// socket.
    LocalPeerCred,
    GetOnly,
    0,
    libc::LOCAL_PEERCRED,
    super::XuCred
);
#[cfg(apple_targets)]
sockopt_impl!(
    /// Get the PID of the peer process of a connected unix domain socket.
    LocalPeerPid,
    GetOnly,
    0,
    libc::LOCAL_PEERPID,
    libc::c_int
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Return the credentials of the foreign process connected to this socket.
    PeerCredentials,
    GetOnly,
    libc::SOL_SOCKET,
    libc::SO_PEERCRED,
    super::UnixCredentials
);
#[cfg(target_os = "freebsd")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Get backlog limit of the socket
    ListenQLimit,
    GetOnly,
    libc::SOL_SOCKET,
    libc::SO_LISTENQLIMIT,
    u32
);
#[cfg(apple_targets)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Specify the amount of time, in seconds, that the connection must be idle
    /// before keepalive probes (if enabled) are sent.
    TcpKeepAlive,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_KEEPALIVE,
    u32
);
#[cfg(any(freebsdlike, linux_android))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The time (in seconds) the connection needs to remain idle before TCP
    /// starts sending keepalive probes
    TcpKeepIdle,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_KEEPIDLE,
    u32
);
cfg_if! {
    if #[cfg(linux_android)] {
        sockopt_impl!(
            /// The maximum segment size for outgoing TCP packets.
            TcpMaxSeg, Both, libc::IPPROTO_TCP, libc::TCP_MAXSEG, u32);
    } else if #[cfg(not(target_os = "redox"))] {
        sockopt_impl!(
            /// The maximum segment size for outgoing TCP packets.
            TcpMaxSeg, GetOnly, libc::IPPROTO_TCP, libc::TCP_MAXSEG, u32);
    }
}
#[cfg(not(any(
    target_os = "openbsd",
    target_os = "haiku",
    target_os = "redox"
)))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The maximum number of keepalive probes TCP should send before
    /// dropping the connection.
    TcpKeepCount,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_KEEPCNT,
    u32
);
#[cfg(any(linux_android, target_os = "fuchsia"))]
sockopt_impl!(
    #[allow(missing_docs)]
    // Not documented by Linux!
    TcpRepair,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_REPAIR,
    u32
);
#[cfg(not(any(
    target_os = "openbsd",
    target_os = "haiku",
    target_os = "redox"
)))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The time (in seconds) between individual keepalive probes.
    TcpKeepInterval,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_KEEPINTVL,
    u32
);
#[cfg(any(target_os = "fuchsia", target_os = "linux"))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Specifies the maximum amount of time in milliseconds that transmitted
    /// data may remain unacknowledged before TCP will forcibly close the
    /// corresponding connection
    TcpUserTimeout,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_USER_TIMEOUT,
    u32
);
#[cfg(linux_android)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Enables TCP Fast Open (RFC 7413) on a connecting socket. If a fast open
    /// cookie is not available (first attempt to connect), `connect` syscall
    /// will behave as usual, except for internally trying to solicit a cookie
    /// from remote peer. When cookie is available, the next `connect` syscall
    /// will immediately succeed without actually establishing TCP connection.
    /// The connection establishment will be defered till the next `write` or
    /// `sendmsg` syscalls on the socket, allowing TCP prtocol to establish
    /// connection and send data in the same packets. Note: calling `read` right
    /// after `connect` without `write` on the socket will cause the blocking
    /// socket to be blocked forever.
    TcpFastOpenConnect,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_FASTOPEN_CONNECT,
    bool
);
sockopt_impl!(
    /// Sets or gets the maximum socket receive buffer in bytes.
    RcvBuf,
    Both,
    libc::SOL_SOCKET,
    libc::SO_RCVBUF,
    usize
);
sockopt_impl!(
    /// Sets or gets the maximum socket send buffer in bytes.
    SndBuf,
    Both,
    libc::SOL_SOCKET,
    libc::SO_SNDBUF,
    usize
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Using this socket option, a privileged (`CAP_NET_ADMIN`) process can
    /// perform the same task as `SO_RCVBUF`, but the `rmem_max limit` can be
    /// overridden.
    RcvBufForce,
    SetOnly,
    libc::SOL_SOCKET,
    libc::SO_RCVBUFFORCE,
    usize
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Using this socket option, a privileged (`CAP_NET_ADMIN`)  process can
    /// perform the same task as `SO_SNDBUF`, but the `wmem_max` limit can be
    /// overridden.
    SndBufForce,
    SetOnly,
    libc::SOL_SOCKET,
    libc::SO_SNDBUFFORCE,
    usize
);
sockopt_impl!(
    /// Gets the socket type as an integer.
    SockType,
    GetOnly,
    libc::SOL_SOCKET,
    libc::SO_TYPE,
    super::SockType,
    GetStruct<i32>
);
sockopt_impl!(
    /// Returns a value indicating whether or not this socket has been marked to
    /// accept connections with `listen(2)`.
    AcceptConn,
    GetOnly,
    libc::SOL_SOCKET,
    libc::SO_ACCEPTCONN,
    bool
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Bind this socket to a particular device like “eth0”.
    BindToDevice,
    Both,
    libc::SOL_SOCKET,
    libc::SO_BINDTODEVICE,
    OsString<[u8; libc::IFNAMSIZ]>
);
#[cfg(linux_android)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    #[allow(missing_docs)]
    // Not documented by Linux!
    OriginalDst,
    GetOnly,
    libc::SOL_IP,
    libc::SO_ORIGINAL_DST,
    libc::sockaddr_in
);
#[cfg(linux_android)]
sockopt_impl!(
    #[allow(missing_docs)]
    // Not documented by Linux!
    Ip6tOriginalDst,
    GetOnly,
    libc::SOL_IPV6,
    libc::IP6T_SO_ORIGINAL_DST,
    libc::sockaddr_in6
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Specifies exact type of timestamping information collected by the kernel
    /// [Further reading](https://www.kernel.org/doc/html/latest/networking/timestamping.html)
    Timestamping,
    Both,
    libc::SOL_SOCKET,
    libc::SO_TIMESTAMPING,
    super::TimestampingFlag
);
#[cfg(not(any(target_os = "aix", target_os = "haiku", target_os = "hurd", target_os = "redox")))]
sockopt_impl!(
    /// Enable or disable the receiving of the `SO_TIMESTAMP` control message.
    ReceiveTimestamp,
    Both,
    libc::SOL_SOCKET,
    libc::SO_TIMESTAMP,
    bool
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Enable or disable the receiving of the `SO_TIMESTAMPNS` control message.
    ReceiveTimestampns,
    Both,
    libc::SOL_SOCKET,
    libc::SO_TIMESTAMPNS,
    bool
);
#[cfg(target_os = "freebsd")]
sockopt_impl!(
    /// Sets a specific timestamp format instead of the classic `SCM_TIMESTAMP`,
    /// to follow up after `SO_TIMESTAMP` is set.
    TsClock,
    Both,
    libc::SOL_SOCKET,
    libc::SO_TS_CLOCK,
    super::SocketTimestamp
);
#[cfg(linux_android)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Setting this boolean option enables transparent proxying on this socket.
    IpTransparent,
    Both,
    libc::SOL_IP,
    libc::IP_TRANSPARENT,
    bool
);
#[cfg(target_os = "openbsd")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Allows the socket to be bound to addresses which are not local to the
    /// machine, so it can be used to make a transparent proxy.
    BindAny,
    Both,
    libc::SOL_SOCKET,
    libc::SO_BINDANY,
    bool
);
#[cfg(target_os = "freebsd")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Can `bind(2)` to any address, even one not bound to any available
    /// network interface in the system.
    BindAny,
    Both,
    libc::IPPROTO_IP,
    libc::IP_BINDANY,
    bool
);
#[cfg(target_os = "freebsd")]
sockopt_impl!(
    /// Set the route table (FIB) for this socket up to the `net.fibs` OID limit
    /// (more specific than the setfib command line/call which are process based).
    Fib,
    SetOnly,
    libc::SOL_SOCKET,
    libc::SO_SETFIB,
    i32
);
#[cfg(target_os = "freebsd")]
sockopt_impl!(
    /// Set `so_user_cookie` for this socket allowing network traffic based
    /// upon it, similar to Linux's netfilter MARK.
    UserCookie,
    SetOnly,
    libc::SOL_SOCKET,
    libc::SO_USER_COOKIE,
    u32
);
#[cfg(target_os = "openbsd")]
sockopt_impl!(
    /// Set the route table for this socket, needs a privileged user if
    /// the process/socket had been set to the non default route.
    Rtable,
    SetOnly,
    libc::SOL_SOCKET,
    libc::SO_RTABLE,
    i32
);
#[cfg(any(target_os = "freebsd", target_os = "netbsd"))]
sockopt_impl!(
    /// Get/set a filter on this socket before accepting connections similarly
    /// to Linux's TCP_DEFER_ACCEPT but after the listen's call.
    AcceptFilter,
    Both,
    libc::SOL_SOCKET,
    libc::SO_ACCEPTFILTER,
    libc::accept_filter_arg
);
#[cfg(target_os = "linux")]
sockopt_impl!(
    /// Set the mark for each packet sent through this socket (similar to the
    /// netfilter MARK target but socket-based).
    Mark,
    Both,
    libc::SOL_SOCKET,
    libc::SO_MARK,
    u32
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Enable or disable the receiving of the `SCM_CREDENTIALS` control
    /// message.
    PassCred,
    Both,
    libc::SOL_SOCKET,
    libc::SO_PASSCRED,
    bool
);
#[cfg(any(target_os = "freebsd", target_os = "linux"))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// This option allows the caller to set the TCP congestion control
    /// algorithm to be used,  on a per-socket basis.
    TcpCongestion,
    Both,
    libc::IPPROTO_TCP,
    libc::TCP_CONGESTION,
    OsString<[u8; TCP_CA_NAME_MAX]>
);
#[cfg(any(linux_android, apple_targets, target_os = "netbsd"))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Pass an `IP_PKTINFO` ancillary message that contains a pktinfo
    /// structure that supplies some information about the incoming packet.
    Ipv4PacketInfo,
    Both,
    libc::IPPROTO_IP,
    libc::IP_PKTINFO,
    bool
);
#[cfg(any(linux_android, target_os = "freebsd", apple_targets, netbsdlike))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// Set delivery of the `IPV6_PKTINFO` control message on incoming
    /// datagrams.
    Ipv6RecvPacketInfo,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_RECVPKTINFO,
    bool
);
#[cfg(bsd)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The `recvmsg(2)` call returns a `struct sockaddr_dl` corresponding to
    /// the interface on which the packet was received.
    Ipv4RecvIf,
    Both,
    libc::IPPROTO_IP,
    libc::IP_RECVIF,
    bool
);
#[cfg(bsd)]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The `recvmsg(2)` call will return the destination IP address for a UDP
    /// datagram.
    Ipv4RecvDstAddr,
    Both,
    libc::IPPROTO_IP,
    libc::IP_RECVDSTADDR,
    bool
);
#[cfg(any(linux_android, target_os = "freebsd"))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The `recvmsg(2)` call will return the destination IP address for a UDP
    /// datagram.
    Ipv4OrigDstAddr,
    Both,
    libc::IPPROTO_IP,
    libc::IP_ORIGDSTADDR,
    bool
);
#[cfg(target_os = "linux")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    #[allow(missing_docs)]
    // Not documented by Linux!
    UdpGsoSegment,
    Both,
    libc::SOL_UDP,
    libc::UDP_SEGMENT,
    libc::c_int
);
#[cfg(target_os = "linux")]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    #[allow(missing_docs)]
    // Not documented by Linux!
    UdpGroSegment,
    Both,
    libc::IPPROTO_UDP,
    libc::UDP_GRO,
    bool
);
#[cfg(target_os = "linux")]
sockopt_impl!(
    /// Configures the behavior of time-based transmission of packets, for use
    /// with the `TxTime` control message.
    TxTime,
    Both,
    libc::SOL_SOCKET,
    libc::SO_TXTIME,
    libc::sock_txtime
);
#[cfg(any(linux_android, target_os = "fuchsia"))]
sockopt_impl!(
    /// Indicates that an unsigned 32-bit value ancillary message (cmsg) should
    /// be attached to received skbs indicating the number of packets dropped by
    /// the socket since its creation.
    RxqOvfl,
    Both,
    libc::SOL_SOCKET,
    libc::SO_RXQ_OVFL,
    libc::c_int
);
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The socket is restricted to sending and receiving IPv6 packets only.
    Ipv6V6Only,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_V6ONLY,
    bool
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Enable extended reliable error message passing.
    Ipv4RecvErr,
    Both,
    libc::IPPROTO_IP,
    libc::IP_RECVERR,
    bool
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Control receiving of asynchronous error options.
    Ipv6RecvErr,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_RECVERR,
    bool
);
#[cfg(linux_android)]
sockopt_impl!(
    /// Fetch the current system-estimated Path MTU.
    IpMtu,
    GetOnly,
    libc::IPPROTO_IP,
    libc::IP_MTU,
    libc::c_int
);
#[cfg(any(linux_android, target_os = "freebsd"))]
sockopt_impl!(
    /// Set or retrieve the current time-to-live field that is used in every
    /// packet sent from this socket.
    Ipv4Ttl,
    Both,
    libc::IPPROTO_IP,
    libc::IP_TTL,
    libc::c_int
);
#[cfg(any(apple_targets, linux_android, target_os = "freebsd"))]
sockopt_impl!(
    /// Set the unicast hop limit for the socket.
    Ipv6Ttl,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_UNICAST_HOPS,
    libc::c_int
);
#[cfg(any(linux_android, target_os = "freebsd"))]
#[cfg(feature = "net")]
sockopt_impl!(
    #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
    /// The `recvmsg(2)` call will return the destination IP address for a UDP
    /// datagram.
    Ipv6OrigDstAddr,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_ORIGDSTADDR,
    bool
);
#[cfg(apple_targets)]
sockopt_impl!(
    /// Set "don't fragment packet" flag on the IP packet.
    IpDontFrag,
    Both,
    libc::IPPROTO_IP,
    libc::IP_DONTFRAG,
    bool
);
#[cfg(any(linux_android, apple_targets))]
sockopt_impl!(
    /// Set "don't fragment packet" flag on the IPv6 packet.
    Ipv6DontFrag,
    Both,
    libc::IPPROTO_IPV6,
    libc::IPV6_DONTFRAG,
    bool
);
#[cfg(apple_targets)]
#[cfg(feature = "net")]
sockopt_impl!(
    /// Get the utun interface name.
    UtunIfname,
    GetOnly,
    libc::SYSPROTO_CONTROL,
    libc::UTUN_OPT_IFNAME,
    CString,
    GetCString<[u8; libc::IFNAMSIZ]>
);

#[allow(missing_docs)]
// Not documented by Linux!
#[cfg(linux_android)]
#[derive(Copy, Clone, Debug)]
pub struct AlgSetAeadAuthSize;

// ALG_SET_AEAD_AUTH_SIZE read the length from passed `option_len`
// See https://elixir.bootlin.com/linux/v4.4/source/crypto/af_alg.c#L222
#[cfg(linux_android)]
impl SetSockOpt for AlgSetAeadAuthSize {
    type Val = usize;

    fn set<F: AsFd>(&self, fd: &F, val: &usize) -> Result<()> {
        unsafe {
            let res = libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                libc::SOL_ALG,
                libc::ALG_SET_AEAD_AUTHSIZE,
                ::std::ptr::null(),
                *val as libc::socklen_t,
            );
            Errno::result(res).map(drop)
        }
    }
}

#[allow(missing_docs)]
// Not documented by Linux!
#[cfg(linux_android)]
#[derive(Clone, Debug)]
pub struct AlgSetKey<T>(::std::marker::PhantomData<T>);

#[cfg(linux_android)]
impl<T> Default for AlgSetKey<T> {
    fn default() -> Self {
        AlgSetKey(Default::default())
    }
}

#[cfg(linux_android)]
impl<T> SetSockOpt for AlgSetKey<T>
where
    T: AsRef<[u8]> + Clone,
{
    type Val = T;

    fn set<F: AsFd>(&self, fd: &F, val: &T) -> Result<()> {
        unsafe {
            let res = libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                libc::SOL_ALG,
                libc::ALG_SET_KEY,
                val.as_ref().as_ptr().cast(),
                val.as_ref().len() as libc::socklen_t,
            );
            Errno::result(res).map(drop)
        }
    }
}

/// Set the Upper Layer Protocol (ULP) on the TCP socket.
///
/// For example, to enable the TLS ULP on a socket, the C function call would be:
///
/// ```c
/// setsockopt(sock, SOL_TCP, TCP_ULP, "tls", sizeof("tls"));
/// ```
///
/// ... and the `nix` equivalent is:
///
/// ```ignore,rust
/// setsockopt(sock, TcpUlp::default(), b"tls");
/// ```
///
/// Note that the ULP name does not need a trailing NUL terminator (`\0`).
#[cfg(linux_android)]
#[derive(Clone, Debug)]
pub struct TcpUlp<T>(::std::marker::PhantomData<T>);

#[cfg(linux_android)]
impl<T> Default for TcpUlp<T> {
    fn default() -> Self {
        TcpUlp(Default::default())
    }
}

#[cfg(linux_android)]
impl<T> SetSockOpt for TcpUlp<T>
where
    T: AsRef<[u8]> + Clone,
{
    type Val = T;

    fn set<F: AsFd>(&self, fd: &F, val: &Self::Val) -> Result<()> {
        unsafe {
            let res = libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                libc::SOL_TCP,
                libc::TCP_ULP,
                val.as_ref().as_ptr().cast(),
                val.as_ref().len() as libc::socklen_t,
            );
            Errno::result(res).map(drop)
        }
    }
}

/// Value used with the [`TcpTlsTx`] and [`TcpTlsRx`] socket options.
#[cfg(target_os = "linux")]
#[derive(Copy, Clone, Debug)]
pub enum TlsCryptoInfo {
    /// AES-128-GCM
    Aes128Gcm(libc::tls12_crypto_info_aes_gcm_128),

    /// AES-256-GCM
    Aes256Gcm(libc::tls12_crypto_info_aes_gcm_256),

    /// CHACHA20-POLY1305
    Chacha20Poly1305(libc::tls12_crypto_info_chacha20_poly1305),
}

/// Set the Kernel TLS write parameters on the TCP socket.
///
/// For example, the C function call would be:
///
/// ```c
/// setsockopt(sock, SOL_TLS, TLS_TX, &crypto_info, sizeof(crypto_info));
/// ```
///
/// ... and the `nix` equivalent is:
///
/// ```ignore,rust
/// setsockopt(sock, TcpTlsTx, &crypto_info);
/// ```
#[cfg(target_os = "linux")]
#[derive(Copy, Clone, Debug)]
pub struct TcpTlsTx;

#[cfg(target_os = "linux")]
impl SetSockOpt for TcpTlsTx {
    type Val = TlsCryptoInfo;

    fn set<F: AsFd>(&self, fd: &F, val: &Self::Val) -> Result<()> {
        let (ffi_ptr, ffi_len) = match val {
            TlsCryptoInfo::Aes128Gcm(crypto_info) => {
                (<*const _>::cast(crypto_info), mem::size_of_val(crypto_info))
            }
            TlsCryptoInfo::Aes256Gcm(crypto_info) => {
                (<*const _>::cast(crypto_info), mem::size_of_val(crypto_info))
            }
            TlsCryptoInfo::Chacha20Poly1305(crypto_info) => {
                (<*const _>::cast(crypto_info), mem::size_of_val(crypto_info))
            }
        };
        unsafe {
            let res = libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                libc::SOL_TLS,
                libc::TLS_TX,
                ffi_ptr,
                ffi_len as libc::socklen_t,
            );
            Errno::result(res).map(drop)
        }
    }
}

/// Set the Kernel TLS read parameters on the TCP socket.
///
/// For example, the C function call would be:
///
/// ```c
/// setsockopt(sock, SOL_TLS, TLS_RX, &crypto_info, sizeof(crypto_info));
/// ```
///
/// ... and the `nix` equivalent is:
///
/// ```ignore,rust
/// setsockopt(sock, TcpTlsRx, &crypto_info);
/// ```
#[cfg(target_os = "linux")]
#[derive(Copy, Clone, Debug)]
pub struct TcpTlsRx;

#[cfg(target_os = "linux")]
impl SetSockOpt for TcpTlsRx {
    type Val = TlsCryptoInfo;

    fn set<F: AsFd>(&self, fd: &F, val: &Self::Val) -> Result<()> {
        let (ffi_ptr, ffi_len) = match val {
            TlsCryptoInfo::Aes128Gcm(crypto_info) => {
                (<*const _>::cast(crypto_info), mem::size_of_val(crypto_info))
            }
            TlsCryptoInfo::Aes256Gcm(crypto_info) => {
                (<*const _>::cast(crypto_info), mem::size_of_val(crypto_info))
            }
            TlsCryptoInfo::Chacha20Poly1305(crypto_info) => {
                (<*const _>::cast(crypto_info), mem::size_of_val(crypto_info))
            }
        };
        unsafe {
            let res = libc::setsockopt(
                fd.as_fd().as_raw_fd(),
                libc::SOL_TLS,
                libc::TLS_RX,
                ffi_ptr,
                ffi_len as libc::socklen_t,
            );
            Errno::result(res).map(drop)
        }
    }
}


/*
 *
 * ===== Accessor helpers =====
 *
 */

/// Helper trait that describes what is expected from a `GetSockOpt` getter.
trait Get<T> {
    /// Returns an uninitialized value.
    fn uninit() -> Self;
    /// Returns a pointer to the stored value. This pointer will be passed to the system's
    /// `getsockopt` call (`man 3p getsockopt`, argument `option_value`).
    fn ffi_ptr(&mut self) -> *mut c_void;
    /// Returns length of the stored value. This pointer will be passed to the system's
    /// `getsockopt` call (`man 3p getsockopt`, argument `option_len`).
    fn ffi_len(&mut self) -> *mut socklen_t;
    /// Returns the hopefully initialized inner value.
    unsafe fn assume_init(self) -> T;
}

/// Helper trait that describes what is expected from a `SetSockOpt` setter.
trait Set<'a, T> {
    /// Initialize the setter with a given value.
    fn new(val: &'a T) -> Self;
    /// Returns a pointer to the stored value. This pointer will be passed to the system's
    /// `setsockopt` call (`man 3p setsockopt`, argument `option_value`).
    fn ffi_ptr(&self) -> *const c_void;
    /// Returns length of the stored value. This pointer will be passed to the system's
    /// `setsockopt` call (`man 3p setsockopt`, argument `option_len`).
    fn ffi_len(&self) -> socklen_t;
}

/// Getter for an arbitrary `struct`.
struct GetStruct<T> {
    len: socklen_t,
    val: MaybeUninit<T>,
}

impl<T> Get<T> for GetStruct<T> {
    fn uninit() -> Self {
        GetStruct {
            len: mem::size_of::<T>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr().cast()
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> T {
        assert_eq!(
            self.len as usize,
            mem::size_of::<T>(),
            "invalid getsockopt implementation"
        );
        unsafe { self.val.assume_init() }
    }
}

/// Setter for an arbitrary `struct`.
struct SetStruct<'a, T: 'static> {
    ptr: &'a T,
}

impl<'a, T> Set<'a, T> for SetStruct<'a, T> {
    fn new(ptr: &'a T) -> SetStruct<'a, T> {
        SetStruct { ptr }
    }

    fn ffi_ptr(&self) -> *const c_void {
        self.ptr as *const T as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of::<T>() as socklen_t
    }
}

/// Getter for a boolean value.
struct GetBool {
    len: socklen_t,
    val: MaybeUninit<c_int>,
}

impl Get<bool> for GetBool {
    fn uninit() -> Self {
        GetBool {
            len: mem::size_of::<c_int>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr().cast()
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> bool {
        assert_eq!(
            self.len as usize,
            mem::size_of::<c_int>(),
            "invalid getsockopt implementation"
        );
        unsafe { self.val.assume_init() != 0 }
    }
}

/// Setter for a boolean value.
struct SetBool {
    val: c_int,
}

impl<'a> Set<'a, bool> for SetBool {
    fn new(val: &'a bool) -> SetBool {
        SetBool {
            val: i32::from(*val),
        }
    }

    fn ffi_ptr(&self) -> *const c_void {
        &self.val as *const c_int as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of_val(&self.val) as socklen_t
    }
}

/// Getter for an `u8` value.
struct GetU8 {
    len: socklen_t,
    val: MaybeUninit<u8>,
}

impl Get<u8> for GetU8 {
    fn uninit() -> Self {
        GetU8 {
            len: mem::size_of::<u8>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr().cast()
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> u8 {
        assert_eq!(
            self.len as usize,
            mem::size_of::<u8>(),
            "invalid getsockopt implementation"
        );
        unsafe { self.val.assume_init() }
    }
}

/// Setter for an `u8` value.
struct SetU8 {
    val: u8,
}

impl<'a> Set<'a, u8> for SetU8 {
    fn new(val: &'a u8) -> SetU8 {
        SetU8 { val: *val }
    }

    fn ffi_ptr(&self) -> *const c_void {
        &self.val as *const u8 as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of_val(&self.val) as socklen_t
    }
}

/// Getter for an `usize` value.
struct GetUsize {
    len: socklen_t,
    val: MaybeUninit<c_int>,
}

impl Get<usize> for GetUsize {
    fn uninit() -> Self {
        GetUsize {
            len: mem::size_of::<c_int>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr().cast()
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> usize {
        assert_eq!(
            self.len as usize,
            mem::size_of::<c_int>(),
            "invalid getsockopt implementation"
        );
        unsafe { self.val.assume_init() as usize }
    }
}

/// Setter for an `usize` value.
struct SetUsize {
    val: c_int,
}

impl<'a> Set<'a, usize> for SetUsize {
    fn new(val: &'a usize) -> SetUsize {
        SetUsize { val: *val as c_int }
    }

    fn ffi_ptr(&self) -> *const c_void {
        &self.val as *const c_int as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of_val(&self.val) as socklen_t
    }
}

/// Getter for a `OsString` value.
struct GetOsString<T: AsMut<[u8]>> {
    len: socklen_t,
    val: MaybeUninit<T>,
}

impl<T: AsMut<[u8]>> Get<OsString> for GetOsString<T> {
    fn uninit() -> Self {
        GetOsString {
            len: mem::size_of::<T>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr().cast()
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> OsString {
        let len = self.len as usize;
        let mut v = unsafe { self.val.assume_init() };
        OsStr::from_bytes(&v.as_mut()[0..len]).to_owned()
    }
}

/// Setter for a `OsString` value.
struct SetOsString<'a> {
    val: &'a OsStr,
}

impl<'a> Set<'a, OsString> for SetOsString<'a> {
    fn new(val: &'a OsString) -> SetOsString {
        SetOsString {
            val: val.as_os_str(),
        }
    }

    fn ffi_ptr(&self) -> *const c_void {
        self.val.as_bytes().as_ptr().cast()
    }

    fn ffi_len(&self) -> socklen_t {
        self.val.len() as socklen_t
    }
}

/// Getter for a `CString` value.
struct GetCString<T: AsMut<[u8]>> {
    len: socklen_t,
    val: MaybeUninit<T>,
}

impl<T: AsMut<[u8]>> Get<CString> for GetCString<T> {
    fn uninit() -> Self {
        GetCString {
            len: mem::size_of::<T>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr().cast()
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> CString {
        let mut v = unsafe { self.val.assume_init() };
        CStr::from_bytes_until_nul(v.as_mut())
            .expect("string should be null-terminated")
            .to_owned()
    }
}
