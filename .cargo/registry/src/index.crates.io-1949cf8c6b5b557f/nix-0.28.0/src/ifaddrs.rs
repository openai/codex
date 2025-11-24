//! Query network interface addresses
//!
//! Uses the Linux and/or BSD specific function `getifaddrs` to query the list
//! of interfaces and their associated addresses.

use cfg_if::cfg_if;
#[cfg(apple_targets)]
use std::convert::TryFrom;
use std::ffi;
use std::iter::Iterator;
use std::mem;
use std::option::Option;

use crate::net::if_::*;
use crate::sys::socket::{SockaddrLike, SockaddrStorage};
use crate::{Errno, Result};

/// Describes a single address for an interface as returned by `getifaddrs`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InterfaceAddress {
    /// Name of the network interface
    pub interface_name: String,
    /// Flags as from `SIOCGIFFLAGS` ioctl
    pub flags: InterfaceFlags,
    /// Network address of this interface
    pub address: Option<SockaddrStorage>,
    /// Netmask of this interface
    pub netmask: Option<SockaddrStorage>,
    /// Broadcast address of this interface, if applicable
    pub broadcast: Option<SockaddrStorage>,
    /// Point-to-point destination address
    pub destination: Option<SockaddrStorage>,
}

cfg_if! {
    if #[cfg(any(linux_android, target_os = "emscripten", target_os = "fuchsia"))] {
        fn get_ifu_from_sockaddr(info: &libc::ifaddrs) -> *const libc::sockaddr {
            info.ifa_ifu
        }
    } else {
        fn get_ifu_from_sockaddr(info: &libc::ifaddrs) -> *const libc::sockaddr {
            info.ifa_dstaddr
        }
    }
}

/// Workaround a bug in XNU where netmasks will always have the wrong size in
/// the sa_len field due to the kernel ignoring trailing zeroes in the structure
/// when setting the field. See https://github.com/nix-rust/nix/issues/1709#issuecomment-1199304470
///
/// To fix this, we stack-allocate a new sockaddr_storage, zero it out, and
/// memcpy sa_len of the netmask to that new storage. Finally, we reset the
/// ss_len field to sizeof(sockaddr_storage). This is supposedly valid as all
/// members of the sockaddr_storage are "ok" with being zeroed out (there are
/// no pointers).
#[cfg(apple_targets)]
unsafe fn workaround_xnu_bug(info: &libc::ifaddrs) -> Option<SockaddrStorage> {
    let src_sock = info.ifa_netmask;
    if src_sock.is_null() {
        return None;
    }

    let mut dst_sock = mem::MaybeUninit::<libc::sockaddr_storage>::zeroed();

    let dst_sock = unsafe {
        // memcpy only sa_len bytes, assume the rest is zero
        std::ptr::copy_nonoverlapping(
            src_sock as *const u8,
            dst_sock.as_mut_ptr().cast(),
            (*src_sock).sa_len.into(),
        );

        // Initialize ss_len to sizeof(libc::sockaddr_storage).
        (*dst_sock.as_mut_ptr()).ss_len =
            u8::try_from(mem::size_of::<libc::sockaddr_storage>()).unwrap();
        dst_sock.assume_init()
    };

    let dst_sock_ptr =
        &dst_sock as *const libc::sockaddr_storage as *const libc::sockaddr;

    unsafe { SockaddrStorage::from_raw(dst_sock_ptr, None) }
}

impl InterfaceAddress {
    /// Create an `InterfaceAddress` from the libc struct.
    fn from_libc_ifaddrs(info: &libc::ifaddrs) -> InterfaceAddress {
        let ifname = unsafe { ffi::CStr::from_ptr(info.ifa_name) };
        let address = unsafe { SockaddrStorage::from_raw(info.ifa_addr, None) };
        #[cfg(apple_targets)]
        let netmask = unsafe { workaround_xnu_bug(info) };
        #[cfg(not(apple_targets))]
        let netmask =
            unsafe { SockaddrStorage::from_raw(info.ifa_netmask, None) };
        let mut addr = InterfaceAddress {
            interface_name: ifname.to_string_lossy().to_string(),
            flags: InterfaceFlags::from_bits_truncate(
                info.ifa_flags as IflagsType,
            ),
            address,
            netmask,
            broadcast: None,
            destination: None,
        };

        let ifu = get_ifu_from_sockaddr(info);
        if addr.flags.contains(InterfaceFlags::IFF_POINTOPOINT) {
            addr.destination = unsafe { SockaddrStorage::from_raw(ifu, None) };
        } else if addr.flags.contains(InterfaceFlags::IFF_BROADCAST) {
            addr.broadcast = unsafe { SockaddrStorage::from_raw(ifu, None) };
        }

        addr
    }
}

/// Holds the results of `getifaddrs`.
///
/// Use the function `getifaddrs` to create this Iterator. Note that the
/// actual list of interfaces can be iterated once and will be freed as
/// soon as the Iterator goes out of scope.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct InterfaceAddressIterator {
    base: *mut libc::ifaddrs,
    next: *mut libc::ifaddrs,
}

impl Drop for InterfaceAddressIterator {
    fn drop(&mut self) {
        unsafe { libc::freeifaddrs(self.base) };
    }
}

impl Iterator for InterfaceAddressIterator {
    type Item = InterfaceAddress;
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        match unsafe { self.next.as_ref() } {
            Some(ifaddr) => {
                self.next = ifaddr.ifa_next;
                Some(InterfaceAddress::from_libc_ifaddrs(ifaddr))
            }
            None => None,
        }
    }
}

/// Get interface addresses using libc's `getifaddrs`
///
/// Note that the underlying implementation differs between OSes. Only the
/// most common address families are supported by the nix crate (due to
/// lack of time and complexity of testing). The address family is encoded
/// in the specific variant of `SockaddrStorage` returned for the fields
/// `address`, `netmask`, `broadcast`, and `destination`. For any entry not
/// supported, the returned list will contain a `None` entry.
///
/// # Example
/// ```
/// let addrs = nix::ifaddrs::getifaddrs().unwrap();
/// for ifaddr in addrs {
///   match ifaddr.address {
///     Some(address) => {
///       println!("interface {} address {}",
///                ifaddr.interface_name, address);
///     },
///     None => {
///       println!("interface {} with unsupported address family",
///                ifaddr.interface_name);
///     }
///   }
/// }
/// ```
pub fn getifaddrs() -> Result<InterfaceAddressIterator> {
    let mut addrs = mem::MaybeUninit::<*mut libc::ifaddrs>::uninit();
    unsafe {
        Errno::result(libc::getifaddrs(addrs.as_mut_ptr())).map(|_| {
            InterfaceAddressIterator {
                base: addrs.assume_init(),
                next: addrs.assume_init(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only checks if `getifaddrs` can be invoked without panicking.
    #[test]
    fn test_getifaddrs() {
        let _ = getifaddrs();
    }

    // Ensures getting the netmask works, and in particular that
    // `workaround_xnu_bug` works properly.
    #[test]
    fn test_getifaddrs_netmask_correct() {
        let addrs = getifaddrs().unwrap();
        for iface in addrs {
            let sock = if let Some(sock) = iface.netmask {
                sock
            } else {
                continue;
            };
            if sock.family() == Some(crate::sys::socket::AddressFamily::Inet) {
                let _ = sock.as_sockaddr_in().unwrap();
                return;
            } else if sock.family()
                == Some(crate::sys::socket::AddressFamily::Inet6)
            {
                let _ = sock.as_sockaddr_in6().unwrap();
                return;
            }
        }
        panic!("No address?");
    }
}
