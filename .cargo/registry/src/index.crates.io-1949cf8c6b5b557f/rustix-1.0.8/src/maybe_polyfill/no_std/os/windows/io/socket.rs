//! The following is derived from Rust's
//! library/std/src/os/windows/io/socket.rs
//! at revision
//! 4f9b394c8a24803e57ba892fa00e539742ebafc0.
//!
//! All code in this file is licensed MIT or Apache 2.0 at your option.

use super::raw::*;
use crate::backend::c;
use core::fmt;
use core::marker::PhantomData;
use core::mem::forget;

/// A borrowed socket.
///
/// This has a lifetime parameter to tie it to the lifetime of something that
/// owns the socket.
///
/// This uses `repr(transparent)` and has the representation of a host socket,
/// so it can be used in FFI in places where a socket is passed as an argument,
/// it is not captured or consumed, and it never has the value
/// `INVALID_SOCKET`.
///
/// This type's `.to_owned()` implementation returns another `BorrowedSocket`
/// rather than an `OwnedSocket`. It just makes a trivial copy of the raw
/// socket, which is then borrowed under the same lifetime.
#[derive(Copy, Clone)]
#[repr(transparent)]
#[cfg_attr(staged_api, rustc_layout_scalar_valid_range_start(0))]
// This is -2, in two's complement. -1 is `INVALID_SOCKET`.
#[cfg_attr(
    all(staged_api, target_pointer_width = "32"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE)
)]
#[cfg_attr(
    all(staged_api, target_pointer_width = "64"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FF_FF_FF_FF_FE)
)]
#[cfg_attr(staged_api, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
pub struct BorrowedSocket<'socket> {
    socket: RawSocket,
    _phantom: PhantomData<&'socket OwnedSocket>,
}

/// An owned socket.
///
/// This closes the socket on drop.
///
/// This uses `repr(transparent)` and has the representation of a host socket,
/// so it can be used in FFI in places where a socket is passed as a consumed
/// argument or returned as an owned value, and it never has the value
/// `INVALID_SOCKET`.
#[repr(transparent)]
#[cfg_attr(staged_api, rustc_layout_scalar_valid_range_start(0))]
// This is -2, in two's complement. -1 is `INVALID_SOCKET`.
#[cfg_attr(
    all(staged_api, target_pointer_width = "32"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE)
)]
#[cfg_attr(
    all(staged_api, target_pointer_width = "64"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FF_FF_FF_FF_FE)
)]
#[cfg_attr(staged_api, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
pub struct OwnedSocket {
    socket: RawSocket,
}

impl BorrowedSocket<'_> {
    /// Returns a `BorrowedSocket` holding the given raw socket.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `raw` must remain open for the duration of
    /// the returned `BorrowedSocket`, and it must not have the value
    /// `INVALID_SOCKET`.
    #[inline]
    #[cfg_attr(
        staged_api,
        rustc_const_stable(feature = "io_safety", since = "1.63.0")
    )]
    #[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
    pub const unsafe fn borrow_raw(socket: RawSocket) -> Self {
        assert!(socket != c::INVALID_SOCKET as RawSocket);
        Self {
            socket,
            _phantom: PhantomData,
        }
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl AsRawSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.socket
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl AsRawSocket for OwnedSocket {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.socket
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl IntoRawSocket for OwnedSocket {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        let socket = self.socket;
        forget(self);
        socket
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl FromRawSocket for OwnedSocket {
    #[inline]
    unsafe fn from_raw_socket(socket: RawSocket) -> Self {
        debug_assert_ne!(socket, c::INVALID_SOCKET as RawSocket);
        Self { socket }
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl Drop for OwnedSocket {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let _ = c::closesocket(self.socket as c::SOCKET);
        }
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl fmt::Debug for BorrowedSocket<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedSocket")
            .field("socket", &self.socket)
            .finish()
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl fmt::Debug for OwnedSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedSocket")
            .field("socket", &self.socket)
            .finish()
    }
}

/// A trait to borrow the socket from an underlying object.
#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
pub trait AsSocket {
    /// Borrows the socket.
    #[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
    fn as_socket(&self) -> BorrowedSocket<'_>;
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl<T: AsSocket> AsSocket for &T {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        T::as_socket(self)
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl<T: AsSocket> AsSocket for &mut T {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        T::as_socket(self)
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl AsSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        *self
    }
}

#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
impl AsSocket for OwnedSocket {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        // Safety: `OwnedSocket` and `BorrowedSocket` have the same validity
        // invariants, and the `BorrowdSocket` is bounded by the lifetime
        // of `&self`.
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}
