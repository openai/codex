use super::arc_wake::ArcWake;
use alloc::sync::Arc;
use core::mem;
use core::task::{RawWaker, RawWakerVTable, Waker};

pub(super) fn waker_vtable<W: ArcWake + 'static>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_arc_raw::<W>,
        wake_by_ref_arc_raw::<W>,
        drop_arc_raw::<W>,
    )
}

/// Creates a [`Waker`] from an `Arc<impl ArcWake>`.
///
/// The returned [`Waker`] will call
/// [`ArcWake.wake()`](ArcWake::wake) if awoken.
pub fn waker<W>(wake: Arc<W>) -> Waker
where
    W: ArcWake + 'static,
{
    let ptr = Arc::into_raw(wake).cast::<()>();

    unsafe { Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>())) }
}

// FIXME: panics on Arc::clone / refcount changes could wreak havoc on the
// code here. We should guard against this by aborting.

#[allow(clippy::redundant_clone)] // The clone here isn't actually redundant.
unsafe fn increase_refcount<T: ArcWake + 'static>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(unsafe { Arc::<T>::from_raw(data.cast::<T>()) });
    // Now increase refcount, but don't drop new refcount either
    let _arc_clone: mem::ManuallyDrop<_> = arc.clone();
}

// used by `waker_ref`
#[inline(always)]
unsafe fn clone_arc_raw<T: ArcWake + 'static>(data: *const ()) -> RawWaker {
    unsafe { increase_refcount::<T>(data) }
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_arc_raw<T: ArcWake + 'static>(data: *const ()) {
    let arc: Arc<T> = unsafe { Arc::from_raw(data.cast::<T>()) };
    ArcWake::wake(arc);
}

// used by `waker_ref`
unsafe fn wake_by_ref_arc_raw<T: ArcWake + 'static>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(unsafe { Arc::<T>::from_raw(data.cast::<T>()) });
    ArcWake::wake_by_ref(&arc);
}

unsafe fn drop_arc_raw<T: ArcWake + 'static>(data: *const ()) {
    drop(unsafe { Arc::<T>::from_raw(data.cast::<T>()) })
}
