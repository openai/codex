use alloc::sync::{Arc, Weak};
use core::cell::UnsafeCell;
use core::sync::atomic::Ordering::{self, Relaxed, SeqCst};
use core::sync::atomic::{AtomicBool, AtomicPtr};

use super::abort::abort;
use super::ReadyToRunQueue;
use crate::task::ArcWake;

pub(super) struct Task<Fut> {
    // The future
    pub(super) future: UnsafeCell<Option<Fut>>,

    // Next pointer for linked list tracking all active tasks (use
    // `spin_next_all` to read when access is shared across threads)
    pub(super) next_all: AtomicPtr<Task<Fut>>,

    // Previous task in linked list tracking all active tasks
    pub(super) prev_all: UnsafeCell<*const Task<Fut>>,

    // Length of the linked list tracking all active tasks when this node was
    // inserted (use `spin_next_all` to synchronize before reading when access
    // is shared across threads)
    pub(super) len_all: UnsafeCell<usize>,

    // Next pointer in ready to run queue
    pub(super) next_ready_to_run: AtomicPtr<Task<Fut>>,

    // Queue that we'll be enqueued to when woken
    pub(super) ready_to_run_queue: Weak<ReadyToRunQueue<Fut>>,

    // Whether or not this task is currently in the ready to run queue
    pub(super) queued: AtomicBool,

    // Whether the future was awoken during polling
    // It is possible for this flag to be set to true after the polling,
    // but it will be ignored.
    pub(super) woken: AtomicBool,
}

// `Task` can be sent across threads safely because it ensures that
// the underlying `Fut` type isn't touched from any of its methods.
//
// The parent (`super`) module is trusted not to access `future`
// across different threads.
unsafe impl<Fut> Send for Task<Fut> {}
unsafe impl<Fut> Sync for Task<Fut> {}

impl<Fut> ArcWake for Task<Fut> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let inner = match arc_self.ready_to_run_queue.upgrade() {
            Some(inner) => inner,
            None => return,
        };

        arc_self.woken.store(true, Relaxed);

        // It's our job to enqueue this task it into the ready to run queue. To
        // do this we set the `queued` flag, and if successful we then do the
        // actual queueing operation, ensuring that we're only queued once.
        //
        // Once the task is inserted call `wake` to notify the parent task,
        // as it'll want to come along and run our task later.
        //
        // Note that we don't change the reference count of the task here,
        // we merely enqueue the raw pointer. The `FuturesUnordered`
        // implementation guarantees that if we set the `queued` flag that
        // there's a reference count held by the main `FuturesUnordered` queue
        // still.
        let prev = arc_self.queued.swap(true, SeqCst);
        if !prev {
            inner.enqueue(Arc::as_ptr(arc_self));
            inner.waker.wake();
        }
    }
}

impl<Fut> Task<Fut> {
    /// Returns a waker reference for this task without cloning the Arc.
    pub(super) unsafe fn waker_ref(this: &Arc<Self>) -> waker_ref::WakerRef<'_> {
        unsafe { waker_ref::waker_ref(this) }
    }

    /// Spins until `next_all` is no longer set to `pending_next_all`.
    ///
    /// The temporary `pending_next_all` value is typically overwritten fairly
    /// quickly after a node is inserted into the list of all futures, so this
    /// should rarely spin much.
    ///
    /// When it returns, the correct `next_all` value is returned.
    ///
    /// `Relaxed` or `Acquire` ordering can be used. `Acquire` ordering must be
    /// used before `len_all` can be safely read.
    #[inline]
    pub(super) fn spin_next_all(
        &self,
        pending_next_all: *mut Self,
        ordering: Ordering,
    ) -> *const Self {
        loop {
            let next = self.next_all.load(ordering);
            if next != pending_next_all {
                return next;
            }
        }
    }
}

impl<Fut> Drop for Task<Fut> {
    fn drop(&mut self) {
        // Since `Task<Fut>` is sent across all threads for any lifetime,
        // regardless of `Fut`, we, to guarantee memory safety, can't actually
        // touch `Fut` at any time except when we have a reference to the
        // `FuturesUnordered` itself .
        //
        // Consequently it *should* be the case that we always drop futures from
        // the `FuturesUnordered` instance. This is a bomb, just in case there's
        // a bug in that logic.
        unsafe {
            if (*self.future.get()).is_some() {
                abort("future still here when dropping");
            }
        }
    }
}

mod waker_ref {
    use alloc::sync::Arc;
    use core::marker::PhantomData;
    use core::mem;
    use core::mem::ManuallyDrop;
    use core::ops::Deref;
    use core::task::{RawWaker, RawWakerVTable, Waker};
    use futures_task::ArcWake;

    pub(crate) struct WakerRef<'a> {
        waker: ManuallyDrop<Waker>,
        _marker: PhantomData<&'a ()>,
    }

    impl WakerRef<'_> {
        #[inline]
        fn new_unowned(waker: ManuallyDrop<Waker>) -> Self {
            Self { waker, _marker: PhantomData }
        }
    }

    impl Deref for WakerRef<'_> {
        type Target = Waker;

        #[inline]
        fn deref(&self) -> &Waker {
            &self.waker
        }
    }

    /// Copy of `future_task::waker_ref` without `W: 'static` bound.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that use-after-free will not occur.
    #[inline]
    pub(crate) unsafe fn waker_ref<W>(wake: &Arc<W>) -> WakerRef<'_>
    where
        W: ArcWake,
    {
        // simply copy the pointer instead of using Arc::into_raw,
        // as we don't actually keep a refcount by using ManuallyDrop.<
        let ptr = Arc::as_ptr(wake).cast::<()>();

        let waker =
            ManuallyDrop::new(unsafe { Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>())) });
        WakerRef::new_unowned(waker)
    }

    fn waker_vtable<W: ArcWake>() -> &'static RawWakerVTable {
        &RawWakerVTable::new(
            clone_arc_raw::<W>,
            wake_arc_raw::<W>,
            wake_by_ref_arc_raw::<W>,
            drop_arc_raw::<W>,
        )
    }

    // FIXME: panics on Arc::clone / refcount changes could wreak havoc on the
    // code here. We should guard against this by aborting.

    unsafe fn increase_refcount<T: ArcWake>(data: *const ()) {
        // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
        let arc = mem::ManuallyDrop::new(unsafe { Arc::<T>::from_raw(data.cast::<T>()) });
        // Now increase refcount, but don't drop new refcount either
        let _arc_clone: mem::ManuallyDrop<_> = arc.clone();
    }

    unsafe fn clone_arc_raw<T: ArcWake>(data: *const ()) -> RawWaker {
        unsafe { increase_refcount::<T>(data) }
        RawWaker::new(data, waker_vtable::<T>())
    }

    unsafe fn wake_arc_raw<T: ArcWake>(data: *const ()) {
        let arc: Arc<T> = unsafe { Arc::from_raw(data.cast::<T>()) };
        ArcWake::wake(arc);
    }

    unsafe fn wake_by_ref_arc_raw<T: ArcWake>(data: *const ()) {
        // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
        let arc = mem::ManuallyDrop::new(unsafe { Arc::<T>::from_raw(data.cast::<T>()) });
        ArcWake::wake_by_ref(&arc);
    }

    unsafe fn drop_arc_raw<T: ArcWake>(data: *const ()) {
        drop(unsafe { Arc::<T>::from_raw(data.cast::<T>()) })
    }
}
