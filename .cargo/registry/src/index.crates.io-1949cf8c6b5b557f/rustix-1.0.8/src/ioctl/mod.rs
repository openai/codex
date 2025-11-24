//! Unsafe `ioctl` API.
//!
//! Unix systems expose a number of `ioctl`'s. `ioctl`s have been adopted as a
//! general purpose system call for making calls into the kernel. In addition
//! to the wide variety of system calls that are included by default in the
//! kernel, many drivers expose their own `ioctl`'s for controlling their
//! behavior, some of which are proprietary. Therefore it is impossible to make
//! a safe interface for every `ioctl` call, as they all have wildly varying
//! semantics.
//!
//! This module provides an unsafe interface to write your own `ioctl` API. To
//! start, create a type that implements [`Ioctl`]. Then, pass it to [`ioctl`]
//! to make the `ioctl` call.

#![allow(unsafe_code)]

use crate::fd::{AsFd, BorrowedFd};
use crate::ffi as c;
use crate::io::Result;

#[cfg(any(linux_kernel, bsd))]
use core::mem;

pub use patterns::*;

mod patterns;

#[cfg(linux_kernel)]
mod linux;

#[cfg(bsd)]
mod bsd;

#[cfg(linux_kernel)]
use linux as platform;

#[cfg(bsd)]
use bsd as platform;

/// Perform an `ioctl` call.
///
/// `ioctl` was originally intended to act as a way of modifying the behavior
/// of files, but has since been adopted as a general purpose system call for
/// making calls into the kernel. In addition to the default calls exposed by
/// generic file descriptors, many drivers expose their own `ioctl` calls for
/// controlling their behavior, some of which are proprietary.
///
/// This crate exposes many other `ioctl` interfaces with safe and idiomatic
/// wrappers, like [`ioctl_fionbio`] and [`ioctl_fionread`]. It is recommended
/// to use those instead of this function, as they are safer and more
/// idiomatic. For other cases, implement the [`Ioctl`] API and pass it to this
/// function.
///
/// See documentation for [`Ioctl`] for more information.
///
/// [`ioctl_fionbio`]: crate::io::ioctl_fionbio
/// [`ioctl_fionread`]: crate::io::ioctl_fionread
///
/// # Safety
///
/// While [`Ioctl`] takes much of the unsafety out of `ioctl` calls, callers
/// must still ensure that the opcode value, operand type, and data access
/// correctly reflect what's in the device driver servicing the call. `ioctl`
/// calls form a protocol between the userspace `ioctl` callers and the device
/// drivers in the kernel, and safety depends on both sides agreeing and
/// upholding the expectations of the other.
///
/// And, `ioctl` calls can read and write arbitrary memory and have arbitrary
/// side effects. Callers must ensure that any memory accesses and side effects
/// are compatible with Rust language invariants.
///
/// # References
///  - [Linux]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [Apple]
///  - [Solaris]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/ioctl.2.html
/// [Winsock]: https://learn.microsoft.com/en-us/windows/win32/api/winsock/nf-winsock-ioctlsocket
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=ioctl&sektion=2
/// [NetBSD]: https://man.netbsd.org/ioctl.2
/// [OpenBSD]: https://man.openbsd.org/ioctl.2
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/ioctl.2.html
/// [Solaris]: https://docs.oracle.com/cd/E23824_01/html/821-1463/ioctl-2.html
/// [illumos]: https://illumos.org/man/2/ioctl
#[inline]
pub unsafe fn ioctl<F: AsFd, I: Ioctl>(fd: F, mut ioctl: I) -> Result<I::Output> {
    let fd = fd.as_fd();
    let request = ioctl.opcode();
    let arg = ioctl.as_ptr();

    // SAFETY: The variant of `Ioctl` asserts that this is a valid IOCTL call
    // to make.
    let output = if I::IS_MUTATING {
        _ioctl(fd, request, arg)?
    } else {
        _ioctl_readonly(fd, request, arg)?
    };

    // SAFETY: The variant of `Ioctl` asserts that this is a valid pointer to
    // the output data.
    I::output_from_ptr(output, arg)
}

unsafe fn _ioctl(fd: BorrowedFd<'_>, request: Opcode, arg: *mut c::c_void) -> Result<IoctlOutput> {
    crate::backend::io::syscalls::ioctl(fd, request, arg)
}

unsafe fn _ioctl_readonly(
    fd: BorrowedFd<'_>,
    request: Opcode,
    arg: *mut c::c_void,
) -> Result<IoctlOutput> {
    crate::backend::io::syscalls::ioctl_readonly(fd, request, arg)
}

/// A trait defining the properties of an `ioctl` command.
///
/// Objects implementing this trait can be passed to [`ioctl`] to make an
/// `ioctl` call. The contents of the object represent the inputs to the
/// `ioctl` call. The inputs must be convertible to a pointer through the
/// `as_ptr` method. In most cases, this involves either casting a number to a
/// pointer, or creating a pointer to the actual data. The latter case is
/// necessary for `ioctl` calls that modify userspace data.
///
/// # Safety
///
/// This trait is unsafe to implement because it is impossible to guarantee
/// that the `ioctl` call is safe. The `ioctl` call may be proprietary, or it
/// may be unsafe to call in certain circumstances.
///
/// By implementing this trait, you guarantee that:
///
///  - The `ioctl` call expects the input provided by `as_ptr` and produces the
///    output as indicated by `output`.
///  - That `output_from_ptr` can safely take the pointer from `as_ptr` and
///    cast it to the correct type, *only* after the `ioctl` call.
///  - That the return value of `opcode` uniquely identifies the `ioctl` call.
///  - That, for whatever platforms you are targeting, the `ioctl` call is safe
///    to make.
///  - If `IS_MUTATING` is false, that no userspace data will be modified by
///    the `ioctl` call.
pub unsafe trait Ioctl {
    /// The type of the output data.
    ///
    /// Given a pointer, one should be able to construct an instance of this
    /// type.
    type Output;

    /// Does the `ioctl` mutate any data in the userspace?
    ///
    /// If the `ioctl` call does not mutate any data in the userspace, then
    /// making this `false` enables optimizations that can make the call
    /// faster. When in doubt, set this to `true`.
    ///
    /// # Safety
    ///
    /// This should only be set to `false` if the `ioctl` call does not mutate
    /// any data in the userspace. Undefined behavior may occur if this is set
    /// to `false` when it should be `true`.
    const IS_MUTATING: bool;

    /// Get the opcode used by this `ioctl` command.
    ///
    /// There are different types of opcode depending on the operation. See
    /// documentation for [`opcode`] for more information.
    fn opcode(&self) -> Opcode;

    /// Get a pointer to the data to be passed to the `ioctl` command.
    ///
    /// See trait-level documentation for more information.
    fn as_ptr(&mut self) -> *mut c::c_void;

    /// Cast the output data to the correct type.
    ///
    /// # Safety
    ///
    /// The `extract_output` value must be the resulting value after a
    /// successful `ioctl` call, and `out` is the direct return value of an
    /// `ioctl` call that did not fail. In this case `extract_output` is the
    /// pointer that was passed to the `ioctl` call.
    unsafe fn output_from_ptr(
        out: IoctlOutput,
        extract_output: *mut c::c_void,
    ) -> Result<Self::Output>;
}

/// Const functions for computing opcode values.
///
/// Linux's headers define macros such as `_IO`, `_IOR`, `_IOW`, and `_IOWR`
/// for defining ioctl values in a structured way that encode whether they
/// are reading and/or writing, and other information about the ioctl. The
/// functions in this module correspond to those macros.
///
/// If you're writing a driver and defining your own ioctl numbers, it's
/// recommended to use these functions to compute them.
#[cfg(any(linux_kernel, bsd))]
pub mod opcode {
    use super::*;

    /// Create a new opcode from a direction, group, number, and size.
    ///
    /// This corresponds to the C macro `_IOC(direction, group, number, size)`
    #[doc(alias = "_IOC")]
    #[inline]
    pub const fn from_components(
        direction: Direction,
        group: u8,
        number: u8,
        data_size: usize,
    ) -> Opcode {
        assert!(data_size <= Opcode::MAX as usize, "data size is too large");

        platform::compose_opcode(
            direction,
            group as Opcode,
            number as Opcode,
            data_size as Opcode,
        )
    }

    /// Create a new opcode from a group, a number, that uses no data.
    ///
    /// This corresponds to the C macro `_IO(group, number)`.
    #[doc(alias = "_IO")]
    #[inline]
    pub const fn none(group: u8, number: u8) -> Opcode {
        from_components(Direction::None, group, number, 0)
    }

    /// Create a new reading opcode from a group, a number and the type of
    /// data.
    ///
    /// This corresponds to the C macro `_IOR(group, number, T)`.
    #[doc(alias = "_IOR")]
    #[inline]
    pub const fn read<T>(group: u8, number: u8) -> Opcode {
        from_components(Direction::Read, group, number, mem::size_of::<T>())
    }

    /// Create a new writing opcode from a group, a number and the type of
    /// data.
    ///
    /// This corresponds to the C macro `_IOW(group, number, T)`.
    #[doc(alias = "_IOW")]
    #[inline]
    pub const fn write<T>(group: u8, number: u8) -> Opcode {
        from_components(Direction::Write, group, number, mem::size_of::<T>())
    }

    /// Create a new reading and writing opcode from a group, a number and the
    /// type of data.
    ///
    /// This corresponds to the C macro `_IOWR(group, number, T)`.
    #[doc(alias = "_IOWR")]
    #[inline]
    pub const fn read_write<T>(group: u8, number: u8) -> Opcode {
        from_components(Direction::ReadWrite, group, number, mem::size_of::<T>())
    }
}

/// The direction that an `ioctl` is going.
///
/// The direction is relative to userspace: `Read` means reading data from the
/// kernel, and `Write` means the kernel writing data to userspace.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// None of the above.
    None,

    /// Read data from the kernel.
    Read,

    /// Write data to the kernel.
    Write,

    /// Read and write data to the kernel.
    ReadWrite,
}

/// The type used by the `ioctl` to signify the output.
pub type IoctlOutput = c::c_int;

/// The type used by the `ioctl` to signify the command.
pub type Opcode = _Opcode;

// Under raw Linux, this is an `unsigned int`.
#[cfg(linux_raw)]
type _Opcode = c::c_uint;

// On libc Linux with GNU libc or uclibc, this is an `unsigned long`.
#[cfg(all(
    not(linux_raw),
    target_os = "linux",
    any(target_env = "gnu", target_env = "uclibc")
))]
type _Opcode = c::c_ulong;

// Musl uses `c_int`.
#[cfg(all(
    not(linux_raw),
    target_os = "linux",
    not(target_env = "gnu"),
    not(target_env = "uclibc")
))]
type _Opcode = c::c_int;

// Android uses `c_int`.
#[cfg(all(not(linux_raw), target_os = "android"))]
type _Opcode = c::c_int;

// BSD, Haiku, Hurd, Redox, and Vita use `unsigned long`.
#[cfg(any(
    bsd,
    target_os = "redox",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "hurd",
    target_os = "vita"
))]
type _Opcode = c::c_ulong;

// AIX, Emscripten, Fuchsia, Solaris, and WASI use a `int`.
#[cfg(any(
    solarish,
    target_os = "aix",
    target_os = "cygwin",
    target_os = "fuchsia",
    target_os = "emscripten",
    target_os = "nto",
    target_os = "wasi",
))]
type _Opcode = c::c_int;

// ESP-IDF uses a `c_uint`.
#[cfg(target_os = "espidf")]
type _Opcode = c::c_uint;

// Windows has `ioctlsocket`, which uses `i32`.
#[cfg(windows)]
type _Opcode = i32;

#[cfg(linux_kernel)]
#[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_funcs() {
        // `TUNGETDEVNETNS` is defined as `_IO('T', 227)`.
        assert_eq!(
            linux_raw_sys::ioctl::TUNGETDEVNETNS as Opcode,
            opcode::none(b'T', 227)
        );
        // `FS_IOC_GETVERSION` is defined as `_IOR('v', 1, long)`.
        assert_eq!(
            linux_raw_sys::ioctl::FS_IOC_GETVERSION as Opcode,
            opcode::read::<c::c_long>(b'v', 1)
        );
        // `TUNSETNOCSUM` is defined as `_IOW('T', 200, int)`.
        assert_eq!(
            linux_raw_sys::ioctl::TUNSETNOCSUM as Opcode,
            opcode::write::<c::c_int>(b'T', 200)
        );
        // `FIFREEZE` is defined as `_IOWR('X', 119, int)`.
        assert_eq!(
            linux_raw_sys::ioctl::FIFREEZE as Opcode,
            opcode::read_write::<c::c_int>(b'X', 119)
        );
    }
}
