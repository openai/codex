//! Linux `futex`.
//!
//! Futex is a very low-level mechanism for implementing concurrency primitives
//! such as mutexes, rwlocks, and condvars. For a higher-level API that
//! provides those abstractions, see [rustix-futex-sync].
//!
//! # Examples
//!
//! ```
//! use rustix::thread::futex;
//! use std::sync::atomic::AtomicU32;
//!
//! # fn test(futex: &AtomicU32) -> rustix::io::Result<()> {
//! // Wake up one waiter.
//! futex::wake(futex, futex::Flags::PRIVATE, 1)?;
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!  - [Linux `futex` system call]
//!  - [Linux `futex` feature]
//!
//! [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
//! [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
//! [rustix-futex-sync]: https://crates.io/crates/rustix-futex-sync
#![allow(unsafe_code)]

use core::ffi::c_void;
use core::num::NonZeroU32;
use core::ptr;
use core::sync::atomic::AtomicU32;

use crate::backend::thread::futex::Operation;
use crate::backend::thread::syscalls::{futex_timeout, futex_val2};
use crate::fd::{FromRawFd as _, OwnedFd, RawFd};
use crate::{backend, io};

pub use crate::clockid::ClockId;
pub use crate::timespec::{Nsecs, Secs, Timespec};

pub use backend::thread::futex::{Flags, WaitFlags, OWNER_DIED, WAITERS};

/// `syscall(SYS_futex, uaddr, FUTEX_WAIT, val, timeout, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn wait(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    timeout: Option<&Timespec>,
) -> io::Result<()> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_timeout(uaddr, Operation::Wait, flags, val, timeout, ptr::null(), 0).map(|val| {
            debug_assert_eq!(
                val, 0,
                "The return value should always equal zero, if the call is successful"
            );
        })
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_WAKE, val, NULL, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn wake(uaddr: &AtomicU32, flags: Flags, val: u32) -> io::Result<usize> {
    // SAFETY: The raw pointers come from references or null.
    unsafe { futex_val2(uaddr, Operation::Wake, flags, val, 0, ptr::null(), 0) }
}

/// `syscall(SYS_futex, uaddr, FUTEX_FD, val, NULL, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn fd(uaddr: &AtomicU32, flags: Flags, val: u32) -> io::Result<OwnedFd> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_val2(uaddr, Operation::Fd, flags, val, 0, ptr::null(), 0).map(|val| {
            let fd = val as RawFd;
            debug_assert_eq!(fd as usize, val, "return value should be a valid fd");
            OwnedFd::from_raw_fd(fd)
        })
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_REQUEUE, val, val2, uaddr2, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn requeue(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    val2: u32,
    uaddr2: &AtomicU32,
) -> io::Result<usize> {
    // SAFETY: The raw pointers come from references or null.
    unsafe { futex_val2(uaddr, Operation::Requeue, flags, val, val2, uaddr2, 0) }
}

/// `syscall(SYS_futex, uaddr, FUTEX_CMP_REQUEUE, val, val2, uaddr2, val3)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn cmp_requeue(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    val2: u32,
    uaddr2: &AtomicU32,
    val3: u32,
) -> io::Result<usize> {
    // SAFETY: The raw pointers come from references or null.
    unsafe { futex_val2(uaddr, Operation::CmpRequeue, flags, val, val2, uaddr2, val3) }
}

/// `FUTEX_OP_*` operations for use with [`wake_op`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
#[allow(clippy::identity_op)]
pub enum WakeOp {
    /// `FUTEX_OP_SET`: `uaddr2 = oparg;`
    Set = 0,
    /// `FUTEX_OP_ADD`: `uaddr2 += oparg;`
    Add = 1,
    /// `FUTEX_OP_OR`: `uaddr2 |= oparg;`
    Or = 2,
    /// `FUTEX_OP_ANDN`: `uaddr2 &= ~oparg;`
    AndN = 3,
    /// `FUTEX_OP_XOR`: `uaddr2 ^= oparg;`
    XOr = 4,
    /// `FUTEX_OP_SET | FUTEX_OP_ARG_SHIFT`: `uaddr2 = (oparg << 1);`
    SetShift = 0 | 8,
    /// `FUTEX_OP_ADD | FUTEX_OP_ARG_SHIFT`: `uaddr2 += (oparg << 1);`
    AddShift = 1 | 8,
    /// `FUTEX_OP_OR | FUTEX_OP_ARG_SHIFT`: `uaddr2 |= (oparg << 1);`
    OrShift = 2 | 8,
    /// `FUTEX_OP_ANDN | FUTEX_OP_ARG_SHIFT`: `uaddr2 &= !(oparg << 1);`
    AndNShift = 3 | 8,
    /// `FUTEX_OP_XOR | FUTEX_OP_ARG_SHIFT`: `uaddr2 ^= (oparg << 1);`
    XOrShift = 4 | 8,
}

/// `FUTEX_OP_CMP_*` operations for use with [`wake_op`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum WakeOpCmp {
    /// `FUTEX_OP_CMP_EQ`: `if oldval == cmparg { wake(); }`
    Eq = 0,
    /// `FUTEX_OP_CMP_EQ`: `if oldval != cmparg { wake(); }`
    Ne = 1,
    /// `FUTEX_OP_CMP_EQ`: `if oldval < cmparg { wake(); }`
    Lt = 2,
    /// `FUTEX_OP_CMP_EQ`: `if oldval <= cmparg { wake(); }`
    Le = 3,
    /// `FUTEX_OP_CMP_EQ`: `if oldval > cmparg { wake(); }`
    Gt = 4,
    /// `FUTEX_OP_CMP_EQ`: `if oldval >= cmparg { wake(); }`
    Ge = 5,
}

/// `syscall(SYS_futex, uaddr, FUTEX_WAKE_OP, val, val2, uaddr2, val3)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn wake_op(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    val2: u32,
    uaddr2: &AtomicU32,
    op: WakeOp,
    cmp: WakeOpCmp,
    oparg: u16,
    cmparg: u16,
) -> io::Result<usize> {
    if oparg >= 1 << 12 || cmparg >= 1 << 12 {
        return Err(io::Errno::INVAL);
    }

    let val3 =
        ((op as u32) << 28) | ((cmp as u32) << 24) | ((oparg as u32) << 12) | (cmparg as u32);

    // SAFETY: The raw pointers come from references or null.
    unsafe { futex_val2(uaddr, Operation::WakeOp, flags, val, val2, uaddr2, val3) }
}

/// `syscall(SYS_futex, uaddr, FUTEX_LOCK_PI, 0, timeout, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn lock_pi(uaddr: &AtomicU32, flags: Flags, timeout: Option<&Timespec>) -> io::Result<()> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_timeout(uaddr, Operation::LockPi, flags, 0, timeout, ptr::null(), 0).map(|val| {
            debug_assert_eq!(
                val, 0,
                "The return value should always equal zero, if the call is successful"
            );
        })
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_UNLOCK_PI, 0, NULL, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn unlock_pi(uaddr: &AtomicU32, flags: Flags) -> io::Result<()> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_val2(uaddr, Operation::UnlockPi, flags, 0, 0, ptr::null(), 0).map(|val| {
            debug_assert_eq!(
                val, 0,
                "The return value should always equal zero, if the call is successful"
            );
        })
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_TRYLOCK_PI, 0, NULL, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn trylock_pi(uaddr: &AtomicU32, flags: Flags) -> io::Result<bool> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_val2(uaddr, Operation::TrylockPi, flags, 0, 0, ptr::null(), 0).map(|ret| ret == 0)
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_WAIT_BITSET, val, timeout, NULL, val3)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn wait_bitset(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    timeout: Option<&Timespec>,
    val3: NonZeroU32,
) -> io::Result<()> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_timeout(
            uaddr,
            Operation::WaitBitset,
            flags,
            val,
            timeout,
            ptr::null(),
            val3.get(),
        )
        .map(|val| {
            debug_assert_eq!(
                val, 0,
                "The return value should always equal zero, if the call is successful"
            );
        })
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_WAKE_BITSET, val, NULL, NULL, val3)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn wake_bitset(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    val3: NonZeroU32,
) -> io::Result<usize> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_val2(
            uaddr,
            Operation::WakeBitset,
            flags,
            val,
            0,
            ptr::null(),
            val3.get(),
        )
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_WAIT_REQUEUE_PI, val, timeout, uaddr2, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn wait_requeue_pi(
    uaddr: &AtomicU32,
    flags: Flags,
    val: u32,
    timeout: Option<&Timespec>,
    uaddr2: &AtomicU32,
) -> io::Result<()> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_timeout(
            uaddr,
            Operation::WaitRequeuePi,
            flags,
            val,
            timeout,
            uaddr2,
            0,
        )
        .map(|val| {
            debug_assert_eq!(
                val, 0,
                "The return value should always equal zero, if the call is successful"
            );
        })
    }
}

/// `syscall(SYS_futex, uaddr, FUTEX_CMP_REQUEUE_PI, 1, val2, uaddr2, val3)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn cmp_requeue_pi(
    uaddr: &AtomicU32,
    flags: Flags,
    val2: u32,
    uaddr2: &AtomicU32,
    val3: u32,
) -> io::Result<usize> {
    // SAFETY: The raw pointers come from references or null.
    unsafe { futex_val2(uaddr, Operation::CmpRequeuePi, flags, 1, val2, uaddr2, val3) }
}

/// `syscall(SYS_futex, uaddr, FUTEX_LOCK_PI2, 0, timeout, NULL, 0)`
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links.
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
#[inline]
pub fn lock_pi2(uaddr: &AtomicU32, flags: Flags, timeout: Option<&Timespec>) -> io::Result<()> {
    // SAFETY: The raw pointers come from references or null.
    unsafe {
        futex_timeout(uaddr, Operation::LockPi2, flags, 0, timeout, ptr::null(), 0).map(|val| {
            debug_assert_eq!(
                val, 0,
                "The return value should always equal zero, if the call is successful"
            );
        })
    }
}

/// A pointer in the [`Wait`] struct.
#[repr(C)]
#[derive(Copy, Clone)]
#[non_exhaustive]
pub struct WaitPtr {
    #[cfg(all(target_pointer_width = "32", target_endian = "big"))]
    #[doc(hidden)]
    pub __pad32: u32,
    #[cfg(all(target_pointer_width = "16", target_endian = "big"))]
    #[doc(hidden)]
    pub __pad16: u16,

    /// The pointer value.
    pub ptr: *mut c_void,

    #[cfg(all(target_pointer_width = "16", target_endian = "little"))]
    #[doc(hidden)]
    pub __pad16: u16,
    #[cfg(all(target_pointer_width = "32", target_endian = "little"))]
    #[doc(hidden)]
    pub __pad32: u32,
}

impl WaitPtr {
    /// Construct a new `WaitPtr` holding the given raw pointer value.
    #[inline]
    pub const fn new(ptr: *mut c_void) -> Self {
        Self {
            ptr,

            #[cfg(target_pointer_width = "16")]
            __pad16: 0,
            #[cfg(any(target_pointer_width = "16", target_pointer_width = "32"))]
            __pad32: 0,
        }
    }
}

impl Default for WaitPtr {
    #[inline]
    fn default() -> Self {
        Self::new(ptr::null_mut())
    }
}

impl From<*mut c_void> for WaitPtr {
    #[inline]
    fn from(ptr: *mut c_void) -> Self {
        Self::new(ptr)
    }
}

impl core::fmt::Debug for WaitPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.ptr.fmt(f)
    }
}

/// For use with [`waitv`].
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub struct Wait {
    /// The expected value.
    pub val: u64,
    /// The address to wait for.
    pub uaddr: WaitPtr,
    /// The type and size of futex to perform.
    pub flags: WaitFlags,

    /// Reserved for future use.
    pub(crate) __reserved: u32,
}

impl Wait {
    /// Construct a zero-initialized `Wait`.
    #[inline]
    pub const fn new() -> Self {
        Self {
            val: 0,
            uaddr: WaitPtr::new(ptr::null_mut()),
            flags: WaitFlags::empty(),
            __reserved: 0,
        }
    }
}

impl Default for Wait {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// `futex_waitv(waiters.as_ptr(), waiters.len(), flags, timeout, clockd)`—
/// Wait on an array of futexes, wake on any.
///
/// This requires Linux ≥ 5.16.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://www.kernel.org/doc/html/latest/userspace-api/futex2.html
#[inline]
pub fn waitv(
    waiters: &[Wait],
    flags: WaitvFlags,
    timeout: Option<&Timespec>,
    clockid: ClockId,
) -> io::Result<usize> {
    backend::thread::syscalls::futex_waitv(waiters, flags, timeout, clockid)
}

bitflags::bitflags! {
    /// Flags for use with the flags argument in [`waitv`].
    ///
    /// At this time, no flags are defined.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct WaitvFlags: u32 {
        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_raw)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layouts() {
        use crate::backend::c;

        check_renamed_struct!(Wait, futex_waitv, val, uaddr, flags, __reserved);
    }
}
