use core::cell::UnsafeCell;
use core::fmt;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicUsize, Ordering};

/// A synchronization primitive which can be used to run a one-time global
/// initialization. Unlike its std equivalent, this is generalized so that the
/// closure returns a value and it is stored. Once therefore acts something like
/// a future, too.
pub struct Once<T> {
    state: AtomicUsize,
    data: UnsafeCell<Option<T>>, // TODO remove option and use mem::uninitialized
}

impl<T: fmt::Debug> fmt::Debug for Once<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.r#try() {
            Some(s) => write!(f, "Once {{ data: ")
                .and_then(|()| s.fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "Once {{ <uninitialized> }}"),
        }
    }
}

// Same unsafe impls as `std::sync::RwLock`, because this also allows for
// concurrent reads.
unsafe impl<T: Send + Sync> Sync for Once<T> {}
unsafe impl<T: Send> Send for Once<T> {}

// Four states that a Once can be in, encoded into the lower bits of `state` in
// the Once structure.
const INCOMPLETE: usize = 0x0;
const RUNNING: usize = 0x1;
const COMPLETE: usize = 0x2;
const PANICKED: usize = 0x3;

use core::hint::unreachable_unchecked as unreachable;

impl<T> Once<T> {
    /// Initialization constant of `Once`.
    pub const INIT: Self = Once {
        state: AtomicUsize::new(INCOMPLETE),
        data: UnsafeCell::new(None),
    };

    /// Creates a new `Once` value.
    pub const fn new() -> Once<T> {
        Self::INIT
    }

    fn force_get<'a>(&'a self) -> &'a T {
        match unsafe { &*self.data.get() }.as_ref() {
            None => unsafe { unreachable() },
            Some(p) => p,
        }
    }

    /// Performs an initialization routine once and only once. The given closure
    /// will be executed if this is the first time `call_once` has been called,
    /// and otherwise the routine will *not* be invoked.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// When this function returns, it is guaranteed that some initialization
    /// has run and completed (it may not be the closure specified). The
    /// returned pointer will point to the result from the closure that was
    /// run.
    pub fn call_once<'a, F>(&'a self, builder: F) -> &'a T
    where
        F: FnOnce() -> T,
    {
        let mut status = self.state.load(Ordering::SeqCst);

        if status == INCOMPLETE {
            status = match self.state.compare_exchange(
                INCOMPLETE,
                RUNNING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(status) => {
                    debug_assert_eq!(
                        status, INCOMPLETE,
                        "if compare_exchange succeeded, previous status must be incomplete",
                    );
                    // We init
                    // We use a guard (Finish) to catch panics caused by builder
                    let mut finish = Finish {
                        state: &self.state,
                        panicked: true,
                    };
                    unsafe { *self.data.get() = Some(builder()) };
                    finish.panicked = false;

                    self.state.store(COMPLETE, Ordering::SeqCst);

                    // This next line is strictly an optimization
                    return self.force_get();
                }
                Err(status) => status,
            }
        }

        loop {
            match status {
                INCOMPLETE => unreachable!(),
                RUNNING => {
                    // We spin
                    spin_loop();
                    status = self.state.load(Ordering::SeqCst)
                }
                PANICKED => panic!("Once has panicked"),
                COMPLETE => return self.force_get(),
                _ => unsafe { unreachable() },
            }
        }
    }

    /// Returns a pointer iff the `Once` was previously initialized
    pub fn r#try<'a>(&'a self) -> Option<&'a T> {
        match self.state.load(Ordering::SeqCst) {
            COMPLETE => Some(self.force_get()),
            _ => None,
        }
    }

    /// Like try, but will spin if the `Once` is in the process of being
    /// initialized
    pub fn wait<'a>(&'a self) -> Option<&'a T> {
        loop {
            match self.state.load(Ordering::SeqCst) {
                INCOMPLETE => return None,

                RUNNING => {
                    spin_loop() // We spin
                }
                COMPLETE => return Some(self.force_get()),
                PANICKED => panic!("Once has panicked"),
                _ => unsafe { unreachable() },
            }
        }
    }
}

struct Finish<'a> {
    state: &'a AtomicUsize,
    panicked: bool,
}

impl<'a> Drop for Finish<'a> {
    fn drop(&mut self) {
        if self.panicked {
            self.state.store(PANICKED, Ordering::SeqCst);
        }
    }
}
