use super::arc_wake::ArcWake;
use super::waker::waker_vtable;
use alloc::sync::Arc;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::task::{RawWaker, Waker};

/// A [`Waker`] that is only valid for a given lifetime.
///
/// Note: this type implements [`Deref<Target = Waker>`](std::ops::Deref),
/// so it can be used to get a `&Waker`.
#[derive(Debug)]
pub struct WakerRef<'a> {
    waker: ManuallyDrop<Waker>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> WakerRef<'a> {
    /// Create a new [`WakerRef`] from a [`Waker`] reference.
    #[inline]
    pub fn new(waker: &'a Waker) -> Self {
        // copy the underlying (raw) waker without calling a clone,
        // as we won't call Waker::drop either.
        let waker = ManuallyDrop::new(unsafe { core::ptr::read(waker) });
        Self { waker, _marker: PhantomData }
    }

    /// Create a new [`WakerRef`] from a [`Waker`] that must not be dropped.
    ///
    /// Note: this if for rare cases where the caller created a [`Waker`] in
    /// an unsafe way (that will be valid only for a lifetime to be determined
    /// by the caller), and the [`Waker`] doesn't need to or must not be
    /// destroyed.
    #[inline]
    pub fn new_unowned(waker: ManuallyDrop<Waker>) -> Self {
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

/// Creates a reference to a [`Waker`] from a reference to `Arc<impl ArcWake>`.
///
/// The resulting [`Waker`] will call
/// [`ArcWake.wake()`](ArcWake::wake) if awoken.
#[inline]
pub fn waker_ref<W>(wake: &Arc<W>) -> WakerRef<'_>
where
    W: ArcWake + 'static,
{
    // simply copy the pointer instead of using Arc::into_raw,
    // as we don't actually keep a refcount by using ManuallyDrop.<
    let ptr = Arc::as_ptr(wake).cast::<()>();

    let waker =
        ManuallyDrop::new(unsafe { Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>())) });
    WakerRef::new_unowned(waker)
}
