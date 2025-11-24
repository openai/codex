use std::cell::UnsafeCell;
use std::fmt;
use std::ops::Deref;
use std::panic;

/// `AtomicU16` providing an additional `unsync_load` function.
pub(crate) struct AtomicU16 {
    inner: UnsafeCell<std::sync::atomic::AtomicU16>,
}

unsafe impl Send for AtomicU16 {}
unsafe impl Sync for AtomicU16 {}
impl panic::RefUnwindSafe for AtomicU16 {}
impl panic::UnwindSafe for AtomicU16 {}

impl AtomicU16 {
    pub(crate) const fn new(val: u16) -> AtomicU16 {
        let inner = UnsafeCell::new(std::sync::atomic::AtomicU16::new(val));
        AtomicU16 { inner }
    }

    /// Performs an unsynchronized load.
    ///
    /// # Safety
    ///
    /// All mutations must have happened before the unsynchronized load.
    /// Additionally, there must be no concurrent mutations.
    pub(crate) unsafe fn unsync_load(&self) -> u16 {
        core::ptr::read(self.inner.get() as *const u16)
    }
}

impl Deref for AtomicU16 {
    type Target = std::sync::atomic::AtomicU16;

    fn deref(&self) -> &Self::Target {
        // safety: it is always safe to access `&self` fns on the inner value as
        // we never perform unsafe mutations.
        unsafe { &*self.inner.get() }
    }
}

impl fmt::Debug for AtomicU16 {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(fmt)
    }
}
