use crate::runtime::task::{Header, RawTask, Schedule};

use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops;
use std::ptr::NonNull;
use std::task::{RawWaker, RawWakerVTable, Waker};

pub(super) struct WakerRef<'a, S: 'static> {
    waker: ManuallyDrop<Waker>,
    _p: PhantomData<(&'a Header, S)>,
}

/// Returns a `WakerRef` which avoids having to preemptively increase the
/// refcount if there is no need to do so.
pub(super) fn waker_ref<S>(header: &NonNull<Header>) -> WakerRef<'_, S>
where
    S: Schedule,
{
    // `Waker::will_wake` uses the VTABLE pointer as part of the check. This
    // means that `will_wake` will always return false when using the current
    // task's waker. (discussion at rust-lang/rust#66281).
    //
    // To fix this, we use a single vtable. Since we pass in a reference at this
    // point and not an *owned* waker, we must ensure that `drop` is never
    // called on this waker instance. This is done by wrapping it with
    // `ManuallyDrop` and then never calling drop.
    let waker = unsafe { ManuallyDrop::new(Waker::from_raw(raw_waker(*header))) };

    WakerRef {
        waker,
        _p: PhantomData,
    }
}

impl<S> ops::Deref for WakerRef<'_, S> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}

cfg_trace! {
    macro_rules! trace {
        ($header:expr, $op:expr) => {
            if let Some(id) = Header::get_tracing_id(&$header) {
                tracing::trace!(
                    target: "tokio::task::waker",
                    op = $op,
                    task.id = id.into_u64(),
                );
            }
        }
    }
}

cfg_not_trace! {
    macro_rules! trace {
        ($header:expr, $op:expr) => {
            // noop
            let _ = &$header;
        }
    }
}

unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
    let header = NonNull::new_unchecked(ptr as *mut Header);
    trace!(header, "waker.clone");
    header.as_ref().state.ref_inc();
    raw_waker(header)
}

unsafe fn drop_waker(ptr: *const ()) {
    let ptr = NonNull::new_unchecked(ptr as *mut Header);
    trace!(ptr, "waker.drop");
    let raw = RawTask::from_raw(ptr);
    raw.drop_reference();
}

unsafe fn wake_by_val(ptr: *const ()) {
    let ptr = NonNull::new_unchecked(ptr as *mut Header);
    trace!(ptr, "waker.wake");
    let raw = RawTask::from_raw(ptr);
    raw.wake_by_val();
}

// Wake without consuming the waker
unsafe fn wake_by_ref(ptr: *const ()) {
    let ptr = NonNull::new_unchecked(ptr as *mut Header);
    trace!(ptr, "waker.wake_by_ref");
    let raw = RawTask::from_raw(ptr);
    raw.wake_by_ref();
}

static WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_waker, wake_by_val, wake_by_ref, drop_waker);

fn raw_waker(header: NonNull<Header>) -> RawWaker {
    let ptr = header.as_ptr() as *const ();
    RawWaker::new(ptr, &WAKER_VTABLE)
}
