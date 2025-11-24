//! Extensions to the standard IP address types for common operations.
//!
//! The [`IpAdd`], [`IpSub`], [`IpBitAnd`], [`IpBitOr`] traits extend
//! the `Ipv4Addr` and `Ipv6Addr` types with methods to perform these
//! operations.

use core::cmp::Ordering::{Less, Equal};
use core::iter::{FusedIterator, DoubleEndedIterator};
use core::mem;
#[cfg(not(feature = "std"))]
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
#[cfg(feature = "std")]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Provides a `saturating_add()` method for `Ipv4Addr` and `Ipv6Addr`.
///
/// Adding an integer to an IP address returns the modified IP address.
/// A `u32` may added to an IPv4 address and a `u128` may be added to
/// an IPv6 address.
///
/// # Examples
///
/// ```
/// # #[cfg(not(feature = "std"))]
/// # use core::net::{Ipv4Addr, Ipv6Addr};
/// # #[cfg(feature = "std")]
/// use std::net::{Ipv4Addr, Ipv6Addr};
/// use ipnet::IpAdd;
///
/// let ip0: Ipv4Addr = "192.168.0.0".parse().unwrap();
/// let ip1: Ipv4Addr = "192.168.0.5".parse().unwrap();
/// let ip2: Ipv4Addr = "255.255.255.254".parse().unwrap();
/// let max: Ipv4Addr = "255.255.255.255".parse().unwrap();
///
/// assert_eq!(ip0.saturating_add(5), ip1);
/// assert_eq!(ip2.saturating_add(1), max);
/// assert_eq!(ip2.saturating_add(5), max);
///
/// let ip0: Ipv6Addr = "fd00::".parse().unwrap();
/// let ip1: Ipv6Addr = "fd00::5".parse().unwrap();
/// let ip2: Ipv6Addr = "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe".parse().unwrap();
/// let max: Ipv6Addr = "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap();
///
/// assert_eq!(ip0.saturating_add(5), ip1);
/// assert_eq!(ip2.saturating_add(1), max);
/// assert_eq!(ip2.saturating_add(5), max);
/// ```
pub trait IpAdd<RHS = Self> {
    type Output;
    fn saturating_add(self, rhs: RHS) -> Self::Output;
}

/// Provides a `saturating_sub()` method for `Ipv4Addr` and `Ipv6Addr`.
///
/// Subtracting an integer from an IP address returns the modified IP
/// address. A `u32` may be subtracted from an IPv4 address and a `u128`
/// may be subtracted from an IPv6 address.
///
/// Subtracting an IP address from another IP address of the same type
/// returns an integer of the appropriate width. A `u32` for IPv4 and a
/// `u128` for IPv6. Subtracting IP addresses is useful for getting
/// the range between two IP addresses.
///
/// # Examples
///
/// ```
/// # #[cfg(not(feature = "std"))]
/// # use core::net::{Ipv4Addr, Ipv6Addr};
/// # #[cfg(feature = "std")]
/// use std::net::{Ipv4Addr, Ipv6Addr};
/// use ipnet::IpSub;
///
/// let min: Ipv4Addr = "0.0.0.0".parse().unwrap();
/// let ip1: Ipv4Addr = "192.168.1.5".parse().unwrap();
/// let ip2: Ipv4Addr = "192.168.1.100".parse().unwrap();
///
/// assert_eq!(min.saturating_sub(ip1), 0);
/// assert_eq!(ip2.saturating_sub(ip1), 95);
/// assert_eq!(min.saturating_sub(5), min);
/// assert_eq!(ip2.saturating_sub(95), ip1);
/// 
/// let min: Ipv6Addr = "::".parse().unwrap();
/// let ip1: Ipv6Addr = "fd00::5".parse().unwrap();
/// let ip2: Ipv6Addr = "fd00::64".parse().unwrap();
///
/// assert_eq!(min.saturating_sub(ip1), 0);
/// assert_eq!(ip2.saturating_sub(ip1), 95);
/// assert_eq!(min.saturating_sub(5u128), min);
/// assert_eq!(ip2.saturating_sub(95u128), ip1);
/// ```
pub trait IpSub<RHS = Self> {
    type Output;
    fn saturating_sub(self, rhs: RHS) -> Self::Output;
}

/// Provides a `bitand()` method for `Ipv4Addr` and `Ipv6Addr`.
///
/// # Examples
///
/// ```
/// # #[cfg(not(feature = "std"))]
/// # use core::net::{Ipv4Addr, Ipv6Addr};
/// # #[cfg(feature = "std")]
/// use std::net::{Ipv4Addr, Ipv6Addr};
/// use ipnet::IpBitAnd;
///
/// let ip: Ipv4Addr = "192.168.1.1".parse().unwrap();
/// let mask: Ipv4Addr = "255.255.0.0".parse().unwrap();
/// let res: Ipv4Addr = "192.168.0.0".parse().unwrap();
///
/// assert_eq!(ip.bitand(mask), res);
/// assert_eq!(ip.bitand(0xffff0000), res);
/// 
/// let ip: Ipv6Addr = "fd00:1234::1".parse().unwrap();
/// let mask: Ipv6Addr = "ffff::".parse().unwrap();
/// let res: Ipv6Addr = "fd00::".parse().unwrap();
///
/// assert_eq!(ip.bitand(mask), res);
/// assert_eq!(ip.bitand(0xffff_0000_0000_0000_0000_0000_0000_0000u128), res);
/// ```
pub trait IpBitAnd<RHS = Self> {
    type Output;
    fn bitand(self, rhs: RHS) -> Self::Output;
}

/// Provides a `bitor()` method for `Ipv4Addr` and `Ipv6Addr`.
///
/// # Examples
///
/// ```
/// # #[cfg(not(feature = "std"))]
/// # use core::net::{Ipv4Addr, Ipv6Addr};
/// # #[cfg(feature = "std")]
/// use std::net::{Ipv4Addr, Ipv6Addr};
/// use ipnet::IpBitOr;
///
/// let ip: Ipv4Addr = "10.1.1.1".parse().unwrap();
/// let mask: Ipv4Addr = "0.0.0.255".parse().unwrap();
/// let res: Ipv4Addr = "10.1.1.255".parse().unwrap();
///
/// assert_eq!(ip.bitor(mask), res);
/// assert_eq!(ip.bitor(0x000000ff), res);
/// 
/// let ip: Ipv6Addr = "fd00::1".parse().unwrap();
/// let mask: Ipv6Addr = "::ffff:ffff".parse().unwrap();
/// let res: Ipv6Addr = "fd00::ffff:ffff".parse().unwrap();
///
/// assert_eq!(ip.bitor(mask), res);
/// assert_eq!(ip.bitor(u128::from(0xffffffffu32)), res);
/// ```
pub trait IpBitOr<RHS = Self> {
    type Output;
    fn bitor(self, rhs: RHS) -> Self::Output;
}

macro_rules! ip_add_impl {
    ($lhs:ty, $rhs:ty, $output:ty, $inner:ty) => (
        impl IpAdd<$rhs> for $lhs {
            type Output = $output;

            fn saturating_add(self, rhs: $rhs) -> $output {
                let lhs: $inner = self.into();
                let rhs: $inner = rhs.into();
                (lhs.saturating_add(rhs.into())).into()
            }
        }
    )
}

macro_rules! ip_sub_impl {
    ($lhs:ty, $rhs:ty, $output:ty, $inner:ty) => (
        impl IpSub<$rhs> for $lhs {
            type Output = $output;

            fn saturating_sub(self, rhs: $rhs) -> $output {
                let lhs: $inner = self.into();
                let rhs: $inner = rhs.into();
                (lhs.saturating_sub(rhs.into())).into()
            }
        }
    )
}

ip_add_impl!(Ipv4Addr, u32, Ipv4Addr, u32);
ip_add_impl!(Ipv6Addr, u128, Ipv6Addr, u128);

ip_sub_impl!(Ipv4Addr, Ipv4Addr, u32, u32);
ip_sub_impl!(Ipv4Addr, u32, Ipv4Addr, u32);
ip_sub_impl!(Ipv6Addr, Ipv6Addr, u128, u128);
ip_sub_impl!(Ipv6Addr, u128, Ipv6Addr, u128);

macro_rules! ip_bitops_impl {
    ($(($lhs:ty, $rhs:ty, $t:ty),)*) => {
    $(
        impl IpBitAnd<$rhs> for $lhs {
            type Output = $lhs;

            fn bitand(self, rhs: $rhs) -> $lhs {
                let lhs: $t = self.into();
                let rhs: $t = rhs.into();
                (lhs & rhs).into()
            }
        }

        impl IpBitOr<$rhs> for $lhs {
            type Output = $lhs;

            fn bitor(self, rhs: $rhs) -> $lhs {
                let lhs: $t = self.into();
                let rhs: $t = rhs.into();
                (lhs | rhs).into()
            }
        }
    )*
    }
}

ip_bitops_impl! {
    (Ipv4Addr, Ipv4Addr, u32),
    (Ipv4Addr, u32, u32),
    (Ipv6Addr, Ipv6Addr, u128),
    (Ipv6Addr, u128, u128),
}

// A barebones copy of the current unstable Step trait used by the
// IpAddrRange, Ipv4AddrRange, and Ipv6AddrRange types below, and the
// Subnets types in ipnet.
pub trait IpStep {
    fn replace_one(&mut self) -> Self;
    fn replace_zero(&mut self) -> Self;
    fn add_one(&self) -> Self;
    fn sub_one(&self) -> Self;
}

impl IpStep for Ipv4Addr {
    fn replace_one(&mut self) -> Self {
        mem::replace(self, Ipv4Addr::new(0, 0, 0, 1))
    }
    fn replace_zero(&mut self) -> Self {
        mem::replace(self, Ipv4Addr::new(0, 0, 0, 0))
    }
    fn add_one(&self) -> Self {
        self.saturating_add(1)
    }
    fn sub_one(&self) -> Self {
        self.saturating_sub(1)
    }
}

impl IpStep for Ipv6Addr {
    fn replace_one(&mut self) -> Self {
        mem::replace(self, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))
    }
    fn replace_zero(&mut self) -> Self {
        mem::replace(self, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0))
    }
    fn add_one(&self) -> Self {
        self.saturating_add(1)
    }
    fn sub_one(&self) -> Self {
        self.saturating_sub(1)
    }
}

/// An `Iterator` over a range of IP addresses, either IPv4 or IPv6.
///
/// # Examples
///
/// ```
/// use std::net::IpAddr;
/// use ipnet::{IpAddrRange, Ipv4AddrRange, Ipv6AddrRange};
///
/// let hosts = IpAddrRange::from(Ipv4AddrRange::new(
///     "10.0.0.0".parse().unwrap(),
///     "10.0.0.3".parse().unwrap(),
/// ));
///
/// assert_eq!(hosts.collect::<Vec<IpAddr>>(), vec![
///     "10.0.0.0".parse::<IpAddr>().unwrap(),
///     "10.0.0.1".parse().unwrap(),
///     "10.0.0.2".parse().unwrap(),
///     "10.0.0.3".parse().unwrap(),
/// ]);
///
/// let hosts = IpAddrRange::from(Ipv6AddrRange::new(
///     "fd00::".parse().unwrap(),
///     "fd00::3".parse().unwrap(),
/// ));
///
/// assert_eq!(hosts.collect::<Vec<IpAddr>>(), vec![
///     "fd00::0".parse::<IpAddr>().unwrap(),
///     "fd00::1".parse().unwrap(),
///     "fd00::2".parse().unwrap(),
///     "fd00::3".parse().unwrap(),
/// ]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum IpAddrRange {
    V4(Ipv4AddrRange),
    V6(Ipv6AddrRange),
}

/// An `Iterator` over a range of IPv4 addresses.
///
/// # Examples
///
/// ```
/// # #[cfg(not(feature = "std"))]
/// # use core::net::Ipv4Addr;
/// # #[cfg(feature = "std")]
/// use std::net::Ipv4Addr;
/// use ipnet::Ipv4AddrRange;
///
/// let hosts = Ipv4AddrRange::new(
///     "10.0.0.0".parse().unwrap(),
///     "10.0.0.3".parse().unwrap(),
/// );
///
/// assert_eq!(hosts.collect::<Vec<Ipv4Addr>>(), vec![
///     "10.0.0.0".parse::<Ipv4Addr>().unwrap(),
///     "10.0.0.1".parse().unwrap(),
///     "10.0.0.2".parse().unwrap(),
///     "10.0.0.3".parse().unwrap(),
/// ]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Ipv4AddrRange {
    start: Ipv4Addr,
    end: Ipv4Addr,
}

/// An `Iterator` over a range of IPv6 addresses.
///
/// # Examples
///
/// ``` 
/// # #[cfg(not(feature = "std"))]
/// # use core::net::Ipv6Addr;
/// # #[cfg(feature = "std")]
/// use std::net::Ipv6Addr;
/// use ipnet::Ipv6AddrRange;
///
/// let hosts = Ipv6AddrRange::new(
///     "fd00::".parse().unwrap(),
///     "fd00::3".parse().unwrap(),
/// );
///
/// assert_eq!(hosts.collect::<Vec<Ipv6Addr>>(), vec![
///     "fd00::".parse::<Ipv6Addr>().unwrap(),
///     "fd00::1".parse().unwrap(),
///     "fd00::2".parse().unwrap(),
///     "fd00::3".parse().unwrap(),
/// ]);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Ipv6AddrRange {
    start: Ipv6Addr,
    end: Ipv6Addr,
}

impl From<Ipv4AddrRange> for IpAddrRange {
    fn from(i: Ipv4AddrRange) -> IpAddrRange {
        IpAddrRange::V4(i)
    }
}

impl From<Ipv6AddrRange> for IpAddrRange {
    fn from(i: Ipv6AddrRange) -> IpAddrRange {
        IpAddrRange::V6(i)
    }
}

impl Ipv4AddrRange {
    pub fn new(start: Ipv4Addr, end: Ipv4Addr) -> Self {
        Ipv4AddrRange {
            start: start,
            end: end,
        }
    }
    /// Counts the number of Ipv4Addr in this range.
    /// This method will never overflow or panic.
    fn count_u64(&self) -> u64 {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let count: u32 = self.end.saturating_sub(self.start);
                let count = count as u64 + 1; // Never overflows
                count
            },
            Some(Equal) => 1,
            _ => 0,
        }
    }
}

impl Ipv6AddrRange {
    pub fn new(start: Ipv6Addr, end: Ipv6Addr) -> Self {
        Ipv6AddrRange {
            start: start,
            end: end,
        }
    }
    /// Counts the number of Ipv6Addr in this range.
    /// This method may overflow or panic if start
    /// is 0 and end is u128::MAX
    fn count_u128(&self) -> u128 {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let count = self.end.saturating_sub(self.start);
                // May overflow or panic
                count + 1
            },
            Some(Equal) => 1,
            _ => 0,
        }
    }
    /// True only if count_u128 does not overflow
    fn can_count_u128(&self) -> bool {
        self.start != Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)
        || self.end != Ipv6Addr::new(0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff)
    }
}

impl Iterator for IpAddrRange {
    type Item = IpAddr;

    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            IpAddrRange::V4(ref mut a) => a.next().map(IpAddr::V4),
            IpAddrRange::V6(ref mut a) => a.next().map(IpAddr::V6),
        }
    }

    fn count(self) -> usize {
        match self {
            IpAddrRange::V4(a) => a.count(),
            IpAddrRange::V6(a) => a.count(),
        }
    }

    fn last(self) -> Option<Self::Item> {
        match self {
            IpAddrRange::V4(a) => a.last().map(IpAddr::V4),
            IpAddrRange::V6(a) => a.last().map(IpAddr::V6),
        }
    }

    fn max(self) -> Option<Self::Item> {
        match self {
            IpAddrRange::V4(a) => Iterator::max(a).map(IpAddr::V4),
            IpAddrRange::V6(a) => Iterator::max(a).map(IpAddr::V6),
        }
    }

    fn min(self) -> Option<Self::Item> {
        match self {
            IpAddrRange::V4(a) => Iterator::min(a).map(IpAddr::V4),
            IpAddrRange::V6(a) => Iterator::min(a).map(IpAddr::V6),
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match *self {
            IpAddrRange::V4(ref mut a) => a.nth(n).map(IpAddr::V4),
            IpAddrRange::V6(ref mut a) => a.nth(n).map(IpAddr::V6),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match *self {
            IpAddrRange::V4(ref a) => a.size_hint(),
            IpAddrRange::V6(ref a) => a.size_hint(),
        }
    }
}

impl Iterator for Ipv4AddrRange {
    type Item = Ipv4Addr;

    fn next(&mut self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let next = self.start.add_one();
                Some(mem::replace(&mut self.start, next))
            },
            Some(Equal) => {
                self.end.replace_zero();
                Some(self.start.replace_one())
            },
            _ => None,
        }
    }

    #[allow(arithmetic_overflow)]
    fn count(self) -> usize {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                // Adding one here might overflow u32.
                // Instead, wait until after converted to usize
                let count: u32 = self.end.saturating_sub(self.start);

                // usize might only be 16 bits,
                // so need to explicitly check for overflow.
                // 'usize::MAX as u32' is okay here - if usize is 64 bits,
                // value truncates to u32::MAX
                if count <= core::usize::MAX as u32 {
                    count as usize + 1
                // count overflows usize
                } else {
                    // emulate standard overflow/panic behavior
                    core::usize::MAX + 2 + count as usize
                }
            },
            Some(Equal) => 1,
            _ => 0
        }
    }

    fn last(self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) | Some(Equal) => Some(self.end),
            _ => None,
        }
    }

    fn max(self) -> Option<Self::Item> {
        self.last()
    }

    fn min(self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) | Some(Equal) => Some(self.start),
            _ => None
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let n = n as u64;
        let count = self.count_u64();
        if n >= count {
            self.end.replace_zero();
            self.start.replace_one();
            None
        } else if n == count - 1 {
            self.start.replace_one();
            Some(self.end.replace_zero())
        } else {
            let nth = self.start.saturating_add(n as u32);
            self.start = nth.add_one();
            Some(nth)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.count_u64();
        if count > core::usize::MAX as u64 {
            (core::usize::MAX, None)
        } else {
            let count = count as usize;
            (count, Some(count))
        }
    }
}

impl Iterator for Ipv6AddrRange {
    type Item = Ipv6Addr;

    fn next(&mut self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let next = self.start.add_one();
                Some(mem::replace(&mut self.start, next))
            },
            Some(Equal) => {
                self.end.replace_zero();
                Some(self.start.replace_one())
            },
            _ => None,
        }
    }

    #[allow(arithmetic_overflow)]
    fn count(self) -> usize {
        let count = self.count_u128();
        // count fits in usize
        if count <= core::usize::MAX as u128 {
            count as usize
        // count does not fit in usize
        } else {
            // emulate standard overflow/panic behavior
            core::usize::MAX + 1 + count as usize
        }
    }

    fn last(self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) | Some(Equal) => Some(self.end),
            _ => None,
        }
    }

    fn max(self) -> Option<Self::Item> {
        self.last()
    }

    fn min(self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) | Some(Equal) => Some(self.start),
            _ => None
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let n = n as u128;
        if self.can_count_u128() {
            let count = self.count_u128();
            if n >= count {
                self.end.replace_zero();
                self.start.replace_one();
                None
            } else if n == count - 1 {
                self.start.replace_one();
                Some(self.end.replace_zero())
            } else {
                let nth = self.start.saturating_add(n);
                self.start = nth.add_one();
                Some(nth)
            }
        // count overflows u128; n is 64-bits at most.
        // therefore, n can never exceed count
        } else {
            let nth = self.start.saturating_add(n);
            self.start = nth.add_one();
            Some(nth)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.can_count_u128() {
            let count = self.count_u128();
            if count > core::usize::MAX as u128 {
                (core::usize::MAX, None)
            } else {
                let count = count as usize;
                (count, Some(count))
            }
        } else {
            (core::usize::MAX, None)
        }
    }
}

impl DoubleEndedIterator for IpAddrRange {
    fn next_back(&mut self) -> Option<Self::Item> {
        match *self {
            IpAddrRange::V4(ref mut a) => a.next_back().map(IpAddr::V4),
            IpAddrRange::V6(ref mut a) => a.next_back().map(IpAddr::V6),
        }
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        match *self {
            IpAddrRange::V4(ref mut a) => a.nth_back(n).map(IpAddr::V4),
            IpAddrRange::V6(ref mut a) => a.nth_back(n).map(IpAddr::V6),
        }
    }
}

impl DoubleEndedIterator for Ipv4AddrRange {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let next_back = self.end.sub_one();
                Some(mem::replace(&mut self.end, next_back))
            },
            Some(Equal) => {
                self.end.replace_zero();
                Some(self.start.replace_one())
            },
            _ => None
        }
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let n = n as u64;
        let count = self.count_u64();
        if n >= count {
            self.end.replace_zero();
            self.start.replace_one();
            None
        } else if n == count - 1 {
            self.end.replace_zero();
            Some(self.start.replace_one())
        } else {
            let nth_back = self.end.saturating_sub(n as u32);
            self.end = nth_back.sub_one();
            Some(nth_back)
        }
    }
}

impl DoubleEndedIterator for Ipv6AddrRange {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.start.partial_cmp(&self.end) {
            Some(Less) => {
                let next_back = self.end.sub_one();
                Some(mem::replace(&mut self.end, next_back))
            },
            Some(Equal) => {
                self.end.replace_zero();
                Some(self.start.replace_one())
            },
            _ => None
        }
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let n = n as u128;
        if self.can_count_u128() {
            let count = self.count_u128();
            if n >= count {
                self.end.replace_zero();
                self.start.replace_one();
                None
            }
            else if n == count - 1 {
                self.end.replace_zero();
                Some(self.start.replace_one())
            } else {
                let nth_back = self.end.saturating_sub(n);
                self.end = nth_back.sub_one();
                Some(nth_back)
            }
        // count overflows u128; n is 64-bits at most.
        // therefore, n can never exceed count
        } else {
            let nth_back = self.end.saturating_sub(n);
            self.end = nth_back.sub_one();
            Some(nth_back)
        }
    }
}

impl FusedIterator for IpAddrRange {}
impl FusedIterator for Ipv4AddrRange {}
impl FusedIterator for Ipv6AddrRange {}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use core::str::FromStr;
    #[cfg(not(feature = "std"))]
    use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    #[cfg(feature = "std")]
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use super::*;

    #[test]
    fn test_ipaddrrange() {
        // Next, Next-Back
        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap()
        );

        assert_eq!(i.collect::<Vec<Ipv4Addr>>(), vec![
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.1").unwrap(),
            Ipv4Addr::from_str("10.0.0.2").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap(),
        ]);

        let mut v = i.collect::<Vec<_>>();
        v.reverse();
        assert_eq!(v, i.rev().collect::<Vec<_>>());

        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("255.255.255.254").unwrap(),
            Ipv4Addr::from_str("255.255.255.255").unwrap()
        );

        assert_eq!(i.collect::<Vec<Ipv4Addr>>(), vec![
            Ipv4Addr::from_str("255.255.255.254").unwrap(),
            Ipv4Addr::from_str("255.255.255.255").unwrap(),
        ]);

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        );

        assert_eq!(i.collect::<Vec<Ipv6Addr>>(), vec![
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::1").unwrap(),
            Ipv6Addr::from_str("fd00::2").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        ]);

        let mut v = i.collect::<Vec<_>>();
        v.reverse();
        assert_eq!(v, i.rev().collect::<Vec<_>>());

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe").unwrap(),
            Ipv6Addr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff").unwrap(),
        );

        assert_eq!(i.collect::<Vec<Ipv6Addr>>(), vec![
            Ipv6Addr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe").unwrap(),
            Ipv6Addr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff").unwrap(),
        ]);
        
        let i = IpAddrRange::from(Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap(),
        ));

        assert_eq!(i.collect::<Vec<IpAddr>>(), vec![
            IpAddr::from_str("10.0.0.0").unwrap(),
            IpAddr::from_str("10.0.0.1").unwrap(),
            IpAddr::from_str("10.0.0.2").unwrap(),
            IpAddr::from_str("10.0.0.3").unwrap(),
        ]);

        let mut v = i.collect::<Vec<_>>();
        v.reverse();
        assert_eq!(v, i.rev().collect::<Vec<_>>());
        
        let i = IpAddrRange::from(Ipv4AddrRange::new(
            Ipv4Addr::from_str("255.255.255.254").unwrap(),
            Ipv4Addr::from_str("255.255.255.255").unwrap()
        ));

        assert_eq!(i.collect::<Vec<IpAddr>>(), vec![
            IpAddr::from_str("255.255.255.254").unwrap(),
            IpAddr::from_str("255.255.255.255").unwrap(),
        ]);

        let i = IpAddrRange::from(Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        ));

        assert_eq!(i.collect::<Vec<IpAddr>>(), vec![
            IpAddr::from_str("fd00::").unwrap(),
            IpAddr::from_str("fd00::1").unwrap(),
            IpAddr::from_str("fd00::2").unwrap(),
            IpAddr::from_str("fd00::3").unwrap(),
        ]);

        let mut v = i.collect::<Vec<_>>();
        v.reverse();
        assert_eq!(v, i.rev().collect::<Vec<_>>());

        let i = IpAddrRange::from(Ipv6AddrRange::new(
            Ipv6Addr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe").unwrap(),
            Ipv6Addr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff").unwrap(),
        ));

        assert_eq!(i.collect::<Vec<IpAddr>>(), vec![
            IpAddr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe").unwrap(),
            IpAddr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff").unwrap(),
        ]);

        // #11 (infinite iterator when start and stop are 0)
        let zero4 = Ipv4Addr::from_str("0.0.0.0").unwrap();
        let zero6 = Ipv6Addr::from_str("::").unwrap();

        let mut i = Ipv4AddrRange::new(zero4, zero4);
        assert_eq!(Some(zero4), i.next());
        assert_eq!(None, i.next());

        let mut i = Ipv6AddrRange::new(zero6, zero6);
        assert_eq!(Some(zero6), i.next());
        assert_eq!(None, i.next());

        // Count
        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap()
        );
        assert_eq!(i.count(), 4);

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        );
        assert_eq!(i.count(), 4);

        // Size Hint
        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap()
        );
        assert_eq!(i.size_hint(), (4, Some(4)));

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        );
        assert_eq!(i.size_hint(), (4, Some(4)));

        // Size Hint: a range where size clearly overflows usize
        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("::").unwrap(),
            Ipv6Addr::from_str("8000::").unwrap(),
        );
        assert_eq!(i.size_hint(), (core::usize::MAX, None));

        // Min, Max, Last
        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap()
        );
        assert_eq!(Iterator::min(i), Some(Ipv4Addr::from_str("10.0.0.0").unwrap()));
        assert_eq!(Iterator::max(i), Some(Ipv4Addr::from_str("10.0.0.3").unwrap()));
        assert_eq!(i.last(), Some(Ipv4Addr::from_str("10.0.0.3").unwrap()));

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        );
        assert_eq!(Iterator::min(i), Some(Ipv6Addr::from_str("fd00::").unwrap()));
        assert_eq!(Iterator::max(i), Some(Ipv6Addr::from_str("fd00::3").unwrap()));
        assert_eq!(i.last(), Some(Ipv6Addr::from_str("fd00::3").unwrap()));

        // Nth
        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap()
        );
        assert_eq!(i.clone().nth(0), Some(Ipv4Addr::from_str("10.0.0.0").unwrap()));
        assert_eq!(i.clone().nth(3), Some(Ipv4Addr::from_str("10.0.0.3").unwrap()));
        assert_eq!(i.clone().nth(4), None);
        assert_eq!(i.clone().nth(99), None);
        let mut i2 = i.clone();
        assert_eq!(i2.nth(1), Some(Ipv4Addr::from_str("10.0.0.1").unwrap()));
        assert_eq!(i2.nth(1), Some(Ipv4Addr::from_str("10.0.0.3").unwrap()));
        assert_eq!(i2.nth(0), None);
        let mut i3 = i.clone();
        assert_eq!(i3.nth(99), None);
        assert_eq!(i3.next(), None);

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        );
        assert_eq!(i.clone().nth(0), Some(Ipv6Addr::from_str("fd00::").unwrap()));
        assert_eq!(i.clone().nth(3), Some(Ipv6Addr::from_str("fd00::3").unwrap()));
        assert_eq!(i.clone().nth(4), None);
        assert_eq!(i.clone().nth(99), None);
        let mut i2 = i.clone();
        assert_eq!(i2.nth(1), Some(Ipv6Addr::from_str("fd00::1").unwrap()));
        assert_eq!(i2.nth(1), Some(Ipv6Addr::from_str("fd00::3").unwrap()));
        assert_eq!(i2.nth(0), None);
        let mut i3 = i.clone();
        assert_eq!(i3.nth(99), None);
        assert_eq!(i3.next(), None);

        // Nth Back
        let i = Ipv4AddrRange::new(
            Ipv4Addr::from_str("10.0.0.0").unwrap(),
            Ipv4Addr::from_str("10.0.0.3").unwrap()
        );
        assert_eq!(i.clone().nth_back(0), Some(Ipv4Addr::from_str("10.0.0.3").unwrap()));
        assert_eq!(i.clone().nth_back(3), Some(Ipv4Addr::from_str("10.0.0.0").unwrap()));
        assert_eq!(i.clone().nth_back(4), None);
        assert_eq!(i.clone().nth_back(99), None);
        let mut i2 = i.clone();
        assert_eq!(i2.nth_back(1), Some(Ipv4Addr::from_str("10.0.0.2").unwrap()));
        assert_eq!(i2.nth_back(1), Some(Ipv4Addr::from_str("10.0.0.0").unwrap()));
        assert_eq!(i2.nth_back(0), None);
        let mut i3 = i.clone();
        assert_eq!(i3.nth_back(99), None);
        assert_eq!(i3.next(), None);

        let i = Ipv6AddrRange::new(
            Ipv6Addr::from_str("fd00::").unwrap(),
            Ipv6Addr::from_str("fd00::3").unwrap(),
        );
        assert_eq!(i.clone().nth_back(0), Some(Ipv6Addr::from_str("fd00::3").unwrap()));
        assert_eq!(i.clone().nth_back(3), Some(Ipv6Addr::from_str("fd00::").unwrap()));
        assert_eq!(i.clone().nth_back(4), None);
        assert_eq!(i.clone().nth_back(99), None);
        let mut i2 = i.clone();
        assert_eq!(i2.nth_back(1), Some(Ipv6Addr::from_str("fd00::2").unwrap()));
        assert_eq!(i2.nth_back(1), Some(Ipv6Addr::from_str("fd00::").unwrap()));
        assert_eq!(i2.nth_back(0), None);
        let mut i3 = i.clone();
        assert_eq!(i3.nth_back(99), None);
        assert_eq!(i3.next(), None);
    }
}
