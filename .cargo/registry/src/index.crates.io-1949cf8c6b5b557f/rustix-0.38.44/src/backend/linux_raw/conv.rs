//! Convert values to [`ArgReg`] and from [`RetReg`].
//!
//! System call arguments and return values are all communicated with inline
//! asm and FFI as `*mut Opaque`. To protect these raw pointers from escaping
//! or being accidentally misused as they travel through the code, we wrap them
//! in [`ArgReg`] and [`RetReg`] structs. This file provides `From`
//! implementations and explicit conversion functions for converting values
//! into and out of these wrapper structs.
//!
//! # Safety
//!
//! Some of this code is `unsafe` in order to work with raw file descriptors,
//! and some is `unsafe` to interpret the values in a `RetReg`.
#![allow(unsafe_code)]

use super::c;
use super::fd::{AsRawFd, BorrowedFd, FromRawFd, RawFd};
#[cfg(any(feature = "event", feature = "runtime", feature = "system"))]
use super::io::errno::try_decode_error;
#[cfg(target_pointer_width = "64")]
use super::io::errno::try_decode_u64;
#[cfg(not(debug_assertions))]
use super::io::errno::{
    decode_c_int_infallible, decode_c_uint_infallible, decode_usize_infallible,
};
use super::io::errno::{
    try_decode_c_int, try_decode_c_uint, try_decode_raw_fd, try_decode_usize, try_decode_void,
    try_decode_void_star,
};
use super::reg::{raw_arg, ArgNumber, ArgReg, RetReg, R0};
#[cfg(feature = "time")]
use super::time::types::TimerfdClockId;
#[cfg(any(feature = "thread", feature = "time", target_arch = "x86"))]
use crate::clockid::ClockId;
use crate::fd::OwnedFd;
use crate::ffi::CStr;
use crate::io;
#[cfg(any(feature = "process", feature = "runtime", feature = "termios"))]
use crate::pid::Pid;
#[cfg(feature = "process")]
use crate::process::Resource;
#[cfg(any(feature = "process", feature = "runtime"))]
use crate::signal::Signal;
use crate::utils::{as_mut_ptr, as_ptr};
use core::mem::MaybeUninit;
use core::ptr::null_mut;
#[cfg(any(feature = "thread", feature = "time", target_arch = "x86"))]
use linux_raw_sys::general::__kernel_clockid_t;
#[cfg(target_pointer_width = "64")]
use linux_raw_sys::general::__kernel_loff_t;
#[cfg(feature = "net")]
use linux_raw_sys::net::socklen_t;

/// Convert `SYS_*` constants for socketcall.
#[cfg(target_arch = "x86")]
#[inline]
pub(super) fn x86_sys<'a, Num: ArgNumber>(sys: u32) -> ArgReg<'a, Num> {
    pass_usize(sys as usize)
}

/// Pass the "low" half of the endian-specific memory encoding of a `u64`, for
/// 32-bit architectures.
#[cfg(target_pointer_width = "32")]
#[inline]
pub(super) fn lo<'a, Num: ArgNumber>(x: u64) -> ArgReg<'a, Num> {
    #[cfg(target_endian = "little")]
    let x = x >> 32;
    #[cfg(target_endian = "big")]
    let x = x & 0xffff_ffff;

    pass_usize(x as usize)
}

/// Pass the "high" half of the endian-specific memory encoding of a `u64`, for
/// 32-bit architectures.
#[cfg(target_pointer_width = "32")]
#[inline]
pub(super) fn hi<'a, Num: ArgNumber>(x: u64) -> ArgReg<'a, Num> {
    #[cfg(target_endian = "little")]
    let x = x & 0xffff_ffff;
    #[cfg(target_endian = "big")]
    let x = x >> 32;

    pass_usize(x as usize)
}

/// Pass a zero, or null, argument.
#[inline]
pub(super) fn zero<'a, Num: ArgNumber>() -> ArgReg<'a, Num> {
    raw_arg(null_mut())
}

/// Pass the `mem::size_of` of a type.
#[inline]
pub(super) fn size_of<'a, T: Sized, Num: ArgNumber>() -> ArgReg<'a, Num> {
    pass_usize(core::mem::size_of::<T>())
}

/// Pass an arbitrary `usize` value.
///
/// For passing pointers, use `void_star` or other functions which take a raw
/// pointer instead of casting to `usize`, so that provenance is preserved.
#[inline]
pub(super) fn pass_usize<'a, Num: ArgNumber>(t: usize) -> ArgReg<'a, Num> {
    raw_arg(t as *mut _)
}

impl<'a, Num: ArgNumber, T> From<*mut T> for ArgReg<'a, Num> {
    #[inline]
    fn from(c: *mut T) -> Self {
        raw_arg(c.cast())
    }
}

impl<'a, Num: ArgNumber, T> From<*const T> for ArgReg<'a, Num> {
    #[inline]
    fn from(c: *const T) -> Self {
        let mut_ptr = c as *mut T;
        raw_arg(mut_ptr.cast())
    }
}

impl<'a, Num: ArgNumber> From<&'a CStr> for ArgReg<'a, Num> {
    #[inline]
    fn from(c: &'a CStr) -> Self {
        let mut_ptr = c.as_ptr() as *mut u8;
        raw_arg(mut_ptr.cast())
    }
}

impl<'a, Num: ArgNumber> From<Option<&'a CStr>> for ArgReg<'a, Num> {
    #[inline]
    fn from(t: Option<&'a CStr>) -> Self {
        raw_arg(match t {
            Some(s) => {
                let mut_ptr = s.as_ptr() as *mut u8;
                mut_ptr.cast()
            }
            None => null_mut(),
        })
    }
}

/// Pass a borrowed file-descriptor argument.
impl<'a, Num: ArgNumber> From<BorrowedFd<'a>> for ArgReg<'a, Num> {
    #[inline]
    fn from(fd: BorrowedFd<'a>) -> Self {
        // SAFETY: `BorrowedFd` ensures that the file descriptor is valid, and
        // the lifetime parameter on the resulting `ArgReg` ensures that the
        // result is bounded by the `BorrowedFd`'s lifetime.
        unsafe { raw_fd(fd.as_raw_fd()) }
    }
}

/// Pass a raw file-descriptor argument. Most users should use [`ArgReg::from`]
/// instead, to preserve I/O safety as long as possible.
///
/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[inline]
pub(super) unsafe fn raw_fd<'a, Num: ArgNumber>(fd: RawFd) -> ArgReg<'a, Num> {
    // Use `no_fd` when passing `-1` is intended.
    #[cfg(feature = "fs")]
    debug_assert!(fd == crate::fs::CWD.as_raw_fd() || fd == crate::fs::ABS.as_raw_fd() || fd >= 0);

    // Don't pass the `io_uring_register_files_skip` sentry value this way.
    #[cfg(feature = "io_uring")]
    debug_assert_ne!(
        fd,
        crate::io_uring::io_uring_register_files_skip().as_raw_fd()
    );

    // Linux doesn't look at the high bits beyond the `c_int`, so use
    // zero-extension rather than sign-extension because it's a smaller
    // instruction.
    let fd: c::c_int = fd;
    pass_usize(fd as c::c_uint as usize)
}

/// Deliberately pass `-1` to a file-descriptor argument, for system calls
/// like `mmap` where this indicates the argument is omitted.
#[inline]
pub(super) fn no_fd<'a, Num: ArgNumber>() -> ArgReg<'a, Num> {
    pass_usize(!0_usize)
}

#[inline]
pub(super) fn slice_just_addr<T: Sized, Num: ArgNumber>(v: &[T]) -> ArgReg<'_, Num> {
    let mut_ptr = v.as_ptr() as *mut T;
    raw_arg(mut_ptr.cast())
}

#[inline]
pub(super) fn slice_just_addr_mut<T: Sized, Num: ArgNumber>(v: &mut [T]) -> ArgReg<'_, Num> {
    raw_arg(v.as_mut_ptr().cast())
}

#[inline]
pub(super) fn slice<T: Sized, Num0: ArgNumber, Num1: ArgNumber>(
    v: &[T],
) -> (ArgReg<'_, Num0>, ArgReg<'_, Num1>) {
    (slice_just_addr(v), pass_usize(v.len()))
}

#[inline]
pub(super) fn slice_mut<T: Sized, Num0: ArgNumber, Num1: ArgNumber>(
    v: &mut [T],
) -> (ArgReg<'_, Num0>, ArgReg<'_, Num1>) {
    (raw_arg(v.as_mut_ptr().cast()), pass_usize(v.len()))
}

#[inline]
pub(super) fn by_ref<T: Sized, Num: ArgNumber>(t: &T) -> ArgReg<'_, Num> {
    let mut_ptr = as_ptr(t) as *mut T;
    raw_arg(mut_ptr.cast())
}

#[inline]
pub(super) fn by_mut<T: Sized, Num: ArgNumber>(t: &mut T) -> ArgReg<'_, Num> {
    raw_arg(as_mut_ptr(t).cast())
}

/// Convert an optional mutable reference into a `usize` for passing to a
/// syscall.
#[inline]
pub(super) fn opt_mut<T: Sized, Num: ArgNumber>(t: Option<&mut T>) -> ArgReg<'_, Num> {
    // This optimizes into the equivalent of `transmute(t)`, and has the
    // advantage of not requiring `unsafe`.
    match t {
        Some(t) => by_mut(t),
        None => raw_arg(null_mut()),
    }
}

/// Convert an optional immutable reference into a `usize` for passing to a
/// syscall.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
#[inline]
pub(super) fn opt_ref<T: Sized, Num: ArgNumber>(t: Option<&T>) -> ArgReg<'_, Num> {
    // This optimizes into the equivalent of `transmute(t)`, and has the
    // advantage of not requiring `unsafe`.
    match t {
        Some(t) => by_ref(t),
        None => raw_arg(null_mut()),
    }
}

/// Convert a `c_int` into an `ArgReg`.
///
/// Be sure to use `raw_fd` to pass `RawFd` values.
#[inline]
pub(super) fn c_int<'a, Num: ArgNumber>(i: c::c_int) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

/// Convert a `c_uint` into an `ArgReg`.
#[inline]
pub(super) fn c_uint<'a, Num: ArgNumber>(i: c::c_uint) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn loff_t<'a, Num: ArgNumber>(i: __kernel_loff_t) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn loff_t_from_u64<'a, Num: ArgNumber>(i: u64) -> ArgReg<'a, Num> {
    // `loff_t` is signed, but syscalls which expect `loff_t` return `EINVAL`
    // if it's outside the signed `i64` range, so we can silently cast.
    pass_usize(i as usize)
}

#[cfg(any(feature = "thread", feature = "time", target_arch = "x86"))]
impl<'a, Num: ArgNumber> From<ClockId> for ArgReg<'a, Num> {
    #[inline]
    fn from(i: ClockId) -> Self {
        pass_usize(i as __kernel_clockid_t as usize)
    }
}

#[cfg(feature = "time")]
impl<'a, Num: ArgNumber> From<TimerfdClockId> for ArgReg<'a, Num> {
    #[inline]
    fn from(i: TimerfdClockId) -> Self {
        pass_usize(i as __kernel_clockid_t as usize)
    }
}

#[cfg(feature = "net")]
#[inline]
pub(super) fn socklen_t<'a, Num: ArgNumber>(i: socklen_t) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[cfg(any(
    feature = "fs",
    all(
        not(feature = "use-libc-auxv"),
        not(feature = "use-explicitly-provided-auxv"),
        any(
            feature = "param",
            feature = "process",
            feature = "runtime",
            feature = "time",
            target_arch = "x86",
        )
    )
))]
pub(crate) mod fs {
    use super::*;
    use crate::fs::{FileType, Mode, OFlags};
    #[cfg(target_pointer_width = "32")]
    use linux_raw_sys::general::O_LARGEFILE;

    impl<'a, Num: ArgNumber> From<Mode> for ArgReg<'a, Num> {
        #[inline]
        fn from(mode: Mode) -> Self {
            pass_usize(mode.bits() as usize)
        }
    }

    impl<'a, Num: ArgNumber> From<(Mode, FileType)> for ArgReg<'a, Num> {
        #[inline]
        fn from(pair: (Mode, FileType)) -> Self {
            pass_usize(pair.0.as_raw_mode() as usize | pair.1.as_raw_mode() as usize)
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::AtFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::AtFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::XattrFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::XattrFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::inotify::CreateFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::inotify::CreateFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::inotify::WatchFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::inotify::WatchFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::MemfdFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::MemfdFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::RenameFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::RenameFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::StatxFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::StatxFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn oflags_bits(oflags: OFlags) -> c::c_uint {
        let mut bits = oflags.bits();
        // Add `O_LARGEFILE`, unless `O_PATH` is set, as Linux returns `EINVAL`
        // when both are set.
        if !oflags.contains(OFlags::PATH) {
            bits |= O_LARGEFILE;
        }
        bits
    }

    #[cfg(target_pointer_width = "64")]
    #[inline]
    const fn oflags_bits(oflags: OFlags) -> c::c_uint {
        oflags.bits()
    }

    impl<'a, Num: ArgNumber> From<OFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(oflags: OFlags) -> Self {
            pass_usize(oflags_bits(oflags) as usize)
        }
    }

    /// Convert an `OFlags` into a `u64` for use in the `open_how` struct.
    #[inline]
    pub(crate) fn oflags_for_open_how(oflags: OFlags) -> u64 {
        u64::from(oflags_bits(oflags))
    }

    impl<'a, Num: ArgNumber> From<crate::fs::FallocateFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::FallocateFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::Advice> for ArgReg<'a, Num> {
        #[inline]
        fn from(advice: crate::fs::Advice) -> Self {
            c_uint(advice as c::c_uint)
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::SealFlags> for ArgReg<'a, Num> {
        #[inline]
        fn from(flags: crate::fs::SealFlags) -> Self {
            c_uint(flags.bits())
        }
    }

    impl<'a, Num: ArgNumber> From<crate::fs::Access> for ArgReg<'a, Num> {
        #[inline]
        fn from(access: crate::fs::Access) -> Self {
            c_uint(access.bits())
        }
    }
}

#[cfg(any(feature = "fs", feature = "mount"))]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::MountFlagsArg> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::MountFlagsArg) -> Self {
        c_uint(flags.0)
    }
}

// When the deprecated "fs" aliases are removed, we can remove the "fs"
// here too.
#[cfg(any(feature = "fs", feature = "mount"))]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::UnmountFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::UnmountFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::mount::FsConfigCmd> for ArgReg<'a, Num> {
    #[inline]
    fn from(cmd: crate::mount::FsConfigCmd) -> Self {
        c_uint(cmd as c::c_uint)
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::FsOpenFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::FsOpenFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::FsMountFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::FsMountFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::MountAttrFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::MountAttrFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::OpenTreeFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::OpenTreeFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::FsPickFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::FsPickFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mount")]
impl<'a, Num: ArgNumber> From<crate::backend::mount::types::MoveMountFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mount::types::MoveMountFlags) -> Self {
        c_uint(flags.bits())
    }
}

impl<'a, Num: ArgNumber> From<crate::io::FdFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::io::FdFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "pipe")]
impl<'a, Num: ArgNumber> From<crate::pipe::PipeFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::pipe::PipeFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "pipe")]
impl<'a, Num: ArgNumber> From<crate::pipe::SpliceFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::pipe::SpliceFlags) -> Self {
        c_uint(flags.bits())
    }
}

impl<'a, Num: ArgNumber> From<crate::io::DupFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::io::DupFlags) -> Self {
        c_uint(flags.bits())
    }
}

impl<'a, Num: ArgNumber> From<crate::io::ReadWriteFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::io::ReadWriteFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "process")]
impl<'a, Num: ArgNumber> From<crate::process::PidfdFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::process::PidfdFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "pty")]
impl<'a, Num: ArgNumber> From<crate::pty::OpenptFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::pty::OpenptFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "thread")]
impl<'a, Num: ArgNumber> From<crate::thread::UnshareFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::thread::UnshareFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "event")]
impl<'a, Num: ArgNumber> From<crate::event::EventfdFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::event::EventfdFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "event")]
impl<'a, Num: ArgNumber> From<crate::event::epoll::CreateFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::event::epoll::CreateFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::ProtFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::ProtFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::MsyncFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::MsyncFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::MremapFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::MremapFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::MlockFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::MlockFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::MlockAllFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::MlockAllFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::MapFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::MapFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::MprotectFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::MprotectFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "mm")]
impl<'a, Num: ArgNumber> From<crate::backend::mm::types::UserfaultfdFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::backend::mm::types::UserfaultfdFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "process")]
impl<'a, Num: ArgNumber> From<crate::backend::process::types::MembarrierCommand>
    for ArgReg<'a, Num>
{
    #[inline]
    fn from(cmd: crate::backend::process::types::MembarrierCommand) -> Self {
        c_uint(cmd as u32)
    }
}

#[cfg(feature = "process")]
impl<'a, Num: ArgNumber> From<crate::process::Cpuid> for ArgReg<'a, Num> {
    #[inline]
    fn from(cpuid: crate::process::Cpuid) -> Self {
        c_uint(cpuid.as_raw())
    }
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn dev_t<'a, Num: ArgNumber>(dev: u64) -> ArgReg<'a, Num> {
    pass_usize(dev as usize)
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub(super) fn dev_t<'a, Num: ArgNumber>(dev: u64) -> io::Result<ArgReg<'a, Num>> {
    Ok(pass_usize(dev.try_into().map_err(|_err| io::Errno::INVAL)?))
}

/// Convert a `Resource` into a syscall argument.
#[cfg(feature = "process")]
impl<'a, Num: ArgNumber> From<Resource> for ArgReg<'a, Num> {
    #[inline]
    fn from(resource: Resource) -> Self {
        c_uint(resource as c::c_uint)
    }
}

#[cfg(any(feature = "process", feature = "runtime", feature = "termios"))]
impl<'a, Num: ArgNumber> From<Pid> for ArgReg<'a, Num> {
    #[inline]
    fn from(pid: Pid) -> Self {
        pass_usize(pid.as_raw_nonzero().get() as usize)
    }
}

#[cfg(feature = "process")]
#[inline]
pub(super) fn negative_pid<'a, Num: ArgNumber>(pid: Pid) -> ArgReg<'a, Num> {
    pass_usize(pid.as_raw_nonzero().get().wrapping_neg() as usize)
}

#[cfg(any(feature = "process", feature = "runtime"))]
impl<'a, Num: ArgNumber> From<Signal> for ArgReg<'a, Num> {
    #[inline]
    fn from(sig: Signal) -> Self {
        pass_usize(sig as usize)
    }
}

#[cfg(feature = "io_uring")]
impl<'a, Num: ArgNumber> From<crate::io_uring::IoringEnterFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::io_uring::IoringEnterFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "time")]
impl<'a, Num: ArgNumber> From<crate::time::TimerfdFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::time::TimerfdFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "time")]
impl<'a, Num: ArgNumber> From<crate::time::TimerfdTimerFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::time::TimerfdTimerFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "rand")]
impl<'a, Num: ArgNumber> From<crate::rand::GetRandomFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::rand::GetRandomFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<crate::net::RecvFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::net::RecvFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<crate::net::SendFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::net::SendFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<crate::net::SocketFlags> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::net::SocketFlags) -> Self {
        c_uint(flags.bits())
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<crate::net::AddressFamily> for ArgReg<'a, Num> {
    #[inline]
    fn from(family: crate::net::AddressFamily) -> Self {
        c_uint(family.0.into())
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<(crate::net::SocketType, crate::net::SocketFlags)>
    for ArgReg<'a, Num>
{
    #[inline]
    fn from(pair: (crate::net::SocketType, crate::net::SocketFlags)) -> Self {
        c_uint(pair.0 .0 | pair.1.bits())
    }
}

#[cfg(feature = "thread")]
impl<'a, Num: ArgNumber>
    From<(
        crate::backend::thread::futex::Operation,
        crate::thread::futex::Flags,
    )> for ArgReg<'a, Num>
{
    #[inline]
    fn from(
        pair: (
            crate::backend::thread::futex::Operation,
            crate::thread::futex::Flags,
        ),
    ) -> Self {
        c_uint(pair.0 as u32 | pair.1.bits())
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<crate::net::SocketType> for ArgReg<'a, Num> {
    #[inline]
    fn from(type_: crate::net::SocketType) -> Self {
        c_uint(type_.0)
    }
}

#[cfg(feature = "net")]
impl<'a, Num: ArgNumber> From<Option<crate::net::Protocol>> for ArgReg<'a, Num> {
    #[inline]
    fn from(protocol: Option<crate::net::Protocol>) -> Self {
        c_uint(match protocol {
            Some(p) => p.0.get(),
            None => 0,
        })
    }
}

impl<'a, Num: ArgNumber, T> From<&'a mut MaybeUninit<T>> for ArgReg<'a, Num> {
    #[inline]
    fn from(t: &'a mut MaybeUninit<T>) -> Self {
        raw_arg(t.as_mut_ptr().cast())
    }
}

impl<'a, Num: ArgNumber, T> From<&'a mut [MaybeUninit<T>]> for ArgReg<'a, Num> {
    #[inline]
    fn from(t: &'a mut [MaybeUninit<T>]) -> Self {
        raw_arg(t.as_mut_ptr().cast())
    }
}

#[cfg(any(feature = "process", feature = "thread"))]
impl<'a, Num: ArgNumber> From<crate::ugid::Uid> for ArgReg<'a, Num> {
    #[inline]
    fn from(t: crate::ugid::Uid) -> Self {
        c_uint(t.as_raw())
    }
}

#[cfg(any(feature = "process", feature = "thread"))]
impl<'a, Num: ArgNumber> From<crate::ugid::Gid> for ArgReg<'a, Num> {
    #[inline]
    fn from(t: crate::ugid::Gid) -> Self {
        c_uint(t.as_raw())
    }
}

#[cfg(feature = "runtime")]
impl<'a, Num: ArgNumber> From<crate::runtime::How> for ArgReg<'a, Num> {
    #[inline]
    fn from(flags: crate::runtime::How) -> Self {
        c_uint(flags as u32)
    }
}

/// Convert a `usize` returned from a syscall that effectively returns `()` on
/// success.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// just returns 0 on success.
#[inline]
pub(super) unsafe fn ret(raw: RetReg<R0>) -> io::Result<()> {
    try_decode_void(raw)
}

/// Convert a `usize` returned from a syscall that doesn't return on success.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// doesn't return on success.
#[cfg(any(feature = "event", feature = "runtime", feature = "system"))]
#[inline]
pub(super) unsafe fn ret_error(raw: RetReg<R0>) -> io::Errno {
    try_decode_error(raw)
}

/// Convert a `usize` returned from a syscall that effectively always returns
/// `()`.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// always returns `()`.
#[inline]
pub(super) unsafe fn ret_infallible(raw: RetReg<R0>) {
    #[cfg(debug_assertions)]
    {
        try_decode_void(raw).unwrap()
    }
    #[cfg(not(debug_assertions))]
    drop(raw);
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `c_int` on success.
#[inline]
pub(super) fn ret_c_int(raw: RetReg<R0>) -> io::Result<c::c_int> {
    try_decode_c_int(raw)
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `c_uint` on success.
#[inline]
pub(super) fn ret_c_uint(raw: RetReg<R0>) -> io::Result<c::c_uint> {
    try_decode_c_uint(raw)
}

/// Convert a `usize` returned from a syscall that effectively returns a `u64`
/// on success.
#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn ret_u64(raw: RetReg<R0>) -> io::Result<u64> {
    try_decode_u64(raw)
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `usize` on success.
#[inline]
pub(super) fn ret_usize(raw: RetReg<R0>) -> io::Result<usize> {
    try_decode_usize(raw)
}

/// Convert a `usize` returned from a syscall that effectively always
/// returns a `usize`.
///
/// # Safety
///
/// This function must only be used with return values from infallible
/// syscalls.
#[inline]
pub(super) unsafe fn ret_usize_infallible(raw: RetReg<R0>) -> usize {
    #[cfg(debug_assertions)]
    {
        try_decode_usize(raw).unwrap()
    }
    #[cfg(not(debug_assertions))]
    {
        decode_usize_infallible(raw)
    }
}

/// Convert a `c_int` returned from a syscall that effectively always
/// returns a `c_int`.
///
/// # Safety
///
/// This function must only be used with return values from infallible
/// syscalls.
#[inline]
pub(super) unsafe fn ret_c_int_infallible(raw: RetReg<R0>) -> c::c_int {
    #[cfg(debug_assertions)]
    {
        try_decode_c_int(raw).unwrap()
    }
    #[cfg(not(debug_assertions))]
    {
        decode_c_int_infallible(raw)
    }
}

/// Convert a `c_uint` returned from a syscall that effectively always
/// returns a `c_uint`.
///
/// # Safety
///
/// This function must only be used with return values from infallible
/// syscalls.
#[inline]
pub(super) unsafe fn ret_c_uint_infallible(raw: RetReg<R0>) -> c::c_uint {
    #[cfg(debug_assertions)]
    {
        try_decode_c_uint(raw).unwrap()
    }
    #[cfg(not(debug_assertions))]
    {
        decode_c_uint_infallible(raw)
    }
}

/// Convert a `usize` returned from a syscall that effectively returns an
/// `OwnedFd` on success.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// returns an owned file descriptor.
#[inline]
pub(super) unsafe fn ret_owned_fd(raw: RetReg<R0>) -> io::Result<OwnedFd> {
    let raw_fd = try_decode_raw_fd(raw)?;
    Ok(crate::backend::fd::OwnedFd::from_raw_fd(raw_fd))
}

/// Convert the return value of `dup2` and `dup3`.
///
/// When these functions succeed, they return the same value as their second
/// argument, so we don't construct a new `OwnedFd`.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// returns a file descriptor.
#[inline]
pub(super) unsafe fn ret_discarded_fd(raw: RetReg<R0>) -> io::Result<()> {
    let _raw_fd = try_decode_raw_fd(raw)?;
    Ok(())
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `*mut c_void` on success.
#[inline]
pub(super) fn ret_void_star(raw: RetReg<R0>) -> io::Result<*mut c::c_void> {
    try_decode_void_star(raw)
}
