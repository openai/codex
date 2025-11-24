//! Low-level Linux network device access
//!
//! The methods in this module take a socket's file descriptor to communicate
//! with the kernel in their ioctl call:
//!  - glibc uses an `AF_UNIX`, `AF_INET`, or `AF_INET6` socket. The address
//!    family itself does not matter and glibc tries the next address family if
//!    socket creation with one fails.
//!  - Android (bionic) uses an `AF_INET` socket.
//!  - Both create the socket with `SOCK_DGRAM|SOCK_CLOEXEC` type/flag.
//!  - The [manual pages] specify that the ioctl calls “can be used on any
//!    socket's file descriptor regardless of the family or type”.
//!
//! # References
//!  - [Linux]
//!
//! [manual pages]: https://man7.org/linux/man-pages/man7/netdevice.7.html
//! [Linux]: https://man7.org/linux/man-pages/man7/netdevice.7.html

use crate::fd::AsFd;
use crate::io;
#[cfg(feature = "alloc")]
use alloc::string::String;

/// `ioctl(fd, SIOCGIFINDEX, ifreq)`—Returns the interface index for a given
/// name.
///
/// See the [module-level documentation] for information about `fd` usage.
///
/// # References
///  - [Linux]
///
/// [module-level documentation]: self
/// [Linux]: https://man7.org/linux/man-pages/man7/netdevice.7.html
#[inline]
#[doc(alias = "SIOCGIFINDEX")]
pub fn name_to_index<Fd: AsFd>(fd: Fd, if_name: &str) -> io::Result<u32> {
    crate::backend::net::netdevice::name_to_index(fd.as_fd(), if_name)
}

/// `ioctl(fd, SIOCGIFNAME, ifreq)`—Returns the interface name for a given
/// index.
///
/// See the [module-level documentation] for information about `fd` usage.
///
/// # References
///  - [Linux]
///
/// [module-level documentation]: self
/// [Linux]: https://man7.org/linux/man-pages/man7/netdevice.7.html
#[inline]
#[doc(alias = "SIOCGIFNAME")]
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn index_to_name<Fd: AsFd>(fd: Fd, index: u32) -> io::Result<String> {
    crate::backend::net::netdevice::index_to_name(fd.as_fd(), index)
}

#[cfg(test)]
mod tests {
    use crate::backend::net::netdevice::{index_to_name, name_to_index};
    use crate::fd::AsFd;
    use crate::net::{AddressFamily, SocketFlags, SocketType};

    #[test]
    fn test_name_to_index() {
        let fd = crate::net::socket_with(
            AddressFamily::INET,
            SocketType::DGRAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();

        let loopback_index = std::fs::read_to_string("/sys/class/net/lo/ifindex")
            .unwrap()
            .as_str()
            .split_at(1)
            .0
            .parse::<u32>()
            .unwrap();
        assert_eq!(Ok(loopback_index), name_to_index(fd.as_fd(), "lo"));
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_index_to_name() {
        let fd = crate::net::socket_with(
            AddressFamily::INET,
            SocketType::DGRAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();

        let loopback_index = std::fs::read_to_string("/sys/class/net/lo/ifindex")
            .unwrap()
            .as_str()
            .split_at(1)
            .0
            .parse::<u32>()
            .unwrap();
        assert_eq!(
            Ok("lo".to_owned()),
            index_to_name(fd.as_fd(), loopback_index)
        );
    }
}
