#![cfg_attr(not(any(test, feature = "use_std")), no_std)]
#![doc(html_root_url = "https://docs.rs/scopeguard/1/")]

//! A scope guard will run a given closure when it goes out of scope,
//! even if the code between panics.
//! (as long as panic doesn't abort)
//!
//! # Examples
//!
//! ## Hello World
//!
//! This example creates a scope guard with an example function:
//!
//! ```
//! extern crate scopeguard;
//!
//! fn f() {
//!     let _guard = scopeguard::guard((), |_| {
//!         println!("Hello Scope Exit!");
//!     });
//!
//!     // rest of the code here.
//!
//!     // Here, at the end of `_guard`'s scope, the guard's closure is called.
//!     // It is also called if we exit this scope through unwinding instead.
//! }
//! # fn main() {
//! #    f();
//! # }
//! ```
//!
//! ## `defer!`
//!
//! Use the `defer` macro to run an operation at scope exit,
//! either regular scope exit or during unwinding from a panic.
//!
//! ```
//! #[macro_use(defer)] extern crate scopeguard;
//!
//! use std::cell::Cell;
//!
//! fn main() {
//!     // use a cell to observe drops during and after the scope guard is active
//!     let drop_counter = Cell::new(0);
//!     {
//!         // Create a scope guard using `defer!` for the current scope
//!         defer! {
//!             drop_counter.set(1 + drop_counter.get());
//!         }
//!
//!         // Do regular operations here in the meantime.
//!
//!         // Just before scope exit: it hasn't run yet.
//!         assert_eq!(drop_counter.get(), 0);
//!
//!         // The following scope end is where the defer closure is called
//!     }
//!     assert_eq!(drop_counter.get(), 1);
//! }
//! ```
//!
//! ## Scope Guard with Value
//!
//! If the scope guard closure needs to access an outer value that is also
//! mutated outside of the scope guard, then you may want to use the scope guard
//! with a value. The guard works like a smart pointer, so the inner value can
//! be accessed by reference or by mutable reference.
//!
//! ### 1. The guard owns a file
//!
//! In this example, the scope guard owns a file and ensures pending writes are
//! synced at scope exit.
//!
//! ```
//! extern crate scopeguard;
//!
//! use std::fs::*;
//! use std::io::{self, Write};
//! # // Mock file so that we don't actually write a file
//! # struct MockFile;
//! # impl MockFile {
//! #     fn create(_s: &str) -> io::Result<Self> { Ok(MockFile) }
//! #     fn write_all(&self, _b: &[u8]) -> io::Result<()> { Ok(()) }
//! #     fn sync_all(&self) -> io::Result<()> { Ok(()) }
//! # }
//! # use self::MockFile as File;
//!
//! fn try_main() -> io::Result<()> {
//!     let f = File::create("newfile.txt")?;
//!     let mut file = scopeguard::guard(f, |f| {
//!         // ensure we flush file at return or panic
//!         let _ = f.sync_all();
//!     });
//!     // Access the file through the scope guard itself
//!     file.write_all(b"test me\n").map(|_| ())
//! }
//!
//! fn main() {
//!     try_main().unwrap();
//! }
//!
//! ```
//!
//! ### 2. The guard restores an invariant on scope exit
//!
//! ```
//! extern crate scopeguard;
//!
//! use std::mem::ManuallyDrop;
//! use std::ptr;
//!
//! // This function, just for this example, takes the first element
//! // and inserts it into the assumed sorted tail of the vector.
//! //
//! // For optimization purposes we temporarily violate an invariant of the
//! // Vec, that it owns all of its elements.
//! //
//! // The safe approach is to use swap, which means two writes to memory,
//! // the optimization is to use a “hole” which uses only one write of memory
//! // for each position it moves.
//! //
//! // We *must* use a scope guard to run this code safely. We
//! // are running arbitrary user code (comparison operators) that may panic.
//! // The scope guard ensures we restore the invariant after successful
//! // exit or during unwinding from panic.
//! fn insertion_sort_first<T>(v: &mut Vec<T>)
//!     where T: PartialOrd
//! {
//!     struct Hole<'a, T: 'a> {
//!         v: &'a mut Vec<T>,
//!         index: usize,
//!         value: ManuallyDrop<T>,
//!     }
//!
//!     unsafe {
//!         // Create a moved-from location in the vector, a “hole”.
//!         let value = ptr::read(&v[0]);
//!         let mut hole = Hole { v: v, index: 0, value: ManuallyDrop::new(value) };
//!
//!         // Use a scope guard with a value.
//!         // At scope exit, plug the hole so that the vector is fully
//!         // initialized again.
//!         // The scope guard owns the hole, but we can access it through the guard.
//!         let mut hole_guard = scopeguard::guard(hole, |hole| {
//!             // plug the hole in the vector with the value that was // taken out
//!             let index = hole.index;
//!             ptr::copy_nonoverlapping(&*hole.value, &mut hole.v[index], 1);
//!         });
//!
//!         // run algorithm that moves the hole in the vector here
//!         // move the hole until it's in a sorted position
//!         for i in 1..hole_guard.v.len() {
//!             if *hole_guard.value >= hole_guard.v[i] {
//!                 // move the element back and the hole forward
//!                 let index = hole_guard.index;
//!                 hole_guard.v.swap(index, index + 1);
//!                 hole_guard.index += 1;
//!             } else {
//!                 break;
//!             }
//!         }
//!
//!         // When the scope exits here, the Vec becomes whole again!
//!     }
//! }
//!
//! fn main() {
//!     let string = String::from;
//!     let mut data = vec![string("c"), string("a"), string("b"), string("d")];
//!     insertion_sort_first(&mut data);
//!     assert_eq!(data, vec!["a", "b", "c", "d"]);
//! }
//!
//! ```
//!
//!
//! # Crate Features
//!
//! - `use_std`
//!   + Enabled by default. Enables the `OnUnwind` and `OnSuccess` strategies.
//!   + Disable to use `no_std`.
//!
//! # Rust Version
//!
//! This version of the crate requires Rust 1.20 or later.
//!
//! The scopeguard 1.x release series will use a carefully considered version
//! upgrade policy, where in a later 1.x version, we will raise the minimum
//! required Rust version.

#[cfg(not(any(test, feature = "use_std")))]
extern crate core as std;

use std::fmt;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::ptr;

/// Controls in which cases the associated code should be run
pub trait Strategy {
    /// Return `true` if the guard’s associated code should run
    /// (in the context where this method is called).
    fn should_run() -> bool;
}

/// Always run on scope exit.
///
/// “Always” run: on regular exit from a scope or on unwinding from a panic.
/// Can not run on abort, process exit, and other catastrophic events where
/// destructors don’t run.
#[derive(Debug)]
pub enum Always {}

/// Run on scope exit through unwinding.
///
/// Requires crate feature `use_std`.
#[cfg(feature = "use_std")]
#[derive(Debug)]
pub enum OnUnwind {}

/// Run on regular scope exit, when not unwinding.
///
/// Requires crate feature `use_std`.
#[cfg(feature = "use_std")]
#[derive(Debug)]
pub enum OnSuccess {}

impl Strategy for Always {
    #[inline(always)]
    fn should_run() -> bool {
        true
    }
}

#[cfg(feature = "use_std")]
impl Strategy for OnUnwind {
    #[inline]
    fn should_run() -> bool {
        std::thread::panicking()
    }
}

#[cfg(feature = "use_std")]
impl Strategy for OnSuccess {
    #[inline]
    fn should_run() -> bool {
        !std::thread::panicking()
    }
}

/// Macro to create a `ScopeGuard` (always run).
///
/// The macro takes statements, which are the body of a closure
/// that will run when the scope is exited.
#[macro_export]
macro_rules! defer {
    ($($t:tt)*) => {
        let _guard = $crate::guard((), |()| { $($t)* });
    };
}

/// Macro to create a `ScopeGuard` (run on successful scope exit).
///
/// The macro takes statements, which are the body of a closure
/// that will run when the scope is exited.
///
/// Requires crate feature `use_std`.
#[cfg(feature = "use_std")]
#[macro_export]
macro_rules! defer_on_success {
    ($($t:tt)*) => {
        let _guard = $crate::guard_on_success((), |()| { $($t)* });
    };
}

/// Macro to create a `ScopeGuard` (run on unwinding from panic).
///
/// The macro takes statements, which are the body of a closure
/// that will run when the scope is exited.
///
/// Requires crate feature `use_std`.
#[cfg(feature = "use_std")]
#[macro_export]
macro_rules! defer_on_unwind {
    ($($t:tt)*) => {
        let _guard = $crate::guard_on_unwind((), |()| { $($t)* });
    };
}

/// `ScopeGuard` is a scope guard that may own a protected value.
///
/// If you place a guard in a local variable, the closure can
/// run regardless how you leave the scope — through regular return or panic
/// (except if panic or other code aborts; so as long as destructors run).
/// It is run only once.
///
/// The `S` parameter for [`Strategy`](trait.Strategy.html) determines if
/// the closure actually runs.
///
/// The guard's closure will be called with the held value in the destructor.
///
/// The `ScopeGuard` implements `Deref` so that you can access the inner value.
pub struct ScopeGuard<T, F, S = Always>
where
    F: FnOnce(T),
    S: Strategy,
{
    value: ManuallyDrop<T>,
    dropfn: ManuallyDrop<F>,
    // fn(S) -> S is used, so that the S is not taken into account for auto traits.
    strategy: PhantomData<fn(S) -> S>,
}

impl<T, F, S> ScopeGuard<T, F, S>
where
    F: FnOnce(T),
    S: Strategy,
{
    /// Create a `ScopeGuard` that owns `v` (accessible through deref) and calls
    /// `dropfn` when its destructor runs.
    ///
    /// The `Strategy` decides whether the scope guard's closure should run.
    #[inline]
    #[must_use]
    pub fn with_strategy(v: T, dropfn: F) -> ScopeGuard<T, F, S> {
        ScopeGuard {
            value: ManuallyDrop::new(v),
            dropfn: ManuallyDrop::new(dropfn),
            strategy: PhantomData,
        }
    }

    /// “Defuse” the guard and extract the value without calling the closure.
    ///
    /// ```
    /// extern crate scopeguard;
    ///
    /// use scopeguard::{guard, ScopeGuard};
    ///
    /// fn conditional() -> bool { true }
    ///
    /// fn main() {
    ///     let mut guard = guard(Vec::new(), |mut v| v.clear());
    ///     guard.push(1);
    ///
    ///     if conditional() {
    ///         // a condition maybe makes us decide to
    ///         // “defuse” the guard and get back its inner parts
    ///         let value = ScopeGuard::into_inner(guard);
    ///     } else {
    ///         // guard still exists in this branch
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn into_inner(guard: Self) -> T {
        // Cannot move out of `Drop`-implementing types,
        // so `ptr::read` the value and forget the guard.
        let mut guard = ManuallyDrop::new(guard);
        unsafe {
            let value = ptr::read(&*guard.value);
            // Drop the closure after `value` has been read, so that if the
            // closure's `drop` function panics, unwinding still tries to drop
            // `value`.
            ManuallyDrop::drop(&mut guard.dropfn);
            value
        }
    }
}

/// Create a new `ScopeGuard` owning `v` and with deferred closure `dropfn`.
#[inline]
#[must_use]
pub fn guard<T, F>(v: T, dropfn: F) -> ScopeGuard<T, F, Always>
where
    F: FnOnce(T),
{
    ScopeGuard::with_strategy(v, dropfn)
}

/// Create a new `ScopeGuard` owning `v` and with deferred closure `dropfn`.
///
/// Requires crate feature `use_std`.
#[cfg(feature = "use_std")]
#[inline]
#[must_use]
pub fn guard_on_success<T, F>(v: T, dropfn: F) -> ScopeGuard<T, F, OnSuccess>
where
    F: FnOnce(T),
{
    ScopeGuard::with_strategy(v, dropfn)
}

/// Create a new `ScopeGuard` owning `v` and with deferred closure `dropfn`.
///
/// Requires crate feature `use_std`.
///
/// ## Examples
///
/// For performance reasons, or to emulate “only run guard on unwind” in
/// no-std environments, we can also use the default guard and simply manually
/// defuse it at the end of scope like the following example. (The performance
/// reason would be if the [`OnUnwind`]'s call to [std::thread::panicking()] is
/// an issue.)
///
/// ```
/// extern crate scopeguard;
///
/// use scopeguard::ScopeGuard;
/// # fn main() {
/// {
///     let guard = scopeguard::guard((), |_| {});
///
///     // rest of the code here
///
///     // we reached the end of scope without unwinding - defuse it
///     ScopeGuard::into_inner(guard);
/// }
/// # }
/// ```
#[cfg(feature = "use_std")]
#[inline]
#[must_use]
pub fn guard_on_unwind<T, F>(v: T, dropfn: F) -> ScopeGuard<T, F, OnUnwind>
where
    F: FnOnce(T),
{
    ScopeGuard::with_strategy(v, dropfn)
}

// ScopeGuard can be Sync even if F isn't because the closure is
// not accessible from references.
// The guard does not store any instance of S, so it is also irrelevant.
unsafe impl<T, F, S> Sync for ScopeGuard<T, F, S>
where
    T: Sync,
    F: FnOnce(T),
    S: Strategy,
{
}

impl<T, F, S> Deref for ScopeGuard<T, F, S>
where
    F: FnOnce(T),
    S: Strategy,
{
    type Target = T;

    fn deref(&self) -> &T {
        &*self.value
    }
}

impl<T, F, S> DerefMut for ScopeGuard<T, F, S>
where
    F: FnOnce(T),
    S: Strategy,
{
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.value
    }
}

impl<T, F, S> Drop for ScopeGuard<T, F, S>
where
    F: FnOnce(T),
    S: Strategy,
{
    fn drop(&mut self) {
        // This is OK because the fields are `ManuallyDrop`s
        // which will not be dropped by the compiler.
        let (value, dropfn) = unsafe { (ptr::read(&*self.value), ptr::read(&*self.dropfn)) };
        if S::should_run() {
            dropfn(value);
        }
    }
}

impl<T, F, S> fmt::Debug for ScopeGuard<T, F, S>
where
    T: fmt::Debug,
    F: FnOnce(T),
    S: Strategy,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(stringify!(ScopeGuard))
            .field("value", &*self.value)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::panic::catch_unwind;
    use std::panic::AssertUnwindSafe;

    #[test]
    fn test_defer() {
        let drops = Cell::new(0);
        defer!(drops.set(1000));
        assert_eq!(drops.get(), 0);
    }

    #[cfg(feature = "use_std")]
    #[test]
    fn test_defer_success_1() {
        let drops = Cell::new(0);
        {
            defer_on_success!(drops.set(1));
            assert_eq!(drops.get(), 0);
        }
        assert_eq!(drops.get(), 1);
    }

    #[cfg(feature = "use_std")]
    #[test]
    fn test_defer_success_2() {
        let drops = Cell::new(0);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            defer_on_success!(drops.set(1));
            panic!("failure")
        }));
        assert_eq!(drops.get(), 0);
    }

    #[cfg(feature = "use_std")]
    #[test]
    fn test_defer_unwind_1() {
        let drops = Cell::new(0);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            defer_on_unwind!(drops.set(1));
            assert_eq!(drops.get(), 0);
            panic!("failure")
        }));
        assert_eq!(drops.get(), 1);
    }

    #[cfg(feature = "use_std")]
    #[test]
    fn test_defer_unwind_2() {
        let drops = Cell::new(0);
        {
            defer_on_unwind!(drops.set(1));
        }
        assert_eq!(drops.get(), 0);
    }

    #[test]
    fn test_only_dropped_by_closure_when_run() {
        let value_drops = Cell::new(0);
        let value = guard((), |()| value_drops.set(1 + value_drops.get()));
        let closure_drops = Cell::new(0);
        let guard = guard(value, |_| closure_drops.set(1 + closure_drops.get()));
        assert_eq!(value_drops.get(), 0);
        assert_eq!(closure_drops.get(), 0);
        drop(guard);
        assert_eq!(value_drops.get(), 1);
        assert_eq!(closure_drops.get(), 1);
    }

    #[cfg(feature = "use_std")]
    #[test]
    fn test_dropped_once_when_not_run() {
        let value_drops = Cell::new(0);
        let value = guard((), |()| value_drops.set(1 + value_drops.get()));
        let captured_drops = Cell::new(0);
        let captured = guard((), |()| captured_drops.set(1 + captured_drops.get()));
        let closure_drops = Cell::new(0);
        let guard = guard_on_unwind(value, |value| {
            drop(value);
            drop(captured);
            closure_drops.set(1 + closure_drops.get())
        });
        assert_eq!(value_drops.get(), 0);
        assert_eq!(captured_drops.get(), 0);
        assert_eq!(closure_drops.get(), 0);
        drop(guard);
        assert_eq!(value_drops.get(), 1);
        assert_eq!(captured_drops.get(), 1);
        assert_eq!(closure_drops.get(), 0);
    }

    #[test]
    fn test_into_inner() {
        let dropped = Cell::new(false);
        let value = guard(42, |_| dropped.set(true));
        let guard = guard(value, |_| dropped.set(true));
        let inner = ScopeGuard::into_inner(guard);
        assert_eq!(dropped.get(), false);
        assert_eq!(*inner, 42);
    }
}
