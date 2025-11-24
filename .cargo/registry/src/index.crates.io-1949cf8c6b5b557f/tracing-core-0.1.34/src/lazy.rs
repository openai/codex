#[cfg(feature = "std")]
pub(crate) use once_cell::sync::Lazy;

#[cfg(not(feature = "std"))]
pub(crate) use self::spin::Lazy;

#[cfg(not(feature = "std"))]
mod spin {
    //! This is the `once_cell::sync::Lazy` type, but modified to use our
    //! `spin::Once` type rather than `OnceCell`. This is used to replace
    //! `once_cell::sync::Lazy` on `no-std` builds.
    use crate::spin::Once;
    use core::{cell::Cell, fmt, ops::Deref};

    /// Re-implementation of `once_cell::sync::Lazy` on top of `spin::Once`
    /// rather than `OnceCell`.
    ///
    /// This is used when the standard library is disabled.
    pub(crate) struct Lazy<T, F = fn() -> T> {
        cell: Once<T>,
        init: Cell<Option<F>>,
    }

    impl<T: fmt::Debug, F> fmt::Debug for Lazy<T, F> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Lazy")
                .field("cell", &self.cell)
                .field("init", &"..")
                .finish()
        }
    }

    // We never create a `&F` from a `&Lazy<T, F>` so it is fine to not impl
    // `Sync` for `F`. We do create a `&mut Option<F>` in `force`, but this is
    // properly synchronized, so it only happens once so it also does not
    // contribute to this impl.
    unsafe impl<T, F: Send> Sync for Lazy<T, F> where Once<T>: Sync {}
    // auto-derived `Send` impl is OK.

    impl<T, F> Lazy<T, F> {
        /// Creates a new lazy value with the given initializing function.
        pub(crate) const fn new(init: F) -> Lazy<T, F> {
            Lazy {
                cell: Once::new(),
                init: Cell::new(Some(init)),
            }
        }
    }

    impl<T, F: FnOnce() -> T> Lazy<T, F> {
        /// Forces the evaluation of this lazy value and returns a reference to
        /// the result.
        ///
        /// This is equivalent to the `Deref` impl, but is explicit.
        pub(crate) fn force(this: &Lazy<T, F>) -> &T {
            this.cell.call_once(|| match this.init.take() {
                Some(f) => f(),
                None => panic!("Lazy instance has previously been poisoned"),
            })
        }
    }

    impl<T, F: FnOnce() -> T> Deref for Lazy<T, F> {
        type Target = T;
        fn deref(&self) -> &T {
            Lazy::force(self)
        }
    }

    impl<T: Default> Default for Lazy<T> {
        /// Creates a new lazy value using `Default` as the initializing function.
        fn default() -> Lazy<T> {
            Lazy::new(T::default)
        }
    }
}
