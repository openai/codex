use std::hash::Hash;
use std::mem::{self, size_of, MaybeUninit};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::path::Path;
use std::{fmt, io, ptr};

#[cfg(windows)]
use windows_sys::Win32::Networking::WinSock::SOCKADDR_IN6_0;

use crate::sys::{
    c_int, sa_family_t, sockaddr, sockaddr_in, sockaddr_in6, sockaddr_storage, socklen_t, AF_INET,
    AF_INET6, AF_UNIX,
};
use crate::Domain;

/// The address of a socket.
///
/// `SockAddr`s may be constructed directly to and from the standard library
/// [`SocketAddr`], [`SocketAddrV4`], and [`SocketAddrV6`] types.
#[derive(Clone)]
pub struct SockAddr {
    storage: sockaddr_storage,
    len: socklen_t,
}

#[allow(clippy::len_without_is_empty)]
impl SockAddr {
    /// Create a `SockAddr` from the underlying storage and its length.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the address family and length match the type of
    /// storage address. For example if `storage.ss_family` is set to `AF_INET`
    /// the `storage` must be initialised as `sockaddr_in`, setting the content
    /// and length appropriately.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> std::io::Result<()> {
    /// # #[cfg(unix)] {
    /// use std::io;
    /// use std::mem;
    /// use std::os::unix::io::AsRawFd;
    ///
    /// use socket2::{SockAddr, Socket, Domain, Type};
    ///
    /// let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    ///
    /// // Initialise a `SocketAddr` byte calling `getsockname(2)`.
    /// let mut addr_storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
    /// let mut len = mem::size_of_val(&addr_storage) as libc::socklen_t;
    ///
    /// // The `getsockname(2)` system call will intiliase `storage` for
    /// // us, setting `len` to the correct length.
    /// let res = unsafe {
    ///     libc::getsockname(
    ///         socket.as_raw_fd(),
    ///         (&mut addr_storage as *mut libc::sockaddr_storage).cast(),
    ///         &mut len,
    ///     )
    /// };
    /// if res == -1 {
    ///     return Err(io::Error::last_os_error());
    /// }
    ///
    /// let address = unsafe { SockAddr::new(addr_storage, len) };
    /// # drop(address);
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    pub const unsafe fn new(storage: sockaddr_storage, len: socklen_t) -> SockAddr {
        SockAddr { storage, len }
    }

    /// Initialise a `SockAddr` by calling the function `init`.
    ///
    /// The type of the address storage and length passed to the function `init`
    /// is OS/architecture specific.
    ///
    /// The address is zeroed before `init` is called and is thus valid to
    /// dereference and read from. The length initialised to the maximum length
    /// of the storage.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the address family and length match the type of
    /// storage address. For example if `storage.ss_family` is set to `AF_INET`
    /// the `storage` must be initialised as `sockaddr_in`, setting the content
    /// and length appropriately.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> std::io::Result<()> {
    /// # #[cfg(unix)] {
    /// use std::io;
    /// use std::os::unix::io::AsRawFd;
    ///
    /// use socket2::{SockAddr, Socket, Domain, Type};
    ///
    /// let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    ///
    /// // Initialise a `SocketAddr` byte calling `getsockname(2)`.
    /// let (_, address) = unsafe {
    ///     SockAddr::try_init(|addr_storage, len| {
    ///         // The `getsockname(2)` system call will intiliase `storage` for
    ///         // us, setting `len` to the correct length.
    ///         if libc::getsockname(socket.as_raw_fd(), addr_storage.cast(), len) == -1 {
    ///             Err(io::Error::last_os_error())
    ///         } else {
    ///             Ok(())
    ///         }
    ///     })
    /// }?;
    /// # drop(address);
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    pub unsafe fn try_init<F, T>(init: F) -> io::Result<(T, SockAddr)>
    where
        F: FnOnce(*mut sockaddr_storage, *mut socklen_t) -> io::Result<T>,
    {
        const STORAGE_SIZE: socklen_t = size_of::<sockaddr_storage>() as socklen_t;
        // NOTE: `SockAddr::unix` depends on the storage being zeroed before
        // calling `init`.
        // NOTE: calling `recvfrom` with an empty buffer also depends on the
        // storage being zeroed before calling `init` as the OS might not
        // initialise it.
        let mut storage = MaybeUninit::<sockaddr_storage>::zeroed();
        let mut len = STORAGE_SIZE;
        init(storage.as_mut_ptr(), &mut len).map(|res| {
            debug_assert!(len <= STORAGE_SIZE, "overflown address storage");
            let addr = SockAddr {
                // Safety: zeroed-out `sockaddr_storage` is valid, caller must
                // ensure at least `len` bytes are valid.
                storage: storage.assume_init(),
                len,
            };
            (res, addr)
        })
    }

    /// Constructs a `SockAddr` with the family `AF_UNIX` and the provided path.
    ///
    /// Returns an error if the path is longer than `SUN_LEN`.
    pub fn unix<P>(path: P) -> io::Result<SockAddr>
    where
        P: AsRef<Path>,
    {
        crate::sys::unix_sockaddr(path.as_ref())
    }

    /// Set the length of the address.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the address up to `length` bytes are properly
    /// initialised.
    pub unsafe fn set_length(&mut self, length: socklen_t) {
        self.len = length;
    }

    /// Returns this address's family.
    pub const fn family(&self) -> sa_family_t {
        self.storage.ss_family
    }

    /// Returns this address's `Domain`.
    pub const fn domain(&self) -> Domain {
        Domain(self.storage.ss_family as c_int)
    }

    /// Returns the size of this address in bytes.
    pub const fn len(&self) -> socklen_t {
        self.len
    }

    /// Returns a raw pointer to the address.
    pub const fn as_ptr(&self) -> *const sockaddr {
        ptr::addr_of!(self.storage).cast()
    }

    /// Retuns the address as the storage.
    pub const fn as_storage(self) -> sockaddr_storage {
        self.storage
    }

    /// Returns true if this address is in the `AF_INET` (IPv4) family, false otherwise.
    pub const fn is_ipv4(&self) -> bool {
        self.storage.ss_family == AF_INET as sa_family_t
    }

    /// Returns true if this address is in the `AF_INET6` (IPv6) family, false
    /// otherwise.
    pub const fn is_ipv6(&self) -> bool {
        self.storage.ss_family == AF_INET6 as sa_family_t
    }

    /// Returns true if this address is of a unix socket (for local interprocess communication),
    /// i.e. it is from the `AF_UNIX` family, false otherwise.
    pub fn is_unix(&self) -> bool {
        self.storage.ss_family == AF_UNIX as sa_family_t
    }

    /// Returns this address as a `SocketAddr` if it is in the `AF_INET` (IPv4)
    /// or `AF_INET6` (IPv6) family, otherwise returns `None`.
    pub fn as_socket(&self) -> Option<SocketAddr> {
        if self.storage.ss_family == AF_INET as sa_family_t {
            // SAFETY: if the `ss_family` field is `AF_INET` then storage must
            // be a `sockaddr_in`.
            let addr = unsafe { &*(ptr::addr_of!(self.storage).cast::<sockaddr_in>()) };
            let ip = crate::sys::from_in_addr(addr.sin_addr);
            let port = u16::from_be(addr.sin_port);
            Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        } else if self.storage.ss_family == AF_INET6 as sa_family_t {
            // SAFETY: if the `ss_family` field is `AF_INET6` then storage must
            // be a `sockaddr_in6`.
            let addr = unsafe { &*(ptr::addr_of!(self.storage).cast::<sockaddr_in6>()) };
            let ip = crate::sys::from_in6_addr(addr.sin6_addr);
            let port = u16::from_be(addr.sin6_port);
            Some(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                #[cfg(unix)]
                addr.sin6_scope_id,
                #[cfg(windows)]
                unsafe {
                    addr.Anonymous.sin6_scope_id
                },
            )))
        } else {
            None
        }
    }

    /// Returns this address as a [`SocketAddrV4`] if it is in the `AF_INET`
    /// family.
    pub fn as_socket_ipv4(&self) -> Option<SocketAddrV4> {
        match self.as_socket() {
            Some(SocketAddr::V4(addr)) => Some(addr),
            _ => None,
        }
    }

    /// Returns this address as a [`SocketAddrV6`] if it is in the `AF_INET6`
    /// family.
    pub fn as_socket_ipv6(&self) -> Option<SocketAddrV6> {
        match self.as_socket() {
            Some(SocketAddr::V6(addr)) => Some(addr),
            _ => None,
        }
    }

    /// Returns the initialised storage bytes.
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `self.storage` is a C struct which can always be treated a
        // slice of bytes. Furthermore, we ensure we don't read any unitialised
        // bytes by using `self.len`.
        unsafe { std::slice::from_raw_parts(self.as_ptr().cast(), self.len as usize) }
    }
}

impl From<SocketAddr> for SockAddr {
    fn from(addr: SocketAddr) -> SockAddr {
        match addr {
            SocketAddr::V4(addr) => addr.into(),
            SocketAddr::V6(addr) => addr.into(),
        }
    }
}

impl From<SocketAddrV4> for SockAddr {
    fn from(addr: SocketAddrV4) -> SockAddr {
        // SAFETY: a `sockaddr_storage` of all zeros is valid.
        let mut storage = unsafe { mem::zeroed::<sockaddr_storage>() };
        let len = {
            let storage = unsafe { &mut *ptr::addr_of_mut!(storage).cast::<sockaddr_in>() };
            storage.sin_family = AF_INET as sa_family_t;
            storage.sin_port = addr.port().to_be();
            storage.sin_addr = crate::sys::to_in_addr(addr.ip());
            storage.sin_zero = Default::default();
            mem::size_of::<sockaddr_in>() as socklen_t
        };
        #[cfg(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "haiku",
            target_os = "hermit",
            target_os = "ios",
            target_os = "visionos",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "nto",
            target_os = "openbsd",
            target_os = "tvos",
            target_os = "vxworks",
            target_os = "watchos",
        ))]
        {
            storage.ss_len = len as u8;
        }
        SockAddr { storage, len }
    }
}

impl From<SocketAddrV6> for SockAddr {
    fn from(addr: SocketAddrV6) -> SockAddr {
        // SAFETY: a `sockaddr_storage` of all zeros is valid.
        let mut storage = unsafe { mem::zeroed::<sockaddr_storage>() };
        let len = {
            let storage = unsafe { &mut *ptr::addr_of_mut!(storage).cast::<sockaddr_in6>() };
            storage.sin6_family = AF_INET6 as sa_family_t;
            storage.sin6_port = addr.port().to_be();
            storage.sin6_addr = crate::sys::to_in6_addr(addr.ip());
            storage.sin6_flowinfo = addr.flowinfo();
            #[cfg(unix)]
            {
                storage.sin6_scope_id = addr.scope_id();
            }
            #[cfg(windows)]
            {
                storage.Anonymous = SOCKADDR_IN6_0 {
                    sin6_scope_id: addr.scope_id(),
                };
            }
            mem::size_of::<sockaddr_in6>() as socklen_t
        };
        #[cfg(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "haiku",
            target_os = "hermit",
            target_os = "ios",
            target_os = "visionos",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "nto",
            target_os = "openbsd",
            target_os = "tvos",
            target_os = "vxworks",
            target_os = "watchos",
        ))]
        {
            storage.ss_len = len as u8;
        }
        SockAddr { storage, len }
    }
}

impl fmt::Debug for SockAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = fmt.debug_struct("SockAddr");
        #[cfg(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "haiku",
            target_os = "hermit",
            target_os = "ios",
            target_os = "visionos",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "nto",
            target_os = "openbsd",
            target_os = "tvos",
            target_os = "vxworks",
            target_os = "watchos",
        ))]
        f.field("ss_len", &self.storage.ss_len);
        f.field("ss_family", &self.storage.ss_family)
            .field("len", &self.len)
            .finish()
    }
}

impl PartialEq for SockAddr {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for SockAddr {}

impl Hash for SockAddr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4() {
        use std::net::Ipv4Addr;
        let std = SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 9876);
        let addr = SockAddr::from(std);
        assert!(addr.is_ipv4());
        assert!(!addr.is_ipv6());
        assert!(!addr.is_unix());
        assert_eq!(addr.family(), AF_INET as sa_family_t);
        assert_eq!(addr.domain(), Domain::IPV4);
        assert_eq!(addr.len(), size_of::<sockaddr_in>() as socklen_t);
        assert_eq!(addr.as_socket(), Some(SocketAddr::V4(std)));
        assert_eq!(addr.as_socket_ipv4(), Some(std));
        assert!(addr.as_socket_ipv6().is_none());

        let addr = SockAddr::from(SocketAddr::from(std));
        assert_eq!(addr.family(), AF_INET as sa_family_t);
        assert_eq!(addr.len(), size_of::<sockaddr_in>() as socklen_t);
        assert_eq!(addr.as_socket(), Some(SocketAddr::V4(std)));
        assert_eq!(addr.as_socket_ipv4(), Some(std));
        assert!(addr.as_socket_ipv6().is_none());
        #[cfg(unix)]
        {
            assert!(addr.as_pathname().is_none());
            assert!(addr.as_abstract_namespace().is_none());
        }
    }

    #[test]
    fn ipv6() {
        use std::net::Ipv6Addr;
        let std = SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 9876, 11, 12);
        let addr = SockAddr::from(std);
        assert!(addr.is_ipv6());
        assert!(!addr.is_ipv4());
        assert!(!addr.is_unix());
        assert_eq!(addr.family(), AF_INET6 as sa_family_t);
        assert_eq!(addr.domain(), Domain::IPV6);
        assert_eq!(addr.len(), size_of::<sockaddr_in6>() as socklen_t);
        assert_eq!(addr.as_socket(), Some(SocketAddr::V6(std)));
        assert!(addr.as_socket_ipv4().is_none());
        assert_eq!(addr.as_socket_ipv6(), Some(std));

        let addr = SockAddr::from(SocketAddr::from(std));
        assert_eq!(addr.family(), AF_INET6 as sa_family_t);
        assert_eq!(addr.len(), size_of::<sockaddr_in6>() as socklen_t);
        assert_eq!(addr.as_socket(), Some(SocketAddr::V6(std)));
        assert!(addr.as_socket_ipv4().is_none());
        assert_eq!(addr.as_socket_ipv6(), Some(std));
        #[cfg(unix)]
        {
            assert!(addr.as_pathname().is_none());
            assert!(addr.as_abstract_namespace().is_none());
        }
    }

    #[test]
    fn ipv4_eq() {
        use std::net::Ipv4Addr;

        let std1 = SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 9876);
        let std2 = SocketAddrV4::new(Ipv4Addr::new(5, 6, 7, 8), 8765);

        test_eq(
            SockAddr::from(std1),
            SockAddr::from(std1),
            SockAddr::from(std2),
        );
    }

    #[test]
    fn ipv4_hash() {
        use std::net::Ipv4Addr;

        let std1 = SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 9876);
        let std2 = SocketAddrV4::new(Ipv4Addr::new(5, 6, 7, 8), 8765);

        test_hash(
            SockAddr::from(std1),
            SockAddr::from(std1),
            SockAddr::from(std2),
        );
    }

    #[test]
    fn ipv6_eq() {
        use std::net::Ipv6Addr;

        let std1 = SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 9876, 11, 12);
        let std2 = SocketAddrV6::new(Ipv6Addr::new(3, 4, 5, 6, 7, 8, 9, 0), 7654, 13, 14);

        test_eq(
            SockAddr::from(std1),
            SockAddr::from(std1),
            SockAddr::from(std2),
        );
    }

    #[test]
    fn ipv6_hash() {
        use std::net::Ipv6Addr;

        let std1 = SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 9876, 11, 12);
        let std2 = SocketAddrV6::new(Ipv6Addr::new(3, 4, 5, 6, 7, 8, 9, 0), 7654, 13, 14);

        test_hash(
            SockAddr::from(std1),
            SockAddr::from(std1),
            SockAddr::from(std2),
        );
    }

    #[test]
    fn ipv4_ipv6_eq() {
        use std::net::Ipv4Addr;
        use std::net::Ipv6Addr;

        let std1 = SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 9876);
        let std2 = SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 9876, 11, 12);

        test_eq(
            SockAddr::from(std1),
            SockAddr::from(std1),
            SockAddr::from(std2),
        );

        test_eq(
            SockAddr::from(std2),
            SockAddr::from(std2),
            SockAddr::from(std1),
        );
    }

    #[test]
    fn ipv4_ipv6_hash() {
        use std::net::Ipv4Addr;
        use std::net::Ipv6Addr;

        let std1 = SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 9876);
        let std2 = SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 9876, 11, 12);

        test_hash(
            SockAddr::from(std1),
            SockAddr::from(std1),
            SockAddr::from(std2),
        );

        test_hash(
            SockAddr::from(std2),
            SockAddr::from(std2),
            SockAddr::from(std1),
        );
    }

    #[allow(clippy::eq_op)] // allow a0 == a0 check
    fn test_eq(a0: SockAddr, a1: SockAddr, b: SockAddr) {
        assert!(a0 == a0);
        assert!(a0 == a1);
        assert!(a1 == a0);
        assert!(a0 != b);
        assert!(b != a0);
    }

    fn test_hash(a0: SockAddr, a1: SockAddr, b: SockAddr) {
        assert!(calculate_hash(&a0) == calculate_hash(&a0));
        assert!(calculate_hash(&a0) == calculate_hash(&a1));
        // technically unequal values can have the same hash, in this case x != z and both have different hashes
        assert!(calculate_hash(&a0) != calculate_hash(&b));
    }

    fn calculate_hash(x: &SockAddr) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        x.hash(&mut hasher);
        hasher.finish()
    }
}
