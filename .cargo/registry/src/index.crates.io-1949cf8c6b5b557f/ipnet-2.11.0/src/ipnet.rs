use alloc::vec::Vec;
use core::cmp::{min, max};
use core::cmp::Ordering::{Less, Equal};
use core::convert::From;
use core::fmt;
use core::iter::FusedIterator;
use core::option::Option::{Some, None};
#[cfg(not(feature = "std"))]
use core::error::Error;
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(not(feature = "std"))]
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
#[cfg(feature = "std")]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::ipext::{IpAdd, IpSub, IpStep, IpAddrRange, Ipv4AddrRange, Ipv6AddrRange};
use crate::mask::{ip_mask_to_prefix, ipv4_mask_to_prefix, ipv6_mask_to_prefix};

/// An IP network address, either IPv4 or IPv6.
///
/// This enum can contain either an [`Ipv4Net`] or an [`Ipv6Net`]. A
/// [`From`] implementation is provided to convert these into an
/// `IpNet`.
///
/// # Textual representation
///
/// `IpNet` provides a [`FromStr`] implementation for parsing network
/// addresses represented in CIDR notation. See [IETF RFC 4632] for the
/// CIDR notation.
///
/// [`Ipv4Net`]: struct.Ipv4Net.html
/// [`Ipv6Net`]: struct.Ipv6Net.html
/// [`From`]: https://doc.rust-lang.org/std/convert/trait.From.html
/// [`FromStr`]: https://doc.rust-lang.org/std/str/trait.FromStr.html
/// [IETF RFC 4632]: https://tools.ietf.org/html/rfc4632
///
/// # Examples
///
/// ```
/// use std::net::IpAddr;
/// use ipnet::IpNet;
///
/// let net: IpNet = "10.1.1.0/24".parse().unwrap();
/// assert_eq!(Ok(net.network()), "10.1.1.0".parse());
///
/// let net: IpNet = "fd00::/32".parse().unwrap();
/// assert_eq!(Ok(net.network()), "fd00::".parse());
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum IpNet {
    V4(Ipv4Net),
    V6(Ipv6Net),
}

/// An IPv4 network address.
///
/// See [`IpNet`] for a type encompassing both IPv4 and IPv6 network
/// addresses.
///
/// # Textual representation
///
/// `Ipv4Net` provides a [`FromStr`] implementation for parsing network
/// addresses represented in CIDR notation. See [IETF RFC 4632] for the
/// CIDR notation.
///
/// [`IpNet`]: enum.IpNet.html
/// [`FromStr`]: https://doc.rust-lang.org/std/str/trait.FromStr.html
/// [IETF RFC 4632]: https://tools.ietf.org/html/rfc4632
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "std")]
/// # use std::net::Ipv6Addr;
/// # #[cfg(not(feature = "std"))]
/// # use core::net::Ipv6Addr;
/// use ipnet::Ipv4Net;
///
/// let net: Ipv4Net = "10.1.1.0/24".parse().unwrap();
/// assert_eq!(Ok(net.network()), "10.1.1.0".parse());
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Ipv4Net {
    addr: Ipv4Addr,
    prefix_len: u8,
}

/// An IPv6 network address.
///
/// See [`IpNet`] for a type encompassing both IPv4 and IPv6 network
/// addresses.
///
/// # Textual representation
///
/// `Ipv6Net` provides a [`FromStr`] implementation for parsing network
/// addresses represented in CIDR notation. See [IETF RFC 4632] for the
/// CIDR notation.
///
/// [`IpNet`]: enum.IpNet.html
/// [`FromStr`]: https://doc.rust-lang.org/std/str/trait.FromStr.html
/// [IETF RFC 4632]: https://tools.ietf.org/html/rfc4632
///
/// # Examples
///
/// ```
/// use std::net::Ipv6Addr;
/// use ipnet::Ipv6Net;
///
/// let net: Ipv6Net = "fd00::/32".parse().unwrap();
/// assert_eq!(Ok(net.network()), "fd00::".parse());
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Ipv6Net {
    addr: Ipv6Addr,
    prefix_len: u8,
}

/// An error which can be returned when the prefix length is invalid.
///
/// Valid prefix lengths are 0 to 32 for IPv4 and 0 to 128 for IPv6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixLenError;

impl fmt::Display for PrefixLenError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("invalid IP prefix length")
    }
}

impl Error for PrefixLenError {}

impl IpNet {
    /// Creates a new IP network address from an `IpAddr` and prefix
    /// length.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ipnet::{IpNet, PrefixLenError};
    ///
    /// let net = IpNet::new(Ipv6Addr::LOCALHOST.into(), 48);
    /// assert!(net.is_ok());
    /// 
    /// let bad_prefix_len = IpNet::new(Ipv6Addr::LOCALHOST.into(), 129);
    /// assert_eq!(bad_prefix_len, Err(PrefixLenError));
    /// ```
    pub fn new(ip: IpAddr, prefix_len: u8) -> Result<IpNet, PrefixLenError> {
        Ok(match ip {
            IpAddr::V4(a) => Ipv4Net::new(a, prefix_len)?.into(),
            IpAddr::V6(a) => Ipv6Net::new(a, prefix_len)?.into(),
        })
    }

    /// Creates a new IP network address from an `IpAddr` and prefix
    /// length. If called from a const context it will verify prefix length
    /// at compile time. Otherwise it will panic at runtime if prefix length
    /// is incorrect for a given IpAddr type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    /// use ipnet::{IpNet};
    ///
    /// // This code is verified at compile time:
    /// const NET: IpNet = IpNet::new_assert(IpAddr::V4(Ipv4Addr::new(10, 1, 1, 0)), 24);
    /// assert_eq!(NET.prefix_len(), 24);
    ///
    /// // This code is verified at runtime:
    /// let net = IpNet::new_assert(Ipv6Addr::LOCALHOST.into(), 24);
    /// assert_eq!(net.prefix_len(), 24);
    ///
    /// // This code does not compile:
    /// // const BAD_PREFIX_LEN: IpNet = IpNet::new_assert(IpAddr::V4(Ipv4Addr::new(10, 1, 1, 0)), 33);
    ///
    /// // This code panics at runtime:
    /// // let bad_prefix_len = IpNet::new_assert(Ipv6Addr::LOCALHOST.into(), 129);
    /// ```
    pub const fn new_assert(ip: IpAddr, prefix_len: u8) -> IpNet {
        match ip {
            IpAddr::V4(a) => IpNet::V4(Ipv4Net::new_assert(a, prefix_len)),
            IpAddr::V6(a) => IpNet::V6(Ipv6Net::new_assert(a, prefix_len)),
        }
    }

    /// Creates a new IP network address from an `IpAddr` and netmask.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ipnet::{IpNet, PrefixLenError};
    ///
    /// let net = IpNet::with_netmask(Ipv6Addr::LOCALHOST.into(), Ipv6Addr::from(0xffff_ffff_ffff_0000_0000_0000_0000_0000).into());
    /// assert!(net.is_ok());
    ///
    /// let bad_prefix_len = IpNet::with_netmask(Ipv6Addr::LOCALHOST.into(), Ipv6Addr::from(0xffff_ffff_ffff_0000_0001_0000_0000_0000).into());
    /// assert_eq!(bad_prefix_len, Err(PrefixLenError));
    /// ```
    pub fn with_netmask(ip: IpAddr, netmask: IpAddr) -> Result<IpNet, PrefixLenError> {
        let prefix = ip_mask_to_prefix(netmask)?;
        Self::new(ip, prefix)
    }

    /// Returns a copy of the network with the address truncated to the
    /// prefix length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::IpNet;
    /// #
    /// assert_eq!(
    ///     "192.168.12.34/16".parse::<IpNet>().unwrap().trunc(),
    ///     "192.168.0.0/16".parse().unwrap()
    /// );
    ///
    /// assert_eq!(
    ///     "fd00::1:2:3:4/16".parse::<IpNet>().unwrap().trunc(),
    ///     "fd00::/16".parse().unwrap()
    /// );
    /// ```
    pub fn trunc(&self) -> IpNet {
        match *self {
            IpNet::V4(ref a) => IpNet::V4(a.trunc()),
            IpNet::V6(ref a) => IpNet::V6(a.trunc()),
        }
    }

    /// Returns the address.
    pub fn addr(&self) -> IpAddr {
        match *self {
            IpNet::V4(ref a) => IpAddr::V4(a.addr),
            IpNet::V6(ref a) => IpAddr::V6(a.addr),
        }
    }

    /// Returns the prefix length.
    pub fn prefix_len(&self) -> u8 {
        match *self {
            IpNet::V4(ref a) => a.prefix_len(),
            IpNet::V6(ref a) => a.prefix_len(),
        }
    }

    /// Returns the maximum valid prefix length.
    pub fn max_prefix_len(&self) -> u8 {
        match *self {
            IpNet::V4(ref a) => a.max_prefix_len(),
            IpNet::V6(ref a) => a.max_prefix_len(),
        }
    }

    /// Returns the network mask.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::IpAddr;
    /// # use ipnet::IpNet;
    /// #
    /// let net: IpNet = "10.1.0.0/20".parse().unwrap();
    /// assert_eq!(Ok(net.netmask()), "255.255.240.0".parse());
    ///
    /// let net: IpNet = "fd00::/24".parse().unwrap();
    /// assert_eq!(Ok(net.netmask()), "ffff:ff00::".parse());
    /// ```
    pub fn netmask(&self) -> IpAddr {
        match *self {
            IpNet::V4(ref a) => IpAddr::V4(a.netmask()),
            IpNet::V6(ref a) => IpAddr::V6(a.netmask()),
        }
    }

    /// Returns the host mask.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::IpAddr;
    /// # use ipnet::IpNet;
    /// #
    /// let net: IpNet = "10.1.0.0/20".parse().unwrap();
    /// assert_eq!(Ok(net.hostmask()), "0.0.15.255".parse());
    ///
    /// let net: IpNet = "fd00::/24".parse().unwrap();
    /// assert_eq!(Ok(net.hostmask()), "::ff:ffff:ffff:ffff:ffff:ffff:ffff".parse());
    /// ```
    pub fn hostmask(&self) -> IpAddr {
        match *self {
            IpNet::V4(ref a) => IpAddr::V4(a.hostmask()),
            IpNet::V6(ref a) => IpAddr::V6(a.hostmask()),
        }
    }
    
    /// Returns the network address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::IpAddr;
    /// # use ipnet::IpNet;
    /// #
    /// let net: IpNet = "172.16.123.123/16".parse().unwrap();
    /// assert_eq!(Ok(net.network()), "172.16.0.0".parse());
    ///
    /// let net: IpNet = "fd00:1234:5678::/24".parse().unwrap();
    /// assert_eq!(Ok(net.network()), "fd00:1200::".parse());
    /// ```
    pub fn network(&self) -> IpAddr {
        match *self {
            IpNet::V4(ref a) => IpAddr::V4(a.network()),
            IpNet::V6(ref a) => IpAddr::V6(a.network()),
        }
    }    
    
    /// Returns the broadcast address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::IpAddr;
    /// # use ipnet::IpNet;
    /// #
    /// let net: IpNet = "172.16.0.0/22".parse().unwrap();
    /// assert_eq!(Ok(net.broadcast()), "172.16.3.255".parse());
    ///
    /// let net: IpNet = "fd00:1234:5678::/24".parse().unwrap();
    /// assert_eq!(Ok(net.broadcast()), "fd00:12ff:ffff:ffff:ffff:ffff:ffff:ffff".parse());
    /// ```
    pub fn broadcast(&self) -> IpAddr {
        match *self {
            IpNet::V4(ref a) => IpAddr::V4(a.broadcast()),
            IpNet::V6(ref a) => IpAddr::V6(a.broadcast()),
        }
    }
    
    /// Returns the `IpNet` that contains this one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::IpNet;
    /// #
    /// let n1: IpNet = "172.16.1.0/24".parse().unwrap();
    /// let n2: IpNet = "172.16.0.0/23".parse().unwrap();
    /// let n3: IpNet = "172.16.0.0/0".parse().unwrap();
    ///
    /// assert_eq!(n1.supernet().unwrap(), n2);
    /// assert_eq!(n3.supernet(), None);
    ///
    /// let n1: IpNet = "fd00:ff00::/24".parse().unwrap();
    /// let n2: IpNet = "fd00:fe00::/23".parse().unwrap();
    /// let n3: IpNet = "fd00:fe00::/0".parse().unwrap();
    ///
    /// assert_eq!(n1.supernet().unwrap(), n2);
    /// assert_eq!(n3.supernet(), None);
    /// ```
    pub fn supernet(&self) -> Option<IpNet> {
        match *self {
            IpNet::V4(ref a) => a.supernet().map(IpNet::V4),
            IpNet::V6(ref a) => a.supernet().map(IpNet::V6),
        }
    }

    /// Returns `true` if this network and the given network are 
    /// children of the same supernet.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::IpNet;
    /// #
    /// let n4_1: IpNet = "10.1.0.0/24".parse().unwrap();
    /// let n4_2: IpNet = "10.1.1.0/24".parse().unwrap();
    /// let n4_3: IpNet = "10.1.2.0/24".parse().unwrap();
    /// let n6_1: IpNet = "fd00::/18".parse().unwrap();
    /// let n6_2: IpNet = "fd00:4000::/18".parse().unwrap();
    /// let n6_3: IpNet = "fd00:8000::/18".parse().unwrap();
    ///
    /// assert!( n4_1.is_sibling(&n4_2));
    /// assert!(!n4_2.is_sibling(&n4_3));
    /// assert!( n6_1.is_sibling(&n6_2));
    /// assert!(!n6_2.is_sibling(&n6_3));
    /// assert!(!n4_1.is_sibling(&n6_2));
    /// ```
    pub fn is_sibling(&self, other: &IpNet) -> bool {
        match (*self, *other) {
            (IpNet::V4(ref a), IpNet::V4(ref b)) => a.is_sibling(b),
            (IpNet::V6(ref a), IpNet::V6(ref b)) => a.is_sibling(b),
            _ => false,
        }
    }

    /// Return an `Iterator` over the host addresses in this network.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::IpAddr;
    /// # use ipnet::IpNet;
    /// #
    /// let net: IpNet = "10.0.0.0/30".parse().unwrap();
    /// assert_eq!(net.hosts().collect::<Vec<IpAddr>>(), vec![
    ///     "10.0.0.1".parse::<IpAddr>().unwrap(),
    ///     "10.0.0.2".parse().unwrap(),
    /// ]);
    ///
    /// let net: IpNet = "10.0.0.0/31".parse().unwrap();
    /// assert_eq!(net.hosts().collect::<Vec<IpAddr>>(), vec![
    ///     "10.0.0.0".parse::<IpAddr>().unwrap(),
    ///     "10.0.0.1".parse().unwrap(),
    /// ]);
    ///
    /// let net: IpNet = "fd00::/126".parse().unwrap();
    /// assert_eq!(net.hosts().collect::<Vec<IpAddr>>(), vec![
    ///     "fd00::".parse::<IpAddr>().unwrap(),
    ///     "fd00::1".parse().unwrap(),
    ///     "fd00::2".parse().unwrap(),
    ///     "fd00::3".parse().unwrap(),
    /// ]);
    /// ```
    pub fn hosts(&self) -> IpAddrRange {
        match *self {
            IpNet::V4(ref a) => IpAddrRange::V4(a.hosts()),
            IpNet::V6(ref a) => IpAddrRange::V6(a.hosts()),
        }
    }
    
    /// Returns an `Iterator` over the subnets of this network with the
    /// given prefix length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::{IpNet, PrefixLenError};
    /// #
    /// let net: IpNet = "10.0.0.0/24".parse().unwrap();
    /// assert_eq!(net.subnets(26).unwrap().collect::<Vec<IpNet>>(), vec![
    ///     "10.0.0.0/26".parse::<IpNet>().unwrap(),
    ///     "10.0.0.64/26".parse().unwrap(),
    ///     "10.0.0.128/26".parse().unwrap(),
    ///     "10.0.0.192/26".parse().unwrap(),
    /// ]);
    ///
    /// let net: IpNet = "fd00::/16".parse().unwrap();
    /// assert_eq!(net.subnets(18).unwrap().collect::<Vec<IpNet>>(), vec![
    ///     "fd00::/18".parse::<IpNet>().unwrap(),
    ///     "fd00:4000::/18".parse().unwrap(),
    ///     "fd00:8000::/18".parse().unwrap(),
    ///     "fd00:c000::/18".parse().unwrap(),
    /// ]);
    ///
    /// let net: IpNet = "10.0.0.0/24".parse().unwrap();
    /// assert_eq!(net.subnets(23), Err(PrefixLenError));
    ///
    /// let net: IpNet = "10.0.0.0/24".parse().unwrap();
    /// assert_eq!(net.subnets(33), Err(PrefixLenError));
    ///
    /// let net: IpNet = "fd00::/16".parse().unwrap();
    /// assert_eq!(net.subnets(15), Err(PrefixLenError));
    ///
    /// let net: IpNet = "fd00::/16".parse().unwrap();
    /// assert_eq!(net.subnets(129), Err(PrefixLenError));
    /// ```
    pub fn subnets(&self, new_prefix_len: u8) -> Result<IpSubnets, PrefixLenError> {
        match *self {
            IpNet::V4(ref a) => a.subnets(new_prefix_len).map(IpSubnets::V4),
            IpNet::V6(ref a) => a.subnets(new_prefix_len).map(IpSubnets::V6),
        }
    }

    /// Test if a network address contains either another network
    /// address or an IP address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::IpAddr;
    /// # use ipnet::IpNet;
    /// #
    /// let net4: IpNet = "192.168.0.0/24".parse().unwrap();
    /// let net4_yes: IpNet = "192.168.0.0/25".parse().unwrap();
    /// let net4_no: IpNet = "192.168.0.0/23".parse().unwrap();
    /// let ip4_yes: IpAddr = "192.168.0.1".parse().unwrap();
    /// let ip4_no: IpAddr = "192.168.1.0".parse().unwrap();
    ///
    /// assert!(net4.contains(&net4));
    /// assert!(net4.contains(&net4_yes));
    /// assert!(!net4.contains(&net4_no));
    /// assert!(net4.contains(&ip4_yes));
    /// assert!(!net4.contains(&ip4_no));
    ///
    ///
    /// let net6: IpNet = "fd00::/16".parse().unwrap();
    /// let net6_yes: IpNet = "fd00::/17".parse().unwrap();
    /// let net6_no: IpNet = "fd00::/15".parse().unwrap();
    /// let ip6_yes: IpAddr = "fd00::1".parse().unwrap();
    /// let ip6_no: IpAddr = "fd01::".parse().unwrap();
    ///
    /// assert!(net6.contains(&net6));
    /// assert!(net6.contains(&net6_yes));
    /// assert!(!net6.contains(&net6_no));
    /// assert!(net6.contains(&ip6_yes));
    /// assert!(!net6.contains(&ip6_no));
    ///
    /// assert!(!net4.contains(&net6));
    /// assert!(!net6.contains(&net4));
    /// assert!(!net4.contains(&ip6_no));
    /// assert!(!net6.contains(&ip4_no));
    /// ```
    pub fn contains<T>(&self, other: T) -> bool where Self: Contains<T> {
        Contains::contains(self, other)
    }

    /// Aggregate a `Vec` of `IpNet`s and return the result as a new
    /// `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::IpNet;
    /// #
    /// let nets = vec![
    ///     "10.0.0.0/24".parse::<IpNet>().unwrap(),
    ///     "10.0.1.0/24".parse().unwrap(),
    ///     "10.0.2.0/24".parse().unwrap(),
    ///     "fd00::/18".parse().unwrap(),
    ///     "fd00:4000::/18".parse().unwrap(),
    ///     "fd00:8000::/18".parse().unwrap(),
    /// ];
    ///
    /// assert_eq!(IpNet::aggregate(&nets), vec![
    ///     "10.0.0.0/23".parse::<IpNet>().unwrap(),
    ///     "10.0.2.0/24".parse().unwrap(),
    ///     "fd00::/17".parse().unwrap(),
    ///     "fd00:8000::/18".parse().unwrap(),
    /// ]);
    /// ```
    pub fn aggregate(networks: &Vec<IpNet>) -> Vec<IpNet> {
        // It's 2.5x faster to split the input up and run them using the
        // specific IPv4 and IPV6 implementations. merge_intervals() and
        // the comparisons are much faster running over integers.
        let mut ipv4nets: Vec<Ipv4Net> = Vec::new();
        let mut ipv6nets: Vec<Ipv6Net> = Vec::new();

        for n in networks {
            match *n {
                IpNet::V4(x) => ipv4nets.push(x),
                IpNet::V6(x) => ipv6nets.push(x),
            }
        }

        let mut res: Vec<IpNet> = Vec::new();
        let ipv4aggs = Ipv4Net::aggregate(&ipv4nets);
        let ipv6aggs = Ipv6Net::aggregate(&ipv6nets);
        res.extend::<Vec<IpNet>>(ipv4aggs.into_iter().map(IpNet::V4).collect::<Vec<IpNet>>());
        res.extend::<Vec<IpNet>>(ipv6aggs.into_iter().map(IpNet::V6).collect::<Vec<IpNet>>());
        res
    }
}

impl Default for IpNet {
    fn default() -> Self {
        Self::V4(Ipv4Net::default())
    }
}

impl fmt::Debug for IpNet {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

impl fmt::Display for IpNet {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IpNet::V4(ref a) => a.fmt(fmt),
            IpNet::V6(ref a) => a.fmt(fmt),
        }
    }
}

impl From<Ipv4Net> for IpNet {
    fn from(net: Ipv4Net) -> IpNet {
        IpNet::V4(net)
    }
}

impl From<Ipv6Net> for IpNet {
    fn from(net: Ipv6Net) -> IpNet {
        IpNet::V6(net)
    }
}

impl From<IpAddr> for IpNet {
    fn from(addr: IpAddr) -> IpNet {
        match addr {
            IpAddr::V4(a) => IpNet::V4(a.into()),
            IpAddr::V6(a) => IpNet::V6(a.into()),
        }
    }
}

impl Ipv4Net {
    /// Creates a new IPv4 network address from an `Ipv4Addr` and prefix
    /// length.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ipnet::{Ipv4Net, PrefixLenError};
    ///
    /// let net = Ipv4Net::new(Ipv4Addr::new(10, 1, 1, 0), 24);
    /// assert!(net.is_ok());
    ///
    /// let bad_prefix_len = Ipv4Net::new(Ipv4Addr::new(10, 1, 1, 0), 33);
    /// assert_eq!(bad_prefix_len, Err(PrefixLenError));
    /// ```
    #[inline]
    pub const fn new(ip: Ipv4Addr, prefix_len: u8) -> Result<Ipv4Net, PrefixLenError> {
        if prefix_len > 32 {
            return Err(PrefixLenError);
        }
        Ok(Ipv4Net { addr: ip, prefix_len: prefix_len })
    }

    /// Creates a new IPv4 network address from an `Ipv4Addr` and prefix
    /// length. If called from a const context it will verify prefix length
    /// at compile time. Otherwise it will panic at runtime if prefix length
    /// is not less then or equal to 32.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ipnet::{Ipv4Net};
    ///
    /// // This code is verified at compile time:
    /// const NET: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(10, 1, 1, 0), 24);
    /// assert_eq!(NET.prefix_len(), 24);
    ///
    /// // This code is verified at runtime:
    /// let net = Ipv4Net::new_assert(Ipv4Addr::new(10, 1, 1, 0), 24);
    /// assert_eq!(NET.prefix_len(), 24);
    ///
    /// // This code does not compile:
    /// // const BAD_PREFIX_LEN: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(10, 1, 1, 0), 33);
    ///
    /// // This code panics at runtime:
    /// // let bad_prefix_len = Ipv4Net::new_assert(Ipv4Addr::new(10, 1, 1, 0), 33);
    /// ```
    #[inline]
    pub const fn new_assert(ip: Ipv4Addr, prefix_len: u8) -> Ipv4Net {
        assert!(prefix_len <= 32, "PREFIX_LEN must be less then or equal to 32 for Ipv4Net");
        Ipv4Net { addr: ip, prefix_len: prefix_len }
    }

    /// Creates a new IPv4 network address from an `Ipv4Addr` and netmask.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ipnet::{Ipv4Net, PrefixLenError};
    ///
    /// let net = Ipv4Net::with_netmask(Ipv4Addr::new(10, 1, 1, 0), Ipv4Addr::new(255, 255, 255, 0));
    /// assert!(net.is_ok());
    ///
    /// let bad_prefix_len = Ipv4Net::with_netmask(Ipv4Addr::new(10, 1, 1, 0), Ipv4Addr::new(255, 255, 0, 1));
    /// assert_eq!(bad_prefix_len, Err(PrefixLenError));
    /// ```
    pub fn with_netmask(ip: Ipv4Addr, netmask: Ipv4Addr) -> Result<Ipv4Net, PrefixLenError> {
        let prefix = ipv4_mask_to_prefix(netmask)?;
        Self::new(ip, prefix)
    }

    /// Returns a copy of the network with the address truncated to the
    /// prefix length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv4Net;
    /// #
    /// assert_eq!(
    ///     "192.168.12.34/16".parse::<Ipv4Net>().unwrap().trunc(),
    ///     "192.168.0.0/16".parse().unwrap()
    /// );
    /// ```
    pub fn trunc(&self) -> Ipv4Net {
        Ipv4Net::new(self.network(), self.prefix_len).unwrap()
    }

    /// Returns the address.
    #[inline]
    pub const fn addr(&self) -> Ipv4Addr {
        self.addr
    }

    /// Returns the prefix length.
    #[inline]
    pub const fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    /// Returns the maximum valid prefix length.
    #[inline]
    pub const fn max_prefix_len(&self) -> u8 {
        32
    }
    
    /// Returns the network mask.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv4Addr;
    /// # use ipnet::Ipv4Net;
    /// #
    /// let net: Ipv4Net = "10.1.0.0/20".parse().unwrap();
    /// assert_eq!(Ok(net.netmask()), "255.255.240.0".parse());
    /// ```
    pub fn netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.netmask_u32())
    }

    fn netmask_u32(&self) -> u32 {
        u32::max_value().checked_shl(32 - self.prefix_len as u32).unwrap_or(0)
    }

    /// Returns the host mask.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv4Addr;
    /// # use ipnet::Ipv4Net;
    /// #
    /// let net: Ipv4Net = "10.1.0.0/20".parse().unwrap();
    /// assert_eq!(Ok(net.hostmask()), "0.0.15.255".parse());
    /// ```
    pub fn hostmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.hostmask_u32())
    }

    fn hostmask_u32(&self) -> u32 {
        u32::max_value().checked_shr(self.prefix_len as u32).unwrap_or(0)
    }

    /// Returns the network address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv4Addr;
    /// # use ipnet::Ipv4Net;
    /// #
    /// let net: Ipv4Net = "172.16.123.123/16".parse().unwrap();
    /// assert_eq!(Ok(net.network()), "172.16.0.0".parse());
    /// ```
    pub fn network(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) & self.netmask_u32())
    }

    /// Returns the broadcast address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv4Addr;
    /// # use ipnet::Ipv4Net;
    /// #
    /// let net: Ipv4Net = "172.16.0.0/22".parse().unwrap();
    /// assert_eq!(Ok(net.broadcast()), "172.16.3.255".parse());
    /// ```
    pub fn broadcast(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | self.hostmask_u32())
    }

    /// Returns the `Ipv4Net` that contains this one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv4Net;
    /// #
    /// let n1: Ipv4Net = "172.16.1.0/24".parse().unwrap();
    /// let n2: Ipv4Net = "172.16.0.0/23".parse().unwrap();
    /// let n3: Ipv4Net = "172.16.0.0/0".parse().unwrap();
    ///
    /// assert_eq!(n1.supernet().unwrap(), n2);
    /// assert_eq!(n3.supernet(), None);
    /// ```
    pub fn supernet(&self) -> Option<Ipv4Net> {
        Ipv4Net::new(self.addr, self.prefix_len.wrapping_sub(1)).map(|n| n.trunc()).ok()
    }

    /// Returns `true` if this network and the given network are 
    /// children of the same supernet.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv4Net;
    /// #
    /// let n1: Ipv4Net = "10.1.0.0/24".parse().unwrap();
    /// let n2: Ipv4Net = "10.1.1.0/24".parse().unwrap();
    /// let n3: Ipv4Net = "10.1.2.0/24".parse().unwrap();
    ///
    /// assert!(n1.is_sibling(&n2));
    /// assert!(!n2.is_sibling(&n3));
    /// ```
    pub fn is_sibling(&self, other: &Ipv4Net) -> bool {
        self.prefix_len > 0 &&
        self.prefix_len == other.prefix_len &&
        self.supernet().unwrap().contains(other)
    }
    
    /// Return an `Iterator` over the host addresses in this network.
    ///
    /// If the prefix length is less than 31 both the network address
    /// and broadcast address are excluded. These are only valid host
    /// addresses when the prefix length is 31.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv4Addr;
    /// # use ipnet::Ipv4Net;
    /// #
    /// let net: Ipv4Net = "10.0.0.0/30".parse().unwrap();
    /// assert_eq!(net.hosts().collect::<Vec<Ipv4Addr>>(), vec![
    ///     "10.0.0.1".parse::<Ipv4Addr>().unwrap(),
    ///     "10.0.0.2".parse().unwrap(),
    /// ]);
    ///
    /// let net: Ipv4Net = "10.0.0.0/31".parse().unwrap();
    /// assert_eq!(net.hosts().collect::<Vec<Ipv4Addr>>(), vec![
    ///     "10.0.0.0".parse::<Ipv4Addr>().unwrap(),
    ///     "10.0.0.1".parse().unwrap(),
    /// ]);
    /// ```
    pub fn hosts(&self) -> Ipv4AddrRange {
        let mut start = self.network();
        let mut end = self.broadcast();
        
        if self.prefix_len < 31 {
            start = start.saturating_add(1);
            end = end.saturating_sub(1);
        }
        
        Ipv4AddrRange::new(start, end)
    }

    /// Returns an `Iterator` over the subnets of this network with the
    /// given prefix length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::{Ipv4Net, PrefixLenError};
    /// #
    /// let net: Ipv4Net = "10.0.0.0/24".parse().unwrap();
    /// assert_eq!(net.subnets(26).unwrap().collect::<Vec<Ipv4Net>>(), vec![
    ///     "10.0.0.0/26".parse::<Ipv4Net>().unwrap(),
    ///     "10.0.0.64/26".parse().unwrap(),
    ///     "10.0.0.128/26".parse().unwrap(),
    ///     "10.0.0.192/26".parse().unwrap(),
    /// ]);
    ///
    /// let net: Ipv4Net = "10.0.0.0/30".parse().unwrap();
    /// assert_eq!(net.subnets(32).unwrap().collect::<Vec<Ipv4Net>>(), vec![
    ///     "10.0.0.0/32".parse::<Ipv4Net>().unwrap(),
    ///     "10.0.0.1/32".parse().unwrap(),
    ///     "10.0.0.2/32".parse().unwrap(),
    ///     "10.0.0.3/32".parse().unwrap(),
    /// ]);
    ///
    /// let net: Ipv4Net = "10.0.0.0/24".parse().unwrap();
    /// assert_eq!(net.subnets(23), Err(PrefixLenError));
    ///
    /// let net: Ipv4Net = "10.0.0.0/24".parse().unwrap();
    /// assert_eq!(net.subnets(33), Err(PrefixLenError));
    /// ```
    pub fn subnets(&self, new_prefix_len: u8) -> Result<Ipv4Subnets, PrefixLenError> {
        if self.prefix_len > new_prefix_len || new_prefix_len > 32 {
            return Err(PrefixLenError);
        }
        
        Ok(Ipv4Subnets::new(
            self.network(),
            self.broadcast(),
            new_prefix_len,
        ))
    }

    /// Test if a network address contains either another network
    /// address or an IP address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv4Addr;
    /// # use ipnet::Ipv4Net;
    /// #
    /// let net: Ipv4Net = "192.168.0.0/24".parse().unwrap();
    /// let net_yes: Ipv4Net = "192.168.0.0/25".parse().unwrap();
    /// let net_no: Ipv4Net = "192.168.0.0/23".parse().unwrap();
    /// let ip_yes: Ipv4Addr = "192.168.0.1".parse().unwrap();
    /// let ip_no: Ipv4Addr = "192.168.1.0".parse().unwrap();
    ///
    /// assert!(net.contains(&net));
    /// assert!(net.contains(&net_yes));
    /// assert!(!net.contains(&net_no));
    /// assert!(net.contains(&ip_yes));
    /// assert!(!net.contains(&ip_no));
    /// ```
    pub fn contains<T>(&self, other: T) -> bool where Self: Contains<T> {
        Contains::contains(self, other)
    }

    // It is significantly faster to work on u32 than Ipv4Addr.
    fn interval(&self) -> (u32, u32) {
        (
            u32::from(self.network()),
            u32::from(self.broadcast()).saturating_add(1),
        )
    }

    /// Aggregate a `Vec` of `Ipv4Net`s and return the result as a new
    /// `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv4Net;
    /// #
    /// let nets = vec![
    ///     "10.0.0.0/24".parse::<Ipv4Net>().unwrap(),
    ///     "10.0.1.0/24".parse().unwrap(),
    ///     "10.0.2.0/24".parse().unwrap(),
    /// ];
    ///
    /// assert_eq!(Ipv4Net::aggregate(&nets), vec![
    ///     "10.0.0.0/23".parse::<Ipv4Net>().unwrap(),
    ///     "10.0.2.0/24".parse().unwrap(),
    /// ]);
    pub fn aggregate(networks: &Vec<Ipv4Net>) -> Vec<Ipv4Net> {
        let mut intervals: Vec<(_, _)> = networks.iter().map(|n| n.interval()).collect();
        intervals = merge_intervals(intervals);
        let mut res: Vec<Ipv4Net> = Vec::new();
        
        for (start, mut end) in intervals {
            if end != core::u32::MAX {
                end = end.saturating_sub(1)
            }
            let iter = Ipv4Subnets::new(start.into(), end.into(), 0);
            res.extend(iter);
        }
        res
    }
}

impl Default for Ipv4Net {
    fn default() -> Self {
        Self {
            addr: Ipv4Addr::from(0),
            prefix_len: 0,
        }
    }
}

impl fmt::Debug for Ipv4Net {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

impl fmt::Display for Ipv4Net {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}/{}", self.addr, self.prefix_len)
    }
}

impl From<Ipv4Addr> for Ipv4Net {
    fn from(addr: Ipv4Addr) -> Ipv4Net {
        Ipv4Net { addr, prefix_len: 32 }
    }
}

impl Ipv6Net {    
    /// Creates a new IPv6 network address from an `Ipv6Addr` and prefix
    /// length.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ipnet::{Ipv6Net, PrefixLenError};
    ///
    /// let net = Ipv6Net::new(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), 24);
    /// assert!(net.is_ok());
    ///
    /// let bad_prefix_len = Ipv6Net::new(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), 129);
    /// assert_eq!(bad_prefix_len, Err(PrefixLenError));
    /// ```
    #[inline]
    pub const fn new(ip: Ipv6Addr, prefix_len: u8) -> Result<Ipv6Net, PrefixLenError> {
        if prefix_len > 128 {
            return Err(PrefixLenError);
        }
        Ok(Ipv6Net { addr: ip, prefix_len: prefix_len })
    }

    /// Creates a new IPv6 network address from an `Ipv6Addr` and prefix
    /// length. If called from a const context it will verify prefix length
    /// at compile time. Otherwise it will panic at runtime if prefix length
    /// is not less then or equal to 128.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ipnet::{Ipv6Net};
    ///
    /// // This code is verified at compile time:
    /// const NET: Ipv6Net = Ipv6Net::new_assert(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), 24);
    /// assert_eq!(NET.prefix_len(), 24);
    ///
    /// // This code is verified at runtime:
    /// let net = Ipv6Net::new_assert(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), 24);
    /// assert_eq!(net.prefix_len(), 24);
    ///
    /// // This code does not compile:
    /// // const BAD_PREFIX_LEN: Ipv6Net = Ipv6Net::new_assert(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), 129);
    ///
    /// // This code panics at runtime:
    /// // let bad_prefix_len = Ipv6Addr::new_assert(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), 129);
    /// ```
    #[inline]
    pub const fn new_assert(ip: Ipv6Addr, prefix_len: u8) -> Ipv6Net {
        assert!(prefix_len <= 128, "PREFIX_LEN must be less then or equal to 128 for Ipv6Net");
        Ipv6Net { addr: ip, prefix_len: prefix_len }
    }

    /// Creates a new IPv6 network address from an `Ipv6Addr` and netmask.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ipnet::{Ipv6Net, PrefixLenError};
    ///
    /// let net = Ipv6Net::with_netmask(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), Ipv6Addr::from(0xffff_ff00_0000_0000_0000_0000_0000_0000));
    /// assert!(net.is_ok());
    ///
    /// let bad_prefix_len = Ipv6Net::with_netmask(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0), Ipv6Addr::from(0xffff_ff00_0000_0000_0001_0000_0000_0000));
    /// assert_eq!(bad_prefix_len, Err(PrefixLenError));
    /// ```
    pub fn with_netmask(ip: Ipv6Addr, netmask: Ipv6Addr) -> Result<Ipv6Net, PrefixLenError> {
        let prefix = ipv6_mask_to_prefix(netmask)?;
        Self::new(ip, prefix)
    }

    /// Returns a copy of the network with the address truncated to the
    /// prefix length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv6Net;
    /// #
    /// assert_eq!(
    ///     "fd00::1:2:3:4/16".parse::<Ipv6Net>().unwrap().trunc(),
    ///     "fd00::/16".parse().unwrap()
    /// );
    /// ```
    pub fn trunc(&self) -> Ipv6Net {
        Ipv6Net::new(self.network(), self.prefix_len).unwrap()
    }
    
    /// Returns the address.
    #[inline]
    pub const fn addr(&self) -> Ipv6Addr {
        self.addr
    }

    /// Returns the prefix length.
    #[inline]
    pub const fn prefix_len(&self) -> u8 {
        self.prefix_len
    }
    
    /// Returns the maximum valid prefix length.
    #[inline]
    pub const fn max_prefix_len(&self) -> u8 {
        128
    }

    /// Returns the network mask.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv6Addr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let net: Ipv6Net = "fd00::/24".parse().unwrap();
    /// assert_eq!(Ok(net.netmask()), "ffff:ff00::".parse());
    /// ```
    pub fn netmask(&self) -> Ipv6Addr {
        self.netmask_u128().into()
    }

    fn netmask_u128(&self) -> u128 {
        u128::max_value().checked_shl((128 - self.prefix_len) as u32).unwrap_or(u128::min_value())
    }

    /// Returns the host mask.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv6Addr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let net: Ipv6Net = "fd00::/24".parse().unwrap();
    /// assert_eq!(Ok(net.hostmask()), "::ff:ffff:ffff:ffff:ffff:ffff:ffff".parse());
    /// ```
    pub fn hostmask(&self) -> Ipv6Addr {
        self.hostmask_u128().into()
    }

    fn hostmask_u128(&self) -> u128 {
        u128::max_value().checked_shr(self.prefix_len as u32).unwrap_or(u128::min_value())
    }

    /// Returns the network address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv6Addr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let net: Ipv6Net = "fd00:1234:5678::/24".parse().unwrap();
    /// assert_eq!(Ok(net.network()), "fd00:1200::".parse());
    /// ```
    pub fn network(&self) -> Ipv6Addr {
        (u128::from(self.addr) & self.netmask_u128()).into()
    }
    
    /// Returns the last address.
    ///
    /// Technically there is no such thing as a broadcast address for
    /// IPv6. The name is used for consistency with colloquial usage.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv6Addr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let net: Ipv6Net = "fd00:1234:5678::/24".parse().unwrap();
    /// assert_eq!(Ok(net.broadcast()), "fd00:12ff:ffff:ffff:ffff:ffff:ffff:ffff".parse());
    /// ```
    pub fn broadcast(&self) -> Ipv6Addr {
        (u128::from(self.addr) | self.hostmask_u128()).into()
    }

    /// Returns the `Ipv6Net` that contains this one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::str::FromStr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let n1: Ipv6Net = "fd00:ff00::/24".parse().unwrap();
    /// let n2: Ipv6Net = "fd00:fe00::/23".parse().unwrap();
    /// let n3: Ipv6Net = "fd00:fe00::/0".parse().unwrap();
    ///
    /// assert_eq!(n1.supernet().unwrap(), n2);
    /// assert_eq!(n3.supernet(), None);
    /// ```
    pub fn supernet(&self) -> Option<Ipv6Net> {
        Ipv6Net::new(self.addr, self.prefix_len.wrapping_sub(1)).map(|n| n.trunc()).ok()
    }

    /// Returns `true` if this network and the given network are 
    /// children of the same supernet.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv6Net;
    /// #
    /// let n1: Ipv6Net = "fd00::/18".parse().unwrap();
    /// let n2: Ipv6Net = "fd00:4000::/18".parse().unwrap();
    /// let n3: Ipv6Net = "fd00:8000::/18".parse().unwrap();
    ///
    /// assert!(n1.is_sibling(&n2));
    /// assert!(!n2.is_sibling(&n3));
    /// ```
    pub fn is_sibling(&self, other: &Ipv6Net) -> bool {
        self.prefix_len > 0 &&
        self.prefix_len == other.prefix_len &&
        self.supernet().unwrap().contains(other)
    }
    
    /// Return an `Iterator` over the host addresses in this network.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv6Addr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let net: Ipv6Net = "fd00::/126".parse().unwrap();
    /// assert_eq!(net.hosts().collect::<Vec<Ipv6Addr>>(), vec![
    ///     "fd00::".parse::<Ipv6Addr>().unwrap(),
    ///     "fd00::1".parse().unwrap(),
    ///     "fd00::2".parse().unwrap(),
    ///     "fd00::3".parse().unwrap(),
    /// ]);
    /// ```
    pub fn hosts(&self) -> Ipv6AddrRange {
        Ipv6AddrRange::new(self.network(), self.broadcast())
    }

    /// Returns an `Iterator` over the subnets of this network with the
    /// given prefix length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::{Ipv6Net, PrefixLenError};
    /// #
    /// let net: Ipv6Net = "fd00::/16".parse().unwrap();
    /// assert_eq!(net.subnets(18).unwrap().collect::<Vec<Ipv6Net>>(), vec![
    ///     "fd00::/18".parse::<Ipv6Net>().unwrap(),
    ///     "fd00:4000::/18".parse().unwrap(),
    ///     "fd00:8000::/18".parse().unwrap(),
    ///     "fd00:c000::/18".parse().unwrap(),
    /// ]);
    ///
    /// let net: Ipv6Net = "fd00::/126".parse().unwrap();
    /// assert_eq!(net.subnets(128).unwrap().collect::<Vec<Ipv6Net>>(), vec![
    ///     "fd00::/128".parse::<Ipv6Net>().unwrap(),
    ///     "fd00::1/128".parse().unwrap(),
    ///     "fd00::2/128".parse().unwrap(),
    ///     "fd00::3/128".parse().unwrap(),
    /// ]);
    ///
    /// let net: Ipv6Net = "fd00::/16".parse().unwrap();
    /// assert_eq!(net.subnets(15), Err(PrefixLenError));
    ///
    /// let net: Ipv6Net = "fd00::/16".parse().unwrap();
    /// assert_eq!(net.subnets(129), Err(PrefixLenError));
    /// ```
    pub fn subnets(&self, new_prefix_len: u8) -> Result<Ipv6Subnets, PrefixLenError> {
        if self.prefix_len > new_prefix_len || new_prefix_len > 128 {
            return Err(PrefixLenError);
        }
        
        Ok(Ipv6Subnets::new(
            self.network(),
            self.broadcast(),
            new_prefix_len,
        ))
    }

    /// Test if a network address contains either another network
    /// address or an IP address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::net::Ipv6Addr;
    /// # use ipnet::Ipv6Net;
    /// #
    /// let net: Ipv6Net = "fd00::/16".parse().unwrap();
    /// let net_yes: Ipv6Net = "fd00::/17".parse().unwrap();
    /// let net_no: Ipv6Net = "fd00::/15".parse().unwrap();
    /// let ip_yes: Ipv6Addr = "fd00::1".parse().unwrap();
    /// let ip_no: Ipv6Addr = "fd01::".parse().unwrap();
    ///
    /// assert!(net.contains(&net));
    /// assert!(net.contains(&net_yes));
    /// assert!(!net.contains(&net_no));
    /// assert!(net.contains(&ip_yes));
    /// assert!(!net.contains(&ip_no));
    /// ```
    pub fn contains<T>(&self, other: T) -> bool where Self: Contains<T> {
        Contains::contains(self, other)
    }

    // It is significantly faster to work on u128 that Ipv6Addr.
    fn interval(&self) -> (u128, u128) {
        (
            u128::from(self.network()),
            u128::from(self.broadcast()).saturating_add(1),
        )
    }

    /// Aggregate a `Vec` of `Ipv6Net`s and return the result as a new
    /// `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ipnet::Ipv6Net;
    /// #
    /// let nets = vec![
    ///     "fd00::/18".parse::<Ipv6Net>().unwrap(),
    ///     "fd00:4000::/18".parse().unwrap(),
    ///     "fd00:8000::/18".parse().unwrap(),
    /// ];
    /// assert_eq!(Ipv6Net::aggregate(&nets), vec![
    ///     "fd00::/17".parse::<Ipv6Net>().unwrap(),
    ///     "fd00:8000::/18".parse().unwrap(),
    /// ]);
    /// ```
    pub fn aggregate(networks: &Vec<Ipv6Net>) -> Vec<Ipv6Net> {
        let mut intervals: Vec<(_, _)> = networks.iter().map(|n| n.interval()).collect();
        intervals = merge_intervals(intervals);
        let mut res: Vec<Ipv6Net> = Vec::new();

        for (start, mut end) in intervals {
            if end != core::u128::MAX {
                end = end.saturating_sub(1)
            }
            let iter = Ipv6Subnets::new(start.into(), end.into(), 0);
            res.extend(iter);
        }
        res
    }
}

impl Default for Ipv6Net {
    fn default() -> Self {
        Self {
            addr: Ipv6Addr::from(0),
            prefix_len: 0,
        }
    }
}

impl fmt::Debug for Ipv6Net {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

impl fmt::Display for Ipv6Net {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}/{}", self.addr, self.prefix_len)
    }
}

impl From<Ipv6Addr> for Ipv6Net {
    fn from(addr: Ipv6Addr) -> Ipv6Net {
        Ipv6Net { addr, prefix_len: 128 }
    }
}

/// Provides a method to test if a network address contains either
/// another network address or an IP address.
///
/// # Examples
///
/// ```
/// # use std::net::IpAddr;
/// # use ipnet::IpNet;
/// #
/// let n4_1: IpNet = "10.1.1.0/24".parse().unwrap();
/// let n4_2: IpNet = "10.1.1.0/26".parse().unwrap();
/// let n4_3: IpNet = "10.1.2.0/26".parse().unwrap();
/// let ip4_1: IpAddr = "10.1.1.1".parse().unwrap();
/// let ip4_2: IpAddr = "10.1.2.1".parse().unwrap();
///
/// let n6_1: IpNet = "fd00::/16".parse().unwrap();
/// let n6_2: IpNet = "fd00::/17".parse().unwrap();
/// let n6_3: IpNet = "fd01::/17".parse().unwrap();
/// let ip6_1: IpAddr = "fd00::1".parse().unwrap();
/// let ip6_2: IpAddr = "fd01::1".parse().unwrap();
///
/// assert!(n4_1.contains(&n4_2));
/// assert!(!n4_1.contains(&n4_3));
/// assert!(n4_1.contains(&ip4_1));
/// assert!(!n4_1.contains(&ip4_2));
///
/// assert!(n6_1.contains(&n6_2));
/// assert!(!n6_1.contains(&n6_3));
/// assert!(n6_1.contains(&ip6_1));
/// assert!(!n6_1.contains(&ip6_2));
///
/// assert!(!n4_1.contains(&n6_1) && !n6_1.contains(&n4_1));
/// assert!(!n4_1.contains(&ip6_1) && !n6_1.contains(&ip4_1));
/// ```
pub trait Contains<T> {
    fn contains(&self, other: T) -> bool;
}

impl<'a> Contains<&'a IpNet> for IpNet {
    fn contains(&self, other: &IpNet) -> bool {
        match (*self, *other) {
            (IpNet::V4(ref a), IpNet::V4(ref b)) => a.contains(b),
            (IpNet::V6(ref a), IpNet::V6(ref b)) => a.contains(b),
            _ => false,
        }
    }
}

impl<'a> Contains<&'a IpAddr> for IpNet {
    fn contains(&self, other: &IpAddr) -> bool {
        match (*self, *other) {
            (IpNet::V4(ref a), IpAddr::V4(ref b)) => a.contains(b),
            (IpNet::V6(ref a), IpAddr::V6(ref b)) => a.contains(b),
            _ => false,
        }
    }
}

impl<'a> Contains<&'a Ipv4Net> for Ipv4Net {
    fn contains(&self, other: &'a Ipv4Net) -> bool {
        self.network() <= other.network() && other.broadcast() <= self.broadcast()
    }
}

impl<'a> Contains<&'a Ipv4Addr> for Ipv4Net {
    fn contains(&self, other: &'a Ipv4Addr) -> bool {
        self.network() <= *other && *other <= self.broadcast()
    }
}

impl<'a> Contains<&'a Ipv6Net> for Ipv6Net {
    fn contains(&self, other: &'a Ipv6Net) -> bool {
        self.network() <= other.network() && other.broadcast() <= self.broadcast()
    }
}

impl<'a> Contains<&'a Ipv6Addr> for Ipv6Net {
    fn contains(&self, other: &'a Ipv6Addr) -> bool {
        self.network() <= *other && *other <= self.broadcast()
    }
}

/// An `Iterator` that generates IP network addresses, either IPv4 or
/// IPv6.
///
/// Generates the subnets between the provided `start` and `end` IP
/// addresses inclusive of `end`. Each iteration generates the next
/// network address of the largest valid size it can, while using a
/// prefix length not less than `min_prefix_len`.
///
/// # Examples
///
/// ```
/// # use std::net::{Ipv4Addr, Ipv6Addr};
/// # use std::str::FromStr;
/// # use ipnet::{IpNet, IpSubnets, Ipv4Subnets, Ipv6Subnets};
/// let subnets = IpSubnets::from(Ipv4Subnets::new(
///     "10.0.0.0".parse().unwrap(),
///     "10.0.0.239".parse().unwrap(),
///     26,
/// ));
/// 
/// assert_eq!(subnets.collect::<Vec<IpNet>>(), vec![
///     "10.0.0.0/26".parse().unwrap(),
///     "10.0.0.64/26".parse().unwrap(),
///     "10.0.0.128/26".parse().unwrap(),
///     "10.0.0.192/27".parse().unwrap(),
///     "10.0.0.224/28".parse().unwrap(),
/// ]);
///
/// let subnets = IpSubnets::from(Ipv6Subnets::new(
///     "fd00::".parse().unwrap(),
///     "fd00:ef:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap(),
///     26,
/// ));
/// 
/// assert_eq!(subnets.collect::<Vec<IpNet>>(), vec![
///     "fd00::/26".parse().unwrap(),
///     "fd00:40::/26".parse().unwrap(),
///     "fd00:80::/26".parse().unwrap(),
///     "fd00:c0::/27".parse().unwrap(),
///     "fd00:e0::/28".parse().unwrap(),
/// ]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum IpSubnets {
    V4(Ipv4Subnets),
    V6(Ipv6Subnets),
}

/// An `Iterator` that generates IPv4 network addresses.
///
/// Generates the subnets between the provided `start` and `end` IP
/// addresses inclusive of `end`. Each iteration generates the next
/// network address of the largest valid size it can, while using a
/// prefix length not less than `min_prefix_len`.
///
/// # Examples
///
/// ```
/// # use std::net::Ipv4Addr;
/// # use std::str::FromStr;
/// # use ipnet::{Ipv4Net, Ipv4Subnets};
/// let subnets = Ipv4Subnets::new(
///     "10.0.0.0".parse().unwrap(),
///     "10.0.0.239".parse().unwrap(),
///     26,
/// );
/// 
/// assert_eq!(subnets.collect::<Vec<Ipv4Net>>(), vec![
///     "10.0.0.0/26".parse().unwrap(),
///     "10.0.0.64/26".parse().unwrap(),
///     "10.0.0.128/26".parse().unwrap(),
///     "10.0.0.192/27".parse().unwrap(),
///     "10.0.0.224/28".parse().unwrap(),
/// ]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Ipv4Subnets {
    start: Ipv4Addr,
    end: Ipv4Addr, // end is inclusive
    min_prefix_len: u8,
}

/// An `Iterator` that generates IPv6 network addresses.
///
/// Generates the subnets between the provided `start` and `end` IP
/// addresses inclusive of `end`. Each iteration generates the next
/// network address of the largest valid size it can, while using a
/// prefix length not less than `min_prefix_len`.
///
/// # Examples
///
/// ```
/// # use std::net::Ipv6Addr;
/// # use std::str::FromStr;
/// # use ipnet::{Ipv6Net, Ipv6Subnets};
/// let subnets = Ipv6Subnets::new(
///     "fd00::".parse().unwrap(),
///     "fd00:ef:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap(),
///     26,
/// );
/// 
/// assert_eq!(subnets.collect::<Vec<Ipv6Net>>(), vec![
///     "fd00::/26".parse().unwrap(),
///     "fd00:40::/26".parse().unwrap(),
///     "fd00:80::/26".parse().unwrap(),
///     "fd00:c0::/27".parse().unwrap(),
///     "fd00:e0::/28".parse().unwrap(),
/// ]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Ipv6Subnets {
    start: Ipv6Addr,
    end: Ipv6Addr, // end is inclusive
    min_prefix_len: u8,
}

impl Ipv4Subnets {
    pub fn new(start: Ipv4Addr, end: Ipv4Addr, min_prefix_len: u8) -> Self {
        Ipv4Subnets {
            start: start,
            end: end,
            min_prefix_len: min_prefix_len,
        }
    }
}

impl Ipv6Subnets {
    pub fn new(start: Ipv6Addr, end: Ipv6Addr, min_prefix_len: u8) -> Self {
        Ipv6Subnets {
            start: start,
            end: end,
            min_prefix_len: min_prefix_len,
        }
    }
}

impl From<Ipv4Subnets> for IpSubnets {
    fn from(i: Ipv4Subnets) -> IpSubnets {
        IpSubnets::V4(i)
    }
}

impl From<Ipv6Subnets> for IpSubnets {
    fn from(i: Ipv6Subnets) -> IpSubnets {
        IpSubnets::V6(i)
    }
}

impl Iterator for IpSubnets {
    type Item = IpNet;

    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            IpSubnets::V4(ref mut a) => a.next().map(IpNet::V4),
            IpSubnets::V6(ref mut a) => a.next().map(IpNet::V6),
        }
    }
}

fn next_ipv4_subnet(start: Ipv4Addr, end: Ipv4Addr, min_prefix_len: u8) -> Ipv4Net {
    let range = end.saturating_sub(start).saturating_add(1);
    if range == core::u32::MAX && min_prefix_len == 0 {
        Ipv4Net::new(start, min_prefix_len).unwrap()
    }
    else {
        let range_bits = 32u32.saturating_sub(range.leading_zeros()).saturating_sub(1);
        let start_tz = u32::from(start).trailing_zeros();
        let new_prefix_len = 32 - min(range_bits, start_tz);
        let next_prefix_len = max(new_prefix_len as u8, min_prefix_len);
        Ipv4Net::new(start, next_prefix_len).unwrap()
    }
}

fn next_ipv6_subnet(start: Ipv6Addr, end: Ipv6Addr, min_prefix_len: u8) -> Ipv6Net {
    let range = end.saturating_sub(start).saturating_add(1);
    if range == core::u128::MAX && min_prefix_len == 0 {
        Ipv6Net::new(start, min_prefix_len).unwrap()
    }
    else {
        let range = end.saturating_sub(start).saturating_add(1);
        let range_bits = 128u32.saturating_sub(range.leading_zeros()).saturating_sub(1);
        let start_tz = u128::from(start).trailing_zeros();
        let new_prefix_len = 128 - min(range_bits, start_tz);
        let next_prefix_len = max(new_prefix_len as u8, min_prefix_len);
        Ipv6Net::new(start, next_prefix_len).unwrap()
    }
}

impl Iterator for Ipv4Subnets {
    type Item = Ipv4Net;

    fn next(&mut self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let next = next_ipv4_subnet(self.start, self.end, self.min_prefix_len);
                self.start = next.broadcast().saturating_add(1);

                // Stop the iterator if we saturated self.start. This
                // check worsens performance slightly but overall this
                // approach of operating on Ipv4Addr types is faster
                // than what we were doing before using Ipv4Net.
                if self.start == next.broadcast() {
                    self.end.replace_zero();
                }
                Some(next)
            },
            Some(Equal) => {
                let next = next_ipv4_subnet(self.start, self.end, self.min_prefix_len);
                self.start = next.broadcast().saturating_add(1);
                self.end.replace_zero();
                Some(next)
            },
            _ => None,
        }
    }
}

impl Iterator for Ipv6Subnets {
    type Item = Ipv6Net;

    fn next(&mut self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let next = next_ipv6_subnet(self.start, self.end, self.min_prefix_len);
                self.start = next.broadcast().saturating_add(1);

                // Stop the iterator if we saturated self.start. This
                // check worsens performance slightly but overall this
                // approach of operating on Ipv6Addr types is faster
                // than what we were doing before using Ipv6Net.
                if self.start == next.broadcast() {
                    self.end.replace_zero();
                }
                Some(next)
            },
            Some(Equal) => {
                let next = next_ipv6_subnet(self.start, self.end, self.min_prefix_len);
                self.start = next.broadcast().saturating_add(1);
                self.end.replace_zero();
                Some(next)
            },
            _ => None,
        }
    }
}

impl FusedIterator for IpSubnets {}
impl FusedIterator for Ipv4Subnets {}
impl FusedIterator for Ipv6Subnets {}

// Generic function for merging a vector of intervals.
fn merge_intervals<T: Copy + Ord>(mut intervals: Vec<(T, T)>) -> Vec<(T, T)> {
    if intervals.len() == 0 {
        return intervals;
    }

    intervals.sort();
    let mut res: Vec<(T, T)> = Vec::new();
    let (mut start, mut end) = intervals[0];
    
    let mut i = 1;
    let len = intervals.len();
    while i < len {
        let (next_start, next_end) = intervals[i];
        if end >= next_start {
            start = min(start, next_start);
            end = max(end, next_end);
        }
        else {
            res.push((start, end));
            start = next_start;
            end = next_end;
        }
        i += 1;
    }

    res.push((start, end));
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! make_ipnet_vec {
        ($($x:expr),*) => ( vec![$($x.parse::<IpNet>().unwrap(),)*] );
        ($($x:expr,)*) => ( make_ipnet_vec![$($x),*] );
    }

    #[test]
    fn test_make_ipnet_vec() {
        assert_eq!(
            make_ipnet_vec![
                "10.1.1.1/32", "10.2.2.2/24", "10.3.3.3/16",
                "fd00::1/128", "fd00::2/127", "fd00::3/126",
            ],
            vec![
                "10.1.1.1/32".parse().unwrap(),
                "10.2.2.2/24".parse().unwrap(),
                "10.3.3.3/16".parse().unwrap(),
                "fd00::1/128".parse().unwrap(),
                "fd00::2/127".parse().unwrap(),
                "fd00::3/126".parse().unwrap(),
            ]
        );
    }

    #[test]
    fn test_merge_intervals() {
        let v = vec![
            (0, 1), (1, 2), (2, 3),
            (11, 12), (13, 14), (10, 15), (11, 13),
            (20, 25), (24, 29),
        ];

        let v_ok = vec![
            (0, 3),
            (10, 15),
            (20, 29),
        ];

        let vv = vec![
            ([0, 1], [0, 2]), ([0, 2], [0, 3]), ([0, 0], [0, 1]),
            ([10, 15], [11, 0]), ([10, 0], [10, 16]),
        ];

        let vv_ok = vec![
            ([0, 0], [0, 3]),
            ([10, 0], [11, 0]),
        ];

        assert_eq!(merge_intervals(v), v_ok);
        assert_eq!(merge_intervals(vv), vv_ok);
    }

    macro_rules! make_ipv4_subnets_test {
        ($name:ident, $start:expr, $end:expr, $min_prefix_len:expr, $($x:expr),*) => (
            #[test]
            fn $name() {
                let subnets = IpSubnets::from(Ipv4Subnets::new(
                    $start.parse().unwrap(),
                    $end.parse().unwrap(),
                    $min_prefix_len,
                ));
                let results = make_ipnet_vec![$($x),*];
                assert_eq!(subnets.collect::<Vec<IpNet>>(), results);
            }
        );
        ($name:ident, $start:expr, $end:expr, $min_prefix_len:expr, $($x:expr,)*) => (
            make_ipv4_subnets_test!($name, $start, $end, $min_prefix_len, $($x),*);
        );
    }

    macro_rules! make_ipv6_subnets_test {
        ($name:ident, $start:expr, $end:expr, $min_prefix_len:expr, $($x:expr),*) => (
            #[test]
            fn $name() {
                let subnets = IpSubnets::from(Ipv6Subnets::new(
                    $start.parse().unwrap(),
                    $end.parse().unwrap(),
                    $min_prefix_len,
                ));
                let results = make_ipnet_vec![$($x),*];
                assert_eq!(subnets.collect::<Vec<IpNet>>(), results);
            }
        );
        ($name:ident, $start:expr, $end:expr, $min_prefix_len:expr, $($x:expr,)*) => (
            make_ipv6_subnets_test!($name, $start, $end, $min_prefix_len, $($x),*);
        );
    }

    make_ipv4_subnets_test!(
        test_ipv4_subnets_zero_zero,
        "0.0.0.0", "0.0.0.0", 0,
        "0.0.0.0/32",
    );

    make_ipv4_subnets_test!(
        test_ipv4_subnets_zero_max,
        "0.0.0.0", "255.255.255.255", 0,
        "0.0.0.0/0",
    );

    make_ipv4_subnets_test!(
        test_ipv4_subnets_max_max,
        "255.255.255.255", "255.255.255.255", 0,
        "255.255.255.255/32",
    );
    
    make_ipv4_subnets_test!(
        test_ipv4_subnets_none,
        "0.0.0.1", "0.0.0.0", 0,
    );
    
    make_ipv4_subnets_test!(
        test_ipv4_subnets_one,
        "0.0.0.0", "0.0.0.1", 0,
        "0.0.0.0/31",
    );

    make_ipv4_subnets_test!(
        test_ipv4_subnets_two,
        "0.0.0.0", "0.0.0.2", 0,
        "0.0.0.0/31",
        "0.0.0.2/32",
    );
    
    make_ipv4_subnets_test!(
        test_ipv4_subnets_taper,
        "0.0.0.0", "0.0.0.10", 30,
        "0.0.0.0/30",
        "0.0.0.4/30",
        "0.0.0.8/31",
        "0.0.0.10/32",
    );
    
    make_ipv6_subnets_test!(
        test_ipv6_subnets_zero_zero,
        "::", "::", 0,
        "::/128",
    );

    make_ipv6_subnets_test!(
        test_ipv6_subnets_zero_max,
        "::", "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", 0,
        "::/0",
    );

    make_ipv6_subnets_test!(
        test_ipv6_subnets_max_max,
        "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", 0,
        "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff/128",
    );
    
    make_ipv6_subnets_test!(
        test_ipv6_subnets_none,
        "::1", "::", 0,
    );
    
    make_ipv6_subnets_test!(
        test_ipv6_subnets_one,
        "::", "::1", 0,
        "::/127",
    );

    make_ipv6_subnets_test!(
        test_ipv6_subnets_two,
        "::", "::2", 0,
        "::/127",
        "::2/128",
    );

    make_ipv6_subnets_test!(
        test_ipv6_subnets_taper,
        "::", "::a", 126,
        "::/126",
        "::4/126",
        "::8/127",
        "::a/128",
    );

    #[test]
    fn test_aggregate() {
        let ip_nets = make_ipnet_vec![
            "10.0.0.0/24", "10.0.1.0/24", "10.0.1.1/24", "10.0.1.2/24",
            "10.0.2.0/24",
            "10.1.0.0/24", "10.1.1.0/24",
            "192.168.0.0/24", "192.168.1.0/24", "192.168.2.0/24", "192.168.3.0/24",
            "fd00::/32", "fd00:1::/32",
            "fd00:2::/32",
        ];

        let ip_aggs = make_ipnet_vec![
            "10.0.0.0/23",
            "10.0.2.0/24",
            "10.1.0.0/23",
            "192.168.0.0/22",
            "fd00::/31",
            "fd00:2::/32",
        ];

        let ipv4_nets: Vec<Ipv4Net> = ip_nets.iter().filter_map(|p| if let IpNet::V4(x) = *p { Some(x) } else { None }).collect();
        let ipv4_aggs: Vec<Ipv4Net> = ip_aggs.iter().filter_map(|p| if let IpNet::V4(x) = *p { Some(x) } else { None }).collect();
        let ipv6_nets: Vec<Ipv6Net> = ip_nets.iter().filter_map(|p| if let IpNet::V6(x) = *p { Some(x) } else { None }).collect();
        let ipv6_aggs: Vec<Ipv6Net> = ip_aggs.iter().filter_map(|p| if let IpNet::V6(x) = *p { Some(x) } else { None }).collect();

        assert_eq!(IpNet::aggregate(&ip_nets), ip_aggs);
        assert_eq!(Ipv4Net::aggregate(&ipv4_nets), ipv4_aggs);
        assert_eq!(Ipv6Net::aggregate(&ipv6_nets), ipv6_aggs);
    }
    
    #[test]
    fn test_aggregate_issue44() {
        let nets: Vec<Ipv4Net> = vec!["128.0.0.0/1".parse().unwrap()];
        assert_eq!(Ipv4Net::aggregate(&nets), nets);

        let nets: Vec<Ipv4Net> = vec!["0.0.0.0/1".parse().unwrap(), "128.0.0.0/1".parse().unwrap()];
        assert_eq!(Ipv4Net::aggregate(&nets), vec!["0.0.0.0/0".parse().unwrap()]);

        let nets: Vec<Ipv6Net> = vec!["8000::/1".parse().unwrap()];
        assert_eq!(Ipv6Net::aggregate(&nets), nets);

        let nets: Vec<Ipv6Net> = vec!["::/1".parse().unwrap(), "8000::/1".parse().unwrap()];
        assert_eq!(Ipv6Net::aggregate(&nets), vec!["::/0".parse().unwrap()]);
    }

    #[test]
    fn ipnet_default() {
        let ipnet: IpNet = "0.0.0.0/0".parse().unwrap();
        assert_eq!(ipnet, IpNet::default());
    }

    #[test]
    fn ipv4net_default() {
        let ipnet: Ipv4Net = "0.0.0.0/0".parse().unwrap();
        assert_eq!(ipnet, Ipv4Net::default());
    }

    #[test]
    fn ipv6net_default() {
        let ipnet: Ipv6Net = "::/0".parse().unwrap();
        assert_eq!(ipnet, Ipv6Net::default());
    }

    #[test]
    fn new_assert() {
        const _: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(0, 0, 0, 0), 0);
        const _: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(0, 0, 0, 0), 32);
        const _: Ipv6Net = Ipv6Net::new_assert(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0);
        const _: Ipv6Net = Ipv6Net::new_assert(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 128);

        let _ = Ipv4Net::new_assert(Ipv4Addr::new(0, 0, 0, 0), 0);
        let _ = Ipv4Net::new_assert(Ipv4Addr::new(0, 0, 0, 0), 32);
        let _ = Ipv6Net::new_assert(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0);
        let _ = Ipv6Net::new_assert(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 128);
    }

    #[test]
    #[should_panic]
    fn ipv4net_new_assert_panics() {
        let _ = Ipv4Net::new_assert(Ipv4Addr::new(0, 0, 0, 0), 33);
    }

    #[test]
    #[should_panic]
    fn ipv6net_new_assert_panics() {
        let _ = Ipv6Net::new_assert(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 129);
    }
}
