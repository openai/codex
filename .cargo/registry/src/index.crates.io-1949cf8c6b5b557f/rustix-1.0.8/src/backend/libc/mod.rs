//! The libc backend.
//!
//! On most platforms, this uses the `libc` crate to make system calls. On
//! Windows, this uses the Winsock API in `windows-sys`, which can be adapted
//! to have a very `libc`-like interface.

// Every FFI call requires an unsafe block, and there are a lot of FFI
// calls. For now, set this to allow for the libc backend.
#![allow(clippy::undocumented_unsafe_blocks)]
// Lots of libc types vary between platforms, so we often need a `.into()` on
// one platform where it's redundant on another.
#![allow(clippy::useless_conversion)]

mod conv;

#[cfg(windows)]
pub(crate) mod fd {
    // Re-export `AsSocket` etc. too, as users can't implement `AsFd` etc. on
    // Windows due to them having blanket impls on Windows, so users must
    // implement `AsSocket` etc.
    pub use crate::maybe_polyfill::os::windows::io::{
        AsRawSocket, AsSocket, BorrowedSocket as BorrowedFd, FromRawSocket, IntoRawSocket,
        OwnedSocket as OwnedFd, RawSocket as RawFd,
    };
    pub(crate) use windows_sys::Win32::Networking::WinSock::SOCKET as LibcFd;

    /// A version of [`AsRawFd`] for use with Winsock API.
    ///
    /// [`AsRawFd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.AsRawFd.html
    pub trait AsRawFd {
        /// A version of [`as_raw_fd`] for use with Winsock API.
        ///
        /// [`as_raw_fd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.AsRawFd.html#tymethod.as_raw_fd
        fn as_raw_fd(&self) -> RawFd;
    }
    impl<T: AsRawSocket> AsRawFd for T {
        #[inline]
        fn as_raw_fd(&self) -> RawFd {
            self.as_raw_socket()
        }
    }

    /// A version of [`IntoRawFd`] for use with Winsock API.
    ///
    /// [`IntoRawFd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.IntoRawFd.html
    pub trait IntoRawFd {
        /// A version of [`into_raw_fd`] for use with Winsock API.
        ///
        /// [`into_raw_fd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.IntoRawFd.html#tymethod.into_raw_fd
        fn into_raw_fd(self) -> RawFd;
    }
    impl<T: IntoRawSocket> IntoRawFd for T {
        #[inline]
        fn into_raw_fd(self) -> RawFd {
            self.into_raw_socket()
        }
    }

    /// A version of [`FromRawFd`] for use with Winsock API.
    ///
    /// [`FromRawFd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.FromRawFd.html
    pub trait FromRawFd {
        /// A version of [`from_raw_fd`] for use with Winsock API.
        ///
        /// # Safety
        ///
        /// See the [safety requirements] for [`from_raw_fd`].
        ///
        /// [`from_raw_fd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.FromRawFd.html#tymethod.from_raw_fd
        /// [safety requirements]: https://doc.rust-lang.org/stable/std/os/fd/trait.FromRawFd.html#safety
        unsafe fn from_raw_fd(raw_fd: RawFd) -> Self;
    }
    impl<T: FromRawSocket> FromRawFd for T {
        #[inline]
        unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
            Self::from_raw_socket(raw_fd)
        }
    }

    /// A version of [`AsFd`] for use with Winsock API.
    ///
    /// [`AsFd`]: https://doc.rust-lang.org/stable/std/os/fd/trait.AsFd.html
    pub trait AsFd {
        /// An `as_fd` function for Winsock, where an `Fd` is a `Socket`.
        fn as_fd(&self) -> BorrowedFd<'_>;
    }
    impl<T: AsSocket> AsFd for T {
        #[inline]
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.as_socket()
        }
    }
}
#[cfg(not(windows))]
pub(crate) mod fd {
    pub use crate::maybe_polyfill::os::fd::*;
    #[allow(unused_imports)]
    pub(crate) use RawFd as LibcFd;
}

// On Windows we emulate selected libc-compatible interfaces. On non-Windows,
// we just use libc here, since this is the libc backend.
#[cfg_attr(windows, path = "winsock_c.rs")]
pub(crate) mod c;

#[cfg(feature = "event")]
pub(crate) mod event;
#[cfg(not(windows))]
#[cfg(feature = "fs")]
pub(crate) mod fs;
pub(crate) mod io;
#[cfg(linux_kernel)]
#[cfg(feature = "io_uring")]
pub(crate) mod io_uring;
#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "wasi"
)))]
#[cfg(feature = "mm")]
pub(crate) mod mm;
#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) mod mount;
#[cfg(not(target_os = "wasi"))]
#[cfg(feature = "net")]
pub(crate) mod net;
#[cfg(not(any(windows, target_os = "espidf")))]
#[cfg(any(
    feature = "param",
    feature = "runtime",
    feature = "time",
    target_arch = "x86",
))]
pub(crate) mod param;
#[cfg(not(windows))]
#[cfg(feature = "pipe")]
pub(crate) mod pipe;
#[cfg(not(windows))]
#[cfg(feature = "process")]
pub(crate) mod process;
#[cfg(not(windows))]
#[cfg(not(target_os = "wasi"))]
#[cfg(feature = "pty")]
pub(crate) mod pty;
#[cfg(not(windows))]
#[cfg(feature = "rand")]
pub(crate) mod rand;
#[cfg(not(windows))]
#[cfg(not(target_os = "wasi"))]
#[cfg(feature = "system")]
pub(crate) mod system;
#[cfg(not(any(windows, target_os = "horizon", target_os = "vita")))]
#[cfg(feature = "termios")]
pub(crate) mod termios;
#[cfg(not(windows))]
#[cfg(feature = "thread")]
pub(crate) mod thread;
#[cfg(not(any(windows, target_os = "espidf")))]
#[cfg(feature = "time")]
pub(crate) mod time;

/// If the host libc is glibc, return `true` if it is less than version 2.25.
///
/// To restate and clarify, this function returning true does not mean the libc
/// is glibc just that if it is glibc, it is less than version 2.25.
///
/// For now, this function is only available on Linux, but if it ends up being
/// used beyond that, this could be changed to e.g. `#[cfg(unix)]`.
#[cfg(all(unix, target_env = "gnu"))]
pub(crate) fn if_glibc_is_less_than_2_25() -> bool {
    // This is also defined inside `weak_or_syscall!` in
    // backend/libc/rand/syscalls.rs, but it's not convenient to re-export the
    // weak symbol from that macro, so we duplicate it at a small cost here.
    weak! { fn getrandom(*mut c::c_void, c::size_t, c::c_uint) -> c::ssize_t }

    // glibc 2.25 has `getrandom`, which is how we satisfy the API contract of
    // this function. But, there are likely other libc versions which have it.
    getrandom.get().is_none()
}

// Private modules used by multiple public modules.
#[cfg(any(feature = "process", feature = "runtime"))]
#[cfg(not(any(windows, target_os = "wasi")))]
pub(crate) mod pid;
#[cfg(any(feature = "process", feature = "thread"))]
#[cfg(linux_kernel)]
pub(crate) mod prctl;
#[cfg(not(any(
    windows,
    target_os = "android",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "wasi"
)))]
#[cfg(feature = "shm")]
pub(crate) mod shm;
#[cfg(any(feature = "fs", feature = "thread", feature = "process"))]
#[cfg(not(any(windows, target_os = "wasi")))]
pub(crate) mod ugid;

#[cfg(bsd)]
const MAX_IOV: usize = c::IOV_MAX as usize;

#[cfg(any(linux_kernel, target_os = "emscripten", target_os = "nto"))]
const MAX_IOV: usize = c::UIO_MAXIOV as usize;

#[cfg(not(any(
    bsd,
    linux_kernel,
    windows,
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "nto",
    target_os = "horizon",
)))]
const MAX_IOV: usize = 16; // The minimum value required by POSIX.
