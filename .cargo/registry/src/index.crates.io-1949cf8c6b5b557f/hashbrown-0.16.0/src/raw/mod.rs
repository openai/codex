use crate::alloc::alloc::{handle_alloc_error, Layout};
use crate::control::{BitMaskIter, Group, Tag, TagSliceExt};
use crate::scopeguard::{guard, ScopeGuard};
use crate::util::{invalid_mut, likely, unlikely};
use crate::TryReserveError;
use core::array;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::mem;
use core::ptr::NonNull;
use core::slice;
use core::{hint, ptr};

mod alloc;
#[cfg(test)]
pub(crate) use self::alloc::AllocError;
pub(crate) use self::alloc::{do_alloc, Allocator, Global};

#[inline]
unsafe fn offset_from<T>(to: *const T, from: *const T) -> usize {
    to.offset_from(from) as usize
}

/// Whether memory allocation errors should return an error or abort.
#[derive(Copy, Clone)]
enum Fallibility {
    Fallible,
    Infallible,
}

impl Fallibility {
    /// Error to return on capacity overflow.
    #[cfg_attr(feature = "inline-more", inline)]
    fn capacity_overflow(self) -> TryReserveError {
        match self {
            Fallibility::Fallible => TryReserveError::CapacityOverflow,
            Fallibility::Infallible => panic!("Hash table capacity overflow"),
        }
    }

    /// Error to return on allocation error.
    #[cfg_attr(feature = "inline-more", inline)]
    fn alloc_err(self, layout: Layout) -> TryReserveError {
        match self {
            Fallibility::Fallible => TryReserveError::AllocError { layout },
            Fallibility::Infallible => handle_alloc_error(layout),
        }
    }
}

trait SizedTypeProperties: Sized {
    const IS_ZERO_SIZED: bool = mem::size_of::<Self>() == 0;
    const NEEDS_DROP: bool = mem::needs_drop::<Self>();
}

impl<T> SizedTypeProperties for T {}

/// Primary hash function, used to select the initial bucket to probe from.
#[inline]
#[allow(clippy::cast_possible_truncation)]
fn h1(hash: u64) -> usize {
    // On 32-bit platforms we simply ignore the higher hash bits.
    hash as usize
}

/// Probe sequence based on triangular numbers, which is guaranteed (since our
/// table size is a power of two) to visit every group of elements exactly once.
///
/// A triangular probe has us jump by 1 more group every time. So first we
/// jump by 1 group (meaning we just continue our linear scan), then 2 groups
/// (skipping over 1 group), then 3 groups (skipping over 2 groups), and so on.
///
/// Proof that the probe will visit every group in the table:
/// <https://fgiesen.wordpress.com/2015/02/22/triangular-numbers-mod-2n/>
#[derive(Clone)]
struct ProbeSeq {
    pos: usize,
    stride: usize,
}

impl ProbeSeq {
    #[inline]
    fn move_next(&mut self, bucket_mask: usize) {
        // We should have found an empty bucket by now and ended the probe.
        debug_assert!(
            self.stride <= bucket_mask,
            "Went past end of probe sequence"
        );

        self.stride += Group::WIDTH;
        self.pos += self.stride;
        self.pos &= bucket_mask;
    }
}

/// Returns the number of buckets needed to hold the given number of items,
/// taking the maximum load factor into account.
///
/// Returns `None` if an overflow occurs.
// Workaround for emscripten bug emscripten-core/emscripten-fastcomp#258
#[cfg_attr(target_os = "emscripten", inline(never))]
#[cfg_attr(not(target_os = "emscripten"), inline)]
fn capacity_to_buckets(cap: usize, table_layout: TableLayout) -> Option<usize> {
    debug_assert_ne!(cap, 0);

    // For small tables we require at least 1 empty bucket so that lookups are
    // guaranteed to terminate if an element doesn't exist in the table.
    if cap < 15 {
        // Consider a small TableLayout like { size: 1, ctrl_align: 16 } on a
        // platform with Group::WIDTH of 16 (like x86_64 with SSE2). For small
        // bucket sizes, this ends up wasting quite a few bytes just to pad to
        // the relatively larger ctrl_align:
        //
        // | capacity | buckets | bytes allocated | bytes per item |
        // | -------- | ------- | --------------- | -------------- |
        // |        3 |       4 |              36 | (Yikes!)  12.0 |
        // |        7 |       8 |              40 | (Poor)     5.7 |
        // |       14 |      16 |              48 |            3.4 |
        // |       28 |      32 |              80 |            3.3 |
        //
        // In general, buckets * table_layout.size >= table_layout.ctrl_align
        // must be true to avoid these edges. This is implemented by adjusting
        // the minimum capacity upwards for small items. This code only needs
        // to handle ctrl_align which are less than or equal to Group::WIDTH,
        // because valid layout sizes are always a multiple of the alignment,
        // so anything with alignment over the Group::WIDTH won't hit this edge
        // case.

        // This is brittle, e.g. if we ever add 32 byte groups, it will select
        // 3 regardless of the table_layout.size.
        let min_cap = match (Group::WIDTH, table_layout.size) {
            (16, 0..=1) => 14,
            (16, 2..=3) => 7,
            (8, 0..=1) => 7,
            _ => 3,
        };
        let cap = min_cap.max(cap);
        // We don't bother with a table size of 2 buckets since that can only
        // hold a single element. Instead, we skip directly to a 4 bucket table
        // which can hold 3 elements.
        return Some(if cap < 4 {
            4
        } else if cap < 8 {
            8
        } else {
            16
        });
    }

    // Otherwise require 1/8 buckets to be empty (87.5% load)
    //
    // Be careful when modifying this, calculate_layout relies on the
    // overflow check here.
    let adjusted_cap = cap.checked_mul(8)? / 7;

    // Any overflows will have been caught by the checked_mul. Also, any
    // rounding errors from the division above will be cleaned up by
    // next_power_of_two (which can't overflow because of the previous division).
    Some(adjusted_cap.next_power_of_two())
}

/// Returns the maximum effective capacity for the given bucket mask, taking
/// the maximum load factor into account.
#[inline]
fn bucket_mask_to_capacity(bucket_mask: usize) -> usize {
    if bucket_mask < 8 {
        // For tables with 1/2/4/8 buckets, we always reserve one empty slot.
        // Keep in mind that the bucket mask is one less than the bucket count.
        bucket_mask
    } else {
        // For larger tables we reserve 12.5% of the slots as empty.
        ((bucket_mask + 1) / 8) * 7
    }
}

/// Helper which allows the max calculation for `ctrl_align` to be statically computed for each `T`
/// while keeping the rest of `calculate_layout_for` independent of `T`
#[derive(Copy, Clone)]
struct TableLayout {
    size: usize,
    ctrl_align: usize,
}

impl TableLayout {
    #[inline]
    const fn new<T>() -> Self {
        let layout = Layout::new::<T>();
        Self {
            size: layout.size(),
            ctrl_align: if layout.align() > Group::WIDTH {
                layout.align()
            } else {
                Group::WIDTH
            },
        }
    }

    #[inline]
    fn calculate_layout_for(self, buckets: usize) -> Option<(Layout, usize)> {
        debug_assert!(buckets.is_power_of_two());

        let TableLayout { size, ctrl_align } = self;
        // Manual layout calculation since Layout methods are not yet stable.
        let ctrl_offset =
            size.checked_mul(buckets)?.checked_add(ctrl_align - 1)? & !(ctrl_align - 1);
        let len = ctrl_offset.checked_add(buckets + Group::WIDTH)?;

        // We need an additional check to ensure that the allocation doesn't
        // exceed `isize::MAX` (https://github.com/rust-lang/rust/pull/95295).
        if len > isize::MAX as usize - (ctrl_align - 1) {
            return None;
        }

        Some((
            unsafe { Layout::from_size_align_unchecked(len, ctrl_align) },
            ctrl_offset,
        ))
    }
}

/// A reference to an empty bucket into which an can be inserted.
pub struct InsertSlot {
    index: usize,
}

/// A reference to a hash table bucket containing a `T`.
///
/// This is usually just a pointer to the element itself. However if the element
/// is a ZST, then we instead track the index of the element in the table so
/// that `erase` works properly.
pub struct Bucket<T> {
    // Actually it is pointer to next element than element itself
    // this is needed to maintain pointer arithmetic invariants
    // keeping direct pointer to element introduces difficulty.
    // Using `NonNull` for variance and niche layout
    ptr: NonNull<T>,
}

// This Send impl is needed for rayon support. This is safe since Bucket is
// never exposed in a public API.
unsafe impl<T> Send for Bucket<T> {}

impl<T> Clone for Bucket<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self { ptr: self.ptr }
    }
}

impl<T> Bucket<T> {
    /// Creates a [`Bucket`] that contain pointer to the data.
    /// The pointer calculation is performed by calculating the
    /// offset from given `base` pointer (convenience for
    /// `base.as_ptr().sub(index)`).
    ///
    /// `index` is in units of `T`; e.g., an `index` of 3 represents a pointer
    /// offset of `3 * size_of::<T>()` bytes.
    ///
    /// If the `T` is a ZST, then we instead track the index of the element
    /// in the table so that `erase` works properly (return
    /// `NonNull::new_unchecked((index + 1) as *mut T)`)
    ///
    /// # Safety
    ///
    /// If `mem::size_of::<T>() != 0`, then the safety rules are directly derived
    /// from the safety rules for [`<*mut T>::sub`] method of `*mut T` and the safety
    /// rules of [`NonNull::new_unchecked`] function.
    ///
    /// Thus, in order to uphold the safety contracts for the [`<*mut T>::sub`] method
    /// and [`NonNull::new_unchecked`] function, as well as for the correct
    /// logic of the work of this crate, the following rules are necessary and
    /// sufficient:
    ///
    /// * the `base` pointer must not be `dangling` and must points to the
    ///   end of the first `value element` from the `data part` of the table, i.e.
    ///   must be the pointer that returned by [`RawTable::data_end`] or by
    ///   [`RawTableInner::data_end<T>`];
    ///
    /// * `index` must not be greater than `RawTableInner.bucket_mask`, i.e.
    ///   `index <= RawTableInner.bucket_mask` or, in other words, `(index + 1)`
    ///   must be no greater than the number returned by the function
    ///   [`RawTable::buckets`] or [`RawTableInner::buckets`].
    ///
    /// If `mem::size_of::<T>() == 0`, then the only requirement is that the
    /// `index` must not be greater than `RawTableInner.bucket_mask`, i.e.
    /// `index <= RawTableInner.bucket_mask` or, in other words, `(index + 1)`
    /// must be no greater than the number returned by the function
    /// [`RawTable::buckets`] or [`RawTableInner::buckets`].
    ///
    /// [`Bucket`]: crate::raw::Bucket
    /// [`<*mut T>::sub`]: https://doc.rust-lang.org/core/primitive.pointer.html#method.sub-1
    /// [`NonNull::new_unchecked`]: https://doc.rust-lang.org/stable/std/ptr/struct.NonNull.html#method.new_unchecked
    /// [`RawTable::data_end`]: crate::raw::RawTable::data_end
    /// [`RawTableInner::data_end<T>`]: RawTableInner::data_end<T>
    /// [`RawTable::buckets`]: crate::raw::RawTable::buckets
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    #[inline]
    unsafe fn from_base_index(base: NonNull<T>, index: usize) -> Self {
        // If mem::size_of::<T>() != 0 then return a pointer to an `element` in
        // the data part of the table (we start counting from "0", so that
        // in the expression T[last], the "last" index actually one less than the
        // "buckets" number in the table, i.e. "last = RawTableInner.bucket_mask"):
        //
        //                   `from_base_index(base, 1).as_ptr()` returns a pointer that
        //                   points here in the data part of the table
        //                   (to the start of T1)
        //                        |
        //                        |        `base: NonNull<T>` must point here
        //                        |         (to the end of T0 or to the start of C0)
        //                        v         v
        // [Padding], Tlast, ..., |T1|, T0, |C0, C1, ..., Clast
        //                           ^
        //                           `from_base_index(base, 1)` returns a pointer
        //                           that points here in the data part of the table
        //                           (to the end of T1)
        //
        // where: T0...Tlast - our stored data; C0...Clast - control bytes
        // or metadata for data.
        let ptr = if T::IS_ZERO_SIZED {
            // won't overflow because index must be less than length (bucket_mask)
            // and bucket_mask is guaranteed to be less than `isize::MAX`
            // (see TableLayout::calculate_layout_for method)
            invalid_mut(index + 1)
        } else {
            base.as_ptr().sub(index)
        };
        Self {
            ptr: NonNull::new_unchecked(ptr),
        }
    }

    /// Calculates the index of a [`Bucket`] as distance between two pointers
    /// (convenience for `base.as_ptr().offset_from(self.ptr.as_ptr()) as usize`).
    /// The returned value is in units of T: the distance in bytes divided by
    /// [`core::mem::size_of::<T>()`].
    ///
    /// If the `T` is a ZST, then we return the index of the element in
    /// the table so that `erase` works properly (return `self.ptr.as_ptr() as usize - 1`).
    ///
    /// This function is the inverse of [`from_base_index`].
    ///
    /// # Safety
    ///
    /// If `mem::size_of::<T>() != 0`, then the safety rules are directly derived
    /// from the safety rules for [`<*const T>::offset_from`] method of `*const T`.
    ///
    /// Thus, in order to uphold the safety contracts for [`<*const T>::offset_from`]
    /// method, as well as for the correct logic of the work of this crate, the
    /// following rules are necessary and sufficient:
    ///
    /// * `base` contained pointer must not be `dangling` and must point to the
    ///   end of the first `element` from the `data part` of the table, i.e.
    ///   must be a pointer that returns by [`RawTable::data_end`] or by
    ///   [`RawTableInner::data_end<T>`];
    ///
    /// * `self` also must not contain dangling pointer;
    ///
    /// * both `self` and `base` must be created from the same [`RawTable`]
    ///   (or [`RawTableInner`]).
    ///
    /// If `mem::size_of::<T>() == 0`, this function is always safe.
    ///
    /// [`Bucket`]: crate::raw::Bucket
    /// [`from_base_index`]: crate::raw::Bucket::from_base_index
    /// [`RawTable::data_end`]: crate::raw::RawTable::data_end
    /// [`RawTableInner::data_end<T>`]: RawTableInner::data_end<T>
    /// [`RawTable`]: crate::raw::RawTable
    /// [`RawTableInner`]: RawTableInner
    /// [`<*const T>::offset_from`]: https://doc.rust-lang.org/nightly/core/primitive.pointer.html#method.offset_from
    #[inline]
    unsafe fn to_base_index(&self, base: NonNull<T>) -> usize {
        // If mem::size_of::<T>() != 0 then return an index under which we used to store the
        // `element` in the data part of the table (we start counting from "0", so
        // that in the expression T[last], the "last" index actually is one less than the
        // "buckets" number in the table, i.e. "last = RawTableInner.bucket_mask").
        // For example for 5th element in table calculation is performed like this:
        //
        //                        mem::size_of::<T>()
        //                          |
        //                          |         `self = from_base_index(base, 5)` that returns pointer
        //                          |         that points here in the data part of the table
        //                          |         (to the end of T5)
        //                          |           |                    `base: NonNull<T>` must point here
        //                          v           |                    (to the end of T0 or to the start of C0)
        //                        /???\         v                      v
        // [Padding], Tlast, ..., |T10|, ..., T5|, T4, T3, T2, T1, T0, |C0, C1, C2, C3, C4, C5, ..., C10, ..., Clast
        //                                      \__________  __________/
        //                                                 \/
        //                                     `bucket.to_base_index(base)` = 5
        //                                     (base.as_ptr() as usize - self.ptr.as_ptr() as usize) / mem::size_of::<T>()
        //
        // where: T0...Tlast - our stored data; C0...Clast - control bytes or metadata for data.
        if T::IS_ZERO_SIZED {
            // this can not be UB
            self.ptr.as_ptr() as usize - 1
        } else {
            offset_from(base.as_ptr(), self.ptr.as_ptr())
        }
    }

    /// Acquires the underlying raw pointer `*mut T` to `data`.
    ///
    /// # Note
    ///
    /// If `T` is not [`Copy`], do not use `*mut T` methods that can cause calling the
    /// destructor of `T` (for example the [`<*mut T>::drop_in_place`] method), because
    /// for properly dropping the data we also need to clear `data` control bytes. If we
    /// drop data, but do not clear `data control byte` it leads to double drop when
    /// [`RawTable`] goes out of scope.
    ///
    /// If you modify an already initialized `value`, so [`Hash`] and [`Eq`] on the new
    /// `T` value and its borrowed form *must* match those for the old `T` value, as the map
    /// will not re-evaluate where the new value should go, meaning the value may become
    /// "lost" if their location does not reflect their state.
    ///
    /// [`RawTable`]: crate::raw::RawTable
    /// [`<*mut T>::drop_in_place`]: https://doc.rust-lang.org/core/primitive.pointer.html#method.drop_in_place
    /// [`Hash`]: https://doc.rust-lang.org/core/hash/trait.Hash.html
    /// [`Eq`]: https://doc.rust-lang.org/core/cmp/trait.Eq.html
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        if T::IS_ZERO_SIZED {
            // Just return an arbitrary ZST pointer which is properly aligned
            // invalid pointer is good enough for ZST
            invalid_mut(mem::align_of::<T>())
        } else {
            unsafe { self.ptr.as_ptr().sub(1) }
        }
    }

    /// Acquires the underlying non-null pointer `*mut T` to `data`.
    #[inline]
    fn as_non_null(&self) -> NonNull<T> {
        // SAFETY: `self.ptr` is already a `NonNull`
        unsafe { NonNull::new_unchecked(self.as_ptr()) }
    }

    /// Create a new [`Bucket`] that is offset from the `self` by the given
    /// `offset`. The pointer calculation is performed by calculating the
    /// offset from `self` pointer (convenience for `self.ptr.as_ptr().sub(offset)`).
    /// This function is used for iterators.
    ///
    /// `offset` is in units of `T`; e.g., a `offset` of 3 represents a pointer
    /// offset of `3 * size_of::<T>()` bytes.
    ///
    /// # Safety
    ///
    /// If `mem::size_of::<T>() != 0`, then the safety rules are directly derived
    /// from the safety rules for [`<*mut T>::sub`] method of `*mut T` and safety
    /// rules of [`NonNull::new_unchecked`] function.
    ///
    /// Thus, in order to uphold the safety contracts for [`<*mut T>::sub`] method
    /// and [`NonNull::new_unchecked`] function, as well as for the correct
    /// logic of the work of this crate, the following rules are necessary and
    /// sufficient:
    ///
    /// * `self` contained pointer must not be `dangling`;
    ///
    /// * `self.to_base_index() + offset` must not be greater than `RawTableInner.bucket_mask`,
    ///   i.e. `(self.to_base_index() + offset) <= RawTableInner.bucket_mask` or, in other
    ///   words, `self.to_base_index() + offset + 1` must be no greater than the number returned
    ///   by the function [`RawTable::buckets`] or [`RawTableInner::buckets`].
    ///
    /// If `mem::size_of::<T>() == 0`, then the only requirement is that the
    /// `self.to_base_index() + offset` must not be greater than `RawTableInner.bucket_mask`,
    /// i.e. `(self.to_base_index() + offset) <= RawTableInner.bucket_mask` or, in other words,
    /// `self.to_base_index() + offset + 1` must be no greater than the number returned by the
    /// function [`RawTable::buckets`] or [`RawTableInner::buckets`].
    ///
    /// [`Bucket`]: crate::raw::Bucket
    /// [`<*mut T>::sub`]: https://doc.rust-lang.org/core/primitive.pointer.html#method.sub-1
    /// [`NonNull::new_unchecked`]: https://doc.rust-lang.org/stable/std/ptr/struct.NonNull.html#method.new_unchecked
    /// [`RawTable::buckets`]: crate::raw::RawTable::buckets
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    #[inline]
    unsafe fn next_n(&self, offset: usize) -> Self {
        let ptr = if T::IS_ZERO_SIZED {
            // invalid pointer is good enough for ZST
            invalid_mut(self.ptr.as_ptr() as usize + offset)
        } else {
            self.ptr.as_ptr().sub(offset)
        };
        Self {
            ptr: NonNull::new_unchecked(ptr),
        }
    }

    /// Executes the destructor (if any) of the pointed-to `data`.
    ///
    /// # Safety
    ///
    /// See [`ptr::drop_in_place`] for safety concerns.
    ///
    /// You should use [`RawTable::erase`] instead of this function,
    /// or be careful with calling this function directly, because for
    /// properly dropping the data we need also clear `data` control bytes.
    /// If we drop data, but do not erase `data control byte` it leads to
    /// double drop when [`RawTable`] goes out of scope.
    ///
    /// [`ptr::drop_in_place`]: https://doc.rust-lang.org/core/ptr/fn.drop_in_place.html
    /// [`RawTable`]: crate::raw::RawTable
    /// [`RawTable::erase`]: crate::raw::RawTable::erase
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) unsafe fn drop(&self) {
        self.as_ptr().drop_in_place();
    }

    /// Reads the `value` from `self` without moving it. This leaves the
    /// memory in `self` unchanged.
    ///
    /// # Safety
    ///
    /// See [`ptr::read`] for safety concerns.
    ///
    /// You should use [`RawTable::remove`] instead of this function,
    /// or be careful with calling this function directly, because compiler
    /// calls its destructor when the read `value` goes out of scope. It
    /// can cause double dropping when [`RawTable`] goes out of scope,
    /// because of not erased `data control byte`.
    ///
    /// [`ptr::read`]: https://doc.rust-lang.org/core/ptr/fn.read.html
    /// [`RawTable`]: crate::raw::RawTable
    /// [`RawTable::remove`]: crate::raw::RawTable::remove
    #[inline]
    pub(crate) unsafe fn read(&self) -> T {
        self.as_ptr().read()
    }

    /// Overwrites a memory location with the given `value` without reading
    /// or dropping the old value (like [`ptr::write`] function).
    ///
    /// # Safety
    ///
    /// See [`ptr::write`] for safety concerns.
    ///
    /// # Note
    ///
    /// [`Hash`] and [`Eq`] on the new `T` value and its borrowed form *must* match
    /// those for the old `T` value, as the map will not re-evaluate where the new
    /// value should go, meaning the value may become "lost" if their location
    /// does not reflect their state.
    ///
    /// [`ptr::write`]: https://doc.rust-lang.org/core/ptr/fn.write.html
    /// [`Hash`]: https://doc.rust-lang.org/core/hash/trait.Hash.html
    /// [`Eq`]: https://doc.rust-lang.org/core/cmp/trait.Eq.html
    #[inline]
    pub(crate) unsafe fn write(&self, val: T) {
        self.as_ptr().write(val);
    }

    /// Returns a shared immutable reference to the `value`.
    ///
    /// # Safety
    ///
    /// See [`NonNull::as_ref`] for safety concerns.
    ///
    /// [`NonNull::as_ref`]: https://doc.rust-lang.org/core/ptr/struct.NonNull.html#method.as_ref
    #[inline]
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        &*self.as_ptr()
    }

    /// Returns a unique mutable reference to the `value`.
    ///
    /// # Safety
    ///
    /// See [`NonNull::as_mut`] for safety concerns.
    ///
    /// # Note
    ///
    /// [`Hash`] and [`Eq`] on the new `T` value and its borrowed form *must* match
    /// those for the old `T` value, as the map will not re-evaluate where the new
    /// value should go, meaning the value may become "lost" if their location
    /// does not reflect their state.
    ///
    /// [`NonNull::as_mut`]: https://doc.rust-lang.org/core/ptr/struct.NonNull.html#method.as_mut
    /// [`Hash`]: https://doc.rust-lang.org/core/hash/trait.Hash.html
    /// [`Eq`]: https://doc.rust-lang.org/core/cmp/trait.Eq.html
    #[inline]
    pub unsafe fn as_mut<'a>(&self) -> &'a mut T {
        &mut *self.as_ptr()
    }
}

/// A raw hash table with an unsafe API.
pub struct RawTable<T, A: Allocator = Global> {
    table: RawTableInner,
    alloc: A,
    // Tell dropck that we own instances of T.
    marker: PhantomData<T>,
}

/// Non-generic part of `RawTable` which allows functions to be instantiated only once regardless
/// of how many different key-value types are used.
struct RawTableInner {
    // Mask to get an index from a hash value. The value is one less than the
    // number of buckets in the table.
    bucket_mask: usize,

    // [Padding], T_n, ..., T1, T0, C0, C1, ...
    //                              ^ points here
    ctrl: NonNull<u8>,

    // Number of elements that can be inserted before we need to grow the table
    growth_left: usize,

    // Number of elements in the table, only really used by len()
    items: usize,
}

impl<T> RawTable<T, Global> {
    /// Creates a new empty hash table without allocating any memory.
    ///
    /// In effect this returns a table with exactly 1 bucket. However we can
    /// leave the data pointer dangling since that bucket is never written to
    /// due to our load factor forcing us to always have at least 1 free bucket.
    #[inline]
    #[cfg_attr(feature = "rustc-dep-of-std", rustc_const_stable_indirect)]
    pub const fn new() -> Self {
        Self {
            table: RawTableInner::NEW,
            alloc: Global,
            marker: PhantomData,
        }
    }

    /// Allocates a new hash table with at least enough capacity for inserting
    /// the given number of elements without reallocating.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_in(capacity, Global)
    }
}

impl<T, A: Allocator> RawTable<T, A> {
    const TABLE_LAYOUT: TableLayout = TableLayout::new::<T>();

    /// Creates a new empty hash table without allocating any memory, using the
    /// given allocator.
    ///
    /// In effect this returns a table with exactly 1 bucket. However we can
    /// leave the data pointer dangling since that bucket is never written to
    /// due to our load factor forcing us to always have at least 1 free bucket.
    #[inline]
    #[cfg_attr(feature = "rustc-dep-of-std", rustc_const_stable_indirect)]
    pub const fn new_in(alloc: A) -> Self {
        Self {
            table: RawTableInner::NEW,
            alloc,
            marker: PhantomData,
        }
    }

    /// Allocates a new hash table with the given number of buckets.
    ///
    /// The control bytes are left uninitialized.
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn new_uninitialized(
        alloc: A,
        buckets: usize,
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError> {
        debug_assert!(buckets.is_power_of_two());

        Ok(Self {
            table: RawTableInner::new_uninitialized(
                &alloc,
                Self::TABLE_LAYOUT,
                buckets,
                fallibility,
            )?,
            alloc,
            marker: PhantomData,
        })
    }

    /// Allocates a new hash table using the given allocator, with at least enough capacity for
    /// inserting the given number of elements without reallocating.
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            table: RawTableInner::with_capacity(&alloc, Self::TABLE_LAYOUT, capacity),
            alloc,
            marker: PhantomData,
        }
    }

    /// Returns a reference to the underlying allocator.
    #[inline]
    pub fn allocator(&self) -> &A {
        &self.alloc
    }

    /// Returns pointer to one past last `data` element in the table as viewed from
    /// the start point of the allocation.
    ///
    /// The caller must ensure that the `RawTable` outlives the returned [`NonNull<T>`],
    /// otherwise using it may result in [`undefined behavior`].
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    pub fn data_end(&self) -> NonNull<T> {
        //                        `self.table.ctrl.cast()` returns pointer that
        //                        points here (to the end of `T0`)
        //                          ∨
        // [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, CTa_1, ..., CTa_m
        //                           \________  ________/
        //                                    \/
        //       `n = buckets - 1`, i.e. `RawTable::buckets() - 1`
        //
        // where: T0...T_n  - our stored data;
        //        CT0...CT_n - control bytes or metadata for `data`.
        //        CTa_0...CTa_m - additional control bytes, where `m = Group::WIDTH - 1` (so that the search
        //                        with loading `Group` bytes from the heap works properly, even if the result
        //                        of `h1(hash) & self.bucket_mask` is equal to `self.bucket_mask`). See also
        //                        `RawTableInner::set_ctrl` function.
        //
        // P.S. `h1(hash) & self.bucket_mask` is the same as `hash as usize % self.buckets()` because the number
        // of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
        self.table.ctrl.cast()
    }

    /// Returns pointer to start of data table.
    #[inline]
    #[cfg(feature = "nightly")]
    pub unsafe fn data_start(&self) -> NonNull<T> {
        NonNull::new_unchecked(self.data_end().as_ptr().wrapping_sub(self.buckets()))
    }

    /// Returns the total amount of memory allocated internally by the hash
    /// table, in bytes.
    ///
    /// The returned number is informational only. It is intended to be
    /// primarily used for memory profiling.
    #[inline]
    pub fn allocation_size(&self) -> usize {
        // SAFETY: We use the same `table_layout` that was used to allocate
        // this table.
        unsafe { self.table.allocation_size_or_zero(Self::TABLE_LAYOUT) }
    }

    /// Returns the index of a bucket from a `Bucket`.
    #[inline]
    pub unsafe fn bucket_index(&self, bucket: &Bucket<T>) -> usize {
        bucket.to_base_index(self.data_end())
    }

    /// Returns a pointer to an element in the table.
    ///
    /// The caller must ensure that the `RawTable` outlives the returned [`Bucket<T>`],
    /// otherwise using it may result in [`undefined behavior`].
    ///
    /// # Safety
    ///
    /// If `mem::size_of::<T>() != 0`, then the caller of this function must observe the
    /// following safety rules:
    ///
    /// * The table must already be allocated;
    ///
    /// * The `index` must not be greater than the number returned by the [`RawTable::buckets`]
    ///   function, i.e. `(index + 1) <= self.buckets()`.
    ///
    /// It is safe to call this function with index of zero (`index == 0`) on a table that has
    /// not been allocated, but using the returned [`Bucket`] results in [`undefined behavior`].
    ///
    /// If `mem::size_of::<T>() == 0`, then the only requirement is that the `index` must
    /// not be greater than the number returned by the [`RawTable::buckets`] function, i.e.
    /// `(index + 1) <= self.buckets()`.
    ///
    /// [`RawTable::buckets`]: RawTable::buckets
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    pub unsafe fn bucket(&self, index: usize) -> Bucket<T> {
        // If mem::size_of::<T>() != 0 then return a pointer to the `element` in the `data part` of the table
        // (we start counting from "0", so that in the expression T[n], the "n" index actually one less than
        // the "buckets" number of our `RawTable`, i.e. "n = RawTable::buckets() - 1"):
        //
        //           `table.bucket(3).as_ptr()` returns a pointer that points here in the `data`
        //           part of the `RawTable`, i.e. to the start of T3 (see `Bucket::as_ptr`)
        //                  |
        //                  |               `base = self.data_end()` points here
        //                  |               (to the start of CT0 or to the end of T0)
        //                  v                 v
        // [Pad], T_n, ..., |T3|, T2, T1, T0, |CT0, CT1, CT2, CT3, ..., CT_n, CTa_0, CTa_1, ..., CTa_m
        //                     ^                                              \__________  __________/
        //        `table.bucket(3)` returns a pointer that points                        \/
        //         here in the `data` part of the `RawTable` (to              additional control bytes
        //         the end of T3)                                              `m = Group::WIDTH - 1`
        //
        // where: T0...T_n  - our stored data;
        //        CT0...CT_n - control bytes or metadata for `data`;
        //        CTa_0...CTa_m - additional control bytes (so that the search with loading `Group` bytes from
        //                        the heap works properly, even if the result of `h1(hash) & self.table.bucket_mask`
        //                        is equal to `self.table.bucket_mask`). See also `RawTableInner::set_ctrl` function.
        //
        // P.S. `h1(hash) & self.table.bucket_mask` is the same as `hash as usize % self.buckets()` because the number
        // of buckets is a power of two, and `self.table.bucket_mask = self.buckets() - 1`.
        debug_assert_ne!(self.table.bucket_mask, 0);
        debug_assert!(index < self.buckets());
        Bucket::from_base_index(self.data_end(), index)
    }

    /// Erases an element from the table without dropping it.
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn erase_no_drop(&mut self, item: &Bucket<T>) {
        let index = self.bucket_index(item);
        self.table.erase(index);
    }

    /// Erases an element from the table, dropping it in place.
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::needless_pass_by_value)]
    pub unsafe fn erase(&mut self, item: Bucket<T>) {
        // Erase the element from the table first since drop might panic.
        self.erase_no_drop(&item);
        item.drop();
    }

    /// Removes an element from the table, returning it.
    ///
    /// This also returns an `InsertSlot` pointing to the newly free bucket.
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::needless_pass_by_value)]
    pub unsafe fn remove(&mut self, item: Bucket<T>) -> (T, InsertSlot) {
        self.erase_no_drop(&item);
        (
            item.read(),
            InsertSlot {
                index: self.bucket_index(&item),
            },
        )
    }

    /// Finds and removes an element from the table, returning it.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove_entry(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<T> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.find(hash, eq) {
            Some(bucket) => Some(unsafe { self.remove(bucket).0 }),
            None => None,
        }
    }

    /// Marks all table buckets as empty without dropping their contents.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn clear_no_drop(&mut self) {
        self.table.clear_no_drop();
    }

    /// Removes all elements from the table without freeing the backing memory.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn clear(&mut self) {
        if self.is_empty() {
            // Special case empty table to avoid surprising O(capacity) time.
            return;
        }
        // Ensure that the table is reset even if one of the drops panic
        let mut self_ = guard(self, |self_| self_.clear_no_drop());
        unsafe {
            // SAFETY: ScopeGuard sets to zero the `items` field of the table
            // even in case of panic during the dropping of the elements so
            // that there will be no double drop of the elements.
            self_.table.drop_elements::<T>();
        }
    }

    /// Shrinks the table to fit `max(self.len(), min_size)` elements.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn shrink_to(&mut self, min_size: usize, hasher: impl Fn(&T) -> u64) {
        // Calculate the minimal number of elements that we need to reserve
        // space for.
        let min_size = usize::max(self.table.items, min_size);
        if min_size == 0 {
            let mut old_inner = mem::replace(&mut self.table, RawTableInner::NEW);
            unsafe {
                // SAFETY:
                // 1. We call the function only once;
                // 2. We know for sure that `alloc` and `table_layout` matches the [`Allocator`]
                //    and [`TableLayout`] that were used to allocate this table.
                // 3. If any elements' drop function panics, then there will only be a memory leak,
                //    because we have replaced the inner table with a new one.
                old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
            }
            return;
        }

        // Calculate the number of buckets that we need for this number of
        // elements. If the calculation overflows then the requested bucket
        // count must be larger than what we have right and nothing needs to be
        // done.
        let min_buckets = match capacity_to_buckets(min_size, Self::TABLE_LAYOUT) {
            Some(buckets) => buckets,
            None => return,
        };

        // If we have more buckets than we need, shrink the table.
        if min_buckets < self.buckets() {
            // Fast path if the table is empty
            if self.table.items == 0 {
                let new_inner =
                    RawTableInner::with_capacity(&self.alloc, Self::TABLE_LAYOUT, min_size);
                let mut old_inner = mem::replace(&mut self.table, new_inner);
                unsafe {
                    // SAFETY:
                    // 1. We call the function only once;
                    // 2. We know for sure that `alloc` and `table_layout` matches the [`Allocator`]
                    //    and [`TableLayout`] that were used to allocate this table.
                    // 3. If any elements' drop function panics, then there will only be a memory leak,
                    //    because we have replaced the inner table with a new one.
                    old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
                }
            } else {
                // Avoid `Result::unwrap_or_else` because it bloats LLVM IR.
                unsafe {
                    // SAFETY:
                    // 1. We know for sure that `min_size >= self.table.items`.
                    // 2. The [`RawTableInner`] must already have properly initialized control bytes since
                    //    we will never expose RawTable::new_uninitialized in a public API.
                    if self
                        .resize(min_size, hasher, Fallibility::Infallible)
                        .is_err()
                    {
                        // SAFETY: The result of calling the `resize` function cannot be an error
                        // because `fallibility == Fallibility::Infallible.
                        hint::unreachable_unchecked()
                    }
                }
            }
        }
    }

    /// Ensures that at least `additional` items can be inserted into the table
    /// without reallocation.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn reserve(&mut self, additional: usize, hasher: impl Fn(&T) -> u64) {
        if unlikely(additional > self.table.growth_left) {
            // Avoid `Result::unwrap_or_else` because it bloats LLVM IR.
            unsafe {
                // SAFETY: The [`RawTableInner`] must already have properly initialized control
                // bytes since we will never expose RawTable::new_uninitialized in a public API.
                if self
                    .reserve_rehash(additional, hasher, Fallibility::Infallible)
                    .is_err()
                {
                    // SAFETY: All allocation errors will be caught inside `RawTableInner::reserve_rehash`.
                    hint::unreachable_unchecked()
                }
            }
        }
    }

    /// Tries to ensure that at least `additional` items can be inserted into
    /// the table without reallocation.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn try_reserve(
        &mut self,
        additional: usize,
        hasher: impl Fn(&T) -> u64,
    ) -> Result<(), TryReserveError> {
        if additional > self.table.growth_left {
            // SAFETY: The [`RawTableInner`] must already have properly initialized control
            // bytes since we will never expose RawTable::new_uninitialized in a public API.
            unsafe { self.reserve_rehash(additional, hasher, Fallibility::Fallible) }
        } else {
            Ok(())
        }
    }

    /// Out-of-line slow path for `reserve` and `try_reserve`.
    ///
    /// # Safety
    ///
    /// The [`RawTableInner`] must have properly initialized control bytes,
    /// otherwise calling this function results in [`undefined behavior`]
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cold]
    #[inline(never)]
    unsafe fn reserve_rehash(
        &mut self,
        additional: usize,
        hasher: impl Fn(&T) -> u64,
        fallibility: Fallibility,
    ) -> Result<(), TryReserveError> {
        unsafe {
            // SAFETY:
            // 1. We know for sure that `alloc` and `layout` matches the [`Allocator`] and
            //    [`TableLayout`] that were used to allocate this table.
            // 2. The `drop` function is the actual drop function of the elements stored in
            //    the table.
            // 3. The caller ensures that the control bytes of the `RawTableInner`
            //    are already initialized.
            self.table.reserve_rehash_inner(
                &self.alloc,
                additional,
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                fallibility,
                Self::TABLE_LAYOUT,
                if T::NEEDS_DROP {
                    Some(|ptr| ptr::drop_in_place(ptr as *mut T))
                } else {
                    None
                },
            )
        }
    }

    /// Allocates a new table of a different size and moves the contents of the
    /// current table into it.
    ///
    /// # Safety
    ///
    /// The [`RawTableInner`] must have properly initialized control bytes,
    /// otherwise calling this function results in [`undefined behavior`]
    ///
    /// The caller of this function must ensure that `capacity >= self.table.items`
    /// otherwise:
    ///
    /// * If `self.table.items != 0`, calling of this function with `capacity`
    ///   equal to 0 (`capacity == 0`) results in [`undefined behavior`].
    ///
    /// * If `self.table.items > capacity_to_buckets(capacity, Self::TABLE_LAYOUT)`
    ///   calling this function are never return (will loop infinitely).
    ///
    /// See [`RawTableInner::find_insert_slot`] for more information.
    ///
    /// [`RawTableInner::find_insert_slot`]: RawTableInner::find_insert_slot
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    unsafe fn resize(
        &mut self,
        capacity: usize,
        hasher: impl Fn(&T) -> u64,
        fallibility: Fallibility,
    ) -> Result<(), TryReserveError> {
        // SAFETY:
        // 1. The caller of this function guarantees that `capacity >= self.table.items`.
        // 2. We know for sure that `alloc` and `layout` matches the [`Allocator`] and
        //    [`TableLayout`] that were used to allocate this table.
        // 3. The caller ensures that the control bytes of the `RawTableInner`
        //    are already initialized.
        self.table.resize_inner(
            &self.alloc,
            capacity,
            &|table, index| hasher(table.bucket::<T>(index).as_ref()),
            fallibility,
            Self::TABLE_LAYOUT,
        )
    }

    /// Inserts a new element into the table, and returns its raw bucket.
    ///
    /// This does not check if the given element already exists in the table.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, hash: u64, value: T, hasher: impl Fn(&T) -> u64) -> Bucket<T> {
        unsafe {
            // SAFETY:
            // 1. The [`RawTableInner`] must already have properly initialized control bytes since
            //    we will never expose `RawTable::new_uninitialized` in a public API.
            //
            // 2. We reserve additional space (if necessary) right after calling this function.
            let mut slot = self.table.find_insert_slot(hash);

            // We can avoid growing the table once we have reached our load factor if we are replacing
            // a tombstone. This works since the number of EMPTY slots does not change in this case.
            //
            // SAFETY: The function is guaranteed to return [`InsertSlot`] that contains an index
            // in the range `0..=self.buckets()`.
            let old_ctrl = *self.table.ctrl(slot.index);
            if unlikely(self.table.growth_left == 0 && old_ctrl.special_is_empty()) {
                self.reserve(1, hasher);
                // SAFETY: We know for sure that `RawTableInner` has control bytes
                // initialized and that there is extra space in the table.
                slot = self.table.find_insert_slot(hash);
            }

            self.insert_in_slot(hash, slot, value)
        }
    }

    /// Inserts a new element into the table, and returns a mutable reference to it.
    ///
    /// This does not check if the given element already exists in the table.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_entry(&mut self, hash: u64, value: T, hasher: impl Fn(&T) -> u64) -> &mut T {
        unsafe { self.insert(hash, value, hasher).as_mut() }
    }

    /// Inserts a new element into the table, without growing the table.
    ///
    /// There must be enough space in the table to insert the new element.
    ///
    /// This does not check if the given element already exists in the table.
    #[cfg_attr(feature = "inline-more", inline)]
    #[cfg(feature = "rustc-internal-api")]
    pub unsafe fn insert_no_grow(&mut self, hash: u64, value: T) -> Bucket<T> {
        let (index, old_ctrl) = self.table.prepare_insert_slot(hash);
        let bucket = self.table.bucket(index);

        // If we are replacing a DELETED entry then we don't need to update
        // the load counter.
        self.table.growth_left -= old_ctrl.special_is_empty() as usize;

        bucket.write(value);
        self.table.items += 1;
        bucket
    }

    /// Temporary removes a bucket, applying the given function to the removed
    /// element and optionally put back the returned value in the same bucket.
    ///
    /// Returns `true` if the bucket still contains an element
    ///
    /// This does not check if the given bucket is actually occupied.
    #[cfg_attr(feature = "inline-more", inline)]
    pub unsafe fn replace_bucket_with<F>(&mut self, bucket: Bucket<T>, f: F) -> bool
    where
        F: FnOnce(T) -> Option<T>,
    {
        let index = self.bucket_index(&bucket);
        let old_ctrl = *self.table.ctrl(index);
        debug_assert!(self.is_bucket_full(index));
        let old_growth_left = self.table.growth_left;
        let item = self.remove(bucket).0;
        if let Some(new_item) = f(item) {
            self.table.growth_left = old_growth_left;
            self.table.set_ctrl(index, old_ctrl);
            self.table.items += 1;
            self.bucket(index).write(new_item);
            true
        } else {
            false
        }
    }

    /// Searches for an element in the table. If the element is not found,
    /// returns `Err` with the position of a slot where an element with the
    /// same hash could be inserted.
    ///
    /// This function may resize the table if additional space is required for
    /// inserting an element.
    #[inline]
    pub fn find_or_find_insert_slot(
        &mut self,
        hash: u64,
        mut eq: impl FnMut(&T) -> bool,
        hasher: impl Fn(&T) -> u64,
    ) -> Result<Bucket<T>, InsertSlot> {
        self.reserve(1, hasher);

        unsafe {
            // SAFETY:
            // 1. We know for sure that there is at least one empty `bucket` in the table.
            // 2. The [`RawTableInner`] must already have properly initialized control bytes since we will
            //    never expose `RawTable::new_uninitialized` in a public API.
            // 3. The `find_or_find_insert_slot_inner` function returns the `index` of only the full bucket,
            //    which is in the range `0..self.buckets()` (since there is at least one empty `bucket` in
            //    the table), so calling `self.bucket(index)` and `Bucket::as_ref` is safe.
            match self
                .table
                .find_or_find_insert_slot_inner(hash, &mut |index| eq(self.bucket(index).as_ref()))
            {
                // SAFETY: See explanation above.
                Ok(index) => Ok(self.bucket(index)),
                Err(slot) => Err(slot),
            }
        }
    }

    /// Inserts a new element into the table in the given slot, and returns its
    /// raw bucket.
    ///
    /// # Safety
    ///
    /// `slot` must point to a slot previously returned by
    /// `find_or_find_insert_slot`, and no mutation of the table must have
    /// occurred since that call.
    #[inline]
    pub unsafe fn insert_in_slot(&mut self, hash: u64, slot: InsertSlot, value: T) -> Bucket<T> {
        let old_ctrl = *self.table.ctrl(slot.index);
        self.table.record_item_insert_at(slot.index, old_ctrl, hash);

        let bucket = self.bucket(slot.index);
        bucket.write(value);
        bucket
    }

    /// Searches for an element in the table.
    #[inline]
    pub fn find(&self, hash: u64, mut eq: impl FnMut(&T) -> bool) -> Option<Bucket<T>> {
        unsafe {
            // SAFETY:
            // 1. The [`RawTableInner`] must already have properly initialized control bytes since we
            //    will never expose `RawTable::new_uninitialized` in a public API.
            // 1. The `find_inner` function returns the `index` of only the full bucket, which is in
            //    the range `0..self.buckets()`, so calling `self.bucket(index)` and `Bucket::as_ref`
            //    is safe.
            let result = self
                .table
                .find_inner(hash, &mut |index| eq(self.bucket(index).as_ref()));

            // Avoid `Option::map` because it bloats LLVM IR.
            match result {
                // SAFETY: See explanation above.
                Some(index) => Some(self.bucket(index)),
                None => None,
            }
        }
    }

    /// Gets a reference to an element in the table.
    #[inline]
    pub fn get(&self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&T> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.find(hash, eq) {
            Some(bucket) => Some(unsafe { bucket.as_ref() }),
            None => None,
        }
    }

    /// Gets a mutable reference to an element in the table.
    #[inline]
    pub fn get_mut(&mut self, hash: u64, eq: impl FnMut(&T) -> bool) -> Option<&mut T> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.find(hash, eq) {
            Some(bucket) => Some(unsafe { bucket.as_mut() }),
            None => None,
        }
    }

    /// Attempts to get mutable references to `N` entries in the table at once.
    ///
    /// Returns an array of length `N` with the results of each query.
    ///
    /// At most one mutable reference will be returned to any entry. `None` will be returned if any
    /// of the hashes are duplicates. `None` will be returned if the hash is not found.
    ///
    /// The `eq` argument should be a closure such that `eq(i, k)` returns true if `k` is equal to
    /// the `i`th key to be looked up.
    pub fn get_many_mut<const N: usize>(
        &mut self,
        hashes: [u64; N],
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        unsafe {
            let ptrs = self.get_many_mut_pointers(hashes, eq);

            for (i, cur) in ptrs.iter().enumerate() {
                if cur.is_some() && ptrs[..i].contains(cur) {
                    panic!("duplicate keys found");
                }
            }
            // All bucket are distinct from all previous buckets so we're clear to return the result
            // of the lookup.

            ptrs.map(|ptr| ptr.map(|mut ptr| ptr.as_mut()))
        }
    }

    pub unsafe fn get_many_unchecked_mut<const N: usize>(
        &mut self,
        hashes: [u64; N],
        eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<&'_ mut T>; N] {
        let ptrs = self.get_many_mut_pointers(hashes, eq);
        ptrs.map(|ptr| ptr.map(|mut ptr| ptr.as_mut()))
    }

    unsafe fn get_many_mut_pointers<const N: usize>(
        &mut self,
        hashes: [u64; N],
        mut eq: impl FnMut(usize, &T) -> bool,
    ) -> [Option<NonNull<T>>; N] {
        array::from_fn(|i| {
            self.find(hashes[i], |k| eq(i, k))
                .map(|cur| cur.as_non_null())
        })
    }

    /// Returns the number of elements the map can hold without reallocating.
    ///
    /// This number is a lower bound; the table might be able to hold
    /// more, but is guaranteed to be able to hold at least this many.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.table.items + self.table.growth_left
    }

    /// Returns the number of elements in the table.
    #[inline]
    pub fn len(&self) -> usize {
        self.table.items
    }

    /// Returns `true` if the table contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of buckets in the table.
    #[inline]
    pub fn buckets(&self) -> usize {
        self.table.bucket_mask + 1
    }

    /// Checks whether the bucket at `index` is full.
    ///
    /// # Safety
    ///
    /// The caller must ensure `index` is less than the number of buckets.
    #[inline]
    pub unsafe fn is_bucket_full(&self, index: usize) -> bool {
        self.table.is_bucket_full(index)
    }

    /// Returns an iterator over every element in the table. It is up to
    /// the caller to ensure that the `RawTable` outlives the `RawIter`.
    /// Because we cannot make the `next` method unsafe on the `RawIter`
    /// struct, we have to make the `iter` method unsafe.
    #[inline]
    pub unsafe fn iter(&self) -> RawIter<T> {
        // SAFETY:
        // 1. The caller must uphold the safety contract for `iter` method.
        // 2. The [`RawTableInner`] must already have properly initialized control bytes since
        //    we will never expose RawTable::new_uninitialized in a public API.
        self.table.iter()
    }

    /// Returns an iterator over occupied buckets that could match a given hash.
    ///
    /// `RawTable` only stores 7 bits of the hash value, so this iterator may
    /// return items that have a hash value different than the one provided. You
    /// should always validate the returned values before using them.
    ///
    /// It is up to the caller to ensure that the `RawTable` outlives the
    /// `RawIterHash`. Because we cannot make the `next` method unsafe on the
    /// `RawIterHash` struct, we have to make the `iter_hash` method unsafe.
    #[cfg_attr(feature = "inline-more", inline)]
    pub unsafe fn iter_hash(&self, hash: u64) -> RawIterHash<T> {
        RawIterHash::new(self, hash)
    }

    /// Returns an iterator which removes all elements from the table without
    /// freeing the memory.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn drain(&mut self) -> RawDrain<'_, T, A> {
        unsafe {
            let iter = self.iter();
            self.drain_iter_from(iter)
        }
    }

    /// Returns an iterator which removes all elements from the table without
    /// freeing the memory.
    ///
    /// Iteration starts at the provided iterator's current location.
    ///
    /// It is up to the caller to ensure that the iterator is valid for this
    /// `RawTable` and covers all items that remain in the table.
    #[cfg_attr(feature = "inline-more", inline)]
    pub unsafe fn drain_iter_from(&mut self, iter: RawIter<T>) -> RawDrain<'_, T, A> {
        debug_assert_eq!(iter.len(), self.len());
        RawDrain {
            iter,
            table: mem::replace(&mut self.table, RawTableInner::NEW),
            orig_table: NonNull::from(&mut self.table),
            marker: PhantomData,
        }
    }

    /// Returns an iterator which consumes all elements from the table.
    ///
    /// Iteration starts at the provided iterator's current location.
    ///
    /// It is up to the caller to ensure that the iterator is valid for this
    /// `RawTable` and covers all items that remain in the table.
    pub unsafe fn into_iter_from(self, iter: RawIter<T>) -> RawIntoIter<T, A> {
        debug_assert_eq!(iter.len(), self.len());

        let allocation = self.into_allocation();
        RawIntoIter {
            iter,
            allocation,
            marker: PhantomData,
        }
    }

    /// Converts the table into a raw allocation. The contents of the table
    /// should be dropped using a `RawIter` before freeing the allocation.
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) fn into_allocation(self) -> Option<(NonNull<u8>, Layout, A)> {
        let alloc = if self.table.is_empty_singleton() {
            None
        } else {
            // Avoid `Option::unwrap_or_else` because it bloats LLVM IR.
            let (layout, ctrl_offset) =
                match Self::TABLE_LAYOUT.calculate_layout_for(self.table.buckets()) {
                    Some(lco) => lco,
                    None => unsafe { hint::unreachable_unchecked() },
                };
            Some((
                unsafe { NonNull::new_unchecked(self.table.ctrl.as_ptr().sub(ctrl_offset).cast()) },
                layout,
                unsafe { ptr::read(&self.alloc) },
            ))
        };
        mem::forget(self);
        alloc
    }
}

unsafe impl<T, A: Allocator> Send for RawTable<T, A>
where
    T: Send,
    A: Send,
{
}
unsafe impl<T, A: Allocator> Sync for RawTable<T, A>
where
    T: Sync,
    A: Sync,
{
}

impl RawTableInner {
    const NEW: Self = RawTableInner::new();

    /// Creates a new empty hash table without allocating any memory.
    ///
    /// In effect this returns a table with exactly 1 bucket. However we can
    /// leave the data pointer dangling since that bucket is never accessed
    /// due to our load factor forcing us to always have at least 1 free bucket.
    #[inline]
    const fn new() -> Self {
        Self {
            // Be careful to cast the entire slice to a raw pointer.
            ctrl: unsafe {
                NonNull::new_unchecked(Group::static_empty().as_ptr().cast_mut().cast())
            },
            bucket_mask: 0,
            items: 0,
            growth_left: 0,
        }
    }
}

impl RawTableInner {
    /// Allocates a new [`RawTableInner`] with the given number of buckets.
    /// The control bytes and buckets are left uninitialized.
    ///
    /// # Safety
    ///
    /// The caller of this function must ensure that the `buckets` is power of two
    /// and also initialize all control bytes of the length `self.bucket_mask + 1 +
    /// Group::WIDTH` with the [`Tag::EMPTY`] bytes.
    ///
    /// See also [`Allocator`] API for other safety concerns.
    ///
    /// [`Allocator`]: https://doc.rust-lang.org/alloc/alloc/trait.Allocator.html
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn new_uninitialized<A>(
        alloc: &A,
        table_layout: TableLayout,
        buckets: usize,
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError>
    where
        A: Allocator,
    {
        debug_assert!(buckets.is_power_of_two());

        // Avoid `Option::ok_or_else` because it bloats LLVM IR.
        let (layout, ctrl_offset) = match table_layout.calculate_layout_for(buckets) {
            Some(lco) => lco,
            None => return Err(fallibility.capacity_overflow()),
        };

        let ptr: NonNull<u8> = match do_alloc(alloc, layout) {
            Ok(block) => block.cast(),
            Err(_) => return Err(fallibility.alloc_err(layout)),
        };

        // SAFETY: null pointer will be caught in above check
        let ctrl = NonNull::new_unchecked(ptr.as_ptr().add(ctrl_offset));
        Ok(Self {
            ctrl,
            bucket_mask: buckets - 1,
            items: 0,
            growth_left: bucket_mask_to_capacity(buckets - 1),
        })
    }

    /// Attempts to allocate a new [`RawTableInner`] with at least enough
    /// capacity for inserting the given number of elements without reallocating.
    ///
    /// All the control bytes are initialized with the [`Tag::EMPTY`] bytes.
    #[inline]
    fn fallible_with_capacity<A>(
        alloc: &A,
        table_layout: TableLayout,
        capacity: usize,
        fallibility: Fallibility,
    ) -> Result<Self, TryReserveError>
    where
        A: Allocator,
    {
        if capacity == 0 {
            Ok(Self::NEW)
        } else {
            // SAFETY: We checked that we could successfully allocate the new table, and then
            // initialized all control bytes with the constant `Tag::EMPTY` byte.
            unsafe {
                let buckets = capacity_to_buckets(capacity, table_layout)
                    .ok_or_else(|| fallibility.capacity_overflow())?;

                let mut result =
                    Self::new_uninitialized(alloc, table_layout, buckets, fallibility)?;
                // SAFETY: We checked that the table is allocated and therefore the table already has
                // `self.bucket_mask + 1 + Group::WIDTH` number of control bytes (see TableLayout::calculate_layout_for)
                // so writing `self.num_ctrl_bytes() == bucket_mask + 1 + Group::WIDTH` bytes is safe.
                result.ctrl_slice().fill_empty();

                Ok(result)
            }
        }
    }

    /// Allocates a new [`RawTableInner`] with at least enough capacity for inserting
    /// the given number of elements without reallocating.
    ///
    /// Panics if the new capacity exceeds [`isize::MAX`] bytes and [`abort`] the program
    /// in case of allocation error. Use [`fallible_with_capacity`] instead if you want to
    /// handle memory allocation failure.
    ///
    /// All the control bytes are initialized with the [`Tag::EMPTY`] bytes.
    ///
    /// [`fallible_with_capacity`]: RawTableInner::fallible_with_capacity
    /// [`abort`]: https://doc.rust-lang.org/alloc/alloc/fn.handle_alloc_error.html
    fn with_capacity<A>(alloc: &A, table_layout: TableLayout, capacity: usize) -> Self
    where
        A: Allocator,
    {
        // Avoid `Result::unwrap_or_else` because it bloats LLVM IR.
        match Self::fallible_with_capacity(alloc, table_layout, capacity, Fallibility::Infallible) {
            Ok(table_inner) => table_inner,
            // SAFETY: All allocation errors will be caught inside `RawTableInner::new_uninitialized`.
            Err(_) => unsafe { hint::unreachable_unchecked() },
        }
    }

    /// Fixes up an insertion slot returned by the [`RawTableInner::find_insert_slot_in_group`] method.
    ///
    /// In tables smaller than the group width (`self.buckets() < Group::WIDTH`), trailing control
    /// bytes outside the range of the table are filled with [`Tag::EMPTY`] entries. These will unfortunately
    /// trigger a match of [`RawTableInner::find_insert_slot_in_group`] function. This is because
    /// the `Some(bit)` returned by `group.match_empty_or_deleted().lowest_set_bit()` after masking
    /// (`(probe_seq.pos + bit) & self.bucket_mask`) may point to a full bucket that is already occupied.
    /// We detect this situation here and perform a second scan starting at the beginning of the table.
    /// This second scan is guaranteed to find an empty slot (due to the load factor) before hitting the
    /// trailing control bytes (containing [`Tag::EMPTY`] bytes).
    ///
    /// If this function is called correctly, it is guaranteed to return [`InsertSlot`] with an
    /// index of an empty or deleted bucket in the range `0..self.buckets()` (see `Warning` and
    /// `Safety`).
    ///
    /// # Warning
    ///
    /// The table must have at least 1 empty or deleted `bucket`, otherwise if the table is less than
    /// the group width (`self.buckets() < Group::WIDTH`) this function returns an index outside of the
    /// table indices range `0..self.buckets()` (`0..=self.bucket_mask`). Attempt to write data at that
    /// index will cause immediate [`undefined behavior`].
    ///
    /// # Safety
    ///
    /// The safety rules are directly derived from the safety rules for [`RawTableInner::ctrl`] method.
    /// Thus, in order to uphold those safety contracts, as well as for the correct logic of the work
    /// of this crate, the following rules are necessary and sufficient:
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes otherwise calling this
    ///   function results in [`undefined behavior`].
    ///
    /// * This function must only be used on insertion slots found by [`RawTableInner::find_insert_slot_in_group`]
    ///   (after the `find_insert_slot_in_group` function, but before insertion into the table).
    ///
    /// * The `index` must not be greater than the `self.bucket_mask`, i.e. `(index + 1) <= self.buckets()`
    ///   (this one is provided by the [`RawTableInner::find_insert_slot_in_group`] function).
    ///
    /// Calling this function with an index not provided by [`RawTableInner::find_insert_slot_in_group`]
    /// may result in [`undefined behavior`] even if the index satisfies the safety rules of the
    /// [`RawTableInner::ctrl`] function (`index < self.bucket_mask + 1 + Group::WIDTH`).
    ///
    /// [`RawTableInner::ctrl`]: RawTableInner::ctrl
    /// [`RawTableInner::find_insert_slot_in_group`]: RawTableInner::find_insert_slot_in_group
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn fix_insert_slot(&self, mut index: usize) -> InsertSlot {
        // SAFETY: The caller of this function ensures that `index` is in the range `0..=self.bucket_mask`.
        if unlikely(self.is_bucket_full(index)) {
            debug_assert!(self.bucket_mask < Group::WIDTH);
            // SAFETY:
            //
            // * Since the caller of this function ensures that the control bytes are properly
            //   initialized and `ptr = self.ctrl(0)` points to the start of the array of control
            //   bytes, therefore: `ctrl` is valid for reads, properly aligned to `Group::WIDTH`
            //   and points to the properly initialized control bytes (see also
            //   `TableLayout::calculate_layout_for` and `ptr::read`);
            //
            // * Because the caller of this function ensures that the index was provided by the
            //   `self.find_insert_slot_in_group()` function, so for for tables larger than the
            //   group width (self.buckets() >= Group::WIDTH), we will never end up in the given
            //   branch, since `(probe_seq.pos + bit) & self.bucket_mask` in `find_insert_slot_in_group`
            //   cannot return a full bucket index. For tables smaller than the group width, calling
            //   the `unwrap_unchecked` function is also safe, as the trailing control bytes outside
            //   the range of the table are filled with EMPTY bytes (and we know for sure that there
            //   is at least one FULL bucket), so this second scan either finds an empty slot (due to
            //   the load factor) or hits the trailing control bytes (containing EMPTY).
            index = Group::load_aligned(self.ctrl(0))
                .match_empty_or_deleted()
                .lowest_set_bit()
                .unwrap_unchecked();
        }
        InsertSlot { index }
    }

    /// Finds the position to insert something in a group.
    ///
    /// **This may have false positives and must be fixed up with `fix_insert_slot`
    /// before it's used.**
    ///
    /// The function is guaranteed to return the index of an empty or deleted [`Bucket`]
    /// in the range `0..self.buckets()` (`0..=self.bucket_mask`).
    #[inline]
    fn find_insert_slot_in_group(&self, group: &Group, probe_seq: &ProbeSeq) -> Option<usize> {
        let bit = group.match_empty_or_deleted().lowest_set_bit();

        if likely(bit.is_some()) {
            // This is the same as `(probe_seq.pos + bit) % self.buckets()` because the number
            // of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
            Some((probe_seq.pos + bit.unwrap()) & self.bucket_mask)
        } else {
            None
        }
    }

    /// Searches for an element in the table, or a potential slot where that element could
    /// be inserted (an empty or deleted [`Bucket`] index).
    ///
    /// This uses dynamic dispatch to reduce the amount of code generated, but that is
    /// eliminated by LLVM optimizations.
    ///
    /// This function does not make any changes to the `data` part of the table, or any
    /// changes to the `items` or `growth_left` field of the table.
    ///
    /// The table must have at least 1 empty or deleted `bucket`, otherwise, if the
    /// `eq: &mut dyn FnMut(usize) -> bool` function does not return `true`, this function
    /// will never return (will go into an infinite loop) for tables larger than the group
    /// width, or return an index outside of the table indices range if the table is less
    /// than the group width.
    ///
    /// This function is guaranteed to provide the `eq: &mut dyn FnMut(usize) -> bool`
    /// function with only `FULL` buckets' indices and return the `index` of the found
    /// element (as `Ok(index)`). If the element is not found and there is at least 1
    /// empty or deleted [`Bucket`] in the table, the function is guaranteed to return
    /// [`InsertSlot`] with an index in the range `0..self.buckets()`, but in any case,
    /// if this function returns [`InsertSlot`], it will contain an index in the range
    /// `0..=self.buckets()`.
    ///
    /// # Safety
    ///
    /// The [`RawTableInner`] must have properly initialized control bytes otherwise calling
    /// this function results in [`undefined behavior`].
    ///
    /// Attempt to write data at the [`InsertSlot`] returned by this function when the table is
    /// less than the group width and if there was not at least one empty or deleted bucket in
    /// the table will cause immediate [`undefined behavior`]. This is because in this case the
    /// function will return `self.bucket_mask + 1` as an index due to the trailing [`Tag::EMPTY`]
    /// control bytes outside the table range.
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn find_or_find_insert_slot_inner(
        &self,
        hash: u64,
        eq: &mut dyn FnMut(usize) -> bool,
    ) -> Result<usize, InsertSlot> {
        let mut insert_slot = None;

        let tag_hash = Tag::full(hash);
        let mut probe_seq = self.probe_seq(hash);

        loop {
            // SAFETY:
            // * Caller of this function ensures that the control bytes are properly initialized.
            //
            // * `ProbeSeq.pos` cannot be greater than `self.bucket_mask = self.buckets() - 1`
            //   of the table due to masking with `self.bucket_mask` and also because the number
            //   of buckets is a power of two (see `self.probe_seq` function).
            //
            // * Even if `ProbeSeq.pos` returns `position == self.bucket_mask`, it is safe to
            //   call `Group::load` due to the extended control bytes range, which is
            //  `self.bucket_mask + 1 + Group::WIDTH` (in fact, this means that the last control
            //   byte will never be read for the allocated table);
            //
            // * Also, even if `RawTableInner` is not already allocated, `ProbeSeq.pos` will
            //   always return "0" (zero), so Group::load will read unaligned `Group::static_empty()`
            //   bytes, which is safe (see RawTableInner::new).
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            for bit in group.match_tag(tag_hash) {
                let index = (probe_seq.pos + bit) & self.bucket_mask;

                if likely(eq(index)) {
                    return Ok(index);
                }
            }

            // We didn't find the element we were looking for in the group, try to get an
            // insertion slot from the group if we don't have one yet.
            if likely(insert_slot.is_none()) {
                insert_slot = self.find_insert_slot_in_group(&group, &probe_seq);
            }

            if let Some(insert_slot) = insert_slot {
                // Only stop the search if the group contains at least one empty element.
                // Otherwise, the element that we are looking for might be in a following group.
                if likely(group.match_empty().any_bit_set()) {
                    // We must have found a insert slot by now, since the current group contains at
                    // least one. For tables smaller than the group width, there will still be an
                    // empty element in the current (and only) group due to the load factor.
                    unsafe {
                        // SAFETY:
                        // * Caller of this function ensures that the control bytes are properly initialized.
                        //
                        // * We use this function with the slot / index found by `self.find_insert_slot_in_group`
                        return Err(self.fix_insert_slot(insert_slot));
                    }
                }
            }

            probe_seq.move_next(self.bucket_mask);
        }
    }

    /// Searches for an empty or deleted bucket which is suitable for inserting a new
    /// element and sets the hash for that slot. Returns an index of that slot and the
    /// old control byte stored in the found index.
    ///
    /// This function does not check if the given element exists in the table. Also,
    /// this function does not check if there is enough space in the table to insert
    /// a new element. The caller of the function must make sure that the table has at
    /// least 1 empty or deleted `bucket`, otherwise this function will never return
    /// (will go into an infinite loop) for tables larger than the group width, or
    /// return an index outside of the table indices range if the table is less than
    /// the group width.
    ///
    /// If there is at least 1 empty or deleted `bucket` in the table, the function is
    /// guaranteed to return an `index` in the range `0..self.buckets()`, but in any case,
    /// if this function returns an `index` it will be in the range `0..=self.buckets()`.
    ///
    /// This function does not make any changes to the `data` parts of the table,
    /// or any changes to the `items` or `growth_left` field of the table.
    ///
    /// # Safety
    ///
    /// The safety rules are directly derived from the safety rules for the
    /// [`RawTableInner::set_ctrl_hash`] and [`RawTableInner::find_insert_slot`] methods.
    /// Thus, in order to uphold the safety contracts for that methods, as well as for
    /// the correct logic of the work of this crate, you must observe the following rules
    /// when calling this function:
    ///
    /// * The [`RawTableInner`] has already been allocated and has properly initialized
    ///   control bytes otherwise calling this function results in [`undefined behavior`].
    ///
    /// * The caller of this function must ensure that the "data" parts of the table
    ///   will have an entry in the returned index (matching the given hash) right
    ///   after calling this function.
    ///
    /// Attempt to write data at the `index` returned by this function when the table is
    /// less than the group width and if there was not at least one empty or deleted bucket in
    /// the table will cause immediate [`undefined behavior`]. This is because in this case the
    /// function will return `self.bucket_mask + 1` as an index due to the trailing [`Tag::EMPTY`]
    /// control bytes outside the table range.
    ///
    /// The caller must independently increase the `items` field of the table, and also,
    /// if the old control byte was [`Tag::EMPTY`], then decrease the table's `growth_left`
    /// field, and do not change it if the old control byte was [`Tag::DELETED`].
    ///
    /// See also [`Bucket::as_ptr`] method, for more information about of properly removing
    /// or saving `element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    /// [`RawTableInner::ctrl`]: RawTableInner::ctrl
    /// [`RawTableInner::set_ctrl_hash`]: RawTableInner::set_ctrl_hash
    /// [`RawTableInner::find_insert_slot`]: RawTableInner::find_insert_slot
    #[inline]
    unsafe fn prepare_insert_slot(&mut self, hash: u64) -> (usize, Tag) {
        // SAFETY: Caller of this function ensures that the control bytes are properly initialized.
        let index: usize = self.find_insert_slot(hash).index;
        // SAFETY:
        // 1. The `find_insert_slot` function either returns an `index` less than or
        //    equal to `self.buckets() = self.bucket_mask + 1` of the table, or never
        //    returns if it cannot find an empty or deleted slot.
        // 2. The caller of this function guarantees that the table has already been
        //    allocated
        let old_ctrl = *self.ctrl(index);
        self.set_ctrl_hash(index, hash);
        (index, old_ctrl)
    }

    /// Searches for an empty or deleted bucket which is suitable for inserting
    /// a new element, returning the `index` for the new [`Bucket`].
    ///
    /// This function does not make any changes to the `data` part of the table, or any
    /// changes to the `items` or `growth_left` field of the table.
    ///
    /// The table must have at least 1 empty or deleted `bucket`, otherwise this function
    /// will never return (will go into an infinite loop) for tables larger than the group
    /// width, or return an index outside of the table indices range if the table is less
    /// than the group width.
    ///
    /// If there is at least 1 empty or deleted `bucket` in the table, the function is
    /// guaranteed to return [`InsertSlot`] with an index in the range `0..self.buckets()`,
    /// but in any case, if this function returns [`InsertSlot`], it will contain an index
    /// in the range `0..=self.buckets()`.
    ///
    /// # Safety
    ///
    /// The [`RawTableInner`] must have properly initialized control bytes otherwise calling
    /// this function results in [`undefined behavior`].
    ///
    /// Attempt to write data at the [`InsertSlot`] returned by this function when the table is
    /// less than the group width and if there was not at least one empty or deleted bucket in
    /// the table will cause immediate [`undefined behavior`]. This is because in this case the
    /// function will return `self.bucket_mask + 1` as an index due to the trailing [`Tag::EMPTY`]
    /// control bytes outside the table range.
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn find_insert_slot(&self, hash: u64) -> InsertSlot {
        let mut probe_seq = self.probe_seq(hash);
        loop {
            // SAFETY:
            // * Caller of this function ensures that the control bytes are properly initialized.
            //
            // * `ProbeSeq.pos` cannot be greater than `self.bucket_mask = self.buckets() - 1`
            //   of the table due to masking with `self.bucket_mask` and also because the number
            //   of buckets is a power of two (see `self.probe_seq` function).
            //
            // * Even if `ProbeSeq.pos` returns `position == self.bucket_mask`, it is safe to
            //   call `Group::load` due to the extended control bytes range, which is
            //  `self.bucket_mask + 1 + Group::WIDTH` (in fact, this means that the last control
            //   byte will never be read for the allocated table);
            //
            // * Also, even if `RawTableInner` is not already allocated, `ProbeSeq.pos` will
            //   always return "0" (zero), so Group::load will read unaligned `Group::static_empty()`
            //   bytes, which is safe (see RawTableInner::new).
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            let index = self.find_insert_slot_in_group(&group, &probe_seq);
            if likely(index.is_some()) {
                // SAFETY:
                // * Caller of this function ensures that the control bytes are properly initialized.
                //
                // * We use this function with the slot / index found by `self.find_insert_slot_in_group`
                unsafe {
                    return self.fix_insert_slot(index.unwrap_unchecked());
                }
            }
            probe_seq.move_next(self.bucket_mask);
        }
    }

    /// Searches for an element in a table, returning the `index` of the found element.
    /// This uses dynamic dispatch to reduce the amount of code generated, but it is
    /// eliminated by LLVM optimizations.
    ///
    /// This function does not make any changes to the `data` part of the table, or any
    /// changes to the `items` or `growth_left` field of the table.
    ///
    /// The table must have at least 1 empty `bucket`, otherwise, if the
    /// `eq: &mut dyn FnMut(usize) -> bool` function does not return `true`,
    /// this function will also never return (will go into an infinite loop).
    ///
    /// This function is guaranteed to provide the `eq: &mut dyn FnMut(usize) -> bool`
    /// function with only `FULL` buckets' indices and return the `index` of the found
    /// element as `Some(index)`, so the index will always be in the range
    /// `0..self.buckets()`.
    ///
    /// # Safety
    ///
    /// The [`RawTableInner`] must have properly initialized control bytes otherwise calling
    /// this function results in [`undefined behavior`].
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline(always)]
    unsafe fn find_inner(&self, hash: u64, eq: &mut dyn FnMut(usize) -> bool) -> Option<usize> {
        let tag_hash = Tag::full(hash);
        let mut probe_seq = self.probe_seq(hash);

        loop {
            // SAFETY:
            // * Caller of this function ensures that the control bytes are properly initialized.
            //
            // * `ProbeSeq.pos` cannot be greater than `self.bucket_mask = self.buckets() - 1`
            //   of the table due to masking with `self.bucket_mask`.
            //
            // * Even if `ProbeSeq.pos` returns `position == self.bucket_mask`, it is safe to
            //   call `Group::load` due to the extended control bytes range, which is
            //  `self.bucket_mask + 1 + Group::WIDTH` (in fact, this means that the last control
            //   byte will never be read for the allocated table);
            //
            // * Also, even if `RawTableInner` is not already allocated, `ProbeSeq.pos` will
            //   always return "0" (zero), so Group::load will read unaligned `Group::static_empty()`
            //   bytes, which is safe (see RawTableInner::new_in).
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            for bit in group.match_tag(tag_hash) {
                // This is the same as `(probe_seq.pos + bit) % self.buckets()` because the number
                // of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
                let index = (probe_seq.pos + bit) & self.bucket_mask;

                if likely(eq(index)) {
                    return Some(index);
                }
            }

            if likely(group.match_empty().any_bit_set()) {
                return None;
            }

            probe_seq.move_next(self.bucket_mask);
        }
    }

    /// Prepares for rehashing data in place (that is, without allocating new memory).
    /// Converts all full index `control bytes` to `Tag::DELETED` and all `Tag::DELETED` control
    /// bytes to `Tag::EMPTY`, i.e. performs the following conversion:
    ///
    /// - `Tag::EMPTY` control bytes   -> `Tag::EMPTY`;
    /// - `Tag::DELETED` control bytes -> `Tag::EMPTY`;
    /// - `FULL` control bytes    -> `Tag::DELETED`.
    ///
    /// This function does not make any changes to the `data` parts of the table,
    /// or any changes to the `items` or `growth_left` field of the table.
    ///
    /// # Safety
    ///
    /// You must observe the following safety rules when calling this function:
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * The caller of this function must convert the `Tag::DELETED` bytes back to `FULL`
    ///   bytes when re-inserting them into their ideal position (which was impossible
    ///   to do during the first insert due to tombstones). If the caller does not do
    ///   this, then calling this function may result in a memory leak.
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes otherwise
    ///   calling this function results in [`undefined behavior`].
    ///
    /// Calling this function on a table that has not been allocated results in
    /// [`undefined behavior`].
    ///
    /// See also [`Bucket::as_ptr`] method, for more information about of properly removing
    /// or saving `data element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[allow(clippy::mut_mut)]
    #[inline]
    unsafe fn prepare_rehash_in_place(&mut self) {
        // Bulk convert all full control bytes to DELETED, and all DELETED control bytes to EMPTY.
        // This effectively frees up all buckets containing a DELETED entry.
        //
        // SAFETY:
        // 1. `i` is guaranteed to be within bounds since we are iterating from zero to `buckets - 1`;
        // 2. Even if `i` will be `i == self.bucket_mask`, it is safe to call `Group::load_aligned`
        //    due to the extended control bytes range, which is `self.bucket_mask + 1 + Group::WIDTH`;
        // 3. The caller of this function guarantees that [`RawTableInner`] has already been allocated;
        // 4. We can use `Group::load_aligned` and `Group::store_aligned` here since we start from 0
        //    and go to the end with a step equal to `Group::WIDTH` (see TableLayout::calculate_layout_for).
        for i in (0..self.buckets()).step_by(Group::WIDTH) {
            let group = Group::load_aligned(self.ctrl(i));
            let group = group.convert_special_to_empty_and_full_to_deleted();
            group.store_aligned(self.ctrl(i));
        }

        // Fix up the trailing control bytes. See the comments in set_ctrl
        // for the handling of tables smaller than the group width.
        //
        // SAFETY: The caller of this function guarantees that [`RawTableInner`]
        // has already been allocated
        if unlikely(self.buckets() < Group::WIDTH) {
            // SAFETY: We have `self.bucket_mask + 1 + Group::WIDTH` number of control bytes,
            // so copying `self.buckets() == self.bucket_mask + 1` bytes with offset equal to
            // `Group::WIDTH` is safe
            self.ctrl(0)
                .copy_to(self.ctrl(Group::WIDTH), self.buckets());
        } else {
            // SAFETY: We have `self.bucket_mask + 1 + Group::WIDTH` number of
            // control bytes,so copying `Group::WIDTH` bytes with offset equal
            // to `self.buckets() == self.bucket_mask + 1` is safe
            self.ctrl(0)
                .copy_to(self.ctrl(self.buckets()), Group::WIDTH);
        }
    }

    /// Returns an iterator over every element in the table.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result
    /// is [`undefined behavior`]:
    ///
    /// * The caller has to ensure that the `RawTableInner` outlives the
    ///   `RawIter`. Because we cannot make the `next` method unsafe on
    ///   the `RawIter` struct, we have to make the `iter` method unsafe.
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes.
    ///
    /// The type `T` must be the actual type of the elements stored in the table,
    /// otherwise using the returned [`RawIter`] results in [`undefined behavior`].
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn iter<T>(&self) -> RawIter<T> {
        // SAFETY:
        // 1. Since the caller of this function ensures that the control bytes
        //    are properly initialized and `self.data_end()` points to the start
        //    of the array of control bytes, therefore: `ctrl` is valid for reads,
        //    properly aligned to `Group::WIDTH` and points to the properly initialized
        //    control bytes.
        // 2. `data` bucket index in the table is equal to the `ctrl` index (i.e.
        //    equal to zero).
        // 3. We pass the exact value of buckets of the table to the function.
        //
        //                         `ctrl` points here (to the start
        //                         of the first control byte `CT0`)
        //                          ∨
        // [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, CTa_1, ..., CTa_m
        //                           \________  ________/
        //                                    \/
        //       `n = buckets - 1`, i.e. `RawTableInner::buckets() - 1`
        //
        // where: T0...T_n  - our stored data;
        //        CT0...CT_n - control bytes or metadata for `data`.
        //        CTa_0...CTa_m - additional control bytes, where `m = Group::WIDTH - 1` (so that the search
        //                        with loading `Group` bytes from the heap works properly, even if the result
        //                        of `h1(hash) & self.bucket_mask` is equal to `self.bucket_mask`). See also
        //                        `RawTableInner::set_ctrl` function.
        //
        // P.S. `h1(hash) & self.bucket_mask` is the same as `hash as usize % self.buckets()` because the number
        // of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
        let data = Bucket::from_base_index(self.data_end(), 0);
        RawIter {
            // SAFETY: See explanation above
            iter: RawIterRange::new(self.ctrl.as_ptr(), data, self.buckets()),
            items: self.items,
        }
    }

    /// Executes the destructors (if any) of the values stored in the table.
    ///
    /// # Note
    ///
    /// This function does not erase the control bytes of the table and does
    /// not make any changes to the `items` or `growth_left` fields of the
    /// table. If necessary, the caller of this function must manually set
    /// up these table fields, for example using the [`clear_no_drop`] function.
    ///
    /// Be careful during calling this function, because drop function of
    /// the elements can panic, and this can leave table in an inconsistent
    /// state.
    ///
    /// # Safety
    ///
    /// The type `T` must be the actual type of the elements stored in the table,
    /// otherwise calling this function may result in [`undefined behavior`].
    ///
    /// If `T` is a type that should be dropped and **the table is not empty**,
    /// calling this function more than once results in [`undefined behavior`].
    ///
    /// If `T` is not [`Copy`], attempting to use values stored in the table after
    /// calling this function may result in [`undefined behavior`].
    ///
    /// It is safe to call this function on a table that has not been allocated,
    /// on a table with uninitialized control bytes, and on a table with no actual
    /// data but with `Full` control bytes if `self.items == 0`.
    ///
    /// See also [`Bucket::drop`] / [`Bucket::as_ptr`] methods, for more information
    /// about of properly removing or saving `element` from / into the [`RawTable`] /
    /// [`RawTableInner`].
    ///
    /// [`Bucket::drop`]: Bucket::drop
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`clear_no_drop`]: RawTableInner::clear_no_drop
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    unsafe fn drop_elements<T>(&mut self) {
        // Check that `self.items != 0`. Protects against the possibility
        // of creating an iterator on an table with uninitialized control bytes.
        if T::NEEDS_DROP && self.items != 0 {
            // SAFETY: We know for sure that RawTableInner will outlive the
            // returned `RawIter` iterator, and the caller of this function
            // must uphold the safety contract for `drop_elements` method.
            for item in self.iter::<T>() {
                // SAFETY: The caller must uphold the safety contract for
                // `drop_elements` method.
                item.drop();
            }
        }
    }

    /// Executes the destructors (if any) of the values stored in the table and than
    /// deallocates the table.
    ///
    /// # Note
    ///
    /// Calling this function automatically makes invalid (dangling) all instances of
    /// buckets ([`Bucket`]) and makes invalid (dangling) the `ctrl` field of the table.
    ///
    /// This function does not make any changes to the `bucket_mask`, `items` or `growth_left`
    /// fields of the table. If necessary, the caller of this function must manually set
    /// up these table fields.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is [`undefined behavior`]:
    ///
    /// * Calling this function more than once;
    ///
    /// * The type `T` must be the actual type of the elements stored in the table.
    ///
    /// * The `alloc` must be the same [`Allocator`] as the `Allocator` that was used
    ///   to allocate this table.
    ///
    /// * The `table_layout` must be the same [`TableLayout`] as the `TableLayout` that
    ///   was used to allocate this table.
    ///
    /// The caller of this function should pay attention to the possibility of the
    /// elements' drop function panicking, because this:
    ///
    ///    * May leave the table in an inconsistent state;
    ///
    ///    * Memory is never deallocated, so a memory leak may occur.
    ///
    /// Attempt to use the `ctrl` field of the table (dereference) after calling this
    /// function results in [`undefined behavior`].
    ///
    /// It is safe to call this function on a table that has not been allocated,
    /// on a table with uninitialized control bytes, and on a table with no actual
    /// data but with `Full` control bytes if `self.items == 0`.
    ///
    /// See also [`RawTableInner::drop_elements`] or [`RawTableInner::free_buckets`]
    /// for more  information.
    ///
    /// [`RawTableInner::drop_elements`]: RawTableInner::drop_elements
    /// [`RawTableInner::free_buckets`]: RawTableInner::free_buckets
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    unsafe fn drop_inner_table<T, A: Allocator>(&mut self, alloc: &A, table_layout: TableLayout) {
        if !self.is_empty_singleton() {
            unsafe {
                // SAFETY: The caller must uphold the safety contract for `drop_inner_table` method.
                self.drop_elements::<T>();
                // SAFETY:
                // 1. We have checked that our table is allocated.
                // 2. The caller must uphold the safety contract for `drop_inner_table` method.
                self.free_buckets(alloc, table_layout);
            }
        }
    }

    /// Returns a pointer to an element in the table (convenience for
    /// `Bucket::from_base_index(self.data_end::<T>(), index)`).
    ///
    /// The caller must ensure that the `RawTableInner` outlives the returned [`Bucket<T>`],
    /// otherwise using it may result in [`undefined behavior`].
    ///
    /// # Safety
    ///
    /// If `mem::size_of::<T>() != 0`, then the safety rules are directly derived from the
    /// safety rules of the [`Bucket::from_base_index`] function. Therefore, when calling
    /// this function, the following safety rules must be observed:
    ///
    /// * The table must already be allocated;
    ///
    /// * The `index` must not be greater than the number returned by the [`RawTableInner::buckets`]
    ///   function, i.e. `(index + 1) <= self.buckets()`.
    ///
    /// * The type `T` must be the actual type of the elements stored in the table, otherwise
    ///   using the returned [`Bucket`] may result in [`undefined behavior`].
    ///
    /// It is safe to call this function with index of zero (`index == 0`) on a table that has
    /// not been allocated, but using the returned [`Bucket`] results in [`undefined behavior`].
    ///
    /// If `mem::size_of::<T>() == 0`, then the only requirement is that the `index` must
    /// not be greater than the number returned by the [`RawTable::buckets`] function, i.e.
    /// `(index + 1) <= self.buckets()`.
    ///
    /// ```none
    /// If mem::size_of::<T>() != 0 then return a pointer to the `element` in the `data part` of the table
    /// (we start counting from "0", so that in the expression T[n], the "n" index actually one less than
    /// the "buckets" number of our `RawTableInner`, i.e. "n = RawTableInner::buckets() - 1"):
    ///
    ///           `table.bucket(3).as_ptr()` returns a pointer that points here in the `data`
    ///           part of the `RawTableInner`, i.e. to the start of T3 (see [`Bucket::as_ptr`])
    ///                  |
    ///                  |               `base = table.data_end::<T>()` points here
    ///                  |               (to the start of CT0 or to the end of T0)
    ///                  v                 v
    /// [Pad], T_n, ..., |T3|, T2, T1, T0, |CT0, CT1, CT2, CT3, ..., CT_n, CTa_0, CTa_1, ..., CTa_m
    ///                     ^                                              \__________  __________/
    ///        `table.bucket(3)` returns a pointer that points                        \/
    ///         here in the `data` part of the `RawTableInner`             additional control bytes
    ///         (to the end of T3)                                          `m = Group::WIDTH - 1`
    ///
    /// where: T0...T_n  - our stored data;
    ///        CT0...CT_n - control bytes or metadata for `data`;
    ///        CTa_0...CTa_m - additional control bytes (so that the search with loading `Group` bytes from
    ///                        the heap works properly, even if the result of `h1(hash) & self.bucket_mask`
    ///                        is equal to `self.bucket_mask`). See also `RawTableInner::set_ctrl` function.
    ///
    /// P.S. `h1(hash) & self.bucket_mask` is the same as `hash as usize % self.buckets()` because the number
    /// of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
    /// ```
    ///
    /// [`Bucket::from_base_index`]: Bucket::from_base_index
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn bucket<T>(&self, index: usize) -> Bucket<T> {
        debug_assert_ne!(self.bucket_mask, 0);
        debug_assert!(index < self.buckets());
        Bucket::from_base_index(self.data_end(), index)
    }

    /// Returns a raw `*mut u8` pointer to the start of the `data` element in the table
    /// (convenience for `self.data_end::<u8>().as_ptr().sub((index + 1) * size_of)`).
    ///
    /// The caller must ensure that the `RawTableInner` outlives the returned `*mut u8`,
    /// otherwise using it may result in [`undefined behavior`].
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is [`undefined behavior`]:
    ///
    /// * The table must already be allocated;
    ///
    /// * The `index` must not be greater than the number returned by the [`RawTableInner::buckets`]
    ///   function, i.e. `(index + 1) <= self.buckets()`;
    ///
    /// * The `size_of` must be equal to the size of the elements stored in the table;
    ///
    /// ```none
    /// If mem::size_of::<T>() != 0 then return a pointer to the `element` in the `data part` of the table
    /// (we start counting from "0", so that in the expression T[n], the "n" index actually one less than
    /// the "buckets" number of our `RawTableInner`, i.e. "n = RawTableInner::buckets() - 1"):
    ///
    ///           `table.bucket_ptr(3, mem::size_of::<T>())` returns a pointer that points here in the
    ///           `data` part of the `RawTableInner`, i.e. to the start of T3
    ///                  |
    ///                  |               `base = table.data_end::<u8>()` points here
    ///                  |               (to the start of CT0 or to the end of T0)
    ///                  v                 v
    /// [Pad], T_n, ..., |T3|, T2, T1, T0, |CT0, CT1, CT2, CT3, ..., CT_n, CTa_0, CTa_1, ..., CTa_m
    ///                                                                    \__________  __________/
    ///                                                                               \/
    ///                                                                    additional control bytes
    ///                                                                     `m = Group::WIDTH - 1`
    ///
    /// where: T0...T_n  - our stored data;
    ///        CT0...CT_n - control bytes or metadata for `data`;
    ///        CTa_0...CTa_m - additional control bytes (so that the search with loading `Group` bytes from
    ///                        the heap works properly, even if the result of `h1(hash) & self.bucket_mask`
    ///                        is equal to `self.bucket_mask`). See also `RawTableInner::set_ctrl` function.
    ///
    /// P.S. `h1(hash) & self.bucket_mask` is the same as `hash as usize % self.buckets()` because the number
    /// of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
    /// ```
    ///
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn bucket_ptr(&self, index: usize, size_of: usize) -> *mut u8 {
        debug_assert_ne!(self.bucket_mask, 0);
        debug_assert!(index < self.buckets());
        let base: *mut u8 = self.data_end().as_ptr();
        base.sub((index + 1) * size_of)
    }

    /// Returns pointer to one past last `data` element in the table as viewed from
    /// the start point of the allocation (convenience for `self.ctrl.cast()`).
    ///
    /// This function actually returns a pointer to the end of the `data element` at
    /// index "0" (zero).
    ///
    /// The caller must ensure that the `RawTableInner` outlives the returned [`NonNull<T>`],
    /// otherwise using it may result in [`undefined behavior`].
    ///
    /// # Note
    ///
    /// The type `T` must be the actual type of the elements stored in the table, otherwise
    /// using the returned [`NonNull<T>`] may result in [`undefined behavior`].
    ///
    /// ```none
    ///                        `table.data_end::<T>()` returns pointer that points here
    ///                        (to the end of `T0`)
    ///                          ∨
    /// [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, CTa_0, CTa_1, ..., CTa_m
    ///                           \________  ________/
    ///                                    \/
    ///       `n = buckets - 1`, i.e. `RawTableInner::buckets() - 1`
    ///
    /// where: T0...T_n  - our stored data;
    ///        CT0...CT_n - control bytes or metadata for `data`.
    ///        CTa_0...CTa_m - additional control bytes, where `m = Group::WIDTH - 1` (so that the search
    ///                        with loading `Group` bytes from the heap works properly, even if the result
    ///                        of `h1(hash) & self.bucket_mask` is equal to `self.bucket_mask`). See also
    ///                        `RawTableInner::set_ctrl` function.
    ///
    /// P.S. `h1(hash) & self.bucket_mask` is the same as `hash as usize % self.buckets()` because the number
    /// of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
    /// ```
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    fn data_end<T>(&self) -> NonNull<T> {
        self.ctrl.cast()
    }

    /// Returns an iterator-like object for a probe sequence on the table.
    ///
    /// This iterator never terminates, but is guaranteed to visit each bucket
    /// group exactly once. The loop using `probe_seq` must terminate upon
    /// reaching a group containing an empty bucket.
    #[inline]
    fn probe_seq(&self, hash: u64) -> ProbeSeq {
        ProbeSeq {
            // This is the same as `hash as usize % self.buckets()` because the number
            // of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
            pos: h1(hash) & self.bucket_mask,
            stride: 0,
        }
    }

    #[inline]
    unsafe fn record_item_insert_at(&mut self, index: usize, old_ctrl: Tag, hash: u64) {
        self.growth_left -= usize::from(old_ctrl.special_is_empty());
        self.set_ctrl_hash(index, hash);
        self.items += 1;
    }

    #[inline]
    fn is_in_same_group(&self, i: usize, new_i: usize, hash: u64) -> bool {
        let probe_seq_pos = self.probe_seq(hash).pos;
        let probe_index =
            |pos: usize| (pos.wrapping_sub(probe_seq_pos) & self.bucket_mask) / Group::WIDTH;
        probe_index(i) == probe_index(new_i)
    }

    /// Sets a control byte to the hash, and possibly also the replicated control byte at
    /// the end of the array.
    ///
    /// This function does not make any changes to the `data` parts of the table,
    /// or any changes to the `items` or `growth_left` field of the table.
    ///
    /// # Safety
    ///
    /// The safety rules are directly derived from the safety rules for [`RawTableInner::set_ctrl`]
    /// method. Thus, in order to uphold the safety contracts for the method, you must observe the
    /// following rules when calling this function:
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * The `index` must not be greater than the `RawTableInner.bucket_mask`, i.e.
    ///   `index <= RawTableInner.bucket_mask` or, in other words, `(index + 1)` must
    ///   be no greater than the number returned by the function [`RawTableInner::buckets`].
    ///
    /// Calling this function on a table that has not been allocated results in [`undefined behavior`].
    ///
    /// See also [`Bucket::as_ptr`] method, for more information about of properly removing
    /// or saving `data element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`RawTableInner::set_ctrl`]: RawTableInner::set_ctrl
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn set_ctrl_hash(&mut self, index: usize, hash: u64) {
        // SAFETY: The caller must uphold the safety rules for the [`RawTableInner::set_ctrl_hash`]
        self.set_ctrl(index, Tag::full(hash));
    }

    /// Replaces the hash in the control byte at the given index with the provided one,
    /// and possibly also replicates the new control byte at the end of the array of control
    /// bytes, returning the old control byte.
    ///
    /// This function does not make any changes to the `data` parts of the table,
    /// or any changes to the `items` or `growth_left` field of the table.
    ///
    /// # Safety
    ///
    /// The safety rules are directly derived from the safety rules for [`RawTableInner::set_ctrl_hash`]
    /// and [`RawTableInner::ctrl`] methods. Thus, in order to uphold the safety contracts for both
    /// methods, you must observe the following rules when calling this function:
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * The `index` must not be greater than the `RawTableInner.bucket_mask`, i.e.
    ///   `index <= RawTableInner.bucket_mask` or, in other words, `(index + 1)` must
    ///   be no greater than the number returned by the function [`RawTableInner::buckets`].
    ///
    /// Calling this function on a table that has not been allocated results in [`undefined behavior`].
    ///
    /// See also [`Bucket::as_ptr`] method, for more information about of properly removing
    /// or saving `data element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`RawTableInner::set_ctrl_hash`]: RawTableInner::set_ctrl_hash
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn replace_ctrl_hash(&mut self, index: usize, hash: u64) -> Tag {
        // SAFETY: The caller must uphold the safety rules for the [`RawTableInner::replace_ctrl_hash`]
        let prev_ctrl = *self.ctrl(index);
        self.set_ctrl_hash(index, hash);
        prev_ctrl
    }

    /// Sets a control byte, and possibly also the replicated control byte at
    /// the end of the array.
    ///
    /// This function does not make any changes to the `data` parts of the table,
    /// or any changes to the `items` or `growth_left` field of the table.
    ///
    /// # Safety
    ///
    /// You must observe the following safety rules when calling this function:
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * The `index` must not be greater than the `RawTableInner.bucket_mask`, i.e.
    ///   `index <= RawTableInner.bucket_mask` or, in other words, `(index + 1)` must
    ///   be no greater than the number returned by the function [`RawTableInner::buckets`].
    ///
    /// Calling this function on a table that has not been allocated results in [`undefined behavior`].
    ///
    /// See also [`Bucket::as_ptr`] method, for more information about of properly removing
    /// or saving `data element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn set_ctrl(&mut self, index: usize, ctrl: Tag) {
        // Replicate the first Group::WIDTH control bytes at the end of
        // the array without using a branch. If the tables smaller than
        // the group width (self.buckets() < Group::WIDTH),
        // `index2 = Group::WIDTH + index`, otherwise `index2` is:
        //
        // - If index >= Group::WIDTH then index == index2.
        // - Otherwise index2 == self.bucket_mask + 1 + index.
        //
        // The very last replicated control byte is never actually read because
        // we mask the initial index for unaligned loads, but we write it
        // anyways because it makes the set_ctrl implementation simpler.
        //
        // If there are fewer buckets than Group::WIDTH then this code will
        // replicate the buckets at the end of the trailing group. For example
        // with 2 buckets and a group size of 4, the control bytes will look
        // like this:
        //
        //     Real    |             Replicated
        // ---------------------------------------------
        // | [A] | [B] | [Tag::EMPTY] | [EMPTY] | [A] | [B] |
        // ---------------------------------------------

        // This is the same as `(index.wrapping_sub(Group::WIDTH)) % self.buckets() + Group::WIDTH`
        // because the number of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
        let index2 = ((index.wrapping_sub(Group::WIDTH)) & self.bucket_mask) + Group::WIDTH;

        // SAFETY: The caller must uphold the safety rules for the [`RawTableInner::set_ctrl`]
        *self.ctrl(index) = ctrl;
        *self.ctrl(index2) = ctrl;
    }

    /// Returns a pointer to a control byte.
    ///
    /// # Safety
    ///
    /// For the allocated [`RawTableInner`], the result is [`Undefined Behavior`],
    /// if the `index` is greater than the `self.bucket_mask + 1 + Group::WIDTH`.
    /// In that case, calling this function with `index == self.bucket_mask + 1 + Group::WIDTH`
    /// will return a pointer to the end of the allocated table and it is useless on its own.
    ///
    /// Calling this function with `index >= self.bucket_mask + 1 + Group::WIDTH` on a
    /// table that has not been allocated results in [`Undefined Behavior`].
    ///
    /// So to satisfy both requirements you should always follow the rule that
    /// `index < self.bucket_mask + 1 + Group::WIDTH`
    ///
    /// Calling this function on [`RawTableInner`] that are not already allocated is safe
    /// for read-only purpose.
    ///
    /// See also [`Bucket::as_ptr()`] method, for more information about of properly removing
    /// or saving `data element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`Bucket::as_ptr()`]: Bucket::as_ptr()
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn ctrl(&self, index: usize) -> *mut Tag {
        debug_assert!(index < self.num_ctrl_bytes());
        // SAFETY: The caller must uphold the safety rules for the [`RawTableInner::ctrl`]
        self.ctrl.as_ptr().add(index).cast()
    }

    /// Gets the slice of all control bytes.
    fn ctrl_slice(&mut self) -> &mut [Tag] {
        // SAFETY: We've intiailized all control bytes, and have the correct number.
        unsafe { slice::from_raw_parts_mut(self.ctrl.as_ptr().cast(), self.num_ctrl_bytes()) }
    }

    #[inline]
    fn buckets(&self) -> usize {
        self.bucket_mask + 1
    }

    /// Checks whether the bucket at `index` is full.
    ///
    /// # Safety
    ///
    /// The caller must ensure `index` is less than the number of buckets.
    #[inline]
    unsafe fn is_bucket_full(&self, index: usize) -> bool {
        debug_assert!(index < self.buckets());
        (*self.ctrl(index)).is_full()
    }

    #[inline]
    fn num_ctrl_bytes(&self) -> usize {
        self.bucket_mask + 1 + Group::WIDTH
    }

    #[inline]
    fn is_empty_singleton(&self) -> bool {
        self.bucket_mask == 0
    }

    /// Attempts to allocate a new hash table with at least enough capacity
    /// for inserting the given number of elements without reallocating,
    /// and return it inside `ScopeGuard` to protect against panic in the hash
    /// function.
    ///
    /// # Note
    ///
    /// It is recommended (but not required):
    ///
    /// * That the new table's `capacity` be greater than or equal to `self.items`.
    ///
    /// * The `alloc` is the same [`Allocator`] as the `Allocator` used
    ///   to allocate this table.
    ///
    /// * The `table_layout` is the same [`TableLayout`] as the `TableLayout` used
    ///   to allocate this table.
    ///
    /// If `table_layout` does not match the `TableLayout` that was used to allocate
    /// this table, then using `mem::swap` with the `self` and the new table returned
    /// by this function results in [`undefined behavior`].
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[allow(clippy::mut_mut)]
    #[inline]
    fn prepare_resize<'a, A>(
        &self,
        alloc: &'a A,
        table_layout: TableLayout,
        capacity: usize,
        fallibility: Fallibility,
    ) -> Result<crate::scopeguard::ScopeGuard<Self, impl FnMut(&mut Self) + 'a>, TryReserveError>
    where
        A: Allocator,
    {
        debug_assert!(self.items <= capacity);

        // Allocate and initialize the new table.
        let new_table =
            RawTableInner::fallible_with_capacity(alloc, table_layout, capacity, fallibility)?;

        // The hash function may panic, in which case we simply free the new
        // table without dropping any elements that may have been copied into
        // it.
        //
        // This guard is also used to free the old table on success, see
        // the comment at the bottom of this function.
        Ok(guard(new_table, move |self_| {
            if !self_.is_empty_singleton() {
                // SAFETY:
                // 1. We have checked that our table is allocated.
                // 2. We know for sure that the `alloc` and `table_layout` matches the
                //    [`Allocator`] and [`TableLayout`] used to allocate this table.
                unsafe { self_.free_buckets(alloc, table_layout) };
            }
        }))
    }

    /// Reserves or rehashes to make room for `additional` more elements.
    ///
    /// This uses dynamic dispatch to reduce the amount of
    /// code generated, but it is eliminated by LLVM optimizations when inlined.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is
    /// [`undefined behavior`]:
    ///
    /// * The `alloc` must be the same [`Allocator`] as the `Allocator` used
    ///   to allocate this table.
    ///
    /// * The `layout` must be the same [`TableLayout`] as the `TableLayout`
    ///   used to allocate this table.
    ///
    /// * The `drop` function (`fn(*mut u8)`) must be the actual drop function of
    ///   the elements stored in the table.
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes.
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[allow(clippy::inline_always)]
    #[inline(always)]
    unsafe fn reserve_rehash_inner<A>(
        &mut self,
        alloc: &A,
        additional: usize,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        fallibility: Fallibility,
        layout: TableLayout,
        drop: Option<unsafe fn(*mut u8)>,
    ) -> Result<(), TryReserveError>
    where
        A: Allocator,
    {
        // Avoid `Option::ok_or_else` because it bloats LLVM IR.
        let new_items = match self.items.checked_add(additional) {
            Some(new_items) => new_items,
            None => return Err(fallibility.capacity_overflow()),
        };
        let full_capacity = bucket_mask_to_capacity(self.bucket_mask);
        if new_items <= full_capacity / 2 {
            // Rehash in-place without re-allocating if we have plenty of spare
            // capacity that is locked up due to DELETED entries.

            // SAFETY:
            // 1. We know for sure that `[`RawTableInner`]` has already been allocated
            //    (since new_items <= full_capacity / 2);
            // 2. The caller ensures that `drop` function is the actual drop function of
            //    the elements stored in the table.
            // 3. The caller ensures that `layout` matches the [`TableLayout`] that was
            //    used to allocate this table.
            // 4. The caller ensures that the control bytes of the `RawTableInner`
            //    are already initialized.
            self.rehash_in_place(hasher, layout.size, drop);
            Ok(())
        } else {
            // Otherwise, conservatively resize to at least the next size up
            // to avoid churning deletes into frequent rehashes.
            //
            // SAFETY:
            // 1. We know for sure that `capacity >= self.items`.
            // 2. The caller ensures that `alloc` and `layout` matches the [`Allocator`] and
            //    [`TableLayout`] that were used to allocate this table.
            // 3. The caller ensures that the control bytes of the `RawTableInner`
            //    are already initialized.
            self.resize_inner(
                alloc,
                usize::max(new_items, full_capacity + 1),
                hasher,
                fallibility,
                layout,
            )
        }
    }

    /// Returns an iterator over full buckets indices in the table.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// * The caller has to ensure that the `RawTableInner` outlives the
    ///   `FullBucketsIndices`. Because we cannot make the `next` method
    ///   unsafe on the `FullBucketsIndices` struct, we have to make the
    ///   `full_buckets_indices` method unsafe.
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes.
    #[inline(always)]
    unsafe fn full_buckets_indices(&self) -> FullBucketsIndices {
        // SAFETY:
        // 1. Since the caller of this function ensures that the control bytes
        //    are properly initialized and `self.ctrl(0)` points to the start
        //    of the array of control bytes, therefore: `ctrl` is valid for reads,
        //    properly aligned to `Group::WIDTH` and points to the properly initialized
        //    control bytes.
        // 2. The value of `items` is equal to the amount of data (values) added
        //    to the table.
        //
        //                         `ctrl` points here (to the start
        //                         of the first control byte `CT0`)
        //                          ∨
        // [Pad], T_n, ..., T1, T0, |CT0, CT1, ..., CT_n|, Group::WIDTH
        //                           \________  ________/
        //                                    \/
        //       `n = buckets - 1`, i.e. `RawTableInner::buckets() - 1`
        //
        // where: T0...T_n  - our stored data;
        //        CT0...CT_n - control bytes or metadata for `data`.
        let ctrl = NonNull::new_unchecked(self.ctrl(0).cast::<u8>());

        FullBucketsIndices {
            // Load the first group
            // SAFETY: See explanation above.
            current_group: Group::load_aligned(ctrl.as_ptr().cast())
                .match_full()
                .into_iter(),
            group_first_index: 0,
            ctrl,
            items: self.items,
        }
    }

    /// Allocates a new table of a different size and moves the contents of the
    /// current table into it.
    ///
    /// This uses dynamic dispatch to reduce the amount of
    /// code generated, but it is eliminated by LLVM optimizations when inlined.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is
    /// [`undefined behavior`]:
    ///
    /// * The `alloc` must be the same [`Allocator`] as the `Allocator` used
    ///   to allocate this table;
    ///
    /// * The `layout` must be the same [`TableLayout`] as the `TableLayout`
    ///   used to allocate this table;
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes.
    ///
    /// The caller of this function must ensure that `capacity >= self.items`
    /// otherwise:
    ///
    /// * If `self.items != 0`, calling of this function with `capacity == 0`
    ///   results in [`undefined behavior`].
    ///
    /// * If `capacity_to_buckets(capacity) < Group::WIDTH` and
    ///   `self.items > capacity_to_buckets(capacity)` calling this function
    ///   results in [`undefined behavior`].
    ///
    /// * If `capacity_to_buckets(capacity) >= Group::WIDTH` and
    ///   `self.items > capacity_to_buckets(capacity)` calling this function
    ///   are never return (will go into an infinite loop).
    ///
    /// Note: It is recommended (but not required) that the new table's `capacity`
    /// be greater than or equal to `self.items`. In case if `capacity <= self.items`
    /// this function can never return. See [`RawTableInner::find_insert_slot`] for
    /// more information.
    ///
    /// [`RawTableInner::find_insert_slot`]: RawTableInner::find_insert_slot
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[allow(clippy::inline_always)]
    #[inline(always)]
    unsafe fn resize_inner<A>(
        &mut self,
        alloc: &A,
        capacity: usize,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        fallibility: Fallibility,
        layout: TableLayout,
    ) -> Result<(), TryReserveError>
    where
        A: Allocator,
    {
        // SAFETY: We know for sure that `alloc` and `layout` matches the [`Allocator`] and [`TableLayout`]
        // that were used to allocate this table.
        let mut new_table = self.prepare_resize(alloc, layout, capacity, fallibility)?;

        // SAFETY: We know for sure that RawTableInner will outlive the
        // returned `FullBucketsIndices` iterator, and the caller of this
        // function ensures that the control bytes are properly initialized.
        for full_byte_index in self.full_buckets_indices() {
            // This may panic.
            let hash = hasher(self, full_byte_index);

            // SAFETY:
            // We can use a simpler version of insert() here since:
            // 1. There are no DELETED entries.
            // 2. We know there is enough space in the table.
            // 3. All elements are unique.
            // 4. The caller of this function guarantees that `capacity > 0`
            //    so `new_table` must already have some allocated memory.
            // 5. We set `growth_left` and `items` fields of the new table
            //    after the loop.
            // 6. We insert into the table, at the returned index, the data
            //    matching the given hash immediately after calling this function.
            let (new_index, _) = new_table.prepare_insert_slot(hash);

            // SAFETY:
            //
            // * `src` is valid for reads of `layout.size` bytes, since the
            //   table is alive and the `full_byte_index` is guaranteed to be
            //   within bounds (see `FullBucketsIndices::next_impl`);
            //
            // * `dst` is valid for writes of `layout.size` bytes, since the
            //   caller ensures that `table_layout` matches the [`TableLayout`]
            //   that was used to allocate old table and we have the `new_index`
            //   returned by `prepare_insert_slot`.
            //
            // * Both `src` and `dst` are properly aligned.
            //
            // * Both `src` and `dst` point to different region of memory.
            ptr::copy_nonoverlapping(
                self.bucket_ptr(full_byte_index, layout.size),
                new_table.bucket_ptr(new_index, layout.size),
                layout.size,
            );
        }

        // The hash function didn't panic, so we can safely set the
        // `growth_left` and `items` fields of the new table.
        new_table.growth_left -= self.items;
        new_table.items = self.items;

        // We successfully copied all elements without panicking. Now replace
        // self with the new table. The old table will have its memory freed but
        // the items will not be dropped (since they have been moved into the
        // new table).
        // SAFETY: The caller ensures that `table_layout` matches the [`TableLayout`]
        // that was used to allocate this table.
        mem::swap(self, &mut new_table);

        Ok(())
    }

    /// Rehashes the contents of the table in place (i.e. without changing the
    /// allocation).
    ///
    /// If `hasher` panics then some the table's contents may be lost.
    ///
    /// This uses dynamic dispatch to reduce the amount of
    /// code generated, but it is eliminated by LLVM optimizations when inlined.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is [`undefined behavior`]:
    ///
    /// * The `size_of` must be equal to the size of the elements stored in the table;
    ///
    /// * The `drop` function (`fn(*mut u8)`) must be the actual drop function of
    ///   the elements stored in the table.
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * The [`RawTableInner`] must have properly initialized control bytes.
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[allow(clippy::inline_always)]
    #[cfg_attr(feature = "inline-more", inline(always))]
    #[cfg_attr(not(feature = "inline-more"), inline)]
    unsafe fn rehash_in_place(
        &mut self,
        hasher: &dyn Fn(&mut Self, usize) -> u64,
        size_of: usize,
        drop: Option<unsafe fn(*mut u8)>,
    ) {
        // If the hash function panics then properly clean up any elements
        // that we haven't rehashed yet. We unfortunately can't preserve the
        // element since we lost their hash and have no way of recovering it
        // without risking another panic.
        self.prepare_rehash_in_place();

        let mut guard = guard(self, move |self_| {
            if let Some(drop) = drop {
                for i in 0..self_.buckets() {
                    if *self_.ctrl(i) == Tag::DELETED {
                        self_.set_ctrl(i, Tag::EMPTY);
                        drop(self_.bucket_ptr(i, size_of));
                        self_.items -= 1;
                    }
                }
            }
            self_.growth_left = bucket_mask_to_capacity(self_.bucket_mask) - self_.items;
        });

        // At this point, DELETED elements are elements that we haven't
        // rehashed yet. Find them and re-insert them at their ideal
        // position.
        'outer: for i in 0..guard.buckets() {
            if *guard.ctrl(i) != Tag::DELETED {
                continue;
            }

            let i_p = guard.bucket_ptr(i, size_of);

            'inner: loop {
                // Hash the current item
                let hash = hasher(*guard, i);

                // Search for a suitable place to put it
                //
                // SAFETY: Caller of this function ensures that the control bytes
                // are properly initialized.
                let new_i = guard.find_insert_slot(hash).index;

                // Probing works by scanning through all of the control
                // bytes in groups, which may not be aligned to the group
                // size. If both the new and old position fall within the
                // same unaligned group, then there is no benefit in moving
                // it and we can just continue to the next item.
                if likely(guard.is_in_same_group(i, new_i, hash)) {
                    guard.set_ctrl_hash(i, hash);
                    continue 'outer;
                }

                let new_i_p = guard.bucket_ptr(new_i, size_of);

                // We are moving the current item to a new position. Write
                // our H2 to the control byte of the new position.
                let prev_ctrl = guard.replace_ctrl_hash(new_i, hash);
                if prev_ctrl == Tag::EMPTY {
                    guard.set_ctrl(i, Tag::EMPTY);
                    // If the target slot is empty, simply move the current
                    // element into the new slot and clear the old control
                    // byte.
                    ptr::copy_nonoverlapping(i_p, new_i_p, size_of);
                    continue 'outer;
                } else {
                    // If the target slot is occupied, swap the two elements
                    // and then continue processing the element that we just
                    // swapped into the old slot.
                    debug_assert_eq!(prev_ctrl, Tag::DELETED);
                    ptr::swap_nonoverlapping(i_p, new_i_p, size_of);
                    continue 'inner;
                }
            }
        }

        guard.growth_left = bucket_mask_to_capacity(guard.bucket_mask) - guard.items;

        mem::forget(guard);
    }

    /// Deallocates the table without dropping any entries.
    ///
    /// # Note
    ///
    /// This function must be called only after [`drop_elements`](RawTableInner::drop_elements),
    /// else it can lead to leaking of memory. Also calling this function automatically
    /// makes invalid (dangling) all instances of buckets ([`Bucket`]) and makes invalid
    /// (dangling) the `ctrl` field of the table.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is [`Undefined Behavior`]:
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * The `alloc` must be the same [`Allocator`] as the `Allocator` that was used
    ///   to allocate this table.
    ///
    /// * The `table_layout` must be the same [`TableLayout`] as the `TableLayout` that was used
    ///   to allocate this table.
    ///
    /// See also [`GlobalAlloc::dealloc`] or [`Allocator::deallocate`] for more  information.
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    /// [`GlobalAlloc::dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc
    /// [`Allocator::deallocate`]: https://doc.rust-lang.org/alloc/alloc/trait.Allocator.html#tymethod.deallocate
    #[inline]
    unsafe fn free_buckets<A>(&mut self, alloc: &A, table_layout: TableLayout)
    where
        A: Allocator,
    {
        // SAFETY: The caller must uphold the safety contract for `free_buckets`
        // method.
        let (ptr, layout) = self.allocation_info(table_layout);
        alloc.deallocate(ptr, layout);
    }

    /// Returns a pointer to the allocated memory and the layout that was used to
    /// allocate the table.
    ///
    /// # Safety
    ///
    /// Caller of this function must observe the following safety rules:
    ///
    /// * The [`RawTableInner`] has already been allocated, otherwise
    ///   calling this function results in [`undefined behavior`]
    ///
    /// * The `table_layout` must be the same [`TableLayout`] as the `TableLayout`
    ///   that was used to allocate this table. Failure to comply with this condition
    ///   may result in [`undefined behavior`].
    ///
    /// See also [`GlobalAlloc::dealloc`] or [`Allocator::deallocate`] for more  information.
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    /// [`GlobalAlloc::dealloc`]: https://doc.rust-lang.org/alloc/alloc/trait.GlobalAlloc.html#tymethod.dealloc
    /// [`Allocator::deallocate`]: https://doc.rust-lang.org/alloc/alloc/trait.Allocator.html#tymethod.deallocate
    #[inline]
    unsafe fn allocation_info(&self, table_layout: TableLayout) -> (NonNull<u8>, Layout) {
        debug_assert!(
            !self.is_empty_singleton(),
            "this function can only be called on non-empty tables"
        );

        // Avoid `Option::unwrap_or_else` because it bloats LLVM IR.
        let (layout, ctrl_offset) = match table_layout.calculate_layout_for(self.buckets()) {
            Some(lco) => lco,
            None => unsafe { hint::unreachable_unchecked() },
        };
        (
            // SAFETY: The caller must uphold the safety contract for `allocation_info` method.
            unsafe { NonNull::new_unchecked(self.ctrl.as_ptr().sub(ctrl_offset)) },
            layout,
        )
    }

    /// Returns the total amount of memory allocated internally by the hash
    /// table, in bytes.
    ///
    /// The returned number is informational only. It is intended to be
    /// primarily used for memory profiling.
    ///
    /// # Safety
    ///
    /// The `table_layout` must be the same [`TableLayout`] as the `TableLayout`
    /// that was used to allocate this table. Failure to comply with this condition
    /// may result in [`undefined behavior`].
    ///
    ///
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn allocation_size_or_zero(&self, table_layout: TableLayout) -> usize {
        if self.is_empty_singleton() {
            0
        } else {
            // SAFETY:
            // 1. We have checked that our table is allocated.
            // 2. The caller ensures that `table_layout` matches the [`TableLayout`]
            // that was used to allocate this table.
            unsafe { self.allocation_info(table_layout).1.size() }
        }
    }

    /// Marks all table buckets as empty without dropping their contents.
    #[inline]
    fn clear_no_drop(&mut self) {
        if !self.is_empty_singleton() {
            self.ctrl_slice().fill_empty();
        }
        self.items = 0;
        self.growth_left = bucket_mask_to_capacity(self.bucket_mask);
    }

    /// Erases the [`Bucket`]'s control byte at the given index so that it does not
    /// triggered as full, decreases the `items` of the table and, if it can be done,
    /// increases `self.growth_left`.
    ///
    /// This function does not actually erase / drop the [`Bucket`] itself, i.e. it
    /// does not make any changes to the `data` parts of the table. The caller of this
    /// function must take care to properly drop the `data`, otherwise calling this
    /// function may result in a memory leak.
    ///
    /// # Safety
    ///
    /// You must observe the following safety rules when calling this function:
    ///
    /// * The [`RawTableInner`] has already been allocated;
    ///
    /// * It must be the full control byte at the given position;
    ///
    /// * The `index` must not be greater than the `RawTableInner.bucket_mask`, i.e.
    ///   `index <= RawTableInner.bucket_mask` or, in other words, `(index + 1)` must
    ///   be no greater than the number returned by the function [`RawTableInner::buckets`].
    ///
    /// Calling this function on a table that has not been allocated results in [`undefined behavior`].
    ///
    /// Calling this function on a table with no elements is unspecified, but calling subsequent
    /// functions is likely to result in [`undefined behavior`] due to overflow subtraction
    /// (`self.items -= 1 cause overflow when self.items == 0`).
    ///
    /// See also [`Bucket::as_ptr`] method, for more information about of properly removing
    /// or saving `data element` from / into the [`RawTable`] / [`RawTableInner`].
    ///
    /// [`RawTableInner::buckets`]: RawTableInner::buckets
    /// [`Bucket::as_ptr`]: Bucket::as_ptr
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    unsafe fn erase(&mut self, index: usize) {
        debug_assert!(self.is_bucket_full(index));

        // This is the same as `index.wrapping_sub(Group::WIDTH) % self.buckets()` because
        // the number of buckets is a power of two, and `self.bucket_mask = self.buckets() - 1`.
        let index_before = index.wrapping_sub(Group::WIDTH) & self.bucket_mask;
        // SAFETY:
        // - The caller must uphold the safety contract for `erase` method;
        // - `index_before` is guaranteed to be in range due to masking with `self.bucket_mask`
        let empty_before = Group::load(self.ctrl(index_before)).match_empty();
        let empty_after = Group::load(self.ctrl(index)).match_empty();

        // Inserting and searching in the map is performed by two key functions:
        //
        // - The `find_insert_slot` function that looks up the index of any `Tag::EMPTY` or `Tag::DELETED`
        //   slot in a group to be able to insert. If it doesn't find an `Tag::EMPTY` or `Tag::DELETED`
        //   slot immediately in the first group, it jumps to the next `Group` looking for it,
        //   and so on until it has gone through all the groups in the control bytes.
        //
        // - The `find_inner` function that looks for the index of the desired element by looking
        //   at all the `FULL` bytes in the group. If it did not find the element right away, and
        //   there is no `Tag::EMPTY` byte in the group, then this means that the `find_insert_slot`
        //   function may have found a suitable slot in the next group. Therefore, `find_inner`
        //   jumps further, and if it does not find the desired element and again there is no `Tag::EMPTY`
        //   byte, then it jumps further, and so on. The search stops only if `find_inner` function
        //   finds the desired element or hits an `Tag::EMPTY` slot/byte.
        //
        // Accordingly, this leads to two consequences:
        //
        // - The map must have `Tag::EMPTY` slots (bytes);
        //
        // - You can't just mark the byte to be erased as `Tag::EMPTY`, because otherwise the `find_inner`
        //   function may stumble upon an `Tag::EMPTY` byte before finding the desired element and stop
        //   searching.
        //
        // Thus it is necessary to check all bytes after and before the erased element. If we are in
        // a contiguous `Group` of `FULL` or `Tag::DELETED` bytes (the number of `FULL` or `Tag::DELETED` bytes
        // before and after is greater than or equal to `Group::WIDTH`), then we must mark our byte as
        // `Tag::DELETED` in order for the `find_inner` function to go further. On the other hand, if there
        // is at least one `Tag::EMPTY` slot in the `Group`, then the `find_inner` function will still stumble
        // upon an `Tag::EMPTY` byte, so we can safely mark our erased byte as `Tag::EMPTY` as well.
        //
        // Finally, since `index_before == (index.wrapping_sub(Group::WIDTH) & self.bucket_mask) == index`
        // and given all of the above, tables smaller than the group width (self.buckets() < Group::WIDTH)
        // cannot have `Tag::DELETED` bytes.
        //
        // Note that in this context `leading_zeros` refers to the bytes at the end of a group, while
        // `trailing_zeros` refers to the bytes at the beginning of a group.
        let ctrl = if empty_before.leading_zeros() + empty_after.trailing_zeros() >= Group::WIDTH {
            Tag::DELETED
        } else {
            self.growth_left += 1;
            Tag::EMPTY
        };
        // SAFETY: the caller must uphold the safety contract for `erase` method.
        self.set_ctrl(index, ctrl);
        self.items -= 1;
    }
}

impl<T: Clone, A: Allocator + Clone> Clone for RawTable<T, A> {
    fn clone(&self) -> Self {
        if self.table.is_empty_singleton() {
            Self::new_in(self.alloc.clone())
        } else {
            unsafe {
                // Avoid `Result::ok_or_else` because it bloats LLVM IR.
                //
                // SAFETY: This is safe as we are taking the size of an already allocated table
                // and therefore capacity overflow cannot occur, `self.table.buckets()` is power
                // of two and all allocator errors will be caught inside `RawTableInner::new_uninitialized`.
                let mut new_table = match Self::new_uninitialized(
                    self.alloc.clone(),
                    self.table.buckets(),
                    Fallibility::Infallible,
                ) {
                    Ok(table) => table,
                    Err(_) => hint::unreachable_unchecked(),
                };

                // Cloning elements may fail (the clone function may panic). But we don't
                // need to worry about uninitialized control bits, since:
                // 1. The number of items (elements) in the table is zero, which means that
                //    the control bits will not be read by Drop function.
                // 2. The `clone_from_spec` method will first copy all control bits from
                //    `self` (thus initializing them). But this will not affect the `Drop`
                //    function, since the `clone_from_spec` function sets `items` only after
                //    successfully cloning all elements.
                new_table.clone_from_spec(self);
                new_table
            }
        }
    }

    fn clone_from(&mut self, source: &Self) {
        if source.table.is_empty_singleton() {
            let mut old_inner = mem::replace(&mut self.table, RawTableInner::NEW);
            unsafe {
                // SAFETY:
                // 1. We call the function only once;
                // 2. We know for sure that `alloc` and `table_layout` matches the [`Allocator`]
                //    and [`TableLayout`] that were used to allocate this table.
                // 3. If any elements' drop function panics, then there will only be a memory leak,
                //    because we have replaced the inner table with a new one.
                old_inner.drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
            }
        } else {
            unsafe {
                // Make sure that if any panics occurs, we clear the table and
                // leave it in an empty state.
                let mut self_ = guard(self, |self_| {
                    self_.clear_no_drop();
                });

                // First, drop all our elements without clearing the control
                // bytes. If this panics then the scope guard will clear the
                // table, leaking any elements that were not dropped yet.
                //
                // This leak is unavoidable: we can't try dropping more elements
                // since this could lead to another panic and abort the process.
                //
                // SAFETY: If something gets wrong we clear our table right after
                // dropping the elements, so there is no double drop, since `items`
                // will be equal to zero.
                self_.table.drop_elements::<T>();

                // If necessary, resize our table to match the source.
                if self_.buckets() != source.buckets() {
                    let new_inner = match RawTableInner::new_uninitialized(
                        &self_.alloc,
                        Self::TABLE_LAYOUT,
                        source.buckets(),
                        Fallibility::Infallible,
                    ) {
                        Ok(table) => table,
                        Err(_) => hint::unreachable_unchecked(),
                    };
                    // Replace the old inner with new uninitialized one. It's ok, since if something gets
                    // wrong `ScopeGuard` will initialize all control bytes and leave empty table.
                    let mut old_inner = mem::replace(&mut self_.table, new_inner);
                    if !old_inner.is_empty_singleton() {
                        // SAFETY:
                        // 1. We have checked that our table is allocated.
                        // 2. We know for sure that `alloc` and `table_layout` matches
                        // the [`Allocator`] and [`TableLayout`] that were used to allocate this table.
                        old_inner.free_buckets(&self_.alloc, Self::TABLE_LAYOUT);
                    }
                }

                // Cloning elements may fail (the clone function may panic), but the `ScopeGuard`
                // inside the `clone_from_impl` function will take care of that, dropping all
                // cloned elements if necessary. Our `ScopeGuard` will clear the table.
                self_.clone_from_spec(source);

                // Disarm the scope guard if cloning was successful.
                ScopeGuard::into_inner(self_);
            }
        }
    }
}

/// Specialization of `clone_from` for `Copy` types
trait RawTableClone {
    unsafe fn clone_from_spec(&mut self, source: &Self);
}
impl<T: Clone, A: Allocator + Clone> RawTableClone for RawTable<T, A> {
    default_fn! {
        #[cfg_attr(feature = "inline-more", inline)]
        unsafe fn clone_from_spec(&mut self, source: &Self) {
            self.clone_from_impl(source);
        }
    }
}
#[cfg(feature = "nightly")]
impl<T: Copy, A: Allocator + Clone> RawTableClone for RawTable<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn clone_from_spec(&mut self, source: &Self) {
        source
            .table
            .ctrl(0)
            .copy_to_nonoverlapping(self.table.ctrl(0), self.table.num_ctrl_bytes());
        source
            .data_start()
            .as_ptr()
            .copy_to_nonoverlapping(self.data_start().as_ptr(), self.table.buckets());

        self.table.items = source.table.items;
        self.table.growth_left = source.table.growth_left;
    }
}

impl<T: Clone, A: Allocator + Clone> RawTable<T, A> {
    /// Common code for `clone` and `clone_from`. Assumes:
    /// - `self.buckets() == source.buckets()`.
    /// - Any existing elements have been dropped.
    /// - The control bytes are not initialized yet.
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn clone_from_impl(&mut self, source: &Self) {
        // Copy the control bytes unchanged. We do this in a single pass
        source
            .table
            .ctrl(0)
            .copy_to_nonoverlapping(self.table.ctrl(0), self.table.num_ctrl_bytes());

        // The cloning of elements may panic, in which case we need
        // to make sure we drop only the elements that have been
        // cloned so far.
        let mut guard = guard((0, &mut *self), |(index, self_)| {
            if T::NEEDS_DROP {
                for i in 0..*index {
                    if self_.is_bucket_full(i) {
                        self_.bucket(i).drop();
                    }
                }
            }
        });

        for from in source.iter() {
            let index = source.bucket_index(&from);
            let to = guard.1.bucket(index);
            to.write(from.as_ref().clone());

            // Update the index in case we need to unwind.
            guard.0 = index + 1;
        }

        // Successfully cloned all items, no need to clean up.
        mem::forget(guard);

        self.table.items = source.table.items;
        self.table.growth_left = source.table.growth_left;
    }
}

impl<T, A: Allocator + Default> Default for RawTable<T, A> {
    #[inline]
    fn default() -> Self {
        Self::new_in(Default::default())
    }
}

#[cfg(feature = "nightly")]
unsafe impl<#[may_dangle] T, A: Allocator> Drop for RawTable<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn drop(&mut self) {
        unsafe {
            // SAFETY:
            // 1. We call the function only once;
            // 2. We know for sure that `alloc` and `table_layout` matches the [`Allocator`]
            //    and [`TableLayout`] that were used to allocate this table.
            // 3. If the drop function of any elements fails, then only a memory leak will occur,
            //    and we don't care because we are inside the `Drop` function of the `RawTable`,
            //    so there won't be any table left in an inconsistent state.
            self.table
                .drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
        }
    }
}
#[cfg(not(feature = "nightly"))]
impl<T, A: Allocator> Drop for RawTable<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn drop(&mut self) {
        unsafe {
            // SAFETY:
            // 1. We call the function only once;
            // 2. We know for sure that `alloc` and `table_layout` matches the [`Allocator`]
            //    and [`TableLayout`] that were used to allocate this table.
            // 3. If the drop function of any elements fails, then only a memory leak will occur,
            //    and we don't care because we are inside the `Drop` function of the `RawTable`,
            //    so there won't be any table left in an inconsistent state.
            self.table
                .drop_inner_table::<T, _>(&self.alloc, Self::TABLE_LAYOUT);
        }
    }
}

impl<T, A: Allocator> IntoIterator for RawTable<T, A> {
    type Item = T;
    type IntoIter = RawIntoIter<T, A>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_iter(self) -> RawIntoIter<T, A> {
        unsafe {
            let iter = self.iter();
            self.into_iter_from(iter)
        }
    }
}

/// Iterator over a sub-range of a table. Unlike `RawIter` this iterator does
/// not track an item count.
pub(crate) struct RawIterRange<T> {
    // Mask of full buckets in the current group. Bits are cleared from this
    // mask as each element is processed.
    current_group: BitMaskIter,

    // Pointer to the buckets for the current group.
    data: Bucket<T>,

    // Pointer to the next group of control bytes,
    // Must be aligned to the group size.
    next_ctrl: *const u8,

    // Pointer one past the last control byte of this range.
    end: *const u8,
}

impl<T> RawIterRange<T> {
    /// Returns a `RawIterRange` covering a subset of a table.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is
    /// [`undefined behavior`]:
    ///
    /// * `ctrl` must be [valid] for reads, i.e. table outlives the `RawIterRange`;
    ///
    /// * `ctrl` must be properly aligned to the group size (`Group::WIDTH`);
    ///
    /// * `ctrl` must point to the array of properly initialized control bytes;
    ///
    /// * `data` must be the [`Bucket`] at the `ctrl` index in the table;
    ///
    /// * the value of `len` must be less than or equal to the number of table buckets,
    ///   and the returned value of `ctrl.as_ptr().add(len).offset_from(ctrl.as_ptr())`
    ///   must be positive.
    ///
    /// * The `ctrl.add(len)` pointer must be either in bounds or one
    ///   byte past the end of the same [allocated table].
    ///
    /// * The `len` must be a power of two.
    ///
    /// [valid]: https://doc.rust-lang.org/std/ptr/index.html#safety
    /// [`undefined behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn new(ctrl: *const u8, data: Bucket<T>, len: usize) -> Self {
        debug_assert_ne!(len, 0);
        debug_assert_eq!(ctrl as usize % Group::WIDTH, 0);
        // SAFETY: The caller must uphold the safety rules for the [`RawIterRange::new`]
        let end = ctrl.add(len);

        // Load the first group and advance ctrl to point to the next group
        // SAFETY: The caller must uphold the safety rules for the [`RawIterRange::new`]
        let current_group = Group::load_aligned(ctrl.cast()).match_full();
        let next_ctrl = ctrl.add(Group::WIDTH);

        Self {
            current_group: current_group.into_iter(),
            data,
            next_ctrl,
            end,
        }
    }

    /// Splits a `RawIterRange` into two halves.
    ///
    /// Returns `None` if the remaining range is smaller than or equal to the
    /// group width.
    #[cfg_attr(feature = "inline-more", inline)]
    #[cfg(feature = "rayon")]
    pub(crate) fn split(mut self) -> (Self, Option<RawIterRange<T>>) {
        unsafe {
            if self.end <= self.next_ctrl {
                // Nothing to split if the group that we are current processing
                // is the last one.
                (self, None)
            } else {
                // len is the remaining number of elements after the group that
                // we are currently processing. It must be a multiple of the
                // group size (small tables are caught by the check above).
                let len = offset_from(self.end, self.next_ctrl);
                debug_assert_eq!(len % Group::WIDTH, 0);

                // Split the remaining elements into two halves, but round the
                // midpoint down in case there is an odd number of groups
                // remaining. This ensures that:
                // - The tail is at least 1 group long.
                // - The split is roughly even considering we still have the
                //   current group to process.
                let mid = (len / 2) & !(Group::WIDTH - 1);

                let tail = Self::new(
                    self.next_ctrl.add(mid),
                    self.data.next_n(Group::WIDTH).next_n(mid),
                    len - mid,
                );
                debug_assert_eq!(
                    self.data.next_n(Group::WIDTH).next_n(mid).ptr,
                    tail.data.ptr
                );
                debug_assert_eq!(self.end, tail.end);
                self.end = self.next_ctrl.add(mid);
                debug_assert_eq!(self.end.add(Group::WIDTH), tail.next_ctrl);
                (self, Some(tail))
            }
        }
    }

    /// # Safety
    /// If `DO_CHECK_PTR_RANGE` is false, caller must ensure that we never try to iterate
    /// after yielding all elements.
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn next_impl<const DO_CHECK_PTR_RANGE: bool>(&mut self) -> Option<Bucket<T>> {
        loop {
            if let Some(index) = self.current_group.next() {
                return Some(self.data.next_n(index));
            }

            if DO_CHECK_PTR_RANGE && self.next_ctrl >= self.end {
                return None;
            }

            // We might read past self.end up to the next group boundary,
            // but this is fine because it only occurs on tables smaller
            // than the group size where the trailing control bytes are all
            // EMPTY. On larger tables self.end is guaranteed to be aligned
            // to the group size (since tables are power-of-two sized).
            self.current_group = Group::load_aligned(self.next_ctrl.cast())
                .match_full()
                .into_iter();
            self.data = self.data.next_n(Group::WIDTH);
            self.next_ctrl = self.next_ctrl.add(Group::WIDTH);
        }
    }

    /// Folds every element into an accumulator by applying an operation,
    /// returning the final result.
    ///
    /// `fold_impl()` takes three arguments: the number of items remaining in
    /// the iterator, an initial value, and a closure with two arguments: an
    /// 'accumulator', and an element. The closure returns the value that the
    /// accumulator should have for the next iteration.
    ///
    /// The initial value is the value the accumulator will have on the first call.
    ///
    /// After applying this closure to every element of the iterator, `fold_impl()`
    /// returns the accumulator.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is
    /// [`Undefined Behavior`]:
    ///
    /// * The [`RawTableInner`] / [`RawTable`] must be alive and not moved,
    ///   i.e. table outlives the `RawIterRange`;
    ///
    /// * The provided `n` value must match the actual number of items
    ///   in the table.
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[allow(clippy::while_let_on_iterator)]
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn fold_impl<F, B>(mut self, mut n: usize, mut acc: B, mut f: F) -> B
    where
        F: FnMut(B, Bucket<T>) -> B,
    {
        loop {
            while let Some(index) = self.current_group.next() {
                // The returned `index` will always be in the range `0..Group::WIDTH`,
                // so that calling `self.data.next_n(index)` is safe (see detailed explanation below).
                debug_assert!(n != 0);
                let bucket = self.data.next_n(index);
                acc = f(acc, bucket);
                n -= 1;
            }

            if n == 0 {
                return acc;
            }

            // SAFETY: The caller of this function ensures that:
            //
            // 1. The provided `n` value matches the actual number of items in the table;
            // 2. The table is alive and did not moved.
            //
            // Taking the above into account, we always stay within the bounds, because:
            //
            // 1. For tables smaller than the group width (self.buckets() <= Group::WIDTH),
            //    we will never end up in the given branch, since we should have already
            //    yielded all the elements of the table.
            //
            // 2. For tables larger than the group width. The number of buckets is a
            //    power of two (2 ^ n), Group::WIDTH is also power of two (2 ^ k). Since
            //    `(2 ^ n) > (2 ^ k)`, than `(2 ^ n) % (2 ^ k) = 0`. As we start from the
            //    start of the array of control bytes, and never try to iterate after
            //    getting all the elements, the last `self.current_group` will read bytes
            //    from the `self.buckets() - Group::WIDTH` index.  We know also that
            //    `self.current_group.next()` will always return indices within the range
            //    `0..Group::WIDTH`.
            //
            //    Knowing all of the above and taking into account that we are synchronizing
            //    the `self.data` index with the index we used to read the `self.current_group`,
            //    the subsequent `self.data.next_n(index)` will always return a bucket with
            //    an index number less than `self.buckets()`.
            //
            //    The last `self.next_ctrl`, whose index would be `self.buckets()`, will never
            //    actually be read, since we should have already yielded all the elements of
            //    the table.
            self.current_group = Group::load_aligned(self.next_ctrl.cast())
                .match_full()
                .into_iter();
            self.data = self.data.next_n(Group::WIDTH);
            self.next_ctrl = self.next_ctrl.add(Group::WIDTH);
        }
    }
}

// We make raw iterators unconditionally Send and Sync, and let the PhantomData
// in the actual iterator implementations determine the real Send/Sync bounds.
unsafe impl<T> Send for RawIterRange<T> {}
unsafe impl<T> Sync for RawIterRange<T> {}

impl<T> Clone for RawIterRange<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            next_ctrl: self.next_ctrl,
            current_group: self.current_group.clone(),
            end: self.end,
        }
    }
}

impl<T> Iterator for RawIterRange<T> {
    type Item = Bucket<T>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<Bucket<T>> {
        unsafe {
            // SAFETY: We set checker flag to true.
            self.next_impl::<true>()
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // We don't have an item count, so just guess based on the range size.
        let remaining_buckets = if self.end > self.next_ctrl {
            unsafe { offset_from(self.end, self.next_ctrl) }
        } else {
            0
        };

        // Add a group width to include the group we are currently processing.
        (0, Some(Group::WIDTH + remaining_buckets))
    }
}

impl<T> FusedIterator for RawIterRange<T> {}

/// Iterator which returns a raw pointer to every full bucket in the table.
///
/// For maximum flexibility this iterator is not bound by a lifetime, but you
/// must observe several rules when using it:
/// - You must not free the hash table while iterating (including via growing/shrinking).
/// - It is fine to erase a bucket that has been yielded by the iterator.
/// - Erasing a bucket that has not yet been yielded by the iterator may still
///   result in the iterator yielding that bucket (unless `reflect_remove` is called).
/// - It is unspecified whether an element inserted after the iterator was
///   created will be yielded by that iterator (unless `reflect_insert` is called).
/// - The order in which the iterator yields bucket is unspecified and may
///   change in the future.
pub struct RawIter<T> {
    pub(crate) iter: RawIterRange<T>,
    items: usize,
}

impl<T> RawIter<T> {
    unsafe fn drop_elements(&mut self) {
        if T::NEEDS_DROP && self.items != 0 {
            for item in self {
                item.drop();
            }
        }
    }
}

impl<T> Clone for RawIter<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            items: self.items,
        }
    }
}
impl<T> Default for RawIter<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn default() -> Self {
        // SAFETY: Because the table is static, it always outlives the iter.
        unsafe { RawTableInner::NEW.iter() }
    }
}

impl<T> Iterator for RawIter<T> {
    type Item = Bucket<T>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<Bucket<T>> {
        // Inner iterator iterates over buckets
        // so it can do unnecessary work if we already yielded all items.
        if self.items == 0 {
            return None;
        }

        let nxt = unsafe {
            // SAFETY: We check number of items to yield using `items` field.
            self.iter.next_impl::<false>()
        };

        debug_assert!(nxt.is_some());
        self.items -= 1;

        nxt
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.items, Some(self.items))
    }

    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        unsafe { self.iter.fold_impl(self.items, init, f) }
    }
}

impl<T> ExactSizeIterator for RawIter<T> {}
impl<T> FusedIterator for RawIter<T> {}

/// Iterator which returns an index of every full bucket in the table.
///
/// For maximum flexibility this iterator is not bound by a lifetime, but you
/// must observe several rules when using it:
/// - You must not free the hash table while iterating (including via growing/shrinking).
/// - It is fine to erase a bucket that has been yielded by the iterator.
/// - Erasing a bucket that has not yet been yielded by the iterator may still
///   result in the iterator yielding index of that bucket.
/// - It is unspecified whether an element inserted after the iterator was
///   created will be yielded by that iterator.
/// - The order in which the iterator yields indices of the buckets is unspecified
///   and may change in the future.
pub(crate) struct FullBucketsIndices {
    // Mask of full buckets in the current group. Bits are cleared from this
    // mask as each element is processed.
    current_group: BitMaskIter,

    // Initial value of the bytes' indices of the current group (relative
    // to the start of the control bytes).
    group_first_index: usize,

    // Pointer to the current group of control bytes,
    // Must be aligned to the group size (Group::WIDTH).
    ctrl: NonNull<u8>,

    // Number of elements in the table.
    items: usize,
}

impl FullBucketsIndices {
    /// Advances the iterator and returns the next value.
    ///
    /// # Safety
    ///
    /// If any of the following conditions are violated, the result is
    /// [`Undefined Behavior`]:
    ///
    /// * The [`RawTableInner`] / [`RawTable`] must be alive and not moved,
    ///   i.e. table outlives the `FullBucketsIndices`;
    ///
    /// * It never tries to iterate after getting all elements.
    ///
    /// [`Undefined Behavior`]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline(always)]
    unsafe fn next_impl(&mut self) -> Option<usize> {
        loop {
            if let Some(index) = self.current_group.next() {
                // The returned `self.group_first_index + index` will always
                // be in the range `0..self.buckets()`. See explanation below.
                return Some(self.group_first_index + index);
            }

            // SAFETY: The caller of this function ensures that:
            //
            // 1. It never tries to iterate after getting all the elements;
            // 2. The table is alive and did not moved;
            // 3. The first `self.ctrl` pointed to the start of the array of control bytes.
            //
            // Taking the above into account, we always stay within the bounds, because:
            //
            // 1. For tables smaller than the group width (self.buckets() <= Group::WIDTH),
            //    we will never end up in the given branch, since we should have already
            //    yielded all the elements of the table.
            //
            // 2. For tables larger than the group width. The number of buckets is a
            //    power of two (2 ^ n), Group::WIDTH is also power of two (2 ^ k). Since
            //    `(2 ^ n) > (2 ^ k)`, than `(2 ^ n) % (2 ^ k) = 0`. As we start from the
            //    the start of the array of control bytes, and never try to iterate after
            //    getting all the elements, the last `self.ctrl` will be equal to
            //    the `self.buckets() - Group::WIDTH`, so `self.current_group.next()`
            //    will always contains indices within the range `0..Group::WIDTH`,
            //    and subsequent `self.group_first_index + index` will always return a
            //    number less than `self.buckets()`.
            self.ctrl = NonNull::new_unchecked(self.ctrl.as_ptr().add(Group::WIDTH));

            // SAFETY: See explanation above.
            self.current_group = Group::load_aligned(self.ctrl.as_ptr().cast())
                .match_full()
                .into_iter();
            self.group_first_index += Group::WIDTH;
        }
    }
}

impl Iterator for FullBucketsIndices {
    type Item = usize;

    /// Advances the iterator and returns the next value. It is up to
    /// the caller to ensure that the `RawTable` outlives the `FullBucketsIndices`,
    /// because we cannot make the `next` method unsafe.
    #[inline(always)]
    fn next(&mut self) -> Option<usize> {
        // Return if we already yielded all items.
        if self.items == 0 {
            return None;
        }

        let nxt = unsafe {
            // SAFETY:
            // 1. We check number of items to yield using `items` field.
            // 2. The caller ensures that the table is alive and has not moved.
            self.next_impl()
        };

        debug_assert!(nxt.is_some());
        self.items -= 1;

        nxt
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.items, Some(self.items))
    }
}

impl ExactSizeIterator for FullBucketsIndices {}
impl FusedIterator for FullBucketsIndices {}

/// Iterator which consumes a table and returns elements.
pub struct RawIntoIter<T, A: Allocator = Global> {
    iter: RawIter<T>,
    allocation: Option<(NonNull<u8>, Layout, A)>,
    marker: PhantomData<T>,
}

impl<T, A: Allocator> RawIntoIter<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn iter(&self) -> RawIter<T> {
        self.iter.clone()
    }
}

unsafe impl<T, A: Allocator> Send for RawIntoIter<T, A>
where
    T: Send,
    A: Send,
{
}
unsafe impl<T, A: Allocator> Sync for RawIntoIter<T, A>
where
    T: Sync,
    A: Sync,
{
}

#[cfg(feature = "nightly")]
unsafe impl<#[may_dangle] T, A: Allocator> Drop for RawIntoIter<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn drop(&mut self) {
        unsafe {
            // Drop all remaining elements
            self.iter.drop_elements();

            // Free the table
            if let Some((ptr, layout, ref alloc)) = self.allocation {
                alloc.deallocate(ptr, layout);
            }
        }
    }
}
#[cfg(not(feature = "nightly"))]
impl<T, A: Allocator> Drop for RawIntoIter<T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn drop(&mut self) {
        unsafe {
            // Drop all remaining elements
            self.iter.drop_elements();

            // Free the table
            if let Some((ptr, layout, ref alloc)) = self.allocation {
                alloc.deallocate(ptr, layout);
            }
        }
    }
}

impl<T, A: Allocator> Default for RawIntoIter<T, A> {
    fn default() -> Self {
        Self {
            iter: Default::default(),
            allocation: None,
            marker: PhantomData,
        }
    }
}
impl<T, A: Allocator> Iterator for RawIntoIter<T, A> {
    type Item = T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<T> {
        unsafe { Some(self.iter.next()?.read()) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T, A: Allocator> ExactSizeIterator for RawIntoIter<T, A> {}
impl<T, A: Allocator> FusedIterator for RawIntoIter<T, A> {}

/// Iterator which consumes elements without freeing the table storage.
pub struct RawDrain<'a, T, A: Allocator = Global> {
    iter: RawIter<T>,

    // The table is moved into the iterator for the duration of the drain. This
    // ensures that an empty table is left if the drain iterator is leaked
    // without dropping.
    table: RawTableInner,
    orig_table: NonNull<RawTableInner>,

    // We don't use a &'a mut RawTable<T> because we want RawDrain to be
    // covariant over T.
    marker: PhantomData<&'a RawTable<T, A>>,
}

impl<T, A: Allocator> RawDrain<'_, T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn iter(&self) -> RawIter<T> {
        self.iter.clone()
    }
}

unsafe impl<T, A: Allocator> Send for RawDrain<'_, T, A>
where
    T: Send,
    A: Send,
{
}
unsafe impl<T, A: Allocator> Sync for RawDrain<'_, T, A>
where
    T: Sync,
    A: Sync,
{
}

impl<T, A: Allocator> Drop for RawDrain<'_, T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn drop(&mut self) {
        unsafe {
            // Drop all remaining elements. Note that this may panic.
            self.iter.drop_elements();

            // Reset the contents of the table now that all elements have been
            // dropped.
            self.table.clear_no_drop();

            // Move the now empty table back to its original location.
            self.orig_table
                .as_ptr()
                .copy_from_nonoverlapping(&self.table, 1);
        }
    }
}

impl<T, A: Allocator> Iterator for RawDrain<'_, T, A> {
    type Item = T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<T> {
        unsafe {
            let item = self.iter.next()?;
            Some(item.read())
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T, A: Allocator> ExactSizeIterator for RawDrain<'_, T, A> {}
impl<T, A: Allocator> FusedIterator for RawDrain<'_, T, A> {}

/// Iterator over occupied buckets that could match a given hash.
///
/// `RawTable` only stores 7 bits of the hash value, so this iterator may return
/// items that have a hash value different than the one provided. You should
/// always validate the returned values before using them.
///
/// For maximum flexibility this iterator is not bound by a lifetime, but you
/// must observe several rules when using it:
/// - You must not free the hash table while iterating (including via growing/shrinking).
/// - It is fine to erase a bucket that has been yielded by the iterator.
/// - Erasing a bucket that has not yet been yielded by the iterator may still
///   result in the iterator yielding that bucket.
/// - It is unspecified whether an element inserted after the iterator was
///   created will be yielded by that iterator.
/// - The order in which the iterator yields buckets is unspecified and may
///   change in the future.
pub struct RawIterHash<T> {
    inner: RawIterHashInner,
    _marker: PhantomData<T>,
}

#[derive(Clone)]
struct RawIterHashInner {
    // See `RawTableInner`'s corresponding fields for details.
    // We can't store a `*const RawTableInner` as it would get
    // invalidated by the user calling `&mut` methods on `RawTable`.
    bucket_mask: usize,
    ctrl: NonNull<u8>,

    // The top 7 bits of the hash.
    tag_hash: Tag,

    // The sequence of groups to probe in the search.
    probe_seq: ProbeSeq,

    group: Group,

    // The elements within the group with a matching tag-hash.
    bitmask: BitMaskIter,
}

impl<T> RawIterHash<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn new<A: Allocator>(table: &RawTable<T, A>, hash: u64) -> Self {
        RawIterHash {
            inner: RawIterHashInner::new(&table.table, hash),
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for RawIterHash<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> Default for RawIterHash<T> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn default() -> Self {
        Self {
            // SAFETY: Because the table is static, it always outlives the iter.
            inner: unsafe { RawIterHashInner::new(&RawTableInner::NEW, 0) },
            _marker: PhantomData,
        }
    }
}

impl RawIterHashInner {
    #[cfg_attr(feature = "inline-more", inline)]
    unsafe fn new(table: &RawTableInner, hash: u64) -> Self {
        let tag_hash = Tag::full(hash);
        let probe_seq = table.probe_seq(hash);
        let group = Group::load(table.ctrl(probe_seq.pos));
        let bitmask = group.match_tag(tag_hash).into_iter();

        RawIterHashInner {
            bucket_mask: table.bucket_mask,
            ctrl: table.ctrl,
            tag_hash,
            probe_seq,
            group,
            bitmask,
        }
    }
}

impl<T> Iterator for RawIterHash<T> {
    type Item = Bucket<T>;

    fn next(&mut self) -> Option<Bucket<T>> {
        unsafe {
            match self.inner.next() {
                Some(index) => {
                    // Can't use `RawTable::bucket` here as we don't have
                    // an actual `RawTable` reference to use.
                    debug_assert!(index <= self.inner.bucket_mask);
                    let bucket = Bucket::from_base_index(self.inner.ctrl.cast(), index);
                    Some(bucket)
                }
                None => None,
            }
        }
    }
}

impl Iterator for RawIterHashInner {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                if let Some(bit) = self.bitmask.next() {
                    let index = (self.probe_seq.pos + bit) & self.bucket_mask;
                    return Some(index);
                }
                if likely(self.group.match_empty().any_bit_set()) {
                    return None;
                }
                self.probe_seq.move_next(self.bucket_mask);

                // Can't use `RawTableInner::ctrl` here as we don't have
                // an actual `RawTableInner` reference to use.
                let index = self.probe_seq.pos;
                debug_assert!(index < self.bucket_mask + 1 + Group::WIDTH);
                let group_ctrl = self.ctrl.as_ptr().add(index).cast();

                self.group = Group::load(group_ctrl);
                self.bitmask = self.group.match_tag(self.tag_hash).into_iter();
            }
        }
    }
}

pub(crate) struct RawExtractIf<'a, T, A: Allocator> {
    pub iter: RawIter<T>,
    pub table: &'a mut RawTable<T, A>,
}

impl<T, A: Allocator> RawExtractIf<'_, T, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) fn next<F>(&mut self, mut f: F) -> Option<T>
    where
        F: FnMut(&mut T) -> bool,
    {
        unsafe {
            for item in &mut self.iter {
                if f(item.as_mut()) {
                    return Some(self.table.remove(item).0);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod test_map {
    use super::*;

    #[test]
    fn test_minimum_capacity_for_small_types() {
        #[track_caller]
        fn test_t<T>() {
            let raw_table: RawTable<T> = RawTable::with_capacity(1);
            let actual_buckets = raw_table.buckets();
            let min_buckets = Group::WIDTH / core::mem::size_of::<T>();
            assert!(
                actual_buckets >= min_buckets,
                "expected at least {min_buckets} buckets, got {actual_buckets} buckets"
            );
        }

        test_t::<u8>();

        // This is only "small" for some platforms, like x86_64 with SSE2, but
        // there's no harm in running it on other platforms.
        test_t::<u16>();
    }

    fn rehash_in_place<T>(table: &mut RawTable<T>, hasher: impl Fn(&T) -> u64) {
        unsafe {
            table.table.rehash_in_place(
                &|table, index| hasher(table.bucket::<T>(index).as_ref()),
                mem::size_of::<T>(),
                if mem::needs_drop::<T>() {
                    Some(|ptr| ptr::drop_in_place(ptr as *mut T))
                } else {
                    None
                },
            );
        }
    }

    #[test]
    fn rehash() {
        let mut table = RawTable::new();
        let hasher = |i: &u64| *i;
        for i in 0..100 {
            table.insert(i, i, hasher);
        }

        for i in 0..100 {
            unsafe {
                assert_eq!(table.find(i, |x| *x == i).map(|b| b.read()), Some(i));
            }
            assert!(table.find(i + 100, |x| *x == i + 100).is_none());
        }

        rehash_in_place(&mut table, hasher);

        for i in 0..100 {
            unsafe {
                assert_eq!(table.find(i, |x| *x == i).map(|b| b.read()), Some(i));
            }
            assert!(table.find(i + 100, |x| *x == i + 100).is_none());
        }
    }

    /// CHECKING THAT WE ARE NOT TRYING TO READ THE MEMORY OF
    /// AN UNINITIALIZED TABLE DURING THE DROP
    #[test]
    fn test_drop_uninitialized() {
        use ::alloc::vec::Vec;

        let table = unsafe {
            // SAFETY: The `buckets` is power of two and we're not
            // trying to actually use the returned RawTable.
            RawTable::<(u64, Vec<i32>)>::new_uninitialized(Global, 8, Fallibility::Infallible)
                .unwrap()
        };
        drop(table);
    }

    /// CHECKING THAT WE DON'T TRY TO DROP DATA IF THE `ITEMS`
    /// ARE ZERO, EVEN IF WE HAVE `FULL` CONTROL BYTES.
    #[test]
    fn test_drop_zero_items() {
        use ::alloc::vec::Vec;
        unsafe {
            // SAFETY: The `buckets` is power of two and we're not
            // trying to actually use the returned RawTable.
            let mut table =
                RawTable::<(u64, Vec<i32>)>::new_uninitialized(Global, 8, Fallibility::Infallible)
                    .unwrap();

            // WE SIMULATE, AS IT WERE, A FULL TABLE.

            // SAFETY: We checked that the table is allocated and therefore the table already has
            // `self.bucket_mask + 1 + Group::WIDTH` number of control bytes (see TableLayout::calculate_layout_for)
            // so writing `table.table.num_ctrl_bytes() == bucket_mask + 1 + Group::WIDTH` bytes is safe.
            table.table.ctrl_slice().fill_empty();

            // SAFETY: table.capacity() is guaranteed to be smaller than table.buckets()
            table.table.ctrl(0).write_bytes(0, table.capacity());

            // Fix up the trailing control bytes. See the comments in set_ctrl
            // for the handling of tables smaller than the group width.
            if table.buckets() < Group::WIDTH {
                // SAFETY: We have `self.bucket_mask + 1 + Group::WIDTH` number of control bytes,
                // so copying `self.buckets() == self.bucket_mask + 1` bytes with offset equal to
                // `Group::WIDTH` is safe
                table
                    .table
                    .ctrl(0)
                    .copy_to(table.table.ctrl(Group::WIDTH), table.table.buckets());
            } else {
                // SAFETY: We have `self.bucket_mask + 1 + Group::WIDTH` number of
                // control bytes,so copying `Group::WIDTH` bytes with offset equal
                // to `self.buckets() == self.bucket_mask + 1` is safe
                table
                    .table
                    .ctrl(0)
                    .copy_to(table.table.ctrl(table.table.buckets()), Group::WIDTH);
            }
            drop(table);
        }
    }

    /// CHECKING THAT WE DON'T TRY TO DROP DATA IF THE `ITEMS`
    /// ARE ZERO, EVEN IF WE HAVE `FULL` CONTROL BYTES.
    #[test]
    fn test_catch_panic_clone_from() {
        use super::{AllocError, Allocator, Global};
        use ::alloc::sync::Arc;
        use ::alloc::vec::Vec;
        use core::sync::atomic::{AtomicI8, Ordering};
        use std::thread;

        struct MyAllocInner {
            drop_count: Arc<AtomicI8>,
        }

        #[derive(Clone)]
        struct MyAlloc {
            _inner: Arc<MyAllocInner>,
        }

        impl Drop for MyAllocInner {
            fn drop(&mut self) {
                println!("MyAlloc freed.");
                self.drop_count.fetch_sub(1, Ordering::SeqCst);
            }
        }

        unsafe impl Allocator for MyAlloc {
            fn allocate(&self, layout: Layout) -> std::result::Result<NonNull<[u8]>, AllocError> {
                let g = Global;
                g.allocate(layout)
            }

            unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
                let g = Global;
                g.deallocate(ptr, layout)
            }
        }

        const DISARMED: bool = false;
        const ARMED: bool = true;

        struct CheckedCloneDrop {
            panic_in_clone: bool,
            dropped: bool,
            need_drop: Vec<u64>,
        }

        impl Clone for CheckedCloneDrop {
            fn clone(&self) -> Self {
                if self.panic_in_clone {
                    panic!("panic in clone")
                }
                Self {
                    panic_in_clone: self.panic_in_clone,
                    dropped: self.dropped,
                    need_drop: self.need_drop.clone(),
                }
            }
        }

        impl Drop for CheckedCloneDrop {
            fn drop(&mut self) {
                if self.dropped {
                    panic!("double drop");
                }
                self.dropped = true;
            }
        }

        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(2));

        let mut table = RawTable::new_in(MyAlloc {
            _inner: Arc::new(MyAllocInner {
                drop_count: dropped.clone(),
            }),
        });

        for (idx, panic_in_clone) in core::iter::repeat(DISARMED).take(7).enumerate() {
            let idx = idx as u64;
            table.insert(
                idx,
                (
                    idx,
                    CheckedCloneDrop {
                        panic_in_clone,
                        dropped: false,
                        need_drop: vec![idx],
                    },
                ),
                |(k, _)| *k,
            );
        }

        assert_eq!(table.len(), 7);

        thread::scope(|s| {
            let result = s.spawn(|| {
                let armed_flags = [
                    DISARMED, DISARMED, ARMED, DISARMED, DISARMED, DISARMED, DISARMED,
                ];
                let mut scope_table = RawTable::new_in(MyAlloc {
                    _inner: Arc::new(MyAllocInner {
                        drop_count: dropped.clone(),
                    }),
                });
                for (idx, &panic_in_clone) in armed_flags.iter().enumerate() {
                    let idx = idx as u64;
                    scope_table.insert(
                        idx,
                        (
                            idx,
                            CheckedCloneDrop {
                                panic_in_clone,
                                dropped: false,
                                need_drop: vec![idx + 100],
                            },
                        ),
                        |(k, _)| *k,
                    );
                }
                table.clone_from(&scope_table);
            });
            assert!(result.join().is_err());
        });

        // Let's check that all iterators work fine and do not return elements
        // (especially `RawIterRange`, which does not depend on the number of
        // elements in the table, but looks directly at the control bytes)
        //
        // SAFETY: We know for sure that `RawTable` will outlive
        // the returned `RawIter / RawIterRange` iterator.
        assert_eq!(table.len(), 0);
        assert_eq!(unsafe { table.iter().count() }, 0);
        assert_eq!(unsafe { table.iter().iter.count() }, 0);

        for idx in 0..table.buckets() {
            let idx = idx as u64;
            assert!(
                table.find(idx, |(k, _)| *k == idx).is_none(),
                "Index: {idx}"
            );
        }

        // All allocator clones should already be dropped.
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
}
