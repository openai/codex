//! Types for implementers of socket address types or code that is generic over
//! address types.
//!
//! The concrete address types and [`SocketAddrAny`] are in
//! [the parent module][`super`].

#![allow(unsafe_code)]
use core::mem::{size_of, MaybeUninit};
use core::ptr;

use crate::backend::net::write_sockaddr::{encode_sockaddr_v4, encode_sockaddr_v6};
use crate::utils::as_ptr;

use super::{SocketAddr, SocketAddrAny, SocketAddrV4, SocketAddrV6};

pub use crate::backend::net::addr::SocketAddrStorage;

#[cfg(unix)]
use super::SocketAddrUnix;

/// Opaque type equivalent to `sockaddr` in C.
///
/// This is always used behind a raw pointer that is cast from a pointer to a
/// `sockaddr`-compatible C type, and then cast back to a `sockaddr` pointer to
/// be passed to a system call.
#[repr(C)]
pub struct SocketAddrOpaque {
    _data: [u8; 0],
}

/// A type for the length of a socket address.
///
/// This type will always be big enough to hold any socket address, but never
/// bigger than `usize`.
#[doc(alias = "socklen_t")]
pub type SocketAddrLen = u32;

/// A trait abstracting over the types that can be passed as a `sockaddr`.
///
/// # Safety
///
/// Implementers of this trait must ensure that `with_sockaddr` calls `f` with
/// a pointer that is readable for the passed length, and points to data that
/// is a valid socket address for the system calls that accept `sockaddr` as a
/// const pointer.
pub unsafe trait SocketAddrArg {
    /// Call a closure with the pointer and length to the corresponding C type.
    ///
    /// The memory pointed to by the pointer of size length is guaranteed to be
    /// valid only for the duration of the call.
    ///
    /// The API uses a closure so that:
    ///  - The libc types are not exposed in the rustix API.
    ///  - Types like `SocketAddrUnix` that contain their corresponding C type
    ///    can pass it directly without a copy.
    ///  - Other socket types can construct their C-compatible struct on the
    ///    stack and call the closure with a pointer to it.
    ///
    /// # Safety
    ///
    /// For `f` to use its pointer argument, it'll contain an `unsafe` block.
    /// The caller of `with_sockaddr` here is responsible for ensuring that the
    /// safety condition for that `unsafe` block is satisfied by the guarantee
    /// that `with_sockaddr` here provides.
    unsafe fn with_sockaddr<R>(
        &self,
        f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
    ) -> R;

    /// Convert to `SocketAddrAny`.
    fn as_any(&self) -> SocketAddrAny {
        let mut storage = MaybeUninit::<SocketAddrStorage>::uninit();
        // SAFETY: We've allocated `storage` here, we're writing to it, and
        // we're using the number of bytes written.
        unsafe {
            let len = self.write_sockaddr(storage.as_mut_ptr());
            SocketAddrAny::new(storage, len)
        }
    }

    /// Encode an address into a `SocketAddrStorage`.
    ///
    /// Returns the number of bytes that were written.
    ///
    /// For a safe interface to this functionality, use [`as_any`].
    ///
    /// [`as_any`]: Self::as_any
    ///
    /// # Safety
    ///
    /// `storage` must be valid to write up to `size_of<SocketAddrStorage>()`
    /// bytes to.
    unsafe fn write_sockaddr(&self, storage: *mut SocketAddrStorage) -> SocketAddrLen {
        // The closure dereferences exactly `len` bytes at `ptr`.
        self.with_sockaddr(|ptr, len| {
            ptr::copy_nonoverlapping(ptr.cast::<u8>(), storage.cast::<u8>(), len as usize);
            len
        })
    }
}

/// Helper for implementing `SocketAddrArg::with_sockaddr`.
///
/// # Safety
///
/// This calls `f` with a pointer to an object it has a reference to, with the
/// and the length of that object, so they'll be valid for the duration of the
/// call.
pub(crate) unsafe fn call_with_sockaddr<A, R>(
    addr: &A,
    f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
) -> R {
    let ptr = as_ptr(addr).cast();
    let len = size_of::<A>() as SocketAddrLen;
    f(ptr, len)
}

// SAFETY: This just forwards to the inner `SocketAddrArg` implementations.
unsafe impl SocketAddrArg for SocketAddr {
    unsafe fn with_sockaddr<R>(
        &self,
        f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
    ) -> R {
        match self {
            Self::V4(v4) => v4.with_sockaddr(f),
            Self::V6(v6) => v6.with_sockaddr(f),
        }
    }
}

// SAFETY: `with_sockaddr` calls `f` using `call_with_sockaddr`, which handles
// calling `f` with the needed preconditions.
unsafe impl SocketAddrArg for SocketAddrV4 {
    unsafe fn with_sockaddr<R>(
        &self,
        f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
    ) -> R {
        call_with_sockaddr(&encode_sockaddr_v4(self), f)
    }
}

// SAFETY: `with_sockaddr` calls `f` using `call_with_sockaddr`, which handles
// calling `f` with the needed preconditions.
unsafe impl SocketAddrArg for SocketAddrV6 {
    unsafe fn with_sockaddr<R>(
        &self,
        f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
    ) -> R {
        call_with_sockaddr(&encode_sockaddr_v6(self), f)
    }
}

#[cfg(unix)]
// SAFETY: `with_sockaddr` calls `f` using `call_with_sockaddr`, which handles
// calling `f` with the needed preconditions.
unsafe impl SocketAddrArg for SocketAddrUnix {
    unsafe fn with_sockaddr<R>(
        &self,
        f: impl FnOnce(*const SocketAddrOpaque, SocketAddrLen) -> R,
    ) -> R {
        f(as_ptr(&self.unix).cast(), self.addr_len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::c;

    #[test]
    fn test_layouts() {
        assert_eq_size!(SocketAddrLen, c::socklen_t);

        #[cfg(not(any(windows, target_os = "redox")))]
        assert_eq!(
            memoffset::span_of!(c::msghdr, msg_namelen).len(),
            size_of::<SocketAddrLen>()
        );

        assert!(size_of::<SocketAddrLen>() <= size_of::<usize>());
    }
}
