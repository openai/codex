use std::cell::Cell;
use std::ptr;

/// Scoped thread-local storage
pub(super) struct Scoped<T> {
    pub(super) inner: Cell<*const T>,
}

impl<T> Scoped<T> {
    pub(super) const fn new() -> Scoped<T> {
        Scoped {
            inner: Cell::new(ptr::null()),
        }
    }

    /// Inserts a value into the scoped cell for the duration of the closure
    pub(super) fn set<F, R>(&self, t: &T, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        struct Reset<'a, T> {
            cell: &'a Cell<*const T>,
            prev: *const T,
        }

        impl<T> Drop for Reset<'_, T> {
            fn drop(&mut self) {
                self.cell.set(self.prev);
            }
        }

        let prev = self.inner.get();
        self.inner.set(t as *const _);

        let _reset = Reset {
            cell: &self.inner,
            prev,
        };

        f()
    }

    /// Gets the value out of the scoped cell;
    pub(super) fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        let val = self.inner.get();

        if val.is_null() {
            f(None)
        } else {
            unsafe { f(Some(&*val)) }
        }
    }
}
