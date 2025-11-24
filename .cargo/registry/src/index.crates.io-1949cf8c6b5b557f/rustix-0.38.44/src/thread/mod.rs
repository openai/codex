//! Thread-associated operations.

#[cfg(not(target_os = "redox"))]
mod clock;
#[cfg(linux_kernel)]
pub mod futex;
#[cfg(linux_kernel)]
mod id;
#[cfg(linux_kernel)]
mod libcap;
#[cfg(linux_kernel)]
mod prctl;
#[cfg(linux_kernel)]
mod setns;

#[allow(deprecated)]
#[cfg(linux_kernel)]
pub use crate::backend::thread::futex::FutexOperation;
#[cfg(linux_kernel)]
pub use crate::thread::futex::{
    Flags as FutexFlags, OWNER_DIED as FUTEX_OWNER_DIED, WAITERS as FUTEX_WAITERS,
};
#[cfg(not(target_os = "redox"))]
pub use clock::*;
#[cfg(linux_kernel)]
pub use id::{
    gettid, set_thread_gid, set_thread_groups, set_thread_res_gid, set_thread_res_uid,
    set_thread_uid, Gid, Pid, RawGid, RawPid, RawUid, Uid,
};
#[cfg(linux_kernel)]
pub use libcap::{capabilities, set_capabilities, CapabilityFlags, CapabilitySets};
#[cfg(linux_kernel)]
pub use prctl::*;
#[cfg(linux_kernel)]
pub use setns::*;

/// DEPRECATED: There are now individual functions available to perform futex
/// operations with improved type safety. See the [futex module].
///
/// `futex(uaddr, op, val, utime, uaddr2, val3)`
///
/// # References
///  - [Linux `futex` system call]
///  - [Linux `futex` feature]
///
/// # Safety
///
/// This is a very low-level feature for implementing synchronization
/// primitives. See the references links above.
///
/// [Linux `futex` system call]: https://man7.org/linux/man-pages/man2/futex.2.html
/// [Linux `futex` feature]: https://man7.org/linux/man-pages/man7/futex.7.html
/// [futex module]: mod@crate::thread::futex
#[cfg(linux_kernel)]
#[allow(unsafe_code, deprecated)]
#[inline]
pub unsafe fn futex(
    uaddr: *mut u32,
    op: FutexOperation,
    flags: FutexFlags,
    val: u32,
    utime: *const Timespec,
    uaddr2: *mut u32,
    val3: u32,
) -> crate::io::Result<usize> {
    use crate::backend::thread::futex::Operation;
    use crate::backend::thread::syscalls::{futex_timeout, futex_val2};
    use core::mem::transmute;
    use core::sync::atomic::AtomicU32;
    use FutexOperation::*;

    match op {
        Wait | LockPi | WaitBitset => futex_timeout(
            uaddr as *const AtomicU32,
            transmute::<FutexOperation, Operation>(op),
            flags,
            val,
            utime,
            uaddr2 as *const AtomicU32,
            val3,
        ),
        Wake | Fd | Requeue | CmpRequeue | WakeOp | UnlockPi | TrylockPi => futex_val2(
            uaddr as *const AtomicU32,
            transmute::<FutexOperation, Operation>(op),
            flags,
            val,
            utime as usize as u32,
            uaddr2 as *const AtomicU32,
            val3,
        ),
    }
}
