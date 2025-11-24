//! Network interface name resolution.
//!
//! Uses Linux and/or POSIX functions to resolve interface names like "eth0"
//! or "socan1" into device numbers.

use std::{ffi::{CStr, CString}, fmt};
use crate::{errno::Errno, Error, NixPath, Result};
use libc::{c_uint, IF_NAMESIZE};

#[cfg(not(solarish))]
/// type alias for InterfaceFlags
pub type IflagsType = libc::c_int;
#[cfg(solarish)]
/// type alias for InterfaceFlags
pub type IflagsType = libc::c_longlong;

/// Resolve an interface into an interface number.
pub fn if_nametoindex<P: ?Sized + NixPath>(name: &P) -> Result<c_uint> {
    let if_index = name
        .with_nix_path(|name| unsafe { libc::if_nametoindex(name.as_ptr()) })?;

    if if_index == 0 {
        Err(Error::last())
    } else {
        Ok(if_index)
    }
}

/// Resolve an interface number into an interface.
pub fn if_indextoname(index: c_uint) -> Result<CString> {
    // We need to allocate this anyway, so doing it directly is faster.
    let mut buf = vec![0u8; IF_NAMESIZE];

    let return_buf = unsafe {
        libc::if_indextoname(index, buf.as_mut_ptr().cast())
    };

    Errno::result(return_buf.cast())?;
    Ok(CStr::from_bytes_until_nul(buf.as_slice()).unwrap().to_owned())
}

libc_bitflags!(
    /// Standard interface flags, used by `getifaddrs`
    pub struct InterfaceFlags: IflagsType {
    
        /// Interface is running. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_UP as IflagsType;
        /// Valid broadcast address set. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_BROADCAST as IflagsType;
        /// Internal debugging flag. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(not(target_os = "haiku"))]
        IFF_DEBUG as IflagsType;
        /// Interface is a loopback interface. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_LOOPBACK as IflagsType;
        /// Interface is a point-to-point link. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_POINTOPOINT as IflagsType;
        /// Avoid use of trailers. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(
                  linux_android,
                  solarish,
                  apple_targets,
                  target_os = "fuchsia",
                  target_os = "netbsd"))]
        IFF_NOTRAILERS as IflagsType;
        /// Interface manages own routes.
        #[cfg(any(target_os = "dragonfly"))]
        IFF_SMART as IflagsType;
        /// Resources allocated. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(
                  linux_android,
                  bsd,
                  solarish,
                  target_os = "fuchsia"))]
        IFF_RUNNING as IflagsType;
        /// No arp protocol, L2 destination address not set. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_NOARP as IflagsType;
        /// Interface is in promiscuous mode. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_PROMISC as IflagsType;
        /// Receive all multicast packets. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_ALLMULTI as IflagsType;
        /// Master of a load balancing bundle. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_MASTER;
        /// transmission in progress, tx hardware queue is full
        #[cfg(any(target_os = "freebsd", apple_targets, netbsdlike))]
        IFF_OACTIVE;
        /// Protocol code on board.
        #[cfg(solarish)]
        IFF_INTELLIGENT as IflagsType;
        /// Slave of a load balancing bundle. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_SLAVE;
        /// Can't hear own transmissions.
        #[cfg(bsd)]
        IFF_SIMPLEX;
        /// Supports multicast. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        IFF_MULTICAST as IflagsType;
        /// Per link layer defined bit.
        #[cfg(bsd)]
        IFF_LINK0;
        /// Multicast using broadcast.
        #[cfg(solarish)]
        IFF_MULTI_BCAST as IflagsType;
        /// Is able to select media type via ifmap. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_PORTSEL;
        /// Per link layer defined bit.
        #[cfg(bsd)]
        IFF_LINK1;
        /// Non-unique address.
        #[cfg(solarish)]
        IFF_UNNUMBERED as IflagsType;
        /// Auto media selection active. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_AUTOMEDIA;
        /// Per link layer defined bit.
        #[cfg(bsd)]
        IFF_LINK2;
        /// Use alternate physical connection.
        #[cfg(any(freebsdlike, apple_targets))]
        IFF_ALTPHYS;
        /// DHCP controls interface.
        #[cfg(solarish)]
        IFF_DHCPRUNNING as IflagsType;
        /// The addresses are lost when the interface goes down. (see
        /// [`netdevice(7)`](https://man7.org/linux/man-pages/man7/netdevice.7.html))
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_DYNAMIC;
        /// Do not advertise.
        #[cfg(solarish)]
        IFF_PRIVATE as IflagsType;
        /// Driver signals L1 up. Volatile.
        #[cfg(any(target_os = "fuchsia", target_os = "linux"))]
        IFF_LOWER_UP;
        /// Interface is in polling mode.
        #[cfg(any(target_os = "dragonfly"))]
        IFF_POLLING_COMPAT;
        /// Unconfigurable using ioctl(2).
        #[cfg(any(target_os = "freebsd"))]
        IFF_CANTCONFIG;
        /// Do not transmit packets.
        #[cfg(solarish)]
        IFF_NOXMIT as IflagsType;
        /// Driver signals dormant. Volatile.
        #[cfg(any(target_os = "fuchsia", target_os = "linux"))]
        IFF_DORMANT;
        /// User-requested promisc mode.
        #[cfg(freebsdlike)]
        IFF_PPROMISC;
        /// Just on-link subnet.
        #[cfg(solarish)]
        IFF_NOLOCAL as IflagsType;
        /// Echo sent packets. Volatile.
        #[cfg(any(target_os = "fuchsia", target_os = "linux"))]
        IFF_ECHO;
        /// User-requested monitor mode.
        #[cfg(freebsdlike)]
        IFF_MONITOR;
        /// Address is deprecated.
        #[cfg(solarish)]
        IFF_DEPRECATED as IflagsType;
        /// Static ARP.
        #[cfg(freebsdlike)]
        IFF_STATICARP;
        /// Address from stateless addrconf.
        #[cfg(solarish)]
        IFF_ADDRCONF as IflagsType;
        /// Interface is in polling mode.
        #[cfg(any(target_os = "dragonfly"))]
        IFF_NPOLLING;
        /// Router on interface.
        #[cfg(solarish)]
        IFF_ROUTER as IflagsType;
        /// Interface is in polling mode.
        #[cfg(any(target_os = "dragonfly"))]
        IFF_IDIRECT;
        /// Interface is winding down
        #[cfg(any(target_os = "freebsd"))]
        IFF_DYING;
        /// No NUD on interface.
        #[cfg(solarish)]
        IFF_NONUD as IflagsType;
        /// Interface is being renamed
        #[cfg(any(target_os = "freebsd"))]
        IFF_RENAMING;
        /// Anycast address.
        #[cfg(solarish)]
        IFF_ANYCAST as IflagsType;
        /// Don't exchange routing info.
        #[cfg(solarish)]
        IFF_NORTEXCH as IflagsType;
        /// Do not provide packet information
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_NO_PI as IflagsType;
        /// TUN device (no Ethernet headers)
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_TUN as IflagsType;
        /// TAP device
        #[cfg(any(linux_android, target_os = "fuchsia"))]
        IFF_TAP as IflagsType;
        /// IPv4 interface.
        #[cfg(solarish)]
        IFF_IPV4 as IflagsType;
        /// IPv6 interface.
        #[cfg(solarish)]
        IFF_IPV6 as IflagsType;
        /// in.mpathd test address
        #[cfg(solarish)]
        IFF_NOFAILOVER as IflagsType;
        /// Interface has failed
        #[cfg(solarish)]
        IFF_FAILED as IflagsType;
        /// Interface is a hot-spare
        #[cfg(solarish)]
        IFF_STANDBY as IflagsType;
        /// Functioning but not used
        #[cfg(solarish)]
        IFF_INACTIVE as IflagsType;
        /// Interface is offline
        #[cfg(solarish)]
        IFF_OFFLINE as IflagsType;
        /// Has CoS marking supported
        #[cfg(solarish)]
        IFF_COS_ENABLED as IflagsType;
        /// Prefer as source addr
        #[cfg(solarish)]
        IFF_PREFERRED as IflagsType;
        /// RFC3041
        #[cfg(solarish)]
        IFF_TEMPORARY as IflagsType;
        /// MTU set
        #[cfg(solarish)]
        IFF_FIXEDMTU as IflagsType;
        /// Cannot send/receive packets
        #[cfg(solarish)]
        IFF_VIRTUAL as IflagsType;
        /// Local address in use
        #[cfg(solarish)]
        IFF_DUPLICATE as IflagsType;
        /// IPMP IP interface
        #[cfg(solarish)]
        IFF_IPMP as IflagsType;
    }
);

impl fmt::Display for InterfaceFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        bitflags::parser::to_writer(self, f)
    }
}


#[cfg(any(
    bsd,
    target_os = "fuchsia",
    target_os = "linux",
    solarish,
))]
mod if_nameindex {
    use super::*;

    use std::ffi::CStr;
    use std::fmt;
    use std::marker::PhantomData;
    use std::ptr::NonNull;

    /// A network interface. Has a name like "eth0" or "wlp4s0" or "wlan0", as well as an index
    /// (1, 2, 3, etc) that identifies it in the OS's networking stack.
    #[allow(missing_copy_implementations)]
    #[repr(transparent)]
    pub struct Interface(libc::if_nameindex);

    impl Interface {
        /// Obtain the index of this interface.
        pub fn index(&self) -> c_uint {
            self.0.if_index
        }

        /// Obtain the name of this interface.
        pub fn name(&self) -> &CStr {
            unsafe { CStr::from_ptr(self.0.if_name) }
        }
    }

    impl fmt::Debug for Interface {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("Interface")
                .field("index", &self.index())
                .field("name", &self.name())
                .finish()
        }
    }

    /// A list of the network interfaces available on this system. Obtained from [`if_nameindex()`].
    #[repr(transparent)]
    pub struct Interfaces {
        ptr: NonNull<libc::if_nameindex>,
    }

    impl Interfaces {
        /// Iterate over the interfaces in this list.
        #[inline]
        pub fn iter(&self) -> InterfacesIter<'_> {
            self.into_iter()
        }

        /// Convert this to a slice of interfaces. Note that the underlying interfaces list is
        /// null-terminated, so calling this calculates the length. If random access isn't needed,
        /// [`Interfaces::iter()`] should be used instead.
        pub fn to_slice(&self) -> &[Interface] {
            let ifs = self.ptr.as_ptr().cast();
            let len = self.iter().count();
            unsafe { std::slice::from_raw_parts(ifs, len) }
        }
    }

    impl Drop for Interfaces {
        fn drop(&mut self) {
            unsafe { libc::if_freenameindex(self.ptr.as_ptr()) };
        }
    }

    impl fmt::Debug for Interfaces {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            self.to_slice().fmt(f)
        }
    }

    impl<'a> IntoIterator for &'a Interfaces {
        type IntoIter = InterfacesIter<'a>;
        type Item = &'a Interface;
        #[inline]
        fn into_iter(self) -> Self::IntoIter {
            InterfacesIter {
                ptr: self.ptr.as_ptr(),
                _marker: PhantomData,
            }
        }
    }

    /// An iterator over the interfaces in an [`Interfaces`].
    #[derive(Debug)]
    pub struct InterfacesIter<'a> {
        ptr: *const libc::if_nameindex,
        _marker: PhantomData<&'a Interfaces>,
    }

    impl<'a> Iterator for InterfacesIter<'a> {
        type Item = &'a Interface;
        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            unsafe {
                if (*self.ptr).if_index == 0 {
                    None
                } else {
                    let ret = &*(self.ptr as *const Interface);
                    self.ptr = self.ptr.add(1);
                    Some(ret)
                }
            }
        }
    }

    /// Retrieve a list of the network interfaces available on the local system.
    ///
    /// ```
    /// let interfaces = nix::net::if_::if_nameindex().unwrap();
    /// for iface in &interfaces {
    ///     println!("Interface #{} is called {}", iface.index(), iface.name().to_string_lossy());
    /// }
    /// ```
    pub fn if_nameindex() -> Result<Interfaces> {
        unsafe {
            let ifs = libc::if_nameindex();
            let ptr = NonNull::new(ifs).ok_or_else(Error::last)?;
            Ok(Interfaces { ptr })
        }
    }
}
#[cfg(any(
    bsd,
    target_os = "fuchsia",
    target_os = "linux",
    solarish,
))]
pub use if_nameindex::*;
