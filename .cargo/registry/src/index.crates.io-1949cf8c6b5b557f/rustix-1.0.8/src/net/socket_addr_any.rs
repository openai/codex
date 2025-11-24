//! The [`SocketAddrAny`] type and related utilities.

#![allow(unsafe_code)]

use crate::backend::c;
use crate::backend::net::read_sockaddr;
use crate::io::Errno;
use crate::net::addr::{SocketAddrArg, SocketAddrLen, SocketAddrOpaque, SocketAddrStorage};
#[cfg(unix)]
use crate::net::SocketAddrUnix;
use crate::net::{AddressFamily, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::fmt;
use core::mem::{size_of, MaybeUninit};
use core::num::NonZeroU32;

/// Temporary buffer for creating a `SocketAddrAny` from a syscall that writes
/// to a `sockaddr_t` and `socklen_t`
///
/// Unlike `SocketAddrAny`, this does not maintain the invariant that `len`
/// bytes are initialized.
pub(crate) struct SocketAddrBuf {
    pub(crate) len: c::socklen_t,
    pub(crate) storage: MaybeUninit<SocketAddrStorage>,
}

impl SocketAddrBuf {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            len: size_of::<SocketAddrStorage>() as c::socklen_t,
            storage: MaybeUninit::<SocketAddrStorage>::uninit(),
        }
    }

    /// Convert the buffer into [`SocketAddrAny`].
    ///
    /// # Safety
    ///
    /// A valid address must have been written into `self.storage` and its
    /// length written into `self.len`.
    #[inline]
    pub(crate) unsafe fn into_any(self) -> SocketAddrAny {
        SocketAddrAny::new(self.storage, bitcast!(self.len))
    }

    /// Convert the buffer into [`Option<SocketAddrAny>`].
    ///
    /// This returns `None` if `len` is zero or other platform-specific
    /// conditions define the address as empty.
    ///
    /// # Safety
    ///
    /// Either valid address must have been written into `self.storage` and its
    /// length written into `self.len`, or `self.len` must have been set to 0.
    #[inline]
    pub(crate) unsafe fn into_any_option(self) -> Option<SocketAddrAny> {
        let len = bitcast!(self.len);
        if read_sockaddr::sockaddr_nonempty(self.storage.as_ptr().cast(), len) {
            Some(SocketAddrAny::new(self.storage, len))
        } else {
            None
        }
    }
}

/// A type that can hold any kind of socket address, as a safe abstraction for
/// `sockaddr_storage`.
///
/// Socket addresses can be converted to `SocketAddrAny` via the [`From`] and
/// [`Into`] traits. `SocketAddrAny` can be converted back to a specific socket
/// address type with [`TryFrom`] and [`TryInto`]. These implementations return
/// [`Errno::AFNOSUPPORT`] if the address family does not match the requested
/// type.
#[derive(Clone)]
#[doc(alias = "sockaddr_storage")]
pub struct SocketAddrAny {
    // Invariants:
    //  - `len` is at least `size_of::<backend::c::sa_family_t>()`
    //  - `len` is at most `size_of::<SocketAddrStorage>()`
    //  - The first `len` bytes of `storage` are initialized.
    pub(crate) len: NonZeroU32,
    pub(crate) storage: MaybeUninit<SocketAddrStorage>,
}

impl SocketAddrAny {
    /// Creates a socket address from `storage`, which is initialized for `len`
    /// bytes.
    ///
    /// # Panics
    ///
    /// if `len` is smaller than the sockaddr header or larger than
    /// `SocketAddrStorage`.
    ///
    /// # Safety
    ///
    ///  - `storage` must contain a valid socket address.
    ///  - `len` bytes must be initialized.
    #[inline]
    pub const unsafe fn new(storage: MaybeUninit<SocketAddrStorage>, len: SocketAddrLen) -> Self {
        assert!(len as usize >= size_of::<read_sockaddr::sockaddr_header>());
        assert!(len as usize <= size_of::<SocketAddrStorage>());
        let len = NonZeroU32::new_unchecked(len);
        Self { storage, len }
    }

    /// Creates a socket address from reading from `ptr`, which points at `len`
    /// initialized bytes.
    ///
    /// # Panics
    ///
    /// if `len` is smaller than the sockaddr header or larger than
    /// `SocketAddrStorage`.
    ///
    /// # Safety
    ///
    ///  - `ptr` must be a pointer to memory containing a valid socket address.
    ///  - `len` bytes must be initialized.
    pub unsafe fn read(ptr: *const SocketAddrStorage, len: SocketAddrLen) -> Self {
        assert!(len as usize >= size_of::<read_sockaddr::sockaddr_header>());
        assert!(len as usize <= size_of::<SocketAddrStorage>());
        let mut storage = MaybeUninit::<SocketAddrStorage>::uninit();
        core::ptr::copy_nonoverlapping(
            ptr.cast::<u8>(),
            storage.as_mut_ptr().cast::<u8>(),
            len as usize,
        );
        let len = NonZeroU32::new_unchecked(len);
        Self { storage, len }
    }

    /// Gets the initialized part of the storage as bytes.
    #[inline]
    fn bytes(&self) -> &[u8] {
        let len = self.len.get() as usize;
        unsafe { core::slice::from_raw_parts(self.storage.as_ptr().cast(), len) }
    }

    /// Gets the address family of this socket address.
    #[inline]
    pub fn address_family(&self) -> AddressFamily {
        // SAFETY: Our invariants maintain that the `sa_family` field is
        // initialized.
        unsafe {
            AddressFamily::from_raw(crate::backend::net::read_sockaddr::read_sa_family(
                self.storage.as_ptr().cast(),
            ))
        }
    }

    /// Returns a raw pointer to the sockaddr.
    #[inline]
    pub fn as_ptr(&self) -> *const SocketAddrStorage {
        self.storage.as_ptr()
    }

    /// Returns a raw mutable pointer to the sockaddr.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut SocketAddrStorage {
        self.storage.as_mut_ptr()
    }

    /// Returns the length of the encoded sockaddr.
    #[inline]
    pub fn addr_len(&self) -> SocketAddrLen {
        self.len.get()
    }
}

impl PartialEq<Self> for SocketAddrAny {
    fn eq(&self, other: &Self) -> bool {
        self.bytes() == other.bytes()
    }
}

impl Eq for SocketAddrAny {}

// This just forwards to another `partial_cmp`.
#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd<Self> for SocketAddrAny {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.bytes().partial_cmp(other.bytes())
    }
}

impl Ord for SocketAddrAny {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.bytes().cmp(other.bytes())
    }
}

impl core::hash::Hash for SocketAddrAny {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.bytes().hash(state)
    }
}

impl fmt::Debug for SocketAddrAny {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.address_family() {
            AddressFamily::INET => {
                if let Ok(addr) = SocketAddrV4::try_from(self.clone()) {
                    return addr.fmt(f);
                }
            }
            AddressFamily::INET6 => {
                if let Ok(addr) = SocketAddrV6::try_from(self.clone()) {
                    return addr.fmt(f);
                }
            }
            #[cfg(unix)]
            AddressFamily::UNIX => {
                if let Ok(addr) = SocketAddrUnix::try_from(self.clone()) {
                    return addr.fmt(f);
                }
            }
            #[cfg(target_os = "linux")]
            AddressFamily::XDP => {
                if let Ok(addr) = crate::net::xdp::SocketAddrXdp::try_from(self.clone()) {
                    return addr.fmt(f);
                }
            }
            #[cfg(linux_kernel)]
            AddressFamily::NETLINK => {
                if let Ok(addr) = crate::net::netlink::SocketAddrNetlink::try_from(self.clone()) {
                    return addr.fmt(f);
                }
            }
            _ => {}
        }

        f.debug_struct("SocketAddrAny")
            .field("address_family", &self.address_family())
            .field("namelen", &self.addr_len())
            .finish()
    }
}

// SAFETY: `with_sockaddr` calls `f` with a pointer to its own storage.
unsafe impl SocketAddrArg for SocketAddrAny {
    unsafe fn with_sockaddr<R>(
        &self,
        f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
    ) -> R {
        f(self.as_ptr().cast(), self.addr_len())
    }
}

impl From<SocketAddr> for SocketAddrAny {
    #[inline]
    fn from(from: SocketAddr) -> Self {
        from.as_any()
    }
}

impl TryFrom<SocketAddrAny> for SocketAddr {
    type Error = Errno;

    /// Convert if the address is an IPv4 or IPv6 address.
    ///
    /// Returns `Err(Errno::AFNOSUPPORT)` if the address family is not IPv4 or
    /// IPv6.
    #[inline]
    fn try_from(value: SocketAddrAny) -> Result<Self, Self::Error> {
        match value.address_family() {
            AddressFamily::INET => read_sockaddr::read_sockaddr_v4(&value).map(SocketAddr::V4),
            AddressFamily::INET6 => read_sockaddr::read_sockaddr_v6(&value).map(SocketAddr::V6),
            _ => Err(Errno::AFNOSUPPORT),
        }
    }
}

impl From<SocketAddrV4> for SocketAddrAny {
    #[inline]
    fn from(from: SocketAddrV4) -> Self {
        from.as_any()
    }
}

impl TryFrom<SocketAddrAny> for SocketAddrV4 {
    type Error = Errno;

    /// Convert if the address is an IPv4 address.
    ///
    /// Returns `Err(Errno::AFNOSUPPORT)` if the address family is not IPv4.
    #[inline]
    fn try_from(value: SocketAddrAny) -> Result<Self, Self::Error> {
        read_sockaddr::read_sockaddr_v4(&value)
    }
}

impl From<SocketAddrV6> for SocketAddrAny {
    #[inline]
    fn from(from: SocketAddrV6) -> Self {
        from.as_any()
    }
}

impl TryFrom<SocketAddrAny> for SocketAddrV6 {
    type Error = Errno;

    /// Convert if the address is an IPv6 address.
    ///
    /// Returns `Err(Errno::AFNOSUPPORT)` if the address family is not IPv6.
    #[inline]
    fn try_from(value: SocketAddrAny) -> Result<Self, Self::Error> {
        read_sockaddr::read_sockaddr_v6(&value)
    }
}

#[cfg(unix)]
impl From<SocketAddrUnix> for SocketAddrAny {
    #[inline]
    fn from(from: SocketAddrUnix) -> Self {
        from.as_any()
    }
}

#[cfg(unix)]
impl TryFrom<SocketAddrAny> for SocketAddrUnix {
    type Error = Errno;

    /// Convert if the address is a Unix socket address.
    ///
    /// Returns `Err(Errno::AFNOSUPPORT)` if the address family is not Unix.
    #[inline]
    fn try_from(value: SocketAddrAny) -> Result<Self, Self::Error> {
        read_sockaddr::read_sockaddr_unix(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_read() {
        let localhost = std::net::Ipv6Addr::LOCALHOST;
        let addr = SocketAddrAny::from(SocketAddrV6::new(localhost, 7, 8, 9));
        unsafe {
            let same = SocketAddrAny::read(addr.as_ptr(), addr.addr_len());
            assert_eq!(addr, same);
        }
    }
}
