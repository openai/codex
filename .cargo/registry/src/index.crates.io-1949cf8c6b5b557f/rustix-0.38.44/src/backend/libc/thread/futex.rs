use crate::backend::c;

bitflags::bitflags! {
    /// `FUTEX_*` flags for use with the functions in [`futex`].
    ///
    /// [`futex`]: mod@crate::thread::futex
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct Flags: u32 {
        /// `FUTEX_PRIVATE_FLAG`
        const PRIVATE = bitcast!(c::FUTEX_PRIVATE_FLAG);
        /// `FUTEX_CLOCK_REALTIME`
        const CLOCK_REALTIME = bitcast!(c::FUTEX_CLOCK_REALTIME);
    }
}

/// `FUTEX_*` operations for use with the futex syscall wrappers.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum Operation {
    /// `FUTEX_WAIT`
    Wait = bitcast!(c::FUTEX_WAIT),
    /// `FUTEX_WAKE`
    Wake = bitcast!(c::FUTEX_WAKE),
    /// `FUTEX_FD`
    Fd = bitcast!(c::FUTEX_FD),
    /// `FUTEX_REQUEUE`
    Requeue = bitcast!(c::FUTEX_REQUEUE),
    /// `FUTEX_CMP_REQUEUE`
    CmpRequeue = bitcast!(c::FUTEX_CMP_REQUEUE),
    /// `FUTEX_WAKE_OP`
    WakeOp = bitcast!(c::FUTEX_WAKE_OP),
    /// `FUTEX_LOCK_PI`
    LockPi = bitcast!(c::FUTEX_LOCK_PI),
    /// `FUTEX_UNLOCK_PI`
    UnlockPi = bitcast!(c::FUTEX_UNLOCK_PI),
    /// `FUTEX_TRYLOCK_PI`
    TrylockPi = bitcast!(c::FUTEX_TRYLOCK_PI),
    /// `FUTEX_WAIT_BITSET`
    WaitBitset = bitcast!(c::FUTEX_WAIT_BITSET),
    /// `FUTEX_WAKE_BITSET`
    WakeBitset = bitcast!(c::FUTEX_WAKE_BITSET),
    /// `FUTEX_WAIT_REQUEUE_PI`
    WaitRequeuePi = bitcast!(c::FUTEX_WAIT_REQUEUE_PI),
    /// `FUTEX_CMP_REQUEUE_PI`
    CmpRequeuePi = bitcast!(c::FUTEX_CMP_REQUEUE_PI),
    /// `FUTEX_LOCK_PI2`
    LockPi2 = bitcast!(c::FUTEX_LOCK_PI2),
}

/// `FUTEX_*` operations for use with the [`futex`] function.
///
/// [`futex`]: fn@crate::thread::futex
// TODO: Deprecate this now that we have a new typed API.
/*
#[deprecated(
    since = "0.38.35",
    note = "
    The `futex` function and `FutexOperation` enum are deprecated. There are
    individual functions available to perform futex operations with improved
    type safety. See the `rustix::thread::futex` module."
)]
*/
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum FutexOperation {
    /// `FUTEX_WAIT`
    Wait = bitcast!(c::FUTEX_WAIT),
    /// `FUTEX_WAKE`
    Wake = bitcast!(c::FUTEX_WAKE),
    /// `FUTEX_FD`
    Fd = bitcast!(c::FUTEX_FD),
    /// `FUTEX_REQUEUE`
    Requeue = bitcast!(c::FUTEX_REQUEUE),
    /// `FUTEX_CMP_REQUEUE`
    CmpRequeue = bitcast!(c::FUTEX_CMP_REQUEUE),
    /// `FUTEX_WAKE_OP`
    WakeOp = bitcast!(c::FUTEX_WAKE_OP),
    /// `FUTEX_LOCK_PI`
    LockPi = bitcast!(c::FUTEX_LOCK_PI),
    /// `FUTEX_UNLOCK_PI`
    UnlockPi = bitcast!(c::FUTEX_UNLOCK_PI),
    /// `FUTEX_TRYLOCK_PI`
    TrylockPi = bitcast!(c::FUTEX_TRYLOCK_PI),
    /// `FUTEX_WAIT_BITSET`
    WaitBitset = bitcast!(c::FUTEX_WAIT_BITSET),
}

/// `FUTEX_WAITERS`
pub const WAITERS: u32 = linux_raw_sys::general::FUTEX_WAITERS;

/// `FUTEX_OWNER_DIED`
pub const OWNER_DIED: u32 = linux_raw_sys::general::FUTEX_OWNER_DIED;
