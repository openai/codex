use std::{
    cell::UnsafeCell,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::atomic::{AtomicU8, Ordering},
};

pub(crate) struct OnceCell<T> {
    state: AtomicU8,
    value: UnsafeCell<Option<T>>,
}

const INCOMPLETE: u8 = 0x0;
const RUNNING: u8 = 0x1;
const COMPLETE: u8 = 0x2;

// Why do we need `T: Send`?
// Thread A creates a `OnceCell` and shares it with
// scoped thread B, which fills the cell, which is
// then destroyed by A. That is, destructor observes
// a sent value.
unsafe impl<T: Sync + Send> Sync for OnceCell<T> {}
unsafe impl<T: Send> Send for OnceCell<T> {}

impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for OnceCell<T> {}
impl<T: UnwindSafe> UnwindSafe for OnceCell<T> {}

impl<T> OnceCell<T> {
    pub(crate) const fn new() -> OnceCell<T> {
        OnceCell { state: AtomicU8::new(INCOMPLETE), value: UnsafeCell::new(None) }
    }

    pub(crate) const fn with_value(value: T) -> OnceCell<T> {
        OnceCell { state: AtomicU8::new(COMPLETE), value: UnsafeCell::new(Some(value)) }
    }

    /// Safety: synchronizes with store to value via Release/Acquire.
    #[inline]
    pub(crate) fn is_initialized(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }

    /// Safety: synchronizes with store to value via `is_initialized` or mutex
    /// lock/unlock, writes value only once because of the mutex.
    #[cold]
    pub(crate) fn initialize<F, E>(&self, f: F) -> Result<(), E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mut f = Some(f);
        let mut res: Result<(), E> = Ok(());
        let slot: *mut Option<T> = self.value.get();
        initialize_inner(&self.state, &mut || {
            // We are calling user-supplied function and need to be careful.
            // - if it returns Err, we unlock mutex and return without touching anything
            // - if it panics, we unlock mutex and propagate panic without touching anything
            // - if it calls `set` or `get_or_try_init` re-entrantly, we get a deadlock on
            //   mutex, which is important for safety. We *could* detect this and panic,
            //   but that is more complicated
            // - finally, if it returns Ok, we store the value and store the flag with
            //   `Release`, which synchronizes with `Acquire`s.
            let f = unsafe { f.take().unwrap_unchecked() };
            match f() {
                Ok(value) => unsafe {
                    // Safe b/c we have a unique access and no panic may happen
                    // until the cell is marked as initialized.
                    debug_assert!((*slot).is_none());
                    *slot = Some(value);
                    true
                },
                Err(err) => {
                    res = Err(err);
                    false
                }
            }
        });
        res
    }

    #[cold]
    pub(crate) fn wait(&self) {
        let key = &self.state as *const _ as usize;
        unsafe {
            parking_lot_core::park(
                key,
                || self.state.load(Ordering::Acquire) != COMPLETE,
                || (),
                |_, _| (),
                parking_lot_core::DEFAULT_PARK_TOKEN,
                None,
            );
        }
    }

    /// Get the reference to the underlying value, without checking if the cell
    /// is initialized.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the cell is in initialized state, and that
    /// the contents are acquired by (synchronized to) this thread.
    pub(crate) unsafe fn get_unchecked(&self) -> &T {
        debug_assert!(self.is_initialized());
        let slot = &*self.value.get();
        slot.as_ref().unwrap_unchecked()
    }

    /// Gets the mutable reference to the underlying value.
    /// Returns `None` if the cell is empty.
    pub(crate) fn get_mut(&mut self) -> Option<&mut T> {
        // Safe b/c we have an exclusive access
        let slot: &mut Option<T> = unsafe { &mut *self.value.get() };
        slot.as_mut()
    }

    /// Consumes this `OnceCell`, returning the wrapped value.
    /// Returns `None` if the cell was empty.
    pub(crate) fn into_inner(self) -> Option<T> {
        self.value.into_inner()
    }
}

struct Guard<'a> {
    state: &'a AtomicU8,
    new_state: u8,
}

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        self.state.store(self.new_state, Ordering::Release);
        unsafe {
            let key = self.state as *const AtomicU8 as usize;
            parking_lot_core::unpark_all(key, parking_lot_core::DEFAULT_UNPARK_TOKEN);
        }
    }
}

// Note: this is intentionally monomorphic
#[inline(never)]
fn initialize_inner(state: &AtomicU8, init: &mut dyn FnMut() -> bool) {
    loop {
        let exchange =
            state.compare_exchange_weak(INCOMPLETE, RUNNING, Ordering::Acquire, Ordering::Acquire);
        match exchange {
            Ok(_) => {
                let mut guard = Guard { state, new_state: INCOMPLETE };
                if init() {
                    guard.new_state = COMPLETE;
                }
                return;
            }
            Err(COMPLETE) => return,
            Err(RUNNING) => unsafe {
                let key = state as *const AtomicU8 as usize;
                parking_lot_core::park(
                    key,
                    || state.load(Ordering::Relaxed) == RUNNING,
                    || (),
                    |_, _| (),
                    parking_lot_core::DEFAULT_PARK_TOKEN,
                    None,
                );
            },
            Err(INCOMPLETE) => (),
            Err(_) => debug_assert!(false),
        }
    }
}

#[test]
fn test_size() {
    use std::mem::size_of;

    assert_eq!(size_of::<OnceCell<bool>>(), 1 * size_of::<bool>() + size_of::<u8>());
}
