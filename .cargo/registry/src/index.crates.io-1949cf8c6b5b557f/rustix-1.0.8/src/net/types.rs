//! Types and constants for `rustix::net`.

use crate::backend::c;
use crate::ffi;
use bitflags::bitflags;

/// A type for holding raw integer socket types.
pub type RawSocketType = u32;

/// `SOCK_*` constants for use with [`socket`].
///
/// [`socket`]: crate::net::socket()
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct SocketType(pub(crate) RawSocketType);

#[rustfmt::skip]
impl SocketType {
    /// `SOCK_STREAM`
    pub const STREAM: Self = Self(c::SOCK_STREAM as _);

    /// `SOCK_DGRAM`
    pub const DGRAM: Self = Self(c::SOCK_DGRAM as _);

    /// `SOCK_SEQPACKET`
    #[cfg(not(any(target_os = "espidf", target_os = "horizon")))]
    pub const SEQPACKET: Self = Self(c::SOCK_SEQPACKET as _);

    /// `SOCK_RAW`
    #[cfg(not(any(target_os = "espidf", target_os = "horizon")))]
    pub const RAW: Self = Self(c::SOCK_RAW as _);

    /// `SOCK_RDM`
    #[cfg(not(any(
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox"
    )))]
    pub const RDM: Self = Self(c::SOCK_RDM as _);

    /// Constructs a `SocketType` from a raw integer.
    #[inline]
    pub const fn from_raw(raw: RawSocketType) -> Self {
        Self(raw)
    }

    /// Returns the raw integer for this `SocketType`.
    #[inline]
    pub const fn as_raw(self) -> RawSocketType {
        self.0
    }
}

/// A type for holding raw integer address families.
pub type RawAddressFamily = crate::ffi::c_ushort;

/// `AF_*` constants for use with [`socket`], [`socket_with`], and
/// [`socketpair`].
///
/// [`socket`]: crate::net::socket()
/// [`socket_with`]: crate::net::socket_with
/// [`socketpair`]: crate::net::socketpair()
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct AddressFamily(pub(crate) RawAddressFamily);

#[rustfmt::skip]
#[allow(non_upper_case_globals)]
impl AddressFamily {
    /// `AF_UNSPEC`
    pub const UNSPEC: Self = Self(c::AF_UNSPEC as _);
    /// `AF_INET`
    ///
    /// # References
    ///  - [Linux]
    ///
    /// [Linux]: https://man7.org/linux/man-pages/man7/ip.7.html
    pub const INET: Self = Self(c::AF_INET as _);
    /// `AF_INET6`
    ///
    /// # References
    ///  - [Linux]
    ///
    /// [Linux]: https://man7.org/linux/man-pages/man7/ipv6.7.html
    pub const INET6: Self = Self(c::AF_INET6 as _);
    /// `AF_NETLINK`
    ///
    /// # References
    ///  - [Linux]
    ///
    /// [Linux]: https://man7.org/linux/man-pages/man7/netlink.7.html
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const NETLINK: Self = Self(c::AF_NETLINK as _);
    /// `AF_UNIX`, aka `AF_LOCAL`
    #[doc(alias = "LOCAL")]
    pub const UNIX: Self = Self(c::AF_UNIX as _);
    /// `AF_AX25`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const AX25: Self = Self(c::AF_AX25 as _);
    /// `AF_IPX`
    #[cfg(not(any(
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const IPX: Self = Self(c::AF_IPX as _);
    /// `AF_APPLETALK`
    #[cfg(not(any(
        target_os = "espidf",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const APPLETALK: Self = Self(c::AF_APPLETALK as _);
    /// `AF_NETROM`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const NETROM: Self = Self(c::AF_NETROM as _);
    /// `AF_BRIDGE`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const BRIDGE: Self = Self(c::AF_BRIDGE as _);
    /// `AF_ATMPVC`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ATMPVC: Self = Self(c::AF_ATMPVC as _);
    /// `AF_X25`
    #[cfg(not(any(
        bsd,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const X25: Self = Self(c::AF_X25 as _);
    /// `AF_ROSE`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ROSE: Self = Self(c::AF_ROSE as _);
    /// `AF_DECnet`
    #[cfg(not(any(
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const DECnet: Self = Self(c::AF_DECnet as _);
    /// `AF_NETBEUI`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const NETBEUI: Self = Self(c::AF_NETBEUI as _);
    /// `AF_SECURITY`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const SECURITY: Self = Self(c::AF_SECURITY as _);
    /// `AF_KEY`
    #[cfg(not(any(
        bsd,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const KEY: Self = Self(c::AF_KEY as _);
    /// `AF_PACKET`
    ///
    /// # References
    ///  - [Linux]
    ///
    /// [Linux]: https://man7.org/linux/man-pages/man7/packet.7.html
    #[cfg(not(any(
        bsd,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const PACKET: Self = Self(c::AF_PACKET as _);
    /// `AF_ASH`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ASH: Self = Self(c::AF_ASH as _);
    /// `AF_ECONET`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ECONET: Self = Self(c::AF_ECONET as _);
    /// `AF_ATMSVC`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ATMSVC: Self = Self(c::AF_ATMSVC as _);
    /// `AF_RDS`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const RDS: Self = Self(c::AF_RDS as _);
    /// `AF_SNA`
    #[cfg(not(any(
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const SNA: Self = Self(c::AF_SNA as _);
    /// `AF_IRDA`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const IRDA: Self = Self(c::AF_IRDA as _);
    /// `AF_PPPOX`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const PPPOX: Self = Self(c::AF_PPPOX as _);
    /// `AF_WANPIPE`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const WANPIPE: Self = Self(c::AF_WANPIPE as _);
    /// `AF_LLC`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const LLC: Self = Self(c::AF_LLC as _);
    /// `AF_CAN`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const CAN: Self = Self(c::AF_CAN as _);
    /// `AF_TIPC`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const TIPC: Self = Self(c::AF_TIPC as _);
    /// `AF_BLUETOOTH`
    #[cfg(not(any(
        apple,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const BLUETOOTH: Self = Self(c::AF_BLUETOOTH as _);
    /// `AF_IUCV`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const IUCV: Self = Self(c::AF_IUCV as _);
    /// `AF_RXRPC`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const RXRPC: Self = Self(c::AF_RXRPC as _);
    /// `AF_ISDN`
    #[cfg(not(any(
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ISDN: Self = Self(c::AF_ISDN as _);
    /// `AF_PHONET`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const PHONET: Self = Self(c::AF_PHONET as _);
    /// `AF_IEEE802154`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const IEEE802154: Self = Self(c::AF_IEEE802154 as _);
    /// `AF_802`
    #[cfg(solarish)]
    pub const EIGHT_ZERO_TWO: Self = Self(c::AF_802 as _);
    #[cfg(target_os = "fuchsia")]
    /// `AF_ALG`
    pub const ALG: Self = Self(c::AF_ALG as _);
    #[cfg(any(target_os = "freebsd", target_os = "netbsd", target_os = "nto"))]
    /// `AF_ARP`
    pub const ARP: Self = Self(c::AF_ARP as _);
    /// `AF_ATM`
    #[cfg(freebsdlike)]
    pub const ATM: Self = Self(c::AF_ATM as _);
    /// `AF_CAIF`
    #[cfg(any(target_os = "android", target_os = "emscripten", target_os = "fuchsia"))]
    pub const CAIF: Self = Self(c::AF_CAIF as _);
    /// `AF_CCITT`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const CCITT: Self = Self(c::AF_CCITT as _);
    /// `AF_CHAOS`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const CHAOS: Self = Self(c::AF_CHAOS as _);
    /// `AF_CNT`
    #[cfg(any(bsd, target_os = "nto"))]
    pub const CNT: Self = Self(c::AF_CNT as _);
    /// `AF_COIP`
    #[cfg(any(bsd, target_os = "nto"))]
    pub const COIP: Self = Self(c::AF_COIP as _);
    /// `AF_DATAKIT`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const DATAKIT: Self = Self(c::AF_DATAKIT as _);
    /// `AF_DLI`
    #[cfg(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "nto"
    ))]
    pub const DLI: Self = Self(c::AF_DLI as _);
    /// `AF_E164`
    #[cfg(any(bsd, target_os = "nto"))]
    pub const E164: Self = Self(c::AF_E164 as _);
    /// `AF_ECMA`
    #[cfg(any(
        apple,
        freebsdlike,
        solarish,
        target_os = "aix",
        target_os = "nto",
        target_os = "openbsd"
    ))]
    pub const ECMA: Self = Self(c::AF_ECMA as _);
    /// `AF_ENCAP`
    #[cfg(target_os = "openbsd")]
    pub const ENCAP: Self = Self(c::AF_ENCAP as _);
    /// `AF_FILE`
    #[cfg(solarish)]
    pub const FILE: Self = Self(c::AF_FILE as _);
    /// `AF_GOSIP`
    #[cfg(solarish)]
    pub const GOSIP: Self = Self(c::AF_GOSIP as _);
    /// `AF_HYLINK`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const HYLINK: Self = Self(c::AF_HYLINK as _);
    /// `AF_IB`
    #[cfg(any(target_os = "emscripten", target_os = "fuchsia"))]
    pub const IB: Self = Self(c::AF_IB as _);
    /// `AF_IMPLINK`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const IMPLINK: Self = Self(c::AF_IMPLINK as _);
    /// `AF_IEEE80211`
    #[cfg(any(apple, freebsdlike, target_os = "netbsd"))]
    pub const IEEE80211: Self = Self(c::AF_IEEE80211 as _);
    /// `AF_INET6_SDP`
    #[cfg(target_os = "freebsd")]
    pub const INET6_SDP: Self = Self(c::AF_INET6_SDP as _);
    /// `AF_INET_OFFLOAD`
    #[cfg(solarish)]
    pub const INET_OFFLOAD: Self = Self(c::AF_INET_OFFLOAD as _);
    /// `AF_INET_SDP`
    #[cfg(target_os = "freebsd")]
    pub const INET_SDP: Self = Self(c::AF_INET_SDP as _);
    /// `AF_INTF`
    #[cfg(target_os = "aix")]
    pub const INTF: Self = Self(c::AF_INTF as _);
    /// `AF_ISO`
    #[cfg(any(bsd, target_os = "aix", target_os = "nto"))]
    pub const ISO: Self = Self(c::AF_ISO as _);
    /// `AF_LAT`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const LAT: Self = Self(c::AF_LAT as _);
    /// `AF_LINK`
    #[cfg(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "nto"
    ))]
    pub const LINK: Self = Self(c::AF_LINK as _);
    /// `AF_MPLS`
    #[cfg(any(
        netbsdlike,
        target_os = "dragonfly",
        target_os = "emscripten",
        target_os = "fuchsia"
    ))]
    pub const MPLS: Self = Self(c::AF_MPLS as _);
    /// `AF_NATM`
    #[cfg(any(bsd, target_os = "nto"))]
    pub const NATM: Self = Self(c::AF_NATM as _);
    /// `AF_NBS`
    #[cfg(solarish)]
    pub const NBS: Self = Self(c::AF_NBS as _);
    /// `AF_NCA`
    #[cfg(target_os = "illumos")]
    pub const NCA: Self = Self(c::AF_NCA as _);
    /// `AF_NDD`
    #[cfg(target_os = "aix")]
    pub const NDD: Self = Self(c::AF_NDD as _);
    /// `AF_NDRV`
    #[cfg(apple)]
    pub const NDRV: Self = Self(c::AF_NDRV as _);
    /// `AF_NETBIOS`
    #[cfg(any(apple, freebsdlike))]
    pub const NETBIOS: Self = Self(c::AF_NETBIOS as _);
    /// `AF_NETGRAPH`
    #[cfg(freebsdlike)]
    pub const NETGRAPH: Self = Self(c::AF_NETGRAPH as _);
    /// `AF_NIT`
    #[cfg(solarish)]
    pub const NIT: Self = Self(c::AF_NIT as _);
    /// `AF_NOTIFY`
    #[cfg(target_os = "haiku")]
    pub const NOTIFY: Self = Self(c::AF_NOTIFY as _);
    /// `AF_NFC`
    #[cfg(any(target_os = "emscripten", target_os = "fuchsia"))]
    pub const NFC: Self = Self(c::AF_NFC as _);
    /// `AF_NS`
    #[cfg(any(apple, solarish, netbsdlike, target_os = "aix", target_os = "nto"))]
    pub const NS: Self = Self(c::AF_NS as _);
    /// `AF_OROUTE`
    #[cfg(target_os = "netbsd")]
    pub const OROUTE: Self = Self(c::AF_OROUTE as _);
    /// `AF_OSI`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const OSI: Self = Self(c::AF_OSI as _);
    /// `AF_OSINET`
    #[cfg(solarish)]
    pub const OSINET: Self = Self(c::AF_OSINET as _);
    /// `AF_POLICY`
    #[cfg(solarish)]
    pub const POLICY: Self = Self(c::AF_POLICY as _);
    /// `AF_PPP`
    #[cfg(apple)]
    pub const PPP: Self = Self(c::AF_PPP as _);
    /// `AF_PUP`
    #[cfg(any(bsd, solarish, target_os = "aix", target_os = "nto"))]
    pub const PUP: Self = Self(c::AF_PUP as _);
    /// `AF_RIF`
    #[cfg(target_os = "aix")]
    pub const RIF: Self = Self(c::AF_RIF as _);
    /// `AF_ROUTE`
    #[cfg(any(
        bsd,
        solarish,
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "nto"
    ))]
    pub const ROUTE: Self = Self(c::AF_ROUTE as _);
    /// `AF_SCLUSTER`
    #[cfg(target_os = "freebsd")]
    pub const SCLUSTER: Self = Self(c::AF_SCLUSTER as _);
    /// `AF_SIP`
    #[cfg(any(apple, target_os = "freebsd", target_os = "openbsd"))]
    pub const SIP: Self = Self(c::AF_SIP as _);
    /// `AF_SLOW`
    #[cfg(target_os = "freebsd")]
    pub const SLOW: Self = Self(c::AF_SLOW as _);
    /// `AF_SYS_CONTROL`
    #[cfg(apple)]
    pub const SYS_CONTROL: Self = Self(c::AF_SYS_CONTROL as _);
    /// `AF_SYSTEM`
    #[cfg(apple)]
    pub const SYSTEM: Self = Self(c::AF_SYSTEM as _);
    /// `AF_TRILL`
    #[cfg(solarish)]
    pub const TRILL: Self = Self(c::AF_TRILL as _);
    /// `AF_UTUN`
    #[cfg(apple)]
    pub const UTUN: Self = Self(c::AF_UTUN as _);
    /// `AF_VSOCK`
    #[cfg(any(apple, target_os = "emscripten", target_os = "fuchsia"))]
    pub const VSOCK: Self = Self(c::AF_VSOCK as _);
    /// `AF_XDP`
    #[cfg(target_os = "linux")]
    pub const XDP: Self = Self(c::AF_XDP as _);

    /// Constructs a `AddressFamily` from a raw integer.
    #[inline]
    pub const fn from_raw(raw: RawAddressFamily) -> Self {
        Self(raw)
    }

    /// Returns the raw integer for this `AddressFamily`.
    #[inline]
    pub const fn as_raw(self) -> RawAddressFamily {
        self.0
    }
}

/// A type for holding raw integer protocols.
pub type RawProtocol = core::num::NonZeroU32;

const fn new_raw_protocol(u: u32) -> RawProtocol {
    match RawProtocol::new(u) {
        Some(p) => p,
        None => panic!("new_raw_protocol: protocol must be non-zero"),
    }
}

/// `IPPROTO_*` and other constants for use with [`socket`], [`socket_with`],
/// and [`socketpair`] when a nondefault value is desired.
///
/// See the [`ipproto`], [`sysproto`], and [`netlink`] modules for possible
/// values.
///
/// For the default values, such as `IPPROTO_IP` or `NETLINK_ROUTE`, pass
/// `None` as the `protocol` argument in these functions.
///
/// [`socket`]: crate::net::socket()
/// [`socket_with`]: crate::net::socket_with
/// [`socketpair`]: crate::net::socketpair()
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
#[doc(alias = "IPPROTO_IP")]
#[doc(alias = "NETLINK_ROUTE")]
pub struct Protocol(pub(crate) RawProtocol);

/// `IPPROTO_*` constants.
///
/// For `IPPROTO_IP`, pass `None` as the `protocol` argument.
pub mod ipproto {
    use super::{new_raw_protocol, Protocol};
    use crate::backend::c;

    /// `IPPROTO_ICMP`
    pub const ICMP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_ICMP as _));
    /// `IPPROTO_IGMP`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "vita"
    )))]
    pub const IGMP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_IGMP as _));
    /// `IPPROTO_IPIP`
    #[cfg(not(any(
        solarish,
        windows,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const IPIP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_IPIP as _));
    /// `IPPROTO_TCP`
    pub const TCP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_TCP as _));
    /// `IPPROTO_EGP`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const EGP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_EGP as _));
    /// `IPPROTO_PUP`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "vita"
    )))]
    pub const PUP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_PUP as _));
    /// `IPPROTO_UDP`
    pub const UDP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_UDP as _));
    /// `IPPROTO_IDP`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "vita"
    )))]
    pub const IDP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_IDP as _));
    /// `IPPROTO_TP`
    #[cfg(not(any(
        solarish,
        windows,
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const TP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_TP as _));
    /// `IPPROTO_DCCP`
    #[cfg(not(any(
        apple,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "dragonfly",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const DCCP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_DCCP as _));
    /// `IPPROTO_IPV6`
    pub const IPV6: Protocol = Protocol(new_raw_protocol(c::IPPROTO_IPV6 as _));
    /// `IPPROTO_RSVP`
    #[cfg(not(any(
        solarish,
        windows,
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const RSVP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_RSVP as _));
    /// `IPPROTO_GRE`
    #[cfg(not(any(
        solarish,
        windows,
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const GRE: Protocol = Protocol(new_raw_protocol(c::IPPROTO_GRE as _));
    /// `IPPROTO_ESP`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const ESP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_ESP as _));
    /// `IPPROTO_AH`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const AH: Protocol = Protocol(new_raw_protocol(c::IPPROTO_AH as _));
    /// `IPPROTO_MTP`
    #[cfg(not(any(
        solarish,
        netbsdlike,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const MTP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_MTP as _));
    /// `IPPROTO_BEETPH`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const BEETPH: Protocol = Protocol(new_raw_protocol(c::IPPROTO_BEETPH as _));
    /// `IPPROTO_ENCAP`
    #[cfg(not(any(
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const ENCAP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_ENCAP as _));
    /// `IPPROTO_PIM`
    #[cfg(not(any(
        solarish,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const PIM: Protocol = Protocol(new_raw_protocol(c::IPPROTO_PIM as _));
    /// `IPPROTO_COMP`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const COMP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_COMP as _));
    /// `IPPROTO_SCTP`
    #[cfg(not(any(
        solarish,
        target_os = "cygwin",
        target_os = "dragonfly",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "openbsd",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const SCTP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_SCTP as _));
    /// `IPPROTO_UDPLITE`
    #[cfg(not(any(
        apple,
        netbsdlike,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "dragonfly",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const UDPLITE: Protocol = Protocol(new_raw_protocol(c::IPPROTO_UDPLITE as _));
    /// `IPPROTO_MPLS`
    #[cfg(not(any(
        apple,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "dragonfly",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "netbsd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const MPLS: Protocol = Protocol(new_raw_protocol(c::IPPROTO_MPLS as _));
    /// `IPPROTO_ETHERNET`
    #[cfg(linux_kernel)]
    pub const ETHERNET: Protocol = Protocol(new_raw_protocol(c::IPPROTO_ETHERNET as _));
    /// `IPPROTO_RAW`
    #[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
    pub const RAW: Protocol = Protocol(new_raw_protocol(c::IPPROTO_RAW as _));
    /// `IPPROTO_MPTCP`
    #[cfg(not(any(
        bsd,
        solarish,
        windows,
        target_os = "aix",
        target_os = "cygwin",
        target_os = "emscripten",
        target_os = "espidf",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const MPTCP: Protocol = Protocol(new_raw_protocol(c::IPPROTO_MPTCP as _));
    /// `IPPROTO_FRAGMENT`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const FRAGMENT: Protocol = Protocol(new_raw_protocol(c::IPPROTO_FRAGMENT as _));
    /// `IPPROTO_ICMPV6`
    pub const ICMPV6: Protocol = Protocol(new_raw_protocol(c::IPPROTO_ICMPV6 as _));
    /// `IPPROTO_MH`
    #[cfg(not(any(
        apple,
        netbsdlike,
        solarish,
        windows,
        target_os = "cygwin",
        target_os = "dragonfly",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
    )))]
    pub const MH: Protocol = Protocol(new_raw_protocol(c::IPPROTO_MH as _));
    /// `IPPROTO_ROUTING`
    #[cfg(not(any(
        solarish,
        target_os = "espidf",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "redox",
        target_os = "vita"
    )))]
    pub const ROUTING: Protocol = Protocol(new_raw_protocol(c::IPPROTO_ROUTING as _));
}

/// `SYSPROTO_*` constants.
pub mod sysproto {
    #[cfg(apple)]
    use {
        super::{new_raw_protocol, Protocol},
        crate::backend::c,
    };

    /// `SYSPROTO_EVENT`
    #[cfg(apple)]
    pub const EVENT: Protocol = Protocol(new_raw_protocol(c::SYSPROTO_EVENT as _));

    /// `SYSPROTO_CONTROL`
    #[cfg(apple)]
    pub const CONTROL: Protocol = Protocol(new_raw_protocol(c::SYSPROTO_CONTROL as _));
}

/// `NETLINK_*` constants.
///
/// For `NETLINK_ROUTE`, pass `None` as the `protocol` argument.
pub mod netlink {
    #[cfg(linux_kernel)]
    use {
        super::{new_raw_protocol, Protocol},
        crate::backend::c,
        crate::backend::net::read_sockaddr::read_sockaddr_netlink,
        crate::net::{
            addr::{call_with_sockaddr, SocketAddrArg, SocketAddrLen, SocketAddrOpaque},
            SocketAddrAny,
        },
        core::mem,
    };

    /// `NETLINK_UNUSED`
    #[cfg(linux_kernel)]
    pub const UNUSED: Protocol = Protocol(new_raw_protocol(c::NETLINK_UNUSED as _));
    /// `NETLINK_USERSOCK`
    #[cfg(linux_kernel)]
    pub const USERSOCK: Protocol = Protocol(new_raw_protocol(c::NETLINK_USERSOCK as _));
    /// `NETLINK_FIREWALL`
    #[cfg(linux_kernel)]
    pub const FIREWALL: Protocol = Protocol(new_raw_protocol(c::NETLINK_FIREWALL as _));
    /// `NETLINK_SOCK_DIAG`
    #[cfg(linux_kernel)]
    pub const SOCK_DIAG: Protocol = Protocol(new_raw_protocol(c::NETLINK_SOCK_DIAG as _));
    /// `NETLINK_NFLOG`
    #[cfg(linux_kernel)]
    pub const NFLOG: Protocol = Protocol(new_raw_protocol(c::NETLINK_NFLOG as _));
    /// `NETLINK_XFRM`
    #[cfg(linux_kernel)]
    pub const XFRM: Protocol = Protocol(new_raw_protocol(c::NETLINK_XFRM as _));
    /// `NETLINK_SELINUX`
    #[cfg(linux_kernel)]
    pub const SELINUX: Protocol = Protocol(new_raw_protocol(c::NETLINK_SELINUX as _));
    /// `NETLINK_ISCSI`
    #[cfg(linux_kernel)]
    pub const ISCSI: Protocol = Protocol(new_raw_protocol(c::NETLINK_ISCSI as _));
    /// `NETLINK_AUDIT`
    #[cfg(linux_kernel)]
    pub const AUDIT: Protocol = Protocol(new_raw_protocol(c::NETLINK_AUDIT as _));
    /// `NETLINK_FIB_LOOKUP`
    #[cfg(linux_kernel)]
    pub const FIB_LOOKUP: Protocol = Protocol(new_raw_protocol(c::NETLINK_FIB_LOOKUP as _));
    /// `NETLINK_CONNECTOR`
    #[cfg(linux_kernel)]
    pub const CONNECTOR: Protocol = Protocol(new_raw_protocol(c::NETLINK_CONNECTOR as _));
    /// `NETLINK_NETFILTER`
    #[cfg(linux_kernel)]
    pub const NETFILTER: Protocol = Protocol(new_raw_protocol(c::NETLINK_NETFILTER as _));
    /// `NETLINK_IP6_FW`
    #[cfg(linux_kernel)]
    pub const IP6_FW: Protocol = Protocol(new_raw_protocol(c::NETLINK_IP6_FW as _));
    /// `NETLINK_DNRTMSG`
    #[cfg(linux_kernel)]
    pub const DNRTMSG: Protocol = Protocol(new_raw_protocol(c::NETLINK_DNRTMSG as _));
    /// `NETLINK_KOBJECT_UEVENT`
    #[cfg(linux_kernel)]
    pub const KOBJECT_UEVENT: Protocol = Protocol(new_raw_protocol(c::NETLINK_KOBJECT_UEVENT as _));
    /// `NETLINK_GENERIC`
    // This is defined on FreeBSD too, but it has the value 0, so it doesn't
    // fit in or `NonZeroU32`. It's unclear whether FreeBSD intends
    // `NETLINK_GENERIC` to be the default when Linux has `NETLINK_ROUTE` as
    // the default.
    #[cfg(linux_kernel)]
    pub const GENERIC: Protocol = Protocol(new_raw_protocol(c::NETLINK_GENERIC as _));
    /// `NETLINK_SCSITRANSPORT`
    #[cfg(linux_kernel)]
    pub const SCSITRANSPORT: Protocol = Protocol(new_raw_protocol(c::NETLINK_SCSITRANSPORT as _));
    /// `NETLINK_ECRYPTFS`
    #[cfg(linux_kernel)]
    pub const ECRYPTFS: Protocol = Protocol(new_raw_protocol(c::NETLINK_ECRYPTFS as _));
    /// `NETLINK_RDMA`
    #[cfg(linux_kernel)]
    pub const RDMA: Protocol = Protocol(new_raw_protocol(c::NETLINK_RDMA as _));
    /// `NETLINK_CRYPTO`
    #[cfg(linux_kernel)]
    pub const CRYPTO: Protocol = Protocol(new_raw_protocol(c::NETLINK_CRYPTO as _));
    /// `NETLINK_INET_DIAG`
    #[cfg(linux_kernel)]
    pub const INET_DIAG: Protocol = Protocol(new_raw_protocol(c::NETLINK_INET_DIAG as _));

    /// A Netlink socket address.
    ///
    /// Used to bind to a Netlink socket.
    ///
    /// Not ABI compatible with `struct sockaddr_nl`
    #[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
    #[cfg(linux_kernel)]
    pub struct SocketAddrNetlink {
        /// Port ID
        pid: u32,

        /// Multicast groups mask
        groups: u32,
    }

    #[cfg(linux_kernel)]
    impl SocketAddrNetlink {
        /// Construct a netlink address
        #[inline]
        pub const fn new(pid: u32, groups: u32) -> Self {
            Self { pid, groups }
        }

        /// Return port id.
        #[inline]
        pub const fn pid(&self) -> u32 {
            self.pid
        }

        /// Set port id.
        #[inline]
        pub fn set_pid(&mut self, pid: u32) {
            self.pid = pid;
        }

        /// Return multicast groups mask.
        #[inline]
        pub const fn groups(&self) -> u32 {
            self.groups
        }

        /// Set multicast groups mask.
        #[inline]
        pub fn set_groups(&mut self, groups: u32) {
            self.groups = groups;
        }
    }

    #[cfg(linux_kernel)]
    #[allow(unsafe_code)]
    // SAFETY: `with_sockaddr` calls `f` using `call_with_sockaddr`, which
    // handles calling `f` with the needed preconditions.
    unsafe impl SocketAddrArg for SocketAddrNetlink {
        unsafe fn with_sockaddr<R>(
            &self,
            f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
        ) -> R {
            let mut addr: c::sockaddr_nl = mem::zeroed();
            addr.nl_family = c::AF_NETLINK as _;
            addr.nl_pid = self.pid;
            addr.nl_groups = self.groups;
            call_with_sockaddr(&addr, f)
        }
    }

    #[cfg(linux_kernel)]
    impl From<SocketAddrNetlink> for SocketAddrAny {
        #[inline]
        fn from(from: SocketAddrNetlink) -> Self {
            from.as_any()
        }
    }

    #[cfg(linux_kernel)]
    impl TryFrom<SocketAddrAny> for SocketAddrNetlink {
        type Error = crate::io::Errno;

        fn try_from(addr: SocketAddrAny) -> Result<Self, Self::Error> {
            read_sockaddr_netlink(&addr)
        }
    }
}

/// `ETH_P_*` constants.
// These are translated into 16-bit big-endian form because that's what the
// [`AddressFamily::PACKET`] address family [expects].
//
// [expects]: https://man7.org/linux/man-pages/man7/packet.7.html
pub mod eth {
    #[cfg(linux_kernel)]
    use {
        super::{new_raw_protocol, Protocol},
        crate::backend::c,
    };

    /// `ETH_P_LOOP`
    #[cfg(linux_kernel)]
    pub const LOOP: Protocol = Protocol(new_raw_protocol((c::ETH_P_LOOP as u16).to_be() as u32));
    /// `ETH_P_PUP`
    #[cfg(linux_kernel)]
    pub const PUP: Protocol = Protocol(new_raw_protocol((c::ETH_P_PUP as u16).to_be() as u32));
    /// `ETH_P_PUPAT`
    #[cfg(linux_kernel)]
    pub const PUPAT: Protocol = Protocol(new_raw_protocol((c::ETH_P_PUPAT as u16).to_be() as u32));
    /// `ETH_P_TSN`
    #[cfg(linux_kernel)]
    pub const TSN: Protocol = Protocol(new_raw_protocol((c::ETH_P_TSN as u16).to_be() as u32));
    /// `ETH_P_ERSPAN2`
    #[cfg(linux_kernel)]
    pub const ERSPAN2: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ERSPAN2 as u16).to_be() as u32));
    /// `ETH_P_IP`
    #[cfg(linux_kernel)]
    pub const IP: Protocol = Protocol(new_raw_protocol((c::ETH_P_IP as u16).to_be() as u32));
    /// `ETH_P_X25`
    #[cfg(linux_kernel)]
    pub const X25: Protocol = Protocol(new_raw_protocol((c::ETH_P_X25 as u16).to_be() as u32));
    /// `ETH_P_ARP`
    #[cfg(linux_kernel)]
    pub const ARP: Protocol = Protocol(new_raw_protocol((c::ETH_P_ARP as u16).to_be() as u32));
    /// `ETH_P_BPQ`
    #[cfg(linux_kernel)]
    pub const BPQ: Protocol = Protocol(new_raw_protocol((c::ETH_P_BPQ as u16).to_be() as u32));
    /// `ETH_P_IEEEPUP`
    #[cfg(linux_kernel)]
    pub const IEEEPUP: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_IEEEPUP as u16).to_be() as u32));
    /// `ETH_P_IEEEPUPAT`
    #[cfg(linux_kernel)]
    pub const IEEEPUPAT: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_IEEEPUPAT as u16).to_be() as u32));
    /// `ETH_P_BATMAN`
    #[cfg(linux_kernel)]
    pub const BATMAN: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_BATMAN as u16).to_be() as u32));
    /// `ETH_P_DEC`
    #[cfg(linux_kernel)]
    pub const DEC: Protocol = Protocol(new_raw_protocol((c::ETH_P_DEC as u16).to_be() as u32));
    /// `ETH_P_DNA_DL`
    #[cfg(linux_kernel)]
    pub const DNA_DL: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_DNA_DL as u16).to_be() as u32));
    /// `ETH_P_DNA_RC`
    #[cfg(linux_kernel)]
    pub const DNA_RC: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_DNA_RC as u16).to_be() as u32));
    /// `ETH_P_DNA_RT`
    #[cfg(linux_kernel)]
    pub const DNA_RT: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_DNA_RT as u16).to_be() as u32));
    /// `ETH_P_LAT`
    #[cfg(linux_kernel)]
    pub const LAT: Protocol = Protocol(new_raw_protocol((c::ETH_P_LAT as u16).to_be() as u32));
    /// `ETH_P_DIAG`
    #[cfg(linux_kernel)]
    pub const DIAG: Protocol = Protocol(new_raw_protocol((c::ETH_P_DIAG as u16).to_be() as u32));
    /// `ETH_P_CUST`
    #[cfg(linux_kernel)]
    pub const CUST: Protocol = Protocol(new_raw_protocol((c::ETH_P_CUST as u16).to_be() as u32));
    /// `ETH_P_SCA`
    #[cfg(linux_kernel)]
    pub const SCA: Protocol = Protocol(new_raw_protocol((c::ETH_P_SCA as u16).to_be() as u32));
    /// `ETH_P_TEB`
    #[cfg(linux_kernel)]
    pub const TEB: Protocol = Protocol(new_raw_protocol((c::ETH_P_TEB as u16).to_be() as u32));
    /// `ETH_P_RARP`
    #[cfg(linux_kernel)]
    pub const RARP: Protocol = Protocol(new_raw_protocol((c::ETH_P_RARP as u16).to_be() as u32));
    /// `ETH_P_ATALK`
    #[cfg(linux_kernel)]
    pub const ATALK: Protocol = Protocol(new_raw_protocol((c::ETH_P_ATALK as u16).to_be() as u32));
    /// `ETH_P_AARP`
    #[cfg(linux_kernel)]
    pub const AARP: Protocol = Protocol(new_raw_protocol((c::ETH_P_AARP as u16).to_be() as u32));
    /// `ETH_P_8021Q`
    #[cfg(linux_kernel)]
    pub const P_8021Q: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_8021Q as u16).to_be() as u32));
    /// `ETH_P_ERSPAN`
    #[cfg(linux_kernel)]
    pub const ERSPAN: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ERSPAN as u16).to_be() as u32));
    /// `ETH_P_IPX`
    #[cfg(linux_kernel)]
    pub const IPX: Protocol = Protocol(new_raw_protocol((c::ETH_P_IPX as u16).to_be() as u32));
    /// `ETH_P_IPV6`
    #[cfg(linux_kernel)]
    pub const IPV6: Protocol = Protocol(new_raw_protocol((c::ETH_P_IPV6 as u16).to_be() as u32));
    /// `ETH_P_PAUSE`
    #[cfg(linux_kernel)]
    pub const PAUSE: Protocol = Protocol(new_raw_protocol((c::ETH_P_PAUSE as u16).to_be() as u32));
    /// `ETH_P_SLOW`
    #[cfg(linux_kernel)]
    pub const SLOW: Protocol = Protocol(new_raw_protocol((c::ETH_P_SLOW as u16).to_be() as u32));
    /// `ETH_P_WCCP`
    #[cfg(linux_kernel)]
    pub const WCCP: Protocol = Protocol(new_raw_protocol((c::ETH_P_WCCP as u16).to_be() as u32));
    /// `ETH_P_MPLS_UC`
    #[cfg(linux_kernel)]
    pub const MPLS_UC: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_MPLS_UC as u16).to_be() as u32));
    /// `ETH_P_MPLS_MC`
    #[cfg(linux_kernel)]
    pub const MPLS_MC: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_MPLS_MC as u16).to_be() as u32));
    /// `ETH_P_ATMMPOA`
    #[cfg(linux_kernel)]
    pub const ATMMPOA: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ATMMPOA as u16).to_be() as u32));
    /// `ETH_P_PPP_DISC`
    #[cfg(linux_kernel)]
    pub const PPP_DISC: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PPP_DISC as u16).to_be() as u32));
    /// `ETH_P_PPP_SES`
    #[cfg(linux_kernel)]
    pub const PPP_SES: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PPP_SES as u16).to_be() as u32));
    /// `ETH_P_LINK_CTL`
    #[cfg(linux_kernel)]
    pub const LINK_CTL: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_LINK_CTL as u16).to_be() as u32));
    /// `ETH_P_ATMFATE`
    #[cfg(linux_kernel)]
    pub const ATMFATE: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ATMFATE as u16).to_be() as u32));
    /// `ETH_P_PAE`
    #[cfg(linux_kernel)]
    pub const PAE: Protocol = Protocol(new_raw_protocol((c::ETH_P_PAE as u16).to_be() as u32));
    /// `ETH_P_PROFINET`
    #[cfg(linux_kernel)]
    pub const PROFINET: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PROFINET as u16).to_be() as u32));
    /// `ETH_P_REALTEK`
    #[cfg(linux_kernel)]
    pub const REALTEK: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_REALTEK as u16).to_be() as u32));
    /// `ETH_P_AOE`
    #[cfg(linux_kernel)]
    pub const AOE: Protocol = Protocol(new_raw_protocol((c::ETH_P_AOE as u16).to_be() as u32));
    /// `ETH_P_ETHERCAT`
    #[cfg(linux_kernel)]
    pub const ETHERCAT: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ETHERCAT as u16).to_be() as u32));
    /// `ETH_P_8021AD`
    #[cfg(linux_kernel)]
    pub const P_8021AD: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_8021AD as u16).to_be() as u32));
    /// `ETH_P_802_EX1`
    #[cfg(linux_kernel)]
    pub const P_802_EX1: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_802_EX1 as u16).to_be() as u32));
    /// `ETH_P_PREAUTH`
    #[cfg(linux_kernel)]
    pub const PREAUTH: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PREAUTH as u16).to_be() as u32));
    /// `ETH_P_TIPC`
    #[cfg(linux_kernel)]
    pub const TIPC: Protocol = Protocol(new_raw_protocol((c::ETH_P_TIPC as u16).to_be() as u32));
    /// `ETH_P_LLDP`
    #[cfg(linux_kernel)]
    pub const LLDP: Protocol = Protocol(new_raw_protocol((c::ETH_P_LLDP as u16).to_be() as u32));
    /// `ETH_P_MRP`
    #[cfg(linux_kernel)]
    pub const MRP: Protocol = Protocol(new_raw_protocol((c::ETH_P_MRP as u16).to_be() as u32));
    /// `ETH_P_MACSEC`
    #[cfg(linux_kernel)]
    pub const MACSEC: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_MACSEC as u16).to_be() as u32));
    /// `ETH_P_8021AH`
    #[cfg(linux_kernel)]
    pub const P_8021AH: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_8021AH as u16).to_be() as u32));
    /// `ETH_P_MVRP`
    #[cfg(linux_kernel)]
    pub const MVRP: Protocol = Protocol(new_raw_protocol((c::ETH_P_MVRP as u16).to_be() as u32));
    /// `ETH_P_1588`
    #[cfg(linux_kernel)]
    pub const P_1588: Protocol = Protocol(new_raw_protocol((c::ETH_P_1588 as u16).to_be() as u32));
    /// `ETH_P_NCSI`
    #[cfg(linux_kernel)]
    pub const NCSI: Protocol = Protocol(new_raw_protocol((c::ETH_P_NCSI as u16).to_be() as u32));
    /// `ETH_P_PRP`
    #[cfg(linux_kernel)]
    pub const PRP: Protocol = Protocol(new_raw_protocol((c::ETH_P_PRP as u16).to_be() as u32));
    /// `ETH_P_CFM`
    #[cfg(linux_kernel)]
    pub const CFM: Protocol = Protocol(new_raw_protocol((c::ETH_P_CFM as u16).to_be() as u32));
    /// `ETH_P_FCOE`
    #[cfg(linux_kernel)]
    pub const FCOE: Protocol = Protocol(new_raw_protocol((c::ETH_P_FCOE as u16).to_be() as u32));
    /// `ETH_P_IBOE`
    #[cfg(linux_kernel)]
    pub const IBOE: Protocol = Protocol(new_raw_protocol((c::ETH_P_IBOE as u16).to_be() as u32));
    /// `ETH_P_TDLS`
    #[cfg(linux_kernel)]
    pub const TDLS: Protocol = Protocol(new_raw_protocol((c::ETH_P_TDLS as u16).to_be() as u32));
    /// `ETH_P_FIP`
    #[cfg(linux_kernel)]
    pub const FIP: Protocol = Protocol(new_raw_protocol((c::ETH_P_FIP as u16).to_be() as u32));
    /// `ETH_P_80221`
    #[cfg(linux_kernel)]
    pub const P_80221: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_80221 as u16).to_be() as u32));
    /// `ETH_P_HSR`
    #[cfg(linux_kernel)]
    pub const HSR: Protocol = Protocol(new_raw_protocol((c::ETH_P_HSR as u16).to_be() as u32));
    /// `ETH_P_NSH`
    #[cfg(linux_kernel)]
    pub const NSH: Protocol = Protocol(new_raw_protocol((c::ETH_P_NSH as u16).to_be() as u32));
    /// `ETH_P_LOOPBACK`
    #[cfg(linux_kernel)]
    pub const LOOPBACK: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_LOOPBACK as u16).to_be() as u32));
    /// `ETH_P_QINQ1`
    #[cfg(linux_kernel)]
    pub const QINQ1: Protocol = Protocol(new_raw_protocol((c::ETH_P_QINQ1 as u16).to_be() as u32));
    /// `ETH_P_QINQ2`
    #[cfg(linux_kernel)]
    pub const QINQ2: Protocol = Protocol(new_raw_protocol((c::ETH_P_QINQ2 as u16).to_be() as u32));
    /// `ETH_P_QINQ3`
    #[cfg(linux_kernel)]
    pub const QINQ3: Protocol = Protocol(new_raw_protocol((c::ETH_P_QINQ3 as u16).to_be() as u32));
    /// `ETH_P_EDSA`
    #[cfg(linux_kernel)]
    pub const EDSA: Protocol = Protocol(new_raw_protocol((c::ETH_P_EDSA as u16).to_be() as u32));
    /// `ETH_P_DSA_8021Q`
    #[cfg(linux_kernel)]
    pub const DSA_8021Q: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_DSA_8021Q as u16).to_be() as u32));
    /// `ETH_P_DSA_A5PSW`
    #[cfg(linux_kernel)]
    pub const DSA_A5PSW: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_DSA_A5PSW as u16).to_be() as u32));
    /// `ETH_P_IFE`
    #[cfg(linux_kernel)]
    pub const IFE: Protocol = Protocol(new_raw_protocol((c::ETH_P_IFE as u16).to_be() as u32));
    /// `ETH_P_AF_IUCV`
    #[cfg(linux_kernel)]
    pub const AF_IUCV: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_AF_IUCV as u16).to_be() as u32));
    /// `ETH_P_802_3_MIN`
    #[cfg(linux_kernel)]
    pub const P_802_3_MIN: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_802_3_MIN as u16).to_be() as u32));
    /// `ETH_P_802_3`
    #[cfg(linux_kernel)]
    pub const P_802_3: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_802_3 as u16).to_be() as u32));
    /// `ETH_P_AX25`
    #[cfg(linux_kernel)]
    pub const AX25: Protocol = Protocol(new_raw_protocol((c::ETH_P_AX25 as u16).to_be() as u32));
    /// `ETH_P_ALL`
    #[cfg(linux_kernel)]
    pub const ALL: Protocol = Protocol(new_raw_protocol((c::ETH_P_ALL as u16).to_be() as u32));
    /// `ETH_P_802_2`
    #[cfg(linux_kernel)]
    pub const P_802_2: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_802_2 as u16).to_be() as u32));
    /// `ETH_P_SNAP`
    #[cfg(linux_kernel)]
    pub const SNAP: Protocol = Protocol(new_raw_protocol((c::ETH_P_SNAP as u16).to_be() as u32));
    /// `ETH_P_DDCMP`
    #[cfg(linux_kernel)]
    pub const DDCMP: Protocol = Protocol(new_raw_protocol((c::ETH_P_DDCMP as u16).to_be() as u32));
    /// `ETH_P_WAN_PPP`
    #[cfg(linux_kernel)]
    pub const WAN_PPP: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_WAN_PPP as u16).to_be() as u32));
    /// `ETH_P_PPP_MP`
    #[cfg(linux_kernel)]
    pub const PPP_MP: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PPP_MP as u16).to_be() as u32));
    /// `ETH_P_LOCALTALK`
    #[cfg(linux_kernel)]
    pub const LOCALTALK: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_LOCALTALK as u16).to_be() as u32));
    /// `ETH_P_CAN`
    #[cfg(linux_kernel)]
    pub const CAN: Protocol = Protocol(new_raw_protocol((c::ETH_P_CAN as u16).to_be() as u32));
    /// `ETH_P_CANFD`
    #[cfg(linux_kernel)]
    pub const CANFD: Protocol = Protocol(new_raw_protocol((c::ETH_P_CANFD as u16).to_be() as u32));
    /// `ETH_P_CANXL`
    #[cfg(linux_kernel)]
    pub const CANXL: Protocol = Protocol(new_raw_protocol((c::ETH_P_CANXL as u16).to_be() as u32));
    /// `ETH_P_PPPTALK`
    #[cfg(linux_kernel)]
    pub const PPPTALK: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PPPTALK as u16).to_be() as u32));
    /// `ETH_P_TR_802_2`
    #[cfg(linux_kernel)]
    pub const TR_802_2: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_TR_802_2 as u16).to_be() as u32));
    /// `ETH_P_MOBITEX`
    #[cfg(linux_kernel)]
    pub const MOBITEX: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_MOBITEX as u16).to_be() as u32));
    /// `ETH_P_CONTROL`
    #[cfg(linux_kernel)]
    pub const CONTROL: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_CONTROL as u16).to_be() as u32));
    /// `ETH_P_IRDA`
    #[cfg(linux_kernel)]
    pub const IRDA: Protocol = Protocol(new_raw_protocol((c::ETH_P_IRDA as u16).to_be() as u32));
    /// `ETH_P_ECONET`
    #[cfg(linux_kernel)]
    pub const ECONET: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ECONET as u16).to_be() as u32));
    /// `ETH_P_HDLC`
    #[cfg(linux_kernel)]
    pub const HDLC: Protocol = Protocol(new_raw_protocol((c::ETH_P_HDLC as u16).to_be() as u32));
    /// `ETH_P_ARCNET`
    #[cfg(linux_kernel)]
    pub const ARCNET: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_ARCNET as u16).to_be() as u32));
    /// `ETH_P_DSA`
    #[cfg(linux_kernel)]
    pub const DSA: Protocol = Protocol(new_raw_protocol((c::ETH_P_DSA as u16).to_be() as u32));
    /// `ETH_P_TRAILER`
    #[cfg(linux_kernel)]
    pub const TRAILER: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_TRAILER as u16).to_be() as u32));
    /// `ETH_P_PHONET`
    #[cfg(linux_kernel)]
    pub const PHONET: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_PHONET as u16).to_be() as u32));
    /// `ETH_P_IEEE802154`
    #[cfg(linux_kernel)]
    pub const IEEE802154: Protocol =
        Protocol(new_raw_protocol((c::ETH_P_IEEE802154 as u16).to_be() as u32));
    /// `ETH_P_CAIF`
    #[cfg(linux_kernel)]
    pub const CAIF: Protocol = Protocol(new_raw_protocol((c::ETH_P_CAIF as u16).to_be() as u32));
    /// `ETH_P_XDSA`
    #[cfg(linux_kernel)]
    pub const XDSA: Protocol = Protocol(new_raw_protocol((c::ETH_P_XDSA as u16).to_be() as u32));
    /// `ETH_P_MAP`
    #[cfg(linux_kernel)]
    pub const MAP: Protocol = Protocol(new_raw_protocol((c::ETH_P_MAP as u16).to_be() as u32));
    /// `ETH_P_MCTP`
    #[cfg(linux_kernel)]
    pub const MCTP: Protocol = Protocol(new_raw_protocol((c::ETH_P_MCTP as u16).to_be() as u32));
}

#[rustfmt::skip]
impl Protocol {
    /// Constructs a `Protocol` from a raw integer.
    #[inline]
    pub const fn from_raw(raw: RawProtocol) -> Self {
        Self(raw)
    }

    /// Returns the raw integer for this `Protocol`.
    #[inline]
    pub const fn as_raw(self) -> RawProtocol {
        self.0
    }
}

/// `SHUT_*` constants for use with [`shutdown`].
///
/// [`shutdown`]: crate::net::shutdown
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum Shutdown {
    /// `SHUT_RD`—Disable further read operations.
    Read = c::SHUT_RD as _,
    /// `SHUT_WR`—Disable further write operations.
    Write = c::SHUT_WR as _,
    /// `SHUT_RDWR`—Disable further read and write operations.
    Both = c::SHUT_RDWR as _,
}

bitflags! {
    /// `SOCK_*` constants for use with [`socket_with`], [`accept_with`] and
    /// [`acceptfrom_with`].
    ///
    /// [`socket_with`]: crate::net::socket_with
    /// [`accept_with`]: crate::net::accept_with
    /// [`acceptfrom_with`]: crate::net::acceptfrom_with
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SocketFlags: ffi::c_uint {
        /// `SOCK_NONBLOCK`
        #[cfg(not(any(
            apple,
            windows,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "horizon",
            target_os = "nto",
            target_os = "vita",
        )))]
        const NONBLOCK = bitcast!(c::SOCK_NONBLOCK);

        /// `SOCK_CLOEXEC`
        #[cfg(not(any(apple, windows, target_os = "aix", target_os = "haiku")))]
        const CLOEXEC = bitcast!(c::SOCK_CLOEXEC);

        // This deliberately lacks a `const _ = !0`, so that users can use
        // `from_bits_truncate` to extract the `SocketFlags` from a flags
        // value that also includes a `SocketType`.
    }
}

/// `AF_XDP` related types and constants.
#[cfg(target_os = "linux")]
pub mod xdp {
    use crate::backend::net::read_sockaddr::read_sockaddr_xdp;
    use crate::fd::{AsRawFd, BorrowedFd};
    use crate::net::addr::{call_with_sockaddr, SocketAddrArg, SocketAddrLen, SocketAddrOpaque};
    use crate::net::SocketAddrAny;

    use super::{bitflags, c};

    bitflags! {
        /// `XDP_OPTIONS_*` constants returned by [`get_xdp_options`].
        ///
        /// [`get_xdp_options`]: crate::net::sockopt::get_xdp_options
        #[repr(transparent)]
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
        pub struct XdpOptionsFlags: u32 {
            /// `XDP_OPTIONS_ZEROCOPY`
            const XDP_OPTIONS_ZEROCOPY = bitcast!(c::XDP_OPTIONS_ZEROCOPY);
        }
    }

    // Constant needs to be cast because bindgen does generate a `u32` but the
    // struct expects a `u16`.
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n15>
    bitflags! {
        /// `XDP_*` constants for use in [`SocketAddrXdp`].
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
        pub struct SocketAddrXdpFlags: u16 {
            /// `XDP_SHARED_UMEM`
            const XDP_SHARED_UMEM = bitcast!(c::XDP_SHARED_UMEM as u16);
            /// `XDP_COPY`
            const XDP_COPY = bitcast!(c::XDP_COPY  as u16);
            /// `XDP_COPY`
            const XDP_ZEROCOPY = bitcast!(c::XDP_ZEROCOPY as u16);
            /// `XDP_USE_NEED_WAKEUP`
            const XDP_USE_NEED_WAKEUP = bitcast!(c::XDP_USE_NEED_WAKEUP as u16);
            // requires kernel 6.6
            /// `XDP_USE_SG`
            const XDP_USE_SG = bitcast!(c::XDP_USE_SG as u16);
        }
    }

    bitflags! {
        /// `XDP_RING_*` constants for use in fill and/or Tx ring.
        #[repr(transparent)]
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
        pub struct XdpRingFlags: u32 {
            /// `XDP_RING_NEED_WAKEUP`
            const XDP_RING_NEED_WAKEUP = bitcast!(c::XDP_RING_NEED_WAKEUP);
        }
    }

    bitflags! {
        /// `XDP_UMEM_*` constants for use in [`XdpUmemReg`].
        #[repr(transparent)]
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
        pub struct XdpUmemRegFlags: u32 {
            /// `XDP_UMEM_UNALIGNED_CHUNK_FLAG`
            const XDP_UMEM_UNALIGNED_CHUNK_FLAG = bitcast!(c::XDP_UMEM_UNALIGNED_CHUNK_FLAG);
        }
    }

    /// A XDP socket address.
    ///
    /// Used to bind to XDP socket.
    ///
    /// Not ABI compatible with `struct sockaddr_xdp`.
    ///
    /// To add a shared UMEM file descriptor, use
    /// [`SocketAddrXdpWithSharedUmem`].
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n48>
    #[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
    pub struct SocketAddrXdp {
        /// Flags.
        sxdp_flags: SocketAddrXdpFlags,
        /// Interface index.
        sxdp_ifindex: u32,
        /// Queue ID.
        sxdp_queue_id: u32,
    }

    impl SocketAddrXdp {
        /// Construct a new XDP address.
        #[inline]
        pub const fn new(flags: SocketAddrXdpFlags, interface_index: u32, queue_id: u32) -> Self {
            Self {
                sxdp_flags: flags,
                sxdp_ifindex: interface_index,
                sxdp_queue_id: queue_id,
            }
        }

        /// Return flags.
        #[inline]
        pub fn flags(&self) -> SocketAddrXdpFlags {
            self.sxdp_flags
        }

        /// Set flags.
        #[inline]
        pub fn set_flags(&mut self, flags: SocketAddrXdpFlags) {
            self.sxdp_flags = flags;
        }

        /// Return interface index.
        #[inline]
        pub fn interface_index(&self) -> u32 {
            self.sxdp_ifindex
        }

        /// Set interface index.
        #[inline]
        pub fn set_interface_index(&mut self, interface_index: u32) {
            self.sxdp_ifindex = interface_index;
        }

        /// Return queue ID.
        #[inline]
        pub fn queue_id(&self) -> u32 {
            self.sxdp_queue_id
        }

        /// Set queue ID.
        #[inline]
        pub fn set_queue_id(&mut self, queue_id: u32) {
            self.sxdp_queue_id = queue_id;
        }
    }

    #[allow(unsafe_code)]
    // SAFETY: `with_sockaddr` calls `f` using `call_with_sockaddr`, which
    // handles calling `f` with the needed preconditions.
    unsafe impl SocketAddrArg for SocketAddrXdp {
        unsafe fn with_sockaddr<R>(
            &self,
            f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
        ) -> R {
            let addr = c::sockaddr_xdp {
                sxdp_family: c::AF_XDP as _,
                sxdp_flags: self.flags().bits(),
                sxdp_ifindex: self.interface_index(),
                sxdp_queue_id: self.queue_id(),
                sxdp_shared_umem_fd: !0,
            };

            call_with_sockaddr(&addr, f)
        }
    }

    impl From<SocketAddrXdp> for SocketAddrAny {
        #[inline]
        fn from(from: SocketAddrXdp) -> Self {
            from.as_any()
        }
    }

    impl TryFrom<SocketAddrAny> for SocketAddrXdp {
        type Error = crate::io::Errno;

        fn try_from(addr: SocketAddrAny) -> Result<Self, Self::Error> {
            read_sockaddr_xdp(&addr)
        }
    }

    /// An XDP socket address with a shared UMEM file descriptor.
    ///
    /// This implements `SocketAddrArg` so that it can be passed to [`bind`].
    ///
    /// [`bind`]: crate::net::bind
    #[derive(Debug)]
    pub struct SocketAddrXdpWithSharedUmem<'a> {
        /// XDP address.
        pub addr: SocketAddrXdp,
        /// Shared UMEM file descriptor.
        pub shared_umem_fd: BorrowedFd<'a>,
    }

    #[allow(unsafe_code)]
    // SAFETY: `with_sockaddr` calls `f` using `call_with_sockaddr`, which
    // handles calling `f` with the needed preconditions.
    unsafe impl<'a> SocketAddrArg for SocketAddrXdpWithSharedUmem<'a> {
        unsafe fn with_sockaddr<R>(
            &self,
            f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
        ) -> R {
            let addr = c::sockaddr_xdp {
                sxdp_family: c::AF_XDP as _,
                sxdp_flags: self.addr.flags().bits(),
                sxdp_ifindex: self.addr.interface_index(),
                sxdp_queue_id: self.addr.queue_id(),
                sxdp_shared_umem_fd: self.shared_umem_fd.as_raw_fd() as u32,
            };

            call_with_sockaddr(&addr, f)
        }
    }

    /// XDP ring offset.
    ///
    /// Used to mmap rings from kernel.
    ///
    /// Not ABI compatible with `struct xdp_ring_offset`.
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n59>
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XdpRingOffset {
        /// Producer offset.
        pub producer: u64,
        /// Consumer offset.
        pub consumer: u64,
        /// Descriptors offset.
        pub desc: u64,
        /// Flags offset.
        ///
        /// Is `None` if the kernel version (<5.4) does not yet support flags.
        pub flags: Option<u64>,
    }

    /// XDP mmap offsets.
    ///
    /// Not ABI compatible with `struct xdp_mmap_offsets`
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n66>
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XdpMmapOffsets {
        /// Rx ring offsets.
        pub rx: XdpRingOffset,
        /// Tx ring offsets.
        pub tx: XdpRingOffset,
        /// Fill ring offsets.
        pub fr: XdpRingOffset,
        /// Completion ring offsets.
        pub cr: XdpRingOffset,
    }

    /// XDP umem registration.
    ///
    /// `struct xdp_umem_reg`
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n79>
    #[repr(C)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XdpUmemReg {
        /// Start address of UMEM.
        pub addr: u64,
        /// Umem length in bytes.
        pub len: u64,
        /// Chunk size in bytes.
        pub chunk_size: u32,
        /// Headroom in bytes.
        pub headroom: u32,
        /// Flags.
        ///
        /// Requires kernel version 5.4.
        pub flags: XdpUmemRegFlags,
        /// `AF_XDP` TX metadata length
        ///
        /// Requires kernel version 6.8.
        pub tx_metadata_len: u32,
    }

    /// XDP statistics.
    ///
    /// Not ABI compatible with `struct xdp_statistics`
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n92>
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XdpStatistics {
        /// Rx dropped.
        pub rx_dropped: u64,
        /// Rx invalid descriptors.
        pub rx_invalid_descs: u64,
        /// Tx invalid descriptors.
        pub tx_invalid_descs: u64,
        /// Rx ring full.
        ///
        /// Is `None` if the kernel version (<5.9) does not yet support flags.
        pub rx_ring_full: Option<u64>,
        /// Rx fill ring empty descriptors.
        ///
        /// Is `None` if the kernel version (<5.9) does not yet support flags.
        pub rx_fill_ring_empty_descs: Option<u64>,
        /// Tx ring empty descriptors.
        ///
        /// Is `None` if the kernel version (<5.9) does not yet support flags.
        pub tx_ring_empty_descs: Option<u64>,
    }

    /// XDP options.
    ///
    /// Requires kernel version 5.3.
    /// `struct xdp_options`
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n101>
    #[repr(C)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XdpOptions {
        /// Flags.
        pub flags: XdpOptionsFlags,
    }

    /// XDP rx/tx frame descriptor.
    ///
    /// `struct xdp_desc`
    // <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/if_xdp.h?h=v6.13#n154>
    #[repr(C)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XdpDesc {
        /// Offset from the start of the UMEM.
        pub addr: u64,
        /// Length of packet in bytes.
        pub len: u32,
        /// Options.
        pub options: XdpDescOptions,
    }

    #[cfg(target_os = "linux")]
    bitflags! {
        /// `XDP_*` constants for use in [`XdpDesc`].
        ///
        /// Requires kernel version 6.6.
        #[repr(transparent)]
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
        pub struct XdpDescOptions: u32 {
            /// `XDP_PKT_CONTD`
            const XDP_PKT_CONTD = bitcast!(c::XDP_PKT_CONTD);
        }
    }

    /// Offset for mmapping rx ring.
    pub const XDP_PGOFF_RX_RING: u64 = c::XDP_PGOFF_RX_RING as u64;
    /// Offset for mmapping tx ring.
    pub const XDP_PGOFF_TX_RING: u64 = c::XDP_PGOFF_TX_RING as u64;
    /// Offset for mmapping fill ring.
    pub const XDP_UMEM_PGOFF_FILL_RING: u64 = c::XDP_UMEM_PGOFF_FILL_RING;
    /// Offset for mmapping completion ring.
    pub const XDP_UMEM_PGOFF_COMPLETION_RING: u64 = c::XDP_UMEM_PGOFF_COMPLETION_RING;

    /// Offset used to shift the [`XdpDesc`] addr to the right to extract the
    /// address offset in unaligned mode.
    pub const XSK_UNALIGNED_BUF_OFFSET_SHIFT: u64 = c::XSK_UNALIGNED_BUF_OFFSET_SHIFT as u64;
    /// Mask used to binary `and` the [`XdpDesc`] addr to extract the address
    /// without the offset carried in the upper 16 bits of the address in
    /// unaligned mode.
    pub const XSK_UNALIGNED_BUF_ADDR_MASK: u64 = c::XSK_UNALIGNED_BUF_ADDR_MASK;
}

/// UNIX credentials of socket peer, for use with [`get_socket_peercred`]
/// [`SendAncillaryMessage::ScmCredentials`] and
/// [`RecvAncillaryMessage::ScmCredentials`].
///
/// [`get_socket_peercred`]: crate::net::sockopt::socket_peercred
/// [`SendAncillaryMessage::ScmCredentials`]: crate::net::SendAncillaryMessage::ScmCredentials
/// [`RecvAncillaryMessage::ScmCredentials`]: crate::net::RecvAncillaryMessage::ScmCredentials
#[cfg(linux_kernel)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct UCred {
    /// Process ID of peer
    pub pid: crate::pid::Pid,
    /// User ID of peer
    pub uid: crate::ugid::Uid,
    /// Group ID of peer
    pub gid: crate::ugid::Gid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sizes() {
        #[cfg(target_os = "linux")]
        use crate::backend::c;
        use crate::ffi::c_int;
        use crate::net::addr::SocketAddrStorage;
        use core::mem::transmute;

        // Backend code needs to cast these to `c_int` so make sure that cast isn't
        // lossy.
        assert_eq_size!(RawProtocol, c_int);
        assert_eq_size!(Protocol, c_int);
        assert_eq_size!(Option<RawProtocol>, c_int);
        assert_eq_size!(Option<Protocol>, c_int);
        assert_eq_size!(RawSocketType, c_int);
        assert_eq_size!(SocketType, c_int);
        assert_eq_size!(SocketFlags, c_int);
        assert_eq_size!(SocketAddrStorage, c::sockaddr_storage);

        // Rustix doesn't depend on `Option<Protocol>` matching the ABI of a raw
        // integer for correctness, but it should work nonetheless.
        #[allow(unsafe_code)]
        unsafe {
            let t: Option<Protocol> = None;
            assert_eq!(0_u32, transmute::<Option<Protocol>, u32>(t));

            let t: Option<Protocol> = Some(Protocol::from_raw(RawProtocol::new(4567).unwrap()));
            assert_eq!(4567_u32, transmute::<Option<Protocol>, u32>(t));
        }

        #[cfg(linux_kernel)]
        assert_eq_size!(UCred, libc::ucred);

        #[cfg(target_os = "linux")]
        assert_eq_size!(super::xdp::XdpUmemReg, c::xdp_umem_reg);
        #[cfg(target_os = "linux")]
        assert_eq_size!(super::xdp::XdpOptions, c::xdp_options);
        #[cfg(target_os = "linux")]
        assert_eq_size!(super::xdp::XdpDesc, c::xdp_desc);
    }
}
