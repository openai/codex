//! A lock-free concurrent slab.
//!
//! Slabs provide pre-allocated storage for many instances of a single data
//! type. When a large number of values of a single type are required,
//! this can be more efficient than allocating each item individually. Since the
//! allocated items are the same size, memory fragmentation is reduced, and
//! creating and removing new items can be very cheap.
//!
//! This crate implements a lock-free concurrent slab, indexed by `usize`s.
//!
//! ## Usage
//!
//! First, add this to your `Cargo.toml`:
//!
//! ```toml
//! sharded-slab = "0.1.1"
//! ```
//!
//! This crate provides two  types, [`Slab`] and [`Pool`], which provide
//! slightly different APIs for using a sharded slab.
//!
//! [`Slab`] implements a slab for _storing_ small types, sharing them between
//! threads, and accessing them by index. New entries are allocated by
//! [inserting] data, moving it in by value. Similarly, entries may be
//! deallocated by [taking] from the slab, moving the value out. This API is
//! similar to a `Vec<Option<T>>`, but allowing lock-free concurrent insertion
//! and removal.
//!
//! In contrast, the [`Pool`] type provides an [object pool] style API for
//! _reusing storage_. Rather than constructing values and moving them into the
//! pool, as with [`Slab`], [allocating an entry][create] from the pool takes a
//! closure that's provided with a mutable reference to initialize the entry in
//! place. When entries are deallocated, they are [cleared] in place. Types
//! which own a heap allocation can be cleared by dropping any _data_ they
//! store, but retaining any previously-allocated capacity. This means that a
//! [`Pool`] may be used to reuse a set of existing heap allocations, reducing
//! allocator load.
//!
//! [inserting]: Slab::insert
//! [taking]: Slab::take
//! [create]: Pool::create
//! [cleared]: Clear
//! [object pool]: https://en.wikipedia.org/wiki/Object_pool_pattern
//!
//! # Examples
//!
//! Inserting an item into the slab, returning an index:
//! ```rust
//! # use sharded_slab::Slab;
//! let slab = Slab::new();
//!
//! let key = slab.insert("hello world").unwrap();
//! assert_eq!(slab.get(key).unwrap(), "hello world");
//! ```
//!
//! To share a slab across threads, it may be wrapped in an `Arc`:
//! ```rust
//! # use sharded_slab::Slab;
//! use std::sync::Arc;
//! let slab = Arc::new(Slab::new());
//!
//! let slab2 = slab.clone();
//! let thread2 = std::thread::spawn(move || {
//!     let key = slab2.insert("hello from thread two").unwrap();
//!     assert_eq!(slab2.get(key).unwrap(), "hello from thread two");
//!     key
//! });
//!
//! let key1 = slab.insert("hello from thread one").unwrap();
//! assert_eq!(slab.get(key1).unwrap(), "hello from thread one");
//!
//! // Wait for thread 2 to complete.
//! let key2 = thread2.join().unwrap();
//!
//! // The item inserted by thread 2 remains in the slab.
//! assert_eq!(slab.get(key2).unwrap(), "hello from thread two");
//!```
//!
//! If items in the slab must be mutated, a `Mutex` or `RwLock` may be used for
//! each item, providing granular locking of items rather than of the slab:
//!
//! ```rust
//! # use sharded_slab::Slab;
//! use std::sync::{Arc, Mutex};
//! let slab = Arc::new(Slab::new());
//!
//! let key = slab.insert(Mutex::new(String::from("hello world"))).unwrap();
//!
//! let slab2 = slab.clone();
//! let thread2 = std::thread::spawn(move || {
//!     let hello = slab2.get(key).expect("item missing");
//!     let mut hello = hello.lock().expect("mutex poisoned");
//!     *hello = String::from("hello everyone!");
//! });
//!
//! thread2.join().unwrap();
//!
//! let hello = slab.get(key).expect("item missing");
//! let mut hello = hello.lock().expect("mutex poisoned");
//! assert_eq!(hello.as_str(), "hello everyone!");
//! ```
//!
//! # Configuration
//!
//! For performance reasons, several values used by the slab are calculated as
//! constants. In order to allow users to tune the slab's parameters, we provide
//! a [`Config`] trait which defines these parameters as associated `consts`.
//! The `Slab` type is generic over a `C: Config` parameter.
//!
//! [`Config`]: trait.Config.html
//!
//! # Comparison with Similar Crates
//!
//! - [`slab`]: Carl Lerche's `slab` crate provides a slab implementation with a
//!   similar API, implemented by storing all data in a single vector.
//!
//!   Unlike `sharded_slab`, inserting and removing elements from the slab
//!   requires  mutable access. This means that if the slab is accessed
//!   concurrently by multiple threads, it is necessary for it to be protected
//!   by a `Mutex` or `RwLock`. Items may not be inserted or removed (or
//!   accessed, if a `Mutex` is used) concurrently, even when they are
//!   unrelated. In many cases, the lock can become a significant bottleneck. On
//!   the other hand, this crate allows separate indices in the slab to be
//!   accessed, inserted, and removed concurrently without requiring a global
//!   lock. Therefore, when the slab is shared across multiple threads, this
//!   crate offers significantly better performance than `slab`.
//!
//!   However, the lock free slab introduces some additional constant-factor
//!   overhead. This means that in use-cases where a slab is _not_ shared by
//!   multiple threads and locking is not required, this crate will likely offer
//!   slightly worse performance.
//!
//!   In summary: `sharded-slab` offers significantly improved performance in
//!   concurrent use-cases, while `slab` should be preferred in single-threaded
//!   use-cases.
//!
//! [`slab`]: https://crates.io/crates/loom
//!
//! # Safety and Correctness
//!
//! Most implementations of lock-free data structures in Rust require some
//! amount of unsafe code, and this crate is not an exception. In order to catch
//! potential bugs in this unsafe code, we make use of [`loom`], a
//! permutation-testing tool for concurrent Rust programs. All `unsafe` blocks
//! this crate occur in accesses to `loom` `UnsafeCell`s. This means that when
//! those accesses occur in this crate's tests, `loom` will assert that they are
//! valid under the C11 memory model across multiple permutations of concurrent
//! executions of those tests.
//!
//! In order to guard against the [ABA problem][aba], this crate makes use of
//! _generational indices_. Each slot in the slab tracks a generation counter
//! which is incremented every time a value is inserted into that slot, and the
//! indices returned by [`Slab::insert`] include the generation of the slot when
//! the value was inserted, packed into the high-order bits of the index. This
//! ensures that if a value is inserted, removed,  and a new value is inserted
//! into the same slot in the slab, the key returned by the first call to
//! `insert` will not map to the new value.
//!
//! Since a fixed number of bits are set aside to use for storing the generation
//! counter, the counter will wrap  around after being incremented a number of
//! times. To avoid situations where a returned index lives long enough to see the
//! generation counter wrap around to the same value, it is good to be fairly
//! generous when configuring the allocation of index bits.
//!
//! [`loom`]: https://crates.io/crates/loom
//! [aba]: https://en.wikipedia.org/wiki/ABA_problem
//! [`Slab::insert`]: struct.Slab.html#method.insert
//!
//! # Performance
//!
//! These graphs were produced by [benchmarks] of the sharded slab implementation,
//! using the [`criterion`] crate.
//!
//! The first shows the results of a benchmark where an increasing number of
//! items are inserted and then removed into a slab concurrently by five
//! threads. It compares the performance of the sharded slab implementation
//! with a `RwLock<slab::Slab>`:
//!
//! <img width="1124" alt="Screen Shot 2019-10-01 at 5 09 49 PM" src="https://user-images.githubusercontent.com/2796466/66078398-cd6c9f80-e516-11e9-9923-0ed6292e8498.png">
//!
//! The second graph shows the results of a benchmark where an increasing
//! number of items are inserted and then removed by a _single_ thread. It
//! compares the performance of the sharded slab implementation with an
//! `RwLock<slab::Slab>` and a `mut slab::Slab`.
//!
//! <img width="925" alt="Screen Shot 2019-10-01 at 5 13 45 PM" src="https://user-images.githubusercontent.com/2796466/66078469-f0974f00-e516-11e9-95b5-f65f0aa7e494.png">
//!
//! These benchmarks demonstrate that, while the sharded approach introduces
//! a small constant-factor overhead, it offers significantly better
//! performance across concurrent accesses.
//!
//! [benchmarks]: https://github.com/hawkw/sharded-slab/blob/master/benches/bench.rs
//! [`criterion`]: https://crates.io/crates/criterion
//!
//! # Implementation Notes
//!
//! See [this page](crate::implementation) for details on this crate's design
//! and implementation.
//!
#![doc(html_root_url = "https://docs.rs/sharded-slab/0.1.4")]
#![warn(missing_debug_implementations, missing_docs)]
#![cfg_attr(docsrs, warn(rustdoc::broken_intra_doc_links))]
#[macro_use]
mod macros;

pub mod implementation;
pub mod pool;

pub(crate) mod cfg;
pub(crate) mod sync;

mod clear;
mod iter;
mod page;
mod shard;
mod tid;

pub use self::{
    cfg::{Config, DefaultConfig},
    clear::Clear,
    iter::UniqueIter,
};
#[doc(inline)]
pub use pool::Pool;

pub(crate) use tid::Tid;

use cfg::CfgPrivate;
use shard::Shard;
use std::{fmt, marker::PhantomData, ptr, sync::Arc};

/// A sharded slab.
///
/// See the [crate-level documentation](crate) for details on using this type.
pub struct Slab<T, C: cfg::Config = DefaultConfig> {
    shards: shard::Array<Option<T>, C>,
    _cfg: PhantomData<C>,
}

/// A handle that allows access to an occupied entry in a [`Slab`].
///
/// While the guard exists, it indicates to the slab that the item the guard
/// references is currently being accessed. If the item is removed from the slab
/// while a guard exists, the removal will be deferred until all guards are
/// dropped.
pub struct Entry<'a, T, C: cfg::Config = DefaultConfig> {
    inner: page::slot::Guard<Option<T>, C>,
    value: ptr::NonNull<T>,
    shard: &'a Shard<Option<T>, C>,
    key: usize,
}

/// A handle to a vacant entry in a [`Slab`].
///
/// `VacantEntry` allows constructing values with the key that they will be
/// assigned to.
///
/// # Examples
///
/// ```
/// # use sharded_slab::Slab;
/// let mut slab = Slab::new();
///
/// let hello = {
///     let entry = slab.vacant_entry().unwrap();
///     let key = entry.key();
///
///     entry.insert((key, "hello"));
///     key
/// };
///
/// assert_eq!(hello, slab.get(hello).unwrap().0);
/// assert_eq!("hello", slab.get(hello).unwrap().1);
/// ```
#[derive(Debug)]
pub struct VacantEntry<'a, T, C: cfg::Config = DefaultConfig> {
    inner: page::slot::InitGuard<Option<T>, C>,
    key: usize,
    _lt: PhantomData<&'a ()>,
}

/// An owned reference to an occupied entry in a [`Slab`].
///
/// While the guard exists, it indicates to the slab that the item the guard
/// references is currently being accessed. If the item is removed from the slab
/// while the guard exists, the  removal will be deferred until all guards are
/// dropped.
///
/// Unlike [`Entry`], which borrows the slab, an `OwnedEntry` clones the [`Arc`]
/// around the slab. Therefore, it keeps the slab from being dropped until all
/// such guards have been dropped. This means that an `OwnedEntry` may be held for
/// an arbitrary lifetime.
///
/// # Examples
///
/// ```
/// # use sharded_slab::Slab;
/// use std::sync::Arc;
///
/// let slab: Arc<Slab<&'static str>> = Arc::new(Slab::new());
/// let key = slab.insert("hello world").unwrap();
///
/// // Look up the created key, returning an `OwnedEntry`.
/// let value = slab.clone().get_owned(key).unwrap();
///
/// // Now, the original `Arc` clone of the slab may be dropped, but the
/// // returned `OwnedEntry` can still access the value.
/// assert_eq!(value, "hello world");
/// ```
///
/// Unlike [`Entry`], an `OwnedEntry` may be stored in a struct which must live
/// for the `'static` lifetime:
///
/// ```
/// # use sharded_slab::Slab;
/// use sharded_slab::OwnedEntry;
/// use std::sync::Arc;
///
/// pub struct MyStruct {
///     entry: OwnedEntry<&'static str>,
///     // ... other fields ...
/// }
///
/// // Suppose this is some arbitrary function which requires a value that
/// // lives for the 'static lifetime...
/// fn function_requiring_static<T: 'static>(t: &T) {
///     // ... do something extremely important and interesting ...
/// }
///
/// let slab: Arc<Slab<&'static str>> = Arc::new(Slab::new());
/// let key = slab.insert("hello world").unwrap();
///
/// // Look up the created key, returning an `OwnedEntry`.
/// let entry = slab.clone().get_owned(key).unwrap();
/// let my_struct = MyStruct {
///     entry,
///     // ...
/// };
///
/// // We can use `my_struct` anywhere where it is required to have the
/// // `'static` lifetime:
/// function_requiring_static(&my_struct);
/// ```
///
/// `OwnedEntry`s may be sent between threads:
///
/// ```
/// # use sharded_slab::Slab;
/// use std::{thread, sync::Arc};
///
/// let slab: Arc<Slab<&'static str>> = Arc::new(Slab::new());
/// let key = slab.insert("hello world").unwrap();
///
/// // Look up the created key, returning an `OwnedEntry`.
/// let value = slab.clone().get_owned(key).unwrap();
///
/// thread::spawn(move || {
///     assert_eq!(value, "hello world");
///     // ...
/// }).join().unwrap();
/// ```
///
/// [`get`]: Slab::get
/// [`Arc`]: std::sync::Arc
pub struct OwnedEntry<T, C = DefaultConfig>
where
    C: cfg::Config,
{
    inner: page::slot::Guard<Option<T>, C>,
    value: ptr::NonNull<T>,
    slab: Arc<Slab<T, C>>,
    key: usize,
}

impl<T> Slab<T> {
    /// Returns a new slab with the default configuration parameters.
    pub fn new() -> Self {
        Self::new_with_config()
    }

    /// Returns a new slab with the provided configuration parameters.
    pub fn new_with_config<C: cfg::Config>() -> Slab<T, C> {
        C::validate();
        Slab {
            shards: shard::Array::new(),
            _cfg: PhantomData,
        }
    }
}

impl<T, C: cfg::Config> Slab<T, C> {
    /// The number of bits in each index which are used by the slab.
    ///
    /// If other data is packed into the `usize` indices returned by
    /// [`Slab::insert`], user code is free to use any bits higher than the
    /// `USED_BITS`-th bit freely.
    ///
    /// This is determined by the [`Config`] type that configures the slab's
    /// parameters. By default, all bits are used; this can be changed by
    /// overriding the [`Config::RESERVED_BITS`][res] constant.
    ///
    /// [res]: crate::Config#RESERVED_BITS
    pub const USED_BITS: usize = C::USED_BITS;

    /// Inserts a value into the slab, returning the integer index at which that
    /// value was inserted. This index can then be used to access the entry.
    ///
    /// If this function returns `None`, then the shard for the current thread
    /// is full and no items can be added until some are removed, or the maximum
    /// number of shards has been reached.
    ///
    /// # Examples
    /// ```rust
    /// # use sharded_slab::Slab;
    /// let slab = Slab::new();
    ///
    /// let key = slab.insert("hello world").unwrap();
    /// assert_eq!(slab.get(key).unwrap(), "hello world");
    /// ```
    pub fn insert(&self, value: T) -> Option<usize> {
        let (tid, shard) = self.shards.current();
        test_println!("insert {:?}", tid);
        let mut value = Some(value);
        shard
            .init_with(|idx, slot| {
                let gen = slot.insert(&mut value)?;
                Some(gen.pack(idx))
            })
            .map(|idx| tid.pack(idx))
    }

    /// Return a handle to a vacant entry allowing for further manipulation.
    ///
    /// This function is useful when creating values that must contain their
    /// slab index. The returned [`VacantEntry`] reserves a slot in the slab and
    /// is able to return the index of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sharded_slab::Slab;
    /// let mut slab = Slab::new();
    ///
    /// let hello = {
    ///     let entry = slab.vacant_entry().unwrap();
    ///     let key = entry.key();
    ///
    ///     entry.insert((key, "hello"));
    ///     key
    /// };
    ///
    /// assert_eq!(hello, slab.get(hello).unwrap().0);
    /// assert_eq!("hello", slab.get(hello).unwrap().1);
    /// ```
    pub fn vacant_entry(&self) -> Option<VacantEntry<'_, T, C>> {
        let (tid, shard) = self.shards.current();
        test_println!("vacant_entry {:?}", tid);
        shard.init_with(|idx, slot| {
            let inner = slot.init()?;
            let key = inner.generation().pack(tid.pack(idx));
            Some(VacantEntry {
                inner,
                key,
                _lt: PhantomData,
            })
        })
    }

    /// Remove the value at the given index in the slab, returning `true` if a
    /// value was removed.
    ///
    /// Unlike [`take`], this method does _not_ block the current thread until
    /// the value can be removed. Instead, if another thread is currently
    /// accessing that value, this marks it to be removed by that thread when it
    /// finishes accessing the value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let slab = sharded_slab::Slab::new();
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// // Remove the item from the slab.
    /// assert!(slab.remove(key));
    ///
    /// // Now, the slot is empty.
    /// assert!(!slab.contains(key));
    /// ```
    ///
    /// ```rust
    /// use std::sync::Arc;
    ///
    /// let slab = Arc::new(sharded_slab::Slab::new());
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// let slab2 = slab.clone();
    /// let thread2 = std::thread::spawn(move || {
    ///     // Depending on when this thread begins executing, the item may
    ///     // or may not have already been removed...
    ///     if let Some(item) = slab2.get(key) {
    ///         assert_eq!(item, "hello world");
    ///     }
    /// });
    ///
    /// // The item will be removed by thread2 when it finishes accessing it.
    /// assert!(slab.remove(key));
    ///
    /// thread2.join().unwrap();
    /// assert!(!slab.contains(key));
    /// ```
    /// [`take`]: Slab::take
    pub fn remove(&self, idx: usize) -> bool {
        // The `Drop` impl for `Entry` calls `remove_local` or `remove_remote` based
        // on where the guard was dropped from. If the dropped guard was the last one, this will
        // call `Slot::remove_value` which actually clears storage.
        let tid = C::unpack_tid(idx);

        test_println!("rm_deferred {:?}", tid);
        let shard = self.shards.get(tid.as_usize());
        if tid.is_current() {
            shard.map(|shard| shard.remove_local(idx)).unwrap_or(false)
        } else {
            shard.map(|shard| shard.remove_remote(idx)).unwrap_or(false)
        }
    }

    /// Removes the value associated with the given key from the slab, returning
    /// it.
    ///
    /// If the slab does not contain a value for that key, `None` is returned
    /// instead.
    ///
    /// If the value associated with the given key is currently being
    /// accessed by another thread, this method will block the current thread
    /// until the item is no longer accessed. If this is not desired, use
    /// [`remove`] instead.
    ///
    /// **Note**: This method blocks the calling thread by spinning until the
    /// currently outstanding references are released. Spinning for long periods
    /// of time can result in high CPU time and power consumption. Therefore,
    /// `take` should only be called when other references to the slot are
    /// expected to be dropped soon (e.g., when all accesses are relatively
    /// short).
    ///
    /// # Examples
    ///
    /// ```rust
    /// let slab = sharded_slab::Slab::new();
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// // Remove the item from the slab, returning it.
    /// assert_eq!(slab.take(key), Some("hello world"));
    ///
    /// // Now, the slot is empty.
    /// assert!(!slab.contains(key));
    /// ```
    ///
    /// ```rust
    /// use std::sync::Arc;
    ///
    /// let slab = Arc::new(sharded_slab::Slab::new());
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// let slab2 = slab.clone();
    /// let thread2 = std::thread::spawn(move || {
    ///     // Depending on when this thread begins executing, the item may
    ///     // or may not have already been removed...
    ///     if let Some(item) = slab2.get(key) {
    ///         assert_eq!(item, "hello world");
    ///     }
    /// });
    ///
    /// // The item will only be removed when the other thread finishes
    /// // accessing it.
    /// assert_eq!(slab.take(key), Some("hello world"));
    ///
    /// thread2.join().unwrap();
    /// assert!(!slab.contains(key));
    /// ```
    /// [`remove`]: Slab::remove
    pub fn take(&self, idx: usize) -> Option<T> {
        let tid = C::unpack_tid(idx);

        test_println!("rm {:?}", tid);
        let shard = self.shards.get(tid.as_usize())?;
        if tid.is_current() {
            shard.take_local(idx)
        } else {
            shard.take_remote(idx)
        }
    }

    /// Return a reference to the value associated with the given key.
    ///
    /// If the slab does not contain a value for the given key, or if the
    /// maximum number of concurrent references to the slot has been reached,
    /// `None` is returned instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let slab = sharded_slab::Slab::new();
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// assert_eq!(slab.get(key).unwrap(), "hello world");
    /// assert!(slab.get(12345).is_none());
    /// ```
    pub fn get(&self, key: usize) -> Option<Entry<'_, T, C>> {
        let tid = C::unpack_tid(key);

        test_println!("get {:?}; current={:?}", tid, Tid::<C>::current());
        let shard = self.shards.get(tid.as_usize())?;
        shard.with_slot(key, |slot| {
            let inner = slot.get(C::unpack_gen(key))?;
            let value = ptr::NonNull::from(slot.value().as_ref().unwrap());
            Some(Entry {
                inner,
                value,
                shard,
                key,
            })
        })
    }

    /// Return an owned reference to the value at the given index.
    ///
    /// If the slab does not contain a value for the given key, `None` is
    /// returned instead.
    ///
    /// Unlike [`get`], which borrows the slab, this method _clones_ the [`Arc`]
    /// around the slab. This means that the returned [`OwnedEntry`] can be held
    /// for an arbitrary lifetime. However,  this method requires that the slab
    /// itself be wrapped in an `Arc`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sharded_slab::Slab;
    /// use std::sync::Arc;
    ///
    /// let slab: Arc<Slab<&'static str>> = Arc::new(Slab::new());
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// // Look up the created key, returning an `OwnedEntry`.
    /// let value = slab.clone().get_owned(key).unwrap();
    ///
    /// // Now, the original `Arc` clone of the slab may be dropped, but the
    /// // returned `OwnedEntry` can still access the value.
    /// assert_eq!(value, "hello world");
    /// ```
    ///
    /// Unlike [`Entry`], an `OwnedEntry` may be stored in a struct which must live
    /// for the `'static` lifetime:
    ///
    /// ```
    /// # use sharded_slab::Slab;
    /// use sharded_slab::OwnedEntry;
    /// use std::sync::Arc;
    ///
    /// pub struct MyStruct {
    ///     entry: OwnedEntry<&'static str>,
    ///     // ... other fields ...
    /// }
    ///
    /// // Suppose this is some arbitrary function which requires a value that
    /// // lives for the 'static lifetime...
    /// fn function_requiring_static<T: 'static>(t: &T) {
    ///     // ... do something extremely important and interesting ...
    /// }
    ///
    /// let slab: Arc<Slab<&'static str>> = Arc::new(Slab::new());
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// // Look up the created key, returning an `OwnedEntry`.
    /// let entry = slab.clone().get_owned(key).unwrap();
    /// let my_struct = MyStruct {
    ///     entry,
    ///     // ...
    /// };
    ///
    /// // We can use `my_struct` anywhere where it is required to have the
    /// // `'static` lifetime:
    /// function_requiring_static(&my_struct);
    /// ```
    ///
    /// [`OwnedEntry`]s may be sent between threads:
    ///
    /// ```
    /// # use sharded_slab::Slab;
    /// use std::{thread, sync::Arc};
    ///
    /// let slab: Arc<Slab<&'static str>> = Arc::new(Slab::new());
    /// let key = slab.insert("hello world").unwrap();
    ///
    /// // Look up the created key, returning an `OwnedEntry`.
    /// let value = slab.clone().get_owned(key).unwrap();
    ///
    /// thread::spawn(move || {
    ///     assert_eq!(value, "hello world");
    ///     // ...
    /// }).join().unwrap();
    /// ```
    ///
    /// [`get`]: Slab::get
    /// [`Arc`]: std::sync::Arc
    pub fn get_owned(self: Arc<Self>, key: usize) -> Option<OwnedEntry<T, C>> {
        let tid = C::unpack_tid(key);

        test_println!("get_owned {:?}; current={:?}", tid, Tid::<C>::current());
        let shard = self.shards.get(tid.as_usize())?;
        shard.with_slot(key, |slot| {
            let inner = slot.get(C::unpack_gen(key))?;
            let value = ptr::NonNull::from(slot.value().as_ref().unwrap());
            Some(OwnedEntry {
                inner,
                value,
                slab: self.clone(),
                key,
            })
        })
    }

    /// Returns `true` if the slab contains a value for the given key.
    ///
    /// # Examples
    ///
    /// ```
    /// let slab = sharded_slab::Slab::new();
    ///
    /// let key = slab.insert("hello world").unwrap();
    /// assert!(slab.contains(key));
    ///
    /// slab.take(key).unwrap();
    /// assert!(!slab.contains(key));
    /// ```
    pub fn contains(&self, key: usize) -> bool {
        self.get(key).is_some()
    }

    /// Returns an iterator over all the items in the slab.
    ///
    /// Because this iterator exclusively borrows the slab (i.e. it holds an
    /// `&mut Slab<T>`), elements will not be added or removed while the
    /// iteration is in progress.
    pub fn unique_iter(&mut self) -> iter::UniqueIter<'_, T, C> {
        let mut shards = self.shards.iter_mut();

        let (pages, slots) = match shards.next() {
            Some(shard) => {
                let mut pages = shard.iter();
                let slots = pages.next().and_then(page::Shared::iter);
                (pages, slots)
            }
            None => ([].iter(), None),
        };

        iter::UniqueIter {
            shards,
            pages,
            slots,
        }
    }
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug, C: cfg::Config> fmt::Debug for Slab<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Slab")
            .field("shards", &self.shards)
            .field("config", &C::debug())
            .finish()
    }
}

unsafe impl<T: Send, C: cfg::Config> Send for Slab<T, C> {}
unsafe impl<T: Sync, C: cfg::Config> Sync for Slab<T, C> {}

// === impl Entry ===

impl<'a, T, C: cfg::Config> Entry<'a, T, C> {
    /// Returns the key used to access the guard.
    pub fn key(&self) -> usize {
        self.key
    }

    #[inline(always)]
    fn value(&self) -> &T {
        unsafe {
            // Safety: this is always going to be valid, as it's projected from
            // the safe reference to `self.value` --- this is just to avoid
            // having to `expect` an option in the hot path when dereferencing.
            self.value.as_ref()
        }
    }
}

impl<'a, T, C: cfg::Config> std::ops::Deref for Entry<'a, T, C> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

impl<'a, T, C: cfg::Config> Drop for Entry<'a, T, C> {
    fn drop(&mut self) {
        let should_remove = unsafe {
            // Safety: calling `slot::Guard::release` is unsafe, since the
            // `Guard` value contains a pointer to the slot that may outlive the
            // slab containing that slot. Here, the `Entry` guard owns a
            // borrowed reference to the shard containing that slot, which
            // ensures that the slot will not be dropped while this `Guard`
            // exists.
            self.inner.release()
        };
        if should_remove {
            self.shard.clear_after_release(self.key)
        }
    }
}

impl<'a, T, C> fmt::Debug for Entry<'a, T, C>
where
    T: fmt::Debug,
    C: cfg::Config,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.value(), f)
    }
}

impl<'a, T, C> PartialEq<T> for Entry<'a, T, C>
where
    T: PartialEq<T>,
    C: cfg::Config,
{
    fn eq(&self, other: &T) -> bool {
        self.value().eq(other)
    }
}

// === impl VacantEntry ===

impl<'a, T, C: cfg::Config> VacantEntry<'a, T, C> {
    /// Insert a value in the entry.
    ///
    /// To get the integer index at which this value will be inserted, use
    /// [`key`] prior to calling `insert`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sharded_slab::Slab;
    /// let mut slab = Slab::new();
    ///
    /// let hello = {
    ///     let entry = slab.vacant_entry().unwrap();
    ///     let key = entry.key();
    ///
    ///     entry.insert((key, "hello"));
    ///     key
    /// };
    ///
    /// assert_eq!(hello, slab.get(hello).unwrap().0);
    /// assert_eq!("hello", slab.get(hello).unwrap().1);
    /// ```
    ///
    /// [`key`]: VacantEntry::key
    pub fn insert(mut self, val: T) {
        let value = unsafe {
            // Safety: this `VacantEntry` only lives as long as the `Slab` it was
            // borrowed from, so it cannot outlive the entry's slot.
            self.inner.value_mut()
        };
        debug_assert!(
            value.is_none(),
            "tried to insert to a slot that already had a value!"
        );
        *value = Some(val);
        let _released = unsafe {
            // Safety: again, this `VacantEntry` only lives as long as the
            // `Slab` it was borrowed from, so it cannot outlive the entry's
            // slot.
            self.inner.release()
        };
        debug_assert!(
            !_released,
            "removing a value before it was inserted should be a no-op"
        )
    }

    /// Return the integer index at which this entry will be inserted.
    ///
    /// A value stored in this entry will be associated with this key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sharded_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// let hello = {
    ///     let entry = slab.vacant_entry().unwrap();
    ///     let key = entry.key();
    ///
    ///     entry.insert((key, "hello"));
    ///     key
    /// };
    ///
    /// assert_eq!(hello, slab.get(hello).unwrap().0);
    /// assert_eq!("hello", slab.get(hello).unwrap().1);
    /// ```
    pub fn key(&self) -> usize {
        self.key
    }
}
// === impl OwnedEntry ===

impl<T, C> OwnedEntry<T, C>
where
    C: cfg::Config,
{
    /// Returns the key used to access this guard
    pub fn key(&self) -> usize {
        self.key
    }

    #[inline(always)]
    fn value(&self) -> &T {
        unsafe {
            // Safety: this is always going to be valid, as it's projected from
            // the safe reference to `self.value` --- this is just to avoid
            // having to `expect` an option in the hot path when dereferencing.
            self.value.as_ref()
        }
    }
}

impl<T, C> std::ops::Deref for OwnedEntry<T, C>
where
    C: cfg::Config,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value()
    }
}

impl<T, C> Drop for OwnedEntry<T, C>
where
    C: cfg::Config,
{
    fn drop(&mut self) {
        test_println!("drop OwnedEntry: try clearing data");
        let should_clear = unsafe {
            // Safety: calling `slot::Guard::release` is unsafe, since the
            // `Guard` value contains a pointer to the slot that may outlive the
            // slab containing that slot. Here, the `OwnedEntry` owns an `Arc`
            // clone of the pool, which keeps it alive as long as the `OwnedEntry`
            // exists.
            self.inner.release()
        };
        if should_clear {
            let shard_idx = Tid::<C>::from_packed(self.key);
            test_println!("-> shard={:?}", shard_idx);
            if let Some(shard) = self.slab.shards.get(shard_idx.as_usize()) {
                shard.clear_after_release(self.key)
            } else {
                test_println!("-> shard={:?} does not exist! THIS IS A BUG", shard_idx);
                debug_assert!(std::thread::panicking(), "[internal error] tried to drop an `OwnedEntry` to a slot on a shard that never existed!");
            }
        }
    }
}

impl<T, C> fmt::Debug for OwnedEntry<T, C>
where
    T: fmt::Debug,
    C: cfg::Config,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.value(), f)
    }
}

impl<T, C> PartialEq<T> for OwnedEntry<T, C>
where
    T: PartialEq<T>,
    C: cfg::Config,
{
    fn eq(&self, other: &T) -> bool {
        *self.value() == *other
    }
}

unsafe impl<T, C> Sync for OwnedEntry<T, C>
where
    T: Sync,
    C: cfg::Config,
{
}

unsafe impl<T, C> Send for OwnedEntry<T, C>
where
    T: Sync,
    C: cfg::Config,
{
}

// === pack ===

pub(crate) trait Pack<C: cfg::Config>: Sized {
    // ====== provided by each implementation =================================

    /// The number of bits occupied by this type when packed into a usize.
    ///
    /// This must be provided to determine the number of bits into which to pack
    /// the type.
    const LEN: usize;
    /// The type packed on the less significant side of this type.
    ///
    /// If this type is packed into the least significant bit of a usize, this
    /// should be `()`, which occupies no bytes.
    ///
    /// This is used to calculate the shift amount for packing this value.
    type Prev: Pack<C>;

    // ====== calculated automatically ========================================

    /// A number consisting of `Self::LEN` 1 bits, starting at the least
    /// significant bit.
    ///
    /// This is the higest value this type can represent. This number is shifted
    /// left by `Self::SHIFT` bits to calculate this type's `MASK`.
    ///
    /// This is computed automatically based on `Self::LEN`.
    const BITS: usize = {
        let shift = 1 << (Self::LEN - 1);
        shift | (shift - 1)
    };
    /// The number of bits to shift a number to pack it into a usize with other
    /// values.
    ///
    /// This is caculated automatically based on the `LEN` and `SHIFT` constants
    /// of the previous value.
    const SHIFT: usize = Self::Prev::SHIFT + Self::Prev::LEN;

    /// The mask to extract only this type from a packed `usize`.
    ///
    /// This is calculated by shifting `Self::BITS` left by `Self::SHIFT`.
    const MASK: usize = Self::BITS << Self::SHIFT;

    fn as_usize(&self) -> usize;
    fn from_usize(val: usize) -> Self;

    #[inline(always)]
    fn pack(&self, to: usize) -> usize {
        let value = self.as_usize();
        debug_assert!(value <= Self::BITS);

        (to & !Self::MASK) | (value << Self::SHIFT)
    }

    #[inline(always)]
    fn from_packed(from: usize) -> Self {
        let value = (from & Self::MASK) >> Self::SHIFT;
        debug_assert!(value <= Self::BITS);
        Self::from_usize(value)
    }
}

impl<C: cfg::Config> Pack<C> for () {
    const BITS: usize = 0;
    const LEN: usize = 0;
    const SHIFT: usize = 0;
    const MASK: usize = 0;

    type Prev = ();

    fn as_usize(&self) -> usize {
        unreachable!()
    }
    fn from_usize(_val: usize) -> Self {
        unreachable!()
    }

    fn pack(&self, _to: usize) -> usize {
        unreachable!()
    }

    fn from_packed(_from: usize) -> Self {
        unreachable!()
    }
}

#[cfg(test)]
pub(crate) use self::tests::util as test_util;

#[cfg(test)]
mod tests;
