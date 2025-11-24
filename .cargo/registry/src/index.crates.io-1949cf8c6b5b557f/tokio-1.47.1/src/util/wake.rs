use crate::loom::sync::Arc;

use std::mem::ManuallyDrop;
use std::task::{RawWaker, RawWakerVTable, Waker};

/// Simplified waking interface based on Arcs.
pub(crate) trait Wake: Send + Sync + Sized + 'static {
    /// Wake by value.
    fn wake(arc_self: Arc<Self>);

    /// Wake by reference.
    fn wake_by_ref(arc_self: &Arc<Self>);
}

cfg_rt! {
    use std::marker::PhantomData;
    use std::ops::Deref;

    /// A `Waker` that is only valid for a given lifetime.
    #[derive(Debug)]
    pub(crate) struct WakerRef<'a> {
        waker: ManuallyDrop<Waker>,
        _p: PhantomData<&'a ()>,
    }

    impl Deref for WakerRef<'_> {
        type Target = Waker;

        fn deref(&self) -> &Waker {
            &self.waker
        }
    }

    /// Creates a reference to a `Waker` from a reference to `Arc<impl Wake>`.
    pub(crate) fn waker_ref<W: Wake>(wake: &Arc<W>) -> WakerRef<'_> {
        let ptr = Arc::as_ptr(wake).cast::<()>();

        let waker = unsafe { Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>())) };

        WakerRef {
            waker: ManuallyDrop::new(waker),
            _p: PhantomData,
        }
    }
}

/// Creates a waker from a `Arc<impl Wake>`.
pub(crate) fn waker<W: Wake>(wake: Arc<W>) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            Arc::into_raw(wake).cast(),
            waker_vtable::<W>(),
        ))
    }
}

fn waker_vtable<W: Wake>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_arc_raw::<W>,
        wake_by_ref_arc_raw::<W>,
        drop_arc_raw::<W>,
    )
}

unsafe fn clone_arc_raw<T: Wake>(data: *const ()) -> RawWaker {
    Arc::<T>::increment_strong_count(data as *const T);
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_arc_raw<T: Wake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    Wake::wake(arc);
}

// used by `waker_ref`
unsafe fn wake_by_ref_arc_raw<T: Wake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = ManuallyDrop::new(Arc::<T>::from_raw(data.cast()));
    Wake::wake_by_ref(&arc);
}

unsafe fn drop_arc_raw<T: Wake>(data: *const ()) {
    drop(Arc::<T>::from_raw(data.cast()));
}
