use core::fmt;
use core::mem::ManuallyDrop;
use core::pin::Pin;
use core::task::Poll;

use alloc::sync::Arc;

use super::raw::{RawRead, RawUpgradableRead, RawUpgrade, RawWrite};
use super::{
    RwLock, RwLockReadGuard, RwLockReadGuardArc, RwLockUpgradableReadGuard,
    RwLockUpgradableReadGuardArc, RwLockWriteGuard, RwLockWriteGuardArc,
};

use event_listener_strategy::{easy_wrapper, EventListenerFuture, Strategy};

easy_wrapper! {
    /// The future returned by [`RwLock::read`].
    pub struct Read<'a, T: ?Sized>(ReadInner<'a, T> => RwLockReadGuard<'a, T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLock::read`].
    struct ReadInner<'a, T: ?Sized> {
        // Raw read lock acquisition future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawRead<'a>,

        // Pointer to the value protected by the lock. Covariant in `T`.
        pub(super) value: *const T,
    }
}

unsafe impl<T: Sync + ?Sized> Send for ReadInner<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for ReadInner<'_, T> {}

impl<'x, T: ?Sized> Read<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawRead<'x>, value: *const T) -> Self {
        Self::_new(ReadInner { raw, value })
    }
}

impl<T: ?Sized> fmt::Debug for Read<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Read { .. }")
    }
}

impl<'a, T: ?Sized> EventListenerFuture for ReadInner<'a, T> {
    type Output = RwLockReadGuard<'a, T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));

        Poll::Ready(RwLockReadGuard {
            lock: this.raw.lock,
            value: *this.value,
        })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLock::read_arc`].
    pub struct ReadArc<'a, T>(ReadArcInner<'a, T> => RwLockReadGuardArc<T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLock::read_arc`].
    struct ReadArcInner<'a, T> {
        // Raw read lock acquisition future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawRead<'a>,

        // FIXME: Could be covariant in T
        pub(super) lock: &'a Arc<RwLock<T>>,
    }
}

unsafe impl<T: Send + Sync> Send for ReadArcInner<'_, T> {}
unsafe impl<T: Send + Sync> Sync for ReadArcInner<'_, T> {}

impl<'x, T> ReadArc<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawRead<'x>, lock: &'x Arc<RwLock<T>>) -> Self {
        Self::_new(ReadArcInner { raw, lock })
    }
}

impl<T> fmt::Debug for ReadArc<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadArc { .. }")
    }
}

impl<T> EventListenerFuture for ReadArcInner<'_, T> {
    type Output = RwLockReadGuardArc<T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));

        // SAFETY: we just acquired a read lock
        Poll::Ready(unsafe { RwLockReadGuardArc::from_arc(this.lock.clone()) })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLock::upgradable_read`].
    pub struct UpgradableRead<'a, T: ?Sized>(
        UpgradableReadInner<'a, T> => RwLockUpgradableReadGuard<'a, T>
    );
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLock::upgradable_read`].
    struct UpgradableReadInner<'a, T: ?Sized> {
        // Raw upgradable read lock acquisition future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawUpgradableRead<'a>,

        // Pointer to the value protected by the lock. Invariant in `T`
        // as the upgradable lock could provide write access.
        pub(super) value: *mut T,
    }
}

unsafe impl<T: Send + Sync + ?Sized> Send for UpgradableReadInner<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for UpgradableReadInner<'_, T> {}

impl<'x, T: ?Sized> UpgradableRead<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawUpgradableRead<'x>, value: *mut T) -> Self {
        Self::_new(UpgradableReadInner { raw, value })
    }
}

impl<T: ?Sized> fmt::Debug for UpgradableRead<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UpgradableRead { .. }")
    }
}

impl<'a, T: ?Sized> EventListenerFuture for UpgradableReadInner<'a, T> {
    type Output = RwLockUpgradableReadGuard<'a, T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));

        Poll::Ready(RwLockUpgradableReadGuard {
            lock: this.raw.lock,
            value: *this.value,
        })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLock::upgradable_read_arc`].
    pub struct UpgradableReadArc<'a, T: ?Sized>(
        UpgradableReadArcInner<'a, T> => RwLockUpgradableReadGuardArc<T>
    );
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLock::upgradable_read_arc`].
    struct UpgradableReadArcInner<'a, T: ?Sized> {
        // Raw upgradable read lock acquisition future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawUpgradableRead<'a>,

        pub(super) lock: &'a Arc<RwLock<T>>,
    }
}

unsafe impl<T: Send + Sync + ?Sized> Send for UpgradableReadArcInner<'_, T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for UpgradableReadArcInner<'_, T> {}

impl<'x, T: ?Sized> UpgradableReadArc<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawUpgradableRead<'x>, lock: &'x Arc<RwLock<T>>) -> Self {
        Self::_new(UpgradableReadArcInner { raw, lock })
    }
}

impl<T: ?Sized> fmt::Debug for UpgradableReadArc<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UpgradableReadArc { .. }")
    }
}

impl<T: ?Sized> EventListenerFuture for UpgradableReadArcInner<'_, T> {
    type Output = RwLockUpgradableReadGuardArc<T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));
        Poll::Ready(RwLockUpgradableReadGuardArc {
            lock: this.lock.clone(),
        })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLock::write`].
    pub struct Write<'a, T: ?Sized>(WriteInner<'a, T> => RwLockWriteGuard<'a, T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLock::write`].
    struct WriteInner<'a, T: ?Sized> {
        // Raw write lock acquisition future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawWrite<'a>,

        // Pointer to the value protected by the lock. Invariant in `T`.
        pub(super) value: *mut T,
    }
}

unsafe impl<T: Send + ?Sized> Send for WriteInner<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for WriteInner<'_, T> {}

impl<'x, T: ?Sized> Write<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawWrite<'x>, value: *mut T) -> Self {
        Self::_new(WriteInner { raw, value })
    }
}

impl<T: ?Sized> fmt::Debug for Write<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Write { .. }")
    }
}

impl<'a, T: ?Sized> EventListenerFuture for WriteInner<'a, T> {
    type Output = RwLockWriteGuard<'a, T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));

        Poll::Ready(RwLockWriteGuard {
            lock: this.raw.lock,
            value: *this.value,
        })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLock::write_arc`].
    pub struct WriteArc<'a, T: ?Sized>(WriteArcInner<'a, T> => RwLockWriteGuardArc<T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLock::write_arc`].
    struct WriteArcInner<'a, T: ?Sized> {
        // Raw write lock acquisition future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawWrite<'a>,

        pub(super) lock: &'a Arc<RwLock<T>>,
    }
}

unsafe impl<T: Send + Sync + ?Sized> Send for WriteArcInner<'_, T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for WriteArcInner<'_, T> {}

impl<'x, T: ?Sized> WriteArc<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawWrite<'x>, lock: &'x Arc<RwLock<T>>) -> Self {
        Self::_new(WriteArcInner { raw, lock })
    }
}

impl<T: ?Sized> fmt::Debug for WriteArc<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WriteArc { .. }")
    }
}

impl<T: ?Sized> EventListenerFuture for WriteArcInner<'_, T> {
    type Output = RwLockWriteGuardArc<T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));

        Poll::Ready(RwLockWriteGuardArc {
            lock: this.lock.clone(),
        })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLockUpgradableReadGuard::upgrade`].
    pub struct Upgrade<'a, T: ?Sized>(UpgradeInner<'a, T> => RwLockWriteGuard<'a, T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLockUpgradableReadGuard::upgrade`].
    struct UpgradeInner<'a, T: ?Sized> {
        // Raw read lock upgrade future, doesn't depend on `T`.
        #[pin]
        pub(super) raw: RawUpgrade<'a>,

        // Pointer to the value protected by the lock. Invariant in `T`.
        pub(super) value: *mut T,
    }
}

unsafe impl<T: Send + ?Sized> Send for UpgradeInner<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for UpgradeInner<'_, T> {}

impl<'x, T: ?Sized> Upgrade<'x, T> {
    #[inline]
    pub(super) fn new(raw: RawUpgrade<'x>, value: *mut T) -> Self {
        Self::_new(UpgradeInner { raw, value })
    }
}

impl<T: ?Sized> fmt::Debug for Upgrade<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upgrade").finish()
    }
}

impl<'a, T: ?Sized> EventListenerFuture for UpgradeInner<'a, T> {
    type Output = RwLockWriteGuard<'a, T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();
        let lock = ready!(this.raw.as_mut().poll_with_strategy(strategy, cx));

        Poll::Ready(RwLockWriteGuard {
            lock,
            value: *this.value,
        })
    }
}

easy_wrapper! {
    /// The future returned by [`RwLockUpgradableReadGuardArc::upgrade`].
    pub struct UpgradeArc<T: ?Sized>(UpgradeArcInner<T> => RwLockWriteGuardArc<T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`RwLockUpgradableReadGuardArc::upgrade`].
    struct UpgradeArcInner<T: ?Sized> {
        // Raw read lock upgrade future, doesn't depend on `T`.
        // `'static` is a lie, this field is actually referencing the
        // `Arc` data. But since this struct also stores said `Arc`, we know
        // this value will be alive as long as the struct is.
        //
        // Yes, one field of the `ArcUpgrade` struct is referencing another.
        // Such self-references are usually not sound without pinning.
        // However, in this case, there is an indirection via the heap;
        // moving the `ArcUpgrade` won't move the heap allocation of the `Arc`,
        // so the reference inside `RawUpgrade` isn't invalidated.
        #[pin]
        pub(super) raw: ManuallyDrop<RawUpgrade<'static>>,

        // Pointer to the value protected by the lock. Invariant in `T`.
        pub(super) lock: ManuallyDrop<Arc<RwLock<T>>>,
    }

    impl<T: ?Sized> PinnedDrop for UpgradeArcInner<T> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            let is_ready = this.raw.is_ready();

            // SAFETY: The drop impl for raw assumes that it is pinned.
            unsafe {
                ManuallyDrop::drop(this.raw.get_unchecked_mut());
            }

            if !is_ready {
                // SAFETY: we drop the `Arc` (decrementing the reference count)
                // only if this future was cancelled before returning an
                // upgraded lock.
                unsafe {
                    ManuallyDrop::drop(this.lock);
                };
            }
        }
    }
}

impl<T: ?Sized> UpgradeArc<T> {
    #[inline]
    pub(super) unsafe fn new(
        raw: ManuallyDrop<RawUpgrade<'static>>,
        lock: ManuallyDrop<Arc<RwLock<T>>>,
    ) -> Self {
        Self::_new(UpgradeArcInner { raw, lock })
    }
}

impl<T: ?Sized> fmt::Debug for UpgradeArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArcUpgrade").finish()
    }
}

impl<T: ?Sized> EventListenerFuture for UpgradeArcInner<T> {
    type Output = RwLockWriteGuardArc<T>;

    #[inline]
    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let this = self.project();
        unsafe {
            // SAFETY: Practically, this is a pin projection.
            ready!(Pin::new_unchecked(&mut **this.raw.get_unchecked_mut())
                .poll_with_strategy(strategy, cx));
        }

        Poll::Ready(RwLockWriteGuardArc {
            lock: unsafe { ManuallyDrop::take(this.lock) },
        })
    }
}
