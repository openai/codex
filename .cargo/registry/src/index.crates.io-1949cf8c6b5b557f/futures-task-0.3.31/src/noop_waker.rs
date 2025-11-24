//! Utilities for creating zero-cost wakers that don't do anything.

use core::ptr::null;
use core::task::{RawWaker, RawWakerVTable, Waker};

unsafe fn noop_clone(_data: *const ()) -> RawWaker {
    noop_raw_waker()
}

unsafe fn noop(_data: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

const fn noop_raw_waker() -> RawWaker {
    RawWaker::new(null(), &NOOP_WAKER_VTABLE)
}

/// Create a new [`Waker`] which does
/// nothing when `wake()` is called on it.
///
/// # Examples
///
/// ```
/// use futures::task::noop_waker;
/// let waker = noop_waker();
/// waker.wake();
/// ```
#[inline]
pub fn noop_waker() -> Waker {
    // FIXME: Since 1.46.0 we can use transmute in consts, allowing this function to be const.
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

/// Get a static reference to a [`Waker`] which
/// does nothing when `wake()` is called on it.
///
/// # Examples
///
/// ```
/// use futures::task::noop_waker_ref;
/// let waker = noop_waker_ref();
/// waker.wake_by_ref();
/// ```
#[inline]
pub fn noop_waker_ref() -> &'static Waker {
    struct SyncRawWaker(RawWaker);
    unsafe impl Sync for SyncRawWaker {}

    static NOOP_WAKER_INSTANCE: SyncRawWaker = SyncRawWaker(noop_raw_waker());

    // SAFETY: `Waker` is #[repr(transparent)] over its `RawWaker`.
    unsafe { &*(&NOOP_WAKER_INSTANCE.0 as *const RawWaker as *const Waker) }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "std")]
    fn issue_2091_cross_thread_segfault() {
        let waker = std::thread::spawn(super::noop_waker_ref).join().unwrap();
        waker.wake_by_ref();
    }
}
