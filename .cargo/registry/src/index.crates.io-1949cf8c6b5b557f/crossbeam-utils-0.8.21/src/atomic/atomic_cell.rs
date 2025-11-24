// Necessary for implementing atomic methods for `AtomicUnit`
#![allow(clippy::unit_arg)]

use crate::primitive::sync::atomic::{self, Ordering};
use crate::CachePadded;
use core::cell::UnsafeCell;
use core::cmp;
use core::fmt;
use core::mem::{self, ManuallyDrop, MaybeUninit};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr;

use super::seq_lock::SeqLock;

/// A thread-safe mutable memory location.
///
/// This type is equivalent to [`Cell`], except it can also be shared among multiple threads.
///
/// Operations on `AtomicCell`s use atomic instructions whenever possible, and synchronize using
/// global locks otherwise. You can call [`AtomicCell::<T>::is_lock_free()`] to check whether
/// atomic instructions or locks will be used.
///
/// Atomic loads use the [`Acquire`] ordering and atomic stores use the [`Release`] ordering.
///
/// [`Cell`]: std::cell::Cell
/// [`AtomicCell::<T>::is_lock_free()`]: AtomicCell::is_lock_free
/// [`Acquire`]: std::sync::atomic::Ordering::Acquire
/// [`Release`]: std::sync::atomic::Ordering::Release
#[repr(transparent)]
pub struct AtomicCell<T> {
    /// The inner value.
    ///
    /// If this value can be transmuted into a primitive atomic type, it will be treated as such.
    /// Otherwise, all potentially concurrent operations on this data will be protected by a global
    /// lock.
    ///
    /// Using MaybeUninit to prevent code outside the cell from observing partially initialized state:
    /// <https://github.com/crossbeam-rs/crossbeam/issues/833>
    /// (This rustc bug has been fixed in Rust 1.64.)
    ///
    /// Note:
    /// - we'll never store uninitialized `T` due to our API only using initialized `T`.
    /// - this `MaybeUninit` does *not* fix <https://github.com/crossbeam-rs/crossbeam/issues/315>.
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for AtomicCell<T> {}
unsafe impl<T: Send> Sync for AtomicCell<T> {}

impl<T> UnwindSafe for AtomicCell<T> {}
impl<T> RefUnwindSafe for AtomicCell<T> {}

impl<T> AtomicCell<T> {
    /// Creates a new atomic cell initialized with `val`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(7);
    /// ```
    pub const fn new(val: T) -> AtomicCell<T> {
        AtomicCell {
            value: UnsafeCell::new(MaybeUninit::new(val)),
        }
    }

    /// Consumes the atomic and returns the contained value.
    ///
    /// This is safe because passing `self` by value guarantees that no other threads are
    /// concurrently accessing the atomic data.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(7);
    /// let v = a.into_inner();
    ///
    /// assert_eq!(v, 7);
    /// ```
    pub fn into_inner(self) -> T {
        let this = ManuallyDrop::new(self);
        // SAFETY:
        // - passing `self` by value guarantees that no other threads are concurrently
        //   accessing the atomic data
        // - the raw pointer passed in is valid because we got it from an owned value.
        // - `ManuallyDrop` prevents double dropping `T`
        unsafe { this.as_ptr().read() }
    }

    /// Returns `true` if operations on values of this type are lock-free.
    ///
    /// If the compiler or the platform doesn't support the necessary atomic instructions,
    /// `AtomicCell<T>` will use global locks for every potentially concurrent atomic operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// // This type is internally represented as `AtomicUsize` so we can just use atomic
    /// // operations provided by it.
    /// assert_eq!(AtomicCell::<usize>::is_lock_free(), true);
    ///
    /// // A wrapper struct around `isize`.
    /// struct Foo {
    ///     bar: isize,
    /// }
    /// // `AtomicCell<Foo>` will be internally represented as `AtomicIsize`.
    /// assert_eq!(AtomicCell::<Foo>::is_lock_free(), true);
    ///
    /// // Operations on zero-sized types are always lock-free.
    /// assert_eq!(AtomicCell::<()>::is_lock_free(), true);
    ///
    /// // Very large types cannot be represented as any of the standard atomic types, so atomic
    /// // operations on them will have to use global locks for synchronization.
    /// assert_eq!(AtomicCell::<[u8; 1000]>::is_lock_free(), false);
    /// ```
    pub const fn is_lock_free() -> bool {
        atomic_is_lock_free::<T>()
    }

    /// Stores `val` into the atomic cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(7);
    ///
    /// assert_eq!(a.load(), 7);
    /// a.store(8);
    /// assert_eq!(a.load(), 8);
    /// ```
    pub fn store(&self, val: T) {
        if mem::needs_drop::<T>() {
            drop(self.swap(val));
        } else {
            unsafe {
                atomic_store(self.as_ptr(), val);
            }
        }
    }

    /// Stores `val` into the atomic cell and returns the previous value.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(7);
    ///
    /// assert_eq!(a.load(), 7);
    /// assert_eq!(a.swap(8), 7);
    /// assert_eq!(a.load(), 8);
    /// ```
    pub fn swap(&self, val: T) -> T {
        unsafe { atomic_swap(self.as_ptr(), val) }
    }

    /// Returns a raw pointer to the underlying data in this atomic cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(5);
    ///
    /// let ptr = a.as_ptr();
    /// ```
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.value.get().cast::<T>()
    }
}

impl<T: Default> AtomicCell<T> {
    /// Takes the value of the atomic cell, leaving `Default::default()` in its place.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(5);
    /// let five = a.take();
    ///
    /// assert_eq!(five, 5);
    /// assert_eq!(a.into_inner(), 0);
    /// ```
    pub fn take(&self) -> T {
        self.swap(Default::default())
    }
}

impl<T: Copy> AtomicCell<T> {
    /// Loads a value from the atomic cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(7);
    ///
    /// assert_eq!(a.load(), 7);
    /// ```
    pub fn load(&self) -> T {
        unsafe { atomic_load(self.as_ptr()) }
    }
}

impl<T: Copy + Eq> AtomicCell<T> {
    /// If the current value equals `current`, stores `new` into the atomic cell.
    ///
    /// The return value is always the previous value. If it is equal to `current`, then the value
    /// was updated.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(deprecated)]
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(1);
    ///
    /// assert_eq!(a.compare_and_swap(2, 3), 1);
    /// assert_eq!(a.load(), 1);
    ///
    /// assert_eq!(a.compare_and_swap(1, 2), 1);
    /// assert_eq!(a.load(), 2);
    /// ```
    // TODO: remove in the next major version.
    #[deprecated(note = "Use `compare_exchange` instead")]
    pub fn compare_and_swap(&self, current: T, new: T) -> T {
        match self.compare_exchange(current, new) {
            Ok(v) => v,
            Err(v) => v,
        }
    }

    /// If the current value equals `current`, stores `new` into the atomic cell.
    ///
    /// The return value is a result indicating whether the new value was written and containing
    /// the previous value. On success this value is guaranteed to be equal to `current`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(1);
    ///
    /// assert_eq!(a.compare_exchange(2, 3), Err(1));
    /// assert_eq!(a.load(), 1);
    ///
    /// assert_eq!(a.compare_exchange(1, 2), Ok(1));
    /// assert_eq!(a.load(), 2);
    /// ```
    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
        unsafe { atomic_compare_exchange_weak(self.as_ptr(), current, new) }
    }

    /// Fetches the value, and applies a function to it that returns an optional
    /// new value. Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`, else
    /// `Err(previous_value)`.
    ///
    /// Note: This may call the function multiple times if the value has been changed from other threads in
    /// the meantime, as long as the function returns `Some(_)`, but the function will have been applied
    /// only once to the stored value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(7);
    /// assert_eq!(a.fetch_update(|_| None), Err(7));
    /// assert_eq!(a.fetch_update(|a| Some(a + 1)), Ok(7));
    /// assert_eq!(a.fetch_update(|a| Some(a + 1)), Ok(8));
    /// assert_eq!(a.load(), 9);
    /// ```
    #[inline]
    pub fn fetch_update<F>(&self, mut f: F) -> Result<T, T>
    where
        F: FnMut(T) -> Option<T>,
    {
        let mut prev = self.load();
        while let Some(next) = f(prev) {
            match self.compare_exchange(prev, next) {
                x @ Ok(_) => return x,
                Err(next_prev) => prev = next_prev,
            }
        }
        Err(prev)
    }
}

// `MaybeUninit` prevents `T` from being dropped, so we need to implement `Drop`
// for `AtomicCell` to avoid leaks of non-`Copy` types.
impl<T> Drop for AtomicCell<T> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            // SAFETY:
            // - the mutable reference guarantees that no other threads are concurrently accessing the atomic data
            // - the raw pointer passed in is valid because we got it from a reference
            // - `MaybeUninit` prevents double dropping `T`
            unsafe {
                self.as_ptr().drop_in_place();
            }
        }
    }
}

macro_rules! atomic {
    // If values of type `$t` can be transmuted into values of the primitive atomic type `$atomic`,
    // declares variable `$a` of type `$atomic` and executes `$atomic_op`, breaking out of the loop.
    (@check, $t:ty, $atomic:ty, $a:ident, $atomic_op:expr) => {
        if can_transmute::<$t, $atomic>() {
            let $a: &$atomic;
            break $atomic_op;
        }
    };

    // If values of type `$t` can be transmuted into values of a primitive atomic type, declares
    // variable `$a` of that type and executes `$atomic_op`. Otherwise, just executes
    // `$fallback_op`.
    ($t:ty, $a:ident, $atomic_op:expr, $fallback_op:expr) => {
        loop {
            atomic!(@check, $t, AtomicUnit, $a, $atomic_op);

            atomic!(@check, $t, atomic::AtomicU8, $a, $atomic_op);
            atomic!(@check, $t, atomic::AtomicU16, $a, $atomic_op);
            atomic!(@check, $t, atomic::AtomicU32, $a, $atomic_op);
            #[cfg(target_has_atomic = "64")]
            atomic!(@check, $t, atomic::AtomicU64, $a, $atomic_op);
            // TODO: AtomicU128 is unstable
            // atomic!(@check, $t, atomic::AtomicU128, $a, $atomic_op);

            break $fallback_op;
        }
    };
}

macro_rules! impl_arithmetic {
    ($t:ty, fallback, $example:tt) => {
        impl AtomicCell<$t> {
            /// Increments the current value by `val` and returns the previous value.
            ///
            /// The addition wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_add(3), 7);
            /// assert_eq!(a.load(), 10);
            /// ```
            #[inline]
            pub fn fetch_add(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = value.wrapping_add(val);
                old
            }

            /// Decrements the current value by `val` and returns the previous value.
            ///
            /// The subtraction wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_sub(3), 7);
            /// assert_eq!(a.load(), 4);
            /// ```
            #[inline]
            pub fn fetch_sub(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = value.wrapping_sub(val);
                old
            }

            /// Applies bitwise "and" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_and(3), 7);
            /// assert_eq!(a.load(), 3);
            /// ```
            #[inline]
            pub fn fetch_and(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value &= val;
                old
            }

            /// Applies bitwise "nand" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_nand(3), 7);
            /// assert_eq!(a.load(), !(7 & 3));
            /// ```
            #[inline]
            pub fn fetch_nand(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = !(old & val);
                old
            }

            /// Applies bitwise "or" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_or(16), 7);
            /// assert_eq!(a.load(), 23);
            /// ```
            #[inline]
            pub fn fetch_or(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value |= val;
                old
            }

            /// Applies bitwise "xor" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_xor(2), 7);
            /// assert_eq!(a.load(), 5);
            /// ```
            #[inline]
            pub fn fetch_xor(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value ^= val;
                old
            }

            /// Compares and sets the maximum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_max(2), 7);
            /// assert_eq!(a.load(), 7);
            /// ```
            #[inline]
            pub fn fetch_max(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = cmp::max(old, val);
                old
            }

            /// Compares and sets the minimum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_min(2), 7);
            /// assert_eq!(a.load(), 2);
            /// ```
            #[inline]
            pub fn fetch_min(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = cmp::min(old, val);
                old
            }
        }
    };
    ($t:ty, $atomic:ident, $example:tt) => {
        impl AtomicCell<$t> {
            /// Increments the current value by `val` and returns the previous value.
            ///
            /// The addition wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_add(3), 7);
            /// assert_eq!(a.load(), 10);
            /// ```
            #[inline]
            pub fn fetch_add(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_add(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = value.wrapping_add(val);
                        old
                    }
                }
            }

            /// Decrements the current value by `val` and returns the previous value.
            ///
            /// The subtraction wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_sub(3), 7);
            /// assert_eq!(a.load(), 4);
            /// ```
            #[inline]
            pub fn fetch_sub(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_sub(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = value.wrapping_sub(val);
                        old
                    }
                }
            }

            /// Applies bitwise "and" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_and(3), 7);
            /// assert_eq!(a.load(), 3);
            /// ```
            #[inline]
            pub fn fetch_and(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_and(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value &= val;
                        old
                    }
                }
            }

            /// Applies bitwise "nand" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_nand(3), 7);
            /// assert_eq!(a.load(), !(7 & 3));
            /// ```
            #[inline]
            pub fn fetch_nand(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_nand(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = !(old & val);
                        old
                    }
                }
            }

            /// Applies bitwise "or" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_or(16), 7);
            /// assert_eq!(a.load(), 23);
            /// ```
            #[inline]
            pub fn fetch_or(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_or(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value |= val;
                        old
                    }
                }
            }

            /// Applies bitwise "xor" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_xor(2), 7);
            /// assert_eq!(a.load(), 5);
            /// ```
            #[inline]
            pub fn fetch_xor(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_xor(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value ^= val;
                        old
                    }
                }
            }

            /// Compares and sets the maximum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_max(9), 7);
            /// assert_eq!(a.load(), 9);
            /// ```
            #[inline]
            pub fn fetch_max(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_max(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = cmp::max(old, val);
                        old
                    }
                }
            }

            /// Compares and sets the minimum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_min(2), 7);
            /// assert_eq!(a.load(), 2);
            /// ```
            #[inline]
            pub fn fetch_min(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_min(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = cmp::min(old, val);
                        old
                    }
                }
            }
        }
    };
}

impl_arithmetic!(u8, AtomicU8, "let a = AtomicCell::new(7u8);");
impl_arithmetic!(i8, AtomicI8, "let a = AtomicCell::new(7i8);");
impl_arithmetic!(u16, AtomicU16, "let a = AtomicCell::new(7u16);");
impl_arithmetic!(i16, AtomicI16, "let a = AtomicCell::new(7i16);");

impl_arithmetic!(u32, AtomicU32, "let a = AtomicCell::new(7u32);");
impl_arithmetic!(i32, AtomicI32, "let a = AtomicCell::new(7i32);");

#[cfg(target_has_atomic = "64")]
impl_arithmetic!(u64, AtomicU64, "let a = AtomicCell::new(7u64);");
#[cfg(target_has_atomic = "64")]
impl_arithmetic!(i64, AtomicI64, "let a = AtomicCell::new(7i64);");
#[cfg(not(target_has_atomic = "64"))]
impl_arithmetic!(u64, fallback, "let a = AtomicCell::new(7u64);");
#[cfg(not(target_has_atomic = "64"))]
impl_arithmetic!(i64, fallback, "let a = AtomicCell::new(7i64);");

// TODO: AtomicU128 is unstable
// impl_arithmetic!(u128, AtomicU128, "let a = AtomicCell::new(7u128);");
// impl_arithmetic!(i128, AtomicI128, "let a = AtomicCell::new(7i128);");
impl_arithmetic!(u128, fallback, "let a = AtomicCell::new(7u128);");
impl_arithmetic!(i128, fallback, "let a = AtomicCell::new(7i128);");

impl_arithmetic!(usize, AtomicUsize, "let a = AtomicCell::new(7usize);");
impl_arithmetic!(isize, AtomicIsize, "let a = AtomicCell::new(7isize);");

impl AtomicCell<bool> {
    /// Applies logical "and" to the current value and returns the previous value.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(true);
    ///
    /// assert_eq!(a.fetch_and(true), true);
    /// assert_eq!(a.load(), true);
    ///
    /// assert_eq!(a.fetch_and(false), true);
    /// assert_eq!(a.load(), false);
    /// ```
    #[inline]
    pub fn fetch_and(&self, val: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };
                a.fetch_and(val, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value &= val;
                old
            }
        }
    }

    /// Applies logical "nand" to the current value and returns the previous value.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(true);
    ///
    /// assert_eq!(a.fetch_nand(false), true);
    /// assert_eq!(a.load(), true);
    ///
    /// assert_eq!(a.fetch_nand(true), true);
    /// assert_eq!(a.load(), false);
    ///
    /// assert_eq!(a.fetch_nand(false), false);
    /// assert_eq!(a.load(), true);
    /// ```
    #[inline]
    pub fn fetch_nand(&self, val: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };
                a.fetch_nand(val, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = !(old & val);
                old
            }
        }
    }

    /// Applies logical "or" to the current value and returns the previous value.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(false);
    ///
    /// assert_eq!(a.fetch_or(false), false);
    /// assert_eq!(a.load(), false);
    ///
    /// assert_eq!(a.fetch_or(true), false);
    /// assert_eq!(a.load(), true);
    /// ```
    #[inline]
    pub fn fetch_or(&self, val: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };
                a.fetch_or(val, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value |= val;
                old
            }
        }
    }

    /// Applies logical "xor" to the current value and returns the previous value.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::atomic::AtomicCell;
    ///
    /// let a = AtomicCell::new(true);
    ///
    /// assert_eq!(a.fetch_xor(false), true);
    /// assert_eq!(a.load(), true);
    ///
    /// assert_eq!(a.fetch_xor(true), true);
    /// assert_eq!(a.load(), false);
    /// ```
    #[inline]
    pub fn fetch_xor(&self, val: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };
                a.fetch_xor(val, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value ^= val;
                old
            }
        }
    }
}

impl<T: Default> Default for AtomicCell<T> {
    fn default() -> AtomicCell<T> {
        AtomicCell::new(T::default())
    }
}

impl<T> From<T> for AtomicCell<T> {
    #[inline]
    fn from(val: T) -> AtomicCell<T> {
        AtomicCell::new(val)
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for AtomicCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtomicCell")
            .field("value", &self.load())
            .finish()
    }
}

/// Returns `true` if values of type `A` can be transmuted into values of type `B`.
const fn can_transmute<A, B>() -> bool {
    // Sizes must be equal, but alignment of `A` must be greater or equal than that of `B`.
    (mem::size_of::<A>() == mem::size_of::<B>()) & (mem::align_of::<A>() >= mem::align_of::<B>())
}

/// Returns a reference to the global lock associated with the `AtomicCell` at address `addr`.
///
/// This function is used to protect atomic data which doesn't fit into any of the primitive atomic
/// types in `std::sync::atomic`. Operations on such atomics must therefore use a global lock.
///
/// However, there is not only one global lock but an array of many locks, and one of them is
/// picked based on the given address. Having many locks reduces contention and improves
/// scalability.
#[inline]
#[must_use]
fn lock(addr: usize) -> &'static SeqLock {
    // The number of locks is a prime number because we want to make sure `addr % LEN` gets
    // dispersed across all locks.
    //
    // Note that addresses are always aligned to some power of 2, depending on type `T` in
    // `AtomicCell<T>`. If `LEN` was an even number, then `addr % LEN` would be an even number,
    // too, which means only half of the locks would get utilized!
    //
    // It is also possible for addresses to accidentally get aligned to a number that is not a
    // power of 2. Consider this example:
    //
    // ```
    // #[repr(C)]
    // struct Foo {
    //     a: AtomicCell<u8>,
    //     b: u8,
    //     c: u8,
    // }
    // ```
    //
    // Now, if we have a slice of type `&[Foo]`, it is possible that field `a` in all items gets
    // stored at addresses that are multiples of 3. It'd be too bad if `LEN` was divisible by 3.
    // In order to protect from such cases, we simply choose a large prime number for `LEN`.
    const LEN: usize = 67;
    const L: CachePadded<SeqLock> = CachePadded::new(SeqLock::new());
    static LOCKS: [CachePadded<SeqLock>; LEN] = [L; LEN];

    // If the modulus is a constant number, the compiler will use crazy math to transform this into
    // a sequence of cheap arithmetic operations rather than using the slow modulo instruction.
    &LOCKS[addr % LEN]
}

/// An atomic `()`.
///
/// All operations are noops.
struct AtomicUnit;

impl AtomicUnit {
    #[inline]
    fn load(&self, _order: Ordering) {}

    #[inline]
    fn store(&self, _val: (), _order: Ordering) {}

    #[inline]
    fn swap(&self, _val: (), _order: Ordering) {}

    #[inline]
    fn compare_exchange_weak(
        &self,
        _current: (),
        _new: (),
        _success: Ordering,
        _failure: Ordering,
    ) -> Result<(), ()> {
        Ok(())
    }
}

/// Returns `true` if operations on `AtomicCell<T>` are lock-free.
const fn atomic_is_lock_free<T>() -> bool {
    atomic! { T, _a, true, false }
}

/// Atomically reads data from `src`.
///
/// This operation uses the `Acquire` ordering. If possible, an atomic instructions is used, and a
/// global lock otherwise.
unsafe fn atomic_load<T>(src: *mut T) -> T
where
    T: Copy,
{
    atomic! {
        T, a,
        {
            a = &*(src as *const _ as *const _);
            mem::transmute_copy(&a.load(Ordering::Acquire))
        },
        {
            let lock = lock(src as usize);

            // Try doing an optimistic read first.
            if let Some(stamp) = lock.optimistic_read() {
                // We need a volatile read here because other threads might concurrently modify the
                // value. In theory, data races are *always* UB, even if we use volatile reads and
                // discard the data when a data race is detected. The proper solution would be to
                // do atomic reads and atomic writes, but we can't atomically read and write all
                // kinds of data since `AtomicU8` is not available on stable Rust yet.
                // Load as `MaybeUninit` because we may load a value that is not valid as `T`.
                let val = ptr::read_volatile(src.cast::<MaybeUninit<T>>());

                if lock.validate_read(stamp) {
                    return val.assume_init();
                }
            }

            // Grab a regular write lock so that writers don't starve this load.
            let guard = lock.write();
            let val = ptr::read(src);
            // The value hasn't been changed. Drop the guard without incrementing the stamp.
            guard.abort();
            val
        }
    }
}

/// Atomically writes `val` to `dst`.
///
/// This operation uses the `Release` ordering. If possible, an atomic instructions is used, and a
/// global lock otherwise.
unsafe fn atomic_store<T>(dst: *mut T, val: T) {
    atomic! {
        T, a,
        {
            a = &*(dst as *const _ as *const _);
            a.store(mem::transmute_copy(&val), Ordering::Release);
            mem::forget(val);
        },
        {
            let _guard = lock(dst as usize).write();
            ptr::write(dst, val);
        }
    }
}

/// Atomically swaps data at `dst` with `val`.
///
/// This operation uses the `AcqRel` ordering. If possible, an atomic instructions is used, and a
/// global lock otherwise.
unsafe fn atomic_swap<T>(dst: *mut T, val: T) -> T {
    atomic! {
        T, a,
        {
            a = &*(dst as *const _ as *const _);
            let res = mem::transmute_copy(&a.swap(mem::transmute_copy(&val), Ordering::AcqRel));
            mem::forget(val);
            res
        },
        {
            let _guard = lock(dst as usize).write();
            ptr::replace(dst, val)
        }
    }
}

/// Atomically compares data at `dst` to `current` and, if equal byte-for-byte, exchanges data at
/// `dst` with `new`.
///
/// Returns the old value on success, or the current value at `dst` on failure.
///
/// This operation uses the `AcqRel` ordering. If possible, an atomic instructions is used, and a
/// global lock otherwise.
#[allow(clippy::let_unit_value)]
unsafe fn atomic_compare_exchange_weak<T>(dst: *mut T, mut current: T, new: T) -> Result<T, T>
where
    T: Copy + Eq,
{
    atomic! {
        T, a,
        {
            a = &*(dst as *const _ as *const _);
            let mut current_raw = mem::transmute_copy(&current);
            let new_raw = mem::transmute_copy(&new);

            loop {
                match a.compare_exchange_weak(
                    current_raw,
                    new_raw,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break Ok(current),
                    Err(previous_raw) => {
                        let previous = mem::transmute_copy(&previous_raw);

                        if !T::eq(&previous, &current) {
                            break Err(previous);
                        }

                        // The compare-exchange operation has failed and didn't store `new`. The
                        // failure is either spurious, or `previous` was semantically equal to
                        // `current` but not byte-equal. Let's retry with `previous` as the new
                        // `current`.
                        current = previous;
                        current_raw = previous_raw;
                    }
                }
            }
        },
        {
            let guard = lock(dst as usize).write();

            if T::eq(&*dst, &current) {
                Ok(ptr::replace(dst, new))
            } else {
                let val = ptr::read(dst);
                // The value hasn't been changed. Drop the guard without incrementing the stamp.
                guard.abort();
                Err(val)
            }
        }
    }
}
