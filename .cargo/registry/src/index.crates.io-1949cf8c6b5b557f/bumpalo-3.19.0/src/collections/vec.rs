// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//! A contiguous growable array type with heap-allocated contents, written
//! [`Vec<'bump, T>`].
//!
//! Vectors have `O(1)` indexing, amortized `O(1)` push (to the end) and
//! `O(1)` pop (from the end).
//!
//! This module is a fork of the [`std::vec`] module, that uses a bump allocator.
//!
//! [`std::vec`]: https://doc.rust-lang.org/std/vec/index.html
//!
//! # Examples
//!
//! You can explicitly create a [`Vec<'bump, T>`] with [`new_in`]:
//!
//! ```
//! use bumpalo::{Bump, collections::Vec};
//!
//! let b = Bump::new();
//! let v: Vec<i32> = Vec::new_in(&b);
//! ```
//!
//! ... or by using the [`vec!`] macro:
//!
//! ```
//! use bumpalo::{Bump, collections::Vec};
//!
//! let b = Bump::new();
//!
//! let v: Vec<i32> = bumpalo::vec![in &b];
//!
//! let v = bumpalo::vec![in &b; 1, 2, 3, 4, 5];
//!
//! let v = bumpalo::vec![in &b; 0; 10]; // ten zeroes
//! ```
//!
//! You can [`push`] values onto the end of a vector (which will grow the vector
//! as needed):
//!
//! ```
//! use bumpalo::{Bump, collections::Vec};
//!
//! let b = Bump::new();
//!
//! let mut v = bumpalo::vec![in &b; 1, 2];
//!
//! v.push(3);
//! ```
//!
//! Popping values works in much the same way:
//!
//! ```
//! use bumpalo::{Bump, collections::Vec};
//!
//! let b = Bump::new();
//!
//! let mut v = bumpalo::vec![in &b; 1, 2];
//!
//! assert_eq!(v.pop(), Some(2));
//! ```
//!
//! Vectors also support indexing (through the [`Index`] and [`IndexMut`] traits):
//!
//! ```
//! use bumpalo::{Bump, collections::Vec};
//!
//! let b = Bump::new();
//!
//! let mut v = bumpalo::vec![in &b; 1, 2, 3];
//! assert_eq!(v[2], 3);
//! v[1] += 5;
//! assert_eq!(v, [1, 7, 3]);
//! ```
//!
//! [`Vec<'bump, T>`]: struct.Vec.html
//! [`new_in`]: struct.Vec.html#method.new_in
//! [`push`]: struct.Vec.html#method.push
//! [`Index`]: https://doc.rust-lang.org/std/ops/trait.Index.html
//! [`IndexMut`]: https://doc.rust-lang.org/std/ops/trait.IndexMut.html
//! [`vec!`]: ../../macro.vec.html

use super::raw_vec::RawVec;
use crate::collections::CollectionAllocErr;
use crate::Bump;
use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt;
use core::hash::{self, Hash};
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::mem;
use core::ops;
use core::ops::Bound::{Excluded, Included, Unbounded};
use core::ops::{Index, IndexMut, RangeBounds};
use core::ptr;
use core::ptr::NonNull;
use core::slice;
#[cfg(feature = "std")]
use std::io;

unsafe fn arith_offset<T>(p: *const T, offset: isize) -> *const T {
    p.offset(offset)
}

fn partition_dedup_by<T, F>(s: &mut [T], mut same_bucket: F) -> (&mut [T], &mut [T])
where
    F: FnMut(&mut T, &mut T) -> bool,
{
    // Although we have a mutable reference to `s`, we cannot make
    // *arbitrary* changes. The `same_bucket` calls could panic, so we
    // must ensure that the slice is in a valid state at all times.
    //
    // The way that we handle this is by using swaps; we iterate
    // over all the elements, swapping as we go so that at the end
    // the elements we wish to keep are in the front, and those we
    // wish to reject are at the back. We can then split the slice.
    // This operation is still O(n).
    //
    // Example: We start in this state, where `r` represents "next
    // read" and `w` represents "next_write`.
    //
    //           r
    //     +---+---+---+---+---+---+
    //     | 0 | 1 | 1 | 2 | 3 | 3 |
    //     +---+---+---+---+---+---+
    //           w
    //
    // Comparing s[r] against s[w-1], this is not a duplicate, so
    // we swap s[r] and s[w] (no effect as r==w) and then increment both
    // r and w, leaving us with:
    //
    //               r
    //     +---+---+---+---+---+---+
    //     | 0 | 1 | 1 | 2 | 3 | 3 |
    //     +---+---+---+---+---+---+
    //               w
    //
    // Comparing s[r] against s[w-1], this value is a duplicate,
    // so we increment `r` but leave everything else unchanged:
    //
    //                   r
    //     +---+---+---+---+---+---+
    //     | 0 | 1 | 1 | 2 | 3 | 3 |
    //     +---+---+---+---+---+---+
    //               w
    //
    // Comparing s[r] against s[w-1], this is not a duplicate,
    // so swap s[r] and s[w] and advance r and w:
    //
    //                       r
    //     +---+---+---+---+---+---+
    //     | 0 | 1 | 2 | 1 | 3 | 3 |
    //     +---+---+---+---+---+---+
    //                   w
    //
    // Not a duplicate, repeat:
    //
    //                           r
    //     +---+---+---+---+---+---+
    //     | 0 | 1 | 2 | 3 | 1 | 3 |
    //     +---+---+---+---+---+---+
    //                       w
    //
    // Duplicate, advance r. End of slice. Split at w.

    let len = s.len();
    if len <= 1 {
        return (s, &mut []);
    }

    let ptr = s.as_mut_ptr();
    let mut next_read: usize = 1;
    let mut next_write: usize = 1;

    unsafe {
        // Avoid bounds checks by using raw pointers.
        while next_read < len {
            let ptr_read = ptr.add(next_read);
            let prev_ptr_write = ptr.add(next_write - 1);
            if !same_bucket(&mut *ptr_read, &mut *prev_ptr_write) {
                if next_read != next_write {
                    let ptr_write = prev_ptr_write.offset(1);
                    mem::swap(&mut *ptr_read, &mut *ptr_write);
                }
                next_write += 1;
            }
            next_read += 1;
        }
    }

    s.split_at_mut(next_write)
}

unsafe fn offset_from<T>(p: *const T, origin: *const T) -> isize
where
    T: Sized,
{
    let pointee_size = mem::size_of::<T>();
    assert!(0 < pointee_size && pointee_size <= isize::max_value() as usize);

    // This is the same sequence that Clang emits for pointer subtraction.
    // It can be neither `nsw` nor `nuw` because the input is treated as
    // unsigned but then the output is treated as signed, so neither works.
    let d = isize::wrapping_sub(p as _, origin as _);
    d / (pointee_size as isize)
}

/// Creates a [`Vec`] containing the arguments.
///
/// `vec!` allows `Vec`s to be defined with the same syntax as array expressions.
/// There are two forms of this macro:
///
/// - Create a [`Vec`] containing a given list of elements:
///
/// ```
/// use bumpalo::Bump;
///
/// let b = Bump::new();
/// let v = bumpalo::vec![in &b; 1, 2, 3];
/// assert_eq!(v, [1, 2, 3]);
/// ```
///
/// - Create a [`Vec`] from a given element and size:
///
/// ```
/// use bumpalo::Bump;
///
/// let b = Bump::new();
/// let v = bumpalo::vec![in &b; 1; 3];
/// assert_eq!(v, [1, 1, 1]);
/// ```
///
/// Note that unlike array expressions, this syntax supports all elements
/// which implement [`Clone`] and the number of elements doesn't have to be
/// a constant.
///
/// This will use `clone` to duplicate an expression, so one should be careful
/// using this with types having a non-standard `Clone` implementation. For
/// example, `bumpalo::vec![in &bump; Rc::new(1); 5]` will create a vector of five references
/// to the same boxed integer value, not five references pointing to independently
/// boxed integers.
///
/// [`Vec`]: collections/vec/struct.Vec.html
/// [`Clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html
#[macro_export]
macro_rules! vec {
    (in $bump:expr; $elem:expr; $n:expr) => {{
        let n = $n;
        let mut v = $crate::collections::Vec::with_capacity_in(n, $bump);
        if n > 0 {
            let elem = $elem;
            for _ in 0..n - 1 {
                v.push(elem.clone());
            }
            v.push(elem);
        }
        v
    }};
    (in $bump:expr) => { $crate::collections::Vec::new_in($bump) };
    (in $bump:expr; $($x:expr),*) => {{
        let mut v = $crate::collections::Vec::new_in($bump);
        $( v.push($x); )*
        v
    }};
    (in $bump:expr; $($x:expr,)*) => (bumpalo::vec![in $bump; $($x),*])
}

/// A contiguous growable array type, written `Vec<'bump, T>` but pronounced 'vector'.
///
/// # Examples
///
/// ```
/// use bumpalo::{Bump, collections::Vec};
///
/// let b = Bump::new();
///
/// let mut vec = Vec::new_in(&b);
/// vec.push(1);
/// vec.push(2);
///
/// assert_eq!(vec.len(), 2);
/// assert_eq!(vec[0], 1);
///
/// assert_eq!(vec.pop(), Some(2));
/// assert_eq!(vec.len(), 1);
///
/// vec[0] = 7;
/// assert_eq!(vec[0], 7);
///
/// vec.extend([1, 2, 3].iter().cloned());
///
/// for x in &vec {
///     println!("{}", x);
/// }
/// assert_eq!(vec, [7, 1, 2, 3]);
/// ```
///
/// The [`vec!`] macro is provided to make initialization more convenient:
///
/// ```
/// use bumpalo::{Bump, collections::Vec};
///
/// let b = Bump::new();
///
/// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
/// vec.push(4);
/// assert_eq!(vec, [1, 2, 3, 4]);
/// ```
///
/// It can also initialize each element of a `Vec<'bump, T>` with a given value.
/// This may be more efficient than performing allocation and initialization
/// in separate steps, especially when initializing a vector of zeros:
///
/// ```
/// use bumpalo::{Bump, collections::Vec};
///
/// let b = Bump::new();
///
/// let vec = bumpalo::vec![in &b; 0; 5];
/// assert_eq!(vec, [0, 0, 0, 0, 0]);
///
/// // The following is equivalent, but potentially slower:
/// let mut vec1 = Vec::with_capacity_in(5, &b);
/// vec1.resize(5, 0);
/// ```
///
/// Use a `Vec<'bump, T>` as an efficient stack:
///
/// ```
/// use bumpalo::{Bump, collections::Vec};
///
/// let b = Bump::new();
///
/// let mut stack = Vec::new_in(&b);
///
/// stack.push(1);
/// stack.push(2);
/// stack.push(3);
///
/// while let Some(top) = stack.pop() {
///     // Prints 3, 2, 1
///     println!("{}", top);
/// }
/// ```
///
/// # Indexing
///
/// The `Vec` type allows to access values by index, because it implements the
/// [`Index`] trait. An example will be more explicit:
///
/// ```
/// use bumpalo::{Bump, collections::Vec};
///
/// let b = Bump::new();
///
/// let v = bumpalo::vec![in &b; 0, 2, 4, 6];
/// println!("{}", v[1]); // it will display '2'
/// ```
///
/// However be careful: if you try to access an index which isn't in the `Vec`,
/// your software will panic! You cannot do this:
///
/// ```should_panic
/// use bumpalo::{Bump, collections::Vec};
///
/// let b = Bump::new();
///
/// let v = bumpalo::vec![in &b; 0, 2, 4, 6];
/// println!("{}", v[6]); // it will panic!
/// ```
///
/// In conclusion: always check if the index you want to get really exists
/// before doing it.
///
/// # Slicing
///
/// A `Vec` can be mutable. Slices, on the other hand, are read-only objects.
/// To get a slice, use `&`. Example:
///
/// ```
/// use bumpalo::{Bump, collections::Vec};
///
/// fn read_slice(slice: &[usize]) {
///     // ...
/// }
///
/// let b = Bump::new();
///
/// let v = bumpalo::vec![in &b; 0, 1];
/// read_slice(&v);
///
/// // ... and that's all!
/// // you can also do it like this:
/// let x : &[usize] = &v;
/// ```
///
/// In Rust, it's more common to pass slices as arguments rather than vectors
/// when you just want to provide a read access. The same goes for [`String`] and
/// [`&str`].
///
/// # Capacity and reallocation
///
/// The capacity of a vector is the amount of space allocated for any future
/// elements that will be added onto the vector. This is not to be confused with
/// the *length* of a vector, which specifies the number of actual elements
/// within the vector. If a vector's length exceeds its capacity, its capacity
/// will automatically be increased, but its elements will have to be
/// reallocated.
///
/// For example, a vector with capacity 10 and length 0 would be an empty vector
/// with space for 10 more elements. Pushing 10 or fewer elements onto the
/// vector will not change its capacity or cause reallocation to occur. However,
/// if the vector's length is increased to 11, it will have to reallocate, which
/// can be slow. For this reason, it is recommended to use [`Vec::with_capacity_in`]
/// whenever possible to specify how big the vector is expected to get.
///
/// # Guarantees
///
/// Due to its incredibly fundamental nature, `Vec` makes a lot of guarantees
/// about its design. This ensures that it's as low-overhead as possible in
/// the general case, and can be correctly manipulated in primitive ways
/// by unsafe code. Note that these guarantees refer to an unqualified `Vec<'bump, T>`.
/// If additional type parameters are added (e.g. to support custom allocators),
/// overriding their defaults may change the behavior.
///
/// Most fundamentally, `Vec` is and always will be a (pointer, capacity, length)
/// triplet. No more, no less. The order of these fields is completely
/// unspecified, and you should use the appropriate methods to modify these.
/// The pointer will never be null, so this type is null-pointer-optimized.
///
/// However, the pointer may not actually point to allocated memory. In particular,
/// if you construct a `Vec` with capacity 0 via [`Vec::new_in`], [`bumpalo::vec![in bump]`][`vec!`],
/// [`Vec::with_capacity_in(0)`][`Vec::with_capacity_in`], or by calling [`shrink_to_fit`]
/// on an empty Vec, it will not allocate memory. Similarly, if you store zero-sized
/// types inside a `Vec`, it will not allocate space for them. *Note that in this case
/// the `Vec` may not report a [`capacity`] of 0*. `Vec` will allocate if and only
/// if <code>[`mem::size_of::<T>`]\() * capacity() > 0</code>. In general, `Vec`'s allocation
/// details are very subtle &mdash; if you intend to allocate memory using a `Vec`
/// and use it for something else (either to pass to unsafe code, or to build your
/// own memory-backed collection), be sure to deallocate this memory by using
/// `from_raw_parts` to recover the `Vec` and then dropping it.
///
/// If a `Vec` *has* allocated memory, then the memory it points to is
/// in the [`Bump`] arena used to construct it, and its
/// pointer points to [`len`] initialized, contiguous elements in order (what
/// you would see if you coerced it to a slice), followed by <code>[`capacity`] -
/// [`len`]</code> logically uninitialized, contiguous elements.
///
/// `Vec` will never perform a "small optimization" where elements are actually
/// stored on the stack for two reasons:
///
/// * It would make it more difficult for unsafe code to correctly manipulate
///   a `Vec`. The contents of a `Vec` wouldn't have a stable address if it were
///   only moved, and it would be more difficult to determine if a `Vec` had
///   actually allocated memory.
///
/// * It would penalize the general case, incurring an additional branch
///   on every access.
///
/// `Vec` will never automatically shrink itself, even if completely empty. This
/// ensures no unnecessary allocations or deallocations occur. Emptying a `Vec`
/// and then filling it back up to the same [`len`] should incur no calls to
/// the allocator. If you wish to free up unused memory, use
/// [`shrink_to_fit`][`shrink_to_fit`].
///
/// [`push`] and [`insert`] will never (re)allocate if the reported capacity is
/// sufficient. [`push`] and [`insert`] *will* (re)allocate if
/// <code>[`len`] == [`capacity`]</code>. That is, the reported capacity is completely
/// accurate, and can be relied on. It can even be used to manually free the memory
/// allocated by a `Vec` if desired. Bulk insertion methods *may* reallocate, even
/// when not necessary.
///
/// `Vec` does not guarantee any particular growth strategy when reallocating
/// when full, nor when [`reserve`] is called. The current strategy is basic
/// and it may prove desirable to use a non-constant growth factor. Whatever
/// strategy is used will of course guarantee `O(1)` amortized [`push`].
///
/// `bumpalo::vec![in bump; x; n]`, `bumpalo::vec![in bump; a, b, c, d]`, and
/// [`Vec::with_capacity_in(n)`][`Vec::with_capacity_in`], will all produce a
/// `Vec` with exactly the requested capacity. If <code>[`len`] == [`capacity`]</code>, (as
/// is the case for the [`vec!`] macro), then a `Vec<'bump, T>` can be converted
/// to and from a [`Box<[T]>`][owned slice] without reallocating or moving the
/// elements.
///
/// `Vec` will not specifically overwrite any data that is removed from it,
/// but also won't specifically preserve it. Its uninitialized memory is
/// scratch space that it may use however it wants. It will generally just do
/// whatever is most efficient or otherwise easy to implement. Do not rely on
/// removed data to be erased for security purposes. Even if you drop a `Vec`, its
/// buffer may simply be reused by another `Vec`. Even if you zero a `Vec`'s memory
/// first, that may not actually happen because the optimizer does not consider
/// this a side-effect that must be preserved. There is one case which we will
/// not break, however: using `unsafe` code to write to the excess capacity,
/// and then increasing the length to match, is always valid.
///
/// `Vec` does not currently guarantee the order in which elements are dropped.
/// The order has changed in the past and may change again.
///
/// [`vec!`]: ../../macro.vec.html
/// [`Index`]: https://doc.rust-lang.org/std/ops/trait.Index.html
/// [`String`]: ../string/struct.String.html
/// [`&str`]: https://doc.rust-lang.org/std/primitive.str.html
/// [`Vec::with_capacity_in`]: struct.Vec.html#method.with_capacity_in
/// [`Vec::new_in`]: struct.Vec.html#method.new_in
/// [`shrink_to_fit`]: struct.Vec.html#method.shrink_to_fit
/// [`capacity`]: struct.Vec.html#method.capacity
/// [`mem::size_of::<T>`]: https://doc.rust-lang.org/std/mem/fn.size_of.html
/// [`len`]: struct.Vec.html#method.len
/// [`push`]: struct.Vec.html#method.push
/// [`insert`]: struct.Vec.html#method.insert
/// [`reserve`]: struct.Vec.html#method.reserve
/// [owned slice]: https://doc.rust-lang.org/std/boxed/struct.Box.html
pub struct Vec<'bump, T: 'bump> {
    buf: RawVec<'bump, T>,
    len: usize,
}

////////////////////////////////////////////////////////////////////////////////
// Inherent methods
////////////////////////////////////////////////////////////////////////////////

impl<'bump, T: 'bump> Vec<'bump, T> {
    /// Constructs a new, empty `Vec<'bump, T>`.
    ///
    /// The vector will not allocate until elements are pushed onto it.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let mut vec: Vec<i32> = Vec::new_in(&b);
    /// ```
    #[inline]
    pub fn new_in(bump: &'bump Bump) -> Vec<'bump, T> {
        Vec {
            buf: RawVec::new_in(bump),
            len: 0,
        }
    }

    /// Constructs a new, empty `Vec<'bump, T>` with the specified capacity.
    ///
    /// The vector will be able to hold exactly `capacity` elements without
    /// reallocating. If `capacity` is 0, the vector will not allocate.
    ///
    /// It is important to note that although the returned vector has the
    /// *capacity* specified, the vector will have a zero *length*. For an
    /// explanation of the difference between length and capacity, see
    /// *[Capacity and reallocation]*.
    ///
    /// [Capacity and reallocation]: #capacity-and-reallocation
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = Vec::with_capacity_in(10, &b);
    ///
    /// // The vector contains no items, even though it has capacity for more
    /// assert_eq!(vec.len(), 0);
    ///
    /// // These are all done without reallocating...
    /// for i in 0..10 {
    ///     vec.push(i);
    /// }
    ///
    /// // ...but this may make the vector reallocate
    /// vec.push(11);
    /// ```
    #[inline]
    pub fn with_capacity_in(capacity: usize, bump: &'bump Bump) -> Vec<'bump, T> {
        Vec {
            buf: RawVec::with_capacity_in(capacity, bump),
            len: 0,
        }
    }

    /// Construct a new `Vec` from the given iterator's items.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    /// use std::iter;
    ///
    /// let b = Bump::new();
    /// let v = Vec::from_iter_in(iter::repeat(7).take(3), &b);
    /// assert_eq!(v, [7, 7, 7]);
    /// ```
    pub fn from_iter_in<I: IntoIterator<Item = T>>(iter: I, bump: &'bump Bump) -> Vec<'bump, T> {
        let mut v = Vec::new_in(bump);
        v.extend(iter);
        v
    }

    /// Creates a `Vec<'bump, T>` directly from the raw components of another vector.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * `ptr` needs to have been previously allocated via [`String`]/`Vec<'bump, T>`
    ///   (at least, it's highly likely to be incorrect if it wasn't).
    /// * `ptr`'s `T` needs to have the same size and alignment as it was allocated with.
    /// * `length` needs to be less than or equal to `capacity`.
    /// * `capacity` needs to be the capacity that the pointer was allocated with.
    ///
    /// Violating these may cause problems like corrupting the allocator's
    /// internal data structures. For example it is **not** safe
    /// to build a `Vec<u8>` from a pointer to a C `char` array and a `size_t`.
    ///
    /// The ownership of `ptr` is effectively transferred to the
    /// `Vec<'bump, T>` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// [`String`]: ../string/struct.String.html
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// use std::ptr;
    /// use std::mem;
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = bumpalo::vec![in &b; 1, 2, 3];
    ///
    /// // Pull out the various important pieces of information about `v`
    /// let p = v.as_mut_ptr();
    /// let len = v.len();
    /// let cap = v.capacity();
    ///
    /// unsafe {
    ///     // Cast `v` into the void: no destructor run, so we are in
    ///     // complete control of the allocation to which `p` points.
    ///     mem::forget(v);
    ///
    ///     // Overwrite memory with 4, 5, 6
    ///     for i in 0..len as isize {
    ///         ptr::write(p.offset(i), 4 + i);
    ///     }
    ///
    ///     // Put everything back together into a Vec
    ///     let rebuilt = Vec::from_raw_parts_in(p, len, cap, &b);
    ///     assert_eq!(rebuilt, [4, 5, 6]);
    /// }
    /// ```
    pub unsafe fn from_raw_parts_in(
        ptr: *mut T,
        length: usize,
        capacity: usize,
        bump: &'bump Bump,
    ) -> Vec<'bump, T> {
        Vec {
            buf: RawVec::from_raw_parts_in(ptr, capacity, bump),
            len: length,
        }
    }

    /// Returns a shared reference to the allocator backing this `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// // uses the same allocator as the provided `Vec`
    /// fn add_strings<'bump>(vec: &mut Vec<'bump, &'bump str>) {
    ///     for string in ["foo", "bar", "baz"] {
    ///         vec.push(vec.bump().alloc_str(string));
    ///     }
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn bump(&self) -> &'bump Bump {
        self.buf.bump()
    }

    /// Returns the number of elements the vector can hold without
    /// reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let vec: Vec<i32> = Vec::with_capacity_in(10, &b);
    /// assert_eq!(vec.capacity(), 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.cap()
    }

    /// Reserves capacity for at least `additional` more elements to be inserted
    /// in the given `Vec<'bump, T>`. The collection may reserve more space to avoid
    /// frequent reallocations. After calling `reserve`, capacity will be
    /// greater than or equal to `self.len() + additional`. Does nothing if
    /// capacity is already sufficient.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.reserve(10);
    /// assert!(vec.capacity() >= 11);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(self.len, additional);
    }

    /// Reserves the minimum capacity for exactly `additional` more elements to
    /// be inserted in the given `Vec<'bump, T>`. After calling `reserve_exact`,
    /// capacity will be greater than or equal to `self.len() + additional`.
    /// Does nothing if the capacity is already sufficient.
    ///
    /// Note that the allocator may give the collection more space than it
    /// requests. Therefore capacity can not be relied upon to be precisely
    /// minimal. Prefer `reserve` if future insertions are expected.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.reserve_exact(10);
    /// assert!(vec.capacity() >= 11);
    /// ```
    pub fn reserve_exact(&mut self, additional: usize) {
        self.buf.reserve_exact(self.len, additional);
    }

    /// Attempts to reserve capacity for at least `additional` more elements to be inserted
    /// in the given `Vec<'bump, T>`. The collection may reserve more space to avoid
    /// frequent reallocations. After calling `try_reserve`, capacity will be
    /// greater than or equal to `self.len() + additional`. Does nothing if
    /// capacity is already sufficient.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.try_reserve(10).unwrap();
    /// assert!(vec.capacity() >= 11);
    /// ```
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), CollectionAllocErr> {
        self.buf.try_reserve(self.len, additional)
    }

    /// Attempts to reserve the minimum capacity for exactly `additional` more elements to
    /// be inserted in the given `Vec<'bump, T>`. After calling `try_reserve_exact`,
    /// capacity will be greater than or equal to `self.len() + additional`.
    /// Does nothing if the capacity is already sufficient.
    ///
    /// Note that the allocator may give the collection more space than it
    /// requests. Therefore capacity can not be relied upon to be precisely
    /// minimal. Prefer `try_reserve` if future insertions are expected.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.try_reserve_exact(10).unwrap();
    /// assert!(vec.capacity() >= 11);
    /// ```
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), CollectionAllocErr> {
        self.buf.try_reserve_exact(self.len, additional)
    }

    /// Shrinks the capacity of the vector as much as possible.
    ///
    /// It will drop down as close as possible to the length but the allocator
    /// may still inform the vector that there is space for a few more elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = Vec::with_capacity_in(10, &b);
    /// vec.extend([1, 2, 3].iter().cloned());
    /// assert_eq!(vec.capacity(), 10);
    /// vec.shrink_to_fit();
    /// assert!(vec.capacity() >= 3);
    /// ```
    pub fn shrink_to_fit(&mut self) {
        if self.capacity() != self.len {
            self.buf.shrink_to_fit(self.len);
        }
    }

    /// Converts the vector into `&'bump [T]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let v = bumpalo::vec![in &b; 1, 2, 3];
    ///
    /// let slice = v.into_bump_slice();
    /// assert_eq!(slice, [1, 2, 3]);
    /// ```
    pub fn into_bump_slice(self) -> &'bump [T] {
        unsafe {
            let ptr = self.as_ptr();
            let len = self.len();
            mem::forget(self);
            slice::from_raw_parts(ptr, len)
        }
    }

    /// Converts the vector into `&'bump mut [T]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    /// let v = bumpalo::vec![in &b; 1, 2, 3];
    ///
    /// let mut slice = v.into_bump_slice_mut();
    ///
    /// slice[0] = 3;
    /// slice[2] = 1;
    ///
    /// assert_eq!(slice, [3, 2, 1]);
    /// ```
    pub fn into_bump_slice_mut(mut self) -> &'bump mut [T] {
        let ptr = self.as_mut_ptr();
        let len = self.len();
        mem::forget(self);

        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }

    /// Shortens the vector, keeping the first `len` elements and dropping
    /// the rest.
    ///
    /// If `len` is greater than the vector's current length, this has no
    /// effect.
    ///
    /// The [`drain`] method can emulate `truncate`, but causes the excess
    /// elements to be returned instead of dropped.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the vector.
    ///
    /// # Examples
    ///
    /// Truncating a five element vector to two elements:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3, 4, 5];
    /// vec.truncate(2);
    /// assert_eq!(vec, [1, 2]);
    /// ```
    ///
    /// No truncation occurs when `len` is greater than the vector's current
    /// length:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
    /// vec.truncate(8);
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    ///
    /// Truncating when `len == 0` is equivalent to calling the [`clear`]
    /// method.
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
    /// vec.truncate(0);
    /// assert_eq!(vec, []);
    /// ```
    ///
    /// [`clear`]: #method.clear
    /// [`drain`]: #method.drain
    pub fn truncate(&mut self, len: usize) {
        let current_len = self.len;
        unsafe {
            let mut ptr = self.as_mut_ptr().add(self.len);
            // Set the final length at the end, keeping in mind that
            // dropping an element might panic. Works around a missed
            // optimization, as seen in the following issue:
            // https://github.com/rust-lang/rust/issues/51802
            let mut local_len = SetLenOnDrop::new(&mut self.len);

            // drop any extra elements
            for _ in len..current_len {
                local_len.decrement_len(1);
                ptr = ptr.offset(-1);
                ptr::drop_in_place(ptr);
            }
        }
    }

    /// Extracts a slice containing the entire vector.
    ///
    /// Equivalent to `&s[..]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    /// use std::io::{self, Write};
    ///
    /// let b = Bump::new();
    ///
    /// let buffer = bumpalo::vec![in &b; 1, 2, 3, 5, 8];
    /// io::sink().write(buffer.as_slice()).unwrap();
    /// ```
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self
    }

    /// Extracts a mutable slice of the entire vector.
    ///
    /// Equivalent to `&mut s[..]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    /// use std::io::{self, Read};
    ///
    /// let b = Bump::new();
    /// let mut buffer = bumpalo::vec![in &b; 0; 3];
    /// io::repeat(0b101).read_exact(buffer.as_mut_slice()).unwrap();
    /// ```
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    /// Returns a raw pointer to the vector's buffer, or a dangling raw pointer
    /// valid for zero sized reads if the vector didn't allocate.
    ///
    /// The caller must ensure that the vector outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    /// Modifying the vector may cause its buffer to be reallocated,
    /// which would also make any pointers to it invalid.
    ///
    /// The caller must also ensure that the memory the pointer (non-transitively) points to
    /// is never written to (except inside an `UnsafeCell`) using this pointer or any pointer
    /// derived from it. If you need to mutate the contents of the slice, use [`as_mut_ptr`].
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let bump = Bump::new();
    ///
    /// let x = bumpalo::vec![in &bump; 1, 2, 4];
    /// let x_ptr = x.as_ptr();
    ///
    /// unsafe {
    ///     for i in 0..x.len() {
    ///         assert_eq!(*x_ptr.add(i), 1 << i);
    ///     }
    /// }
    /// ```
    ///
    /// [`as_mut_ptr`]: Vec::as_mut_ptr
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        // We shadow the slice method of the same name to avoid going through
        // `deref`, which creates an intermediate reference.
        let ptr = self.buf.ptr();
        unsafe {
            if ptr.is_null() {
                core::hint::unreachable_unchecked();
            }
        }
        ptr
    }

    /// Returns an unsafe mutable pointer to the vector's buffer, or a dangling
    /// raw pointer valid for zero sized reads if the vector didn't allocate.
    ///
    /// The caller must ensure that the vector outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    /// Modifying the vector may cause its buffer to be reallocated,
    /// which would also make any pointers to it invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let bump = Bump::new();
    ///
    /// // Allocate vector big enough for 4 elements.
    /// let size = 4;
    /// let mut x: Vec<i32> = Vec::with_capacity_in(size, &bump);
    /// let x_ptr = x.as_mut_ptr();
    ///
    /// // Initialize elements via raw pointer writes, then set length.
    /// unsafe {
    ///     for i in 0..size {
    ///         x_ptr.add(i).write(i as i32);
    ///     }
    ///     x.set_len(size);
    /// }
    /// assert_eq!(&*x, &[0, 1, 2, 3]);
    /// ```
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        // We shadow the slice method of the same name to avoid going through
        // `deref_mut`, which creates an intermediate reference.
        let ptr = self.buf.ptr();
        unsafe {
            if ptr.is_null() {
                core::hint::unreachable_unchecked();
            }
        }
        ptr
    }

    /// Sets the length of a vector.
    ///
    /// This will explicitly set the size of the vector, without actually
    /// modifying its buffers, so it is up to the caller to ensure that the
    /// vector is actually the specified size.
    ///
    /// # Safety
    ///
    /// - `new_len` must be less than or equal to [`capacity()`].
    /// - The elements at `old_len..new_len` must be initialized.
    ///
    /// [`capacity()`]: struct.Vec.html#method.capacity
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// use std::ptr;
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 'r', 'u', 's', 't'];
    ///
    /// unsafe {
    ///     ptr::drop_in_place(&mut vec[3]);
    ///     vec.set_len(3);
    /// }
    /// assert_eq!(vec, ['r', 'u', 's']);
    /// ```
    ///
    /// In this example, there is a memory leak since the memory locations
    /// owned by the inner vectors were not freed prior to the `set_len` call:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b;
    ///                             bumpalo::vec![in &b; 1, 0, 0],
    ///                             bumpalo::vec![in &b; 0, 1, 0],
    ///                             bumpalo::vec![in &b; 0, 0, 1]];
    /// unsafe {
    ///     vec.set_len(0);
    /// }
    /// ```
    ///
    /// In this example, the vector gets expanded from zero to four items
    /// but we directly initialize uninitialized memory:
    ///
    // TODO: rely upon `spare_capacity_mut`
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let len = 4;
    /// let b = Bump::new();
    ///
    /// let mut vec: Vec<u8> = Vec::with_capacity_in(len, &b);
    ///
    /// for i in 0..len {
    ///     // SAFETY: we initialize memory via `pointer::write`
    ///     unsafe { vec.as_mut_ptr().add(i).write(b'a') }
    /// }
    ///
    /// unsafe {
    ///     vec.set_len(len);
    /// }
    ///
    /// assert_eq!(b"aaaa", &*vec);
    /// ```
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.len = new_len;
    }

    /// Removes an element from the vector and returns it.
    ///
    /// The removed element is replaced by the last element of the vector.
    ///
    /// This does not preserve ordering, but is O(1).
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = bumpalo::vec![in &b; "foo", "bar", "baz", "qux"];
    ///
    /// assert_eq!(v.swap_remove(1), "bar");
    /// assert_eq!(v, ["foo", "qux", "baz"]);
    ///
    /// assert_eq!(v.swap_remove(0), "foo");
    /// assert_eq!(v, ["baz", "qux"]);
    /// ```
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> T {
        unsafe {
            // We replace self[index] with the last element. Note that if the
            // bounds check on hole succeeds there must be a last element (which
            // can be self[index] itself).
            let hole: *mut T = &mut self[index];
            let last = ptr::read(self.get_unchecked(self.len - 1));
            self.len -= 1;
            ptr::replace(hole, last)
        }
    }

    /// Inserts an element at position `index` within the vector, shifting all
    /// elements after it to the right.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
    /// vec.insert(1, 4);
    /// assert_eq!(vec, [1, 4, 2, 3]);
    /// vec.insert(4, 5);
    /// assert_eq!(vec, [1, 4, 2, 3, 5]);
    /// ```
    pub fn insert(&mut self, index: usize, element: T) {
        let len = self.len();
        assert!(index <= len);

        // space for the new element
        if len == self.buf.cap() {
            self.reserve(1);
        }

        unsafe {
            // infallible
            // The spot to put the new value
            {
                let p = self.as_mut_ptr().add(index);
                // Shift everything over to make space. (Duplicating the
                // `index`th element into two consecutive places.)
                ptr::copy(p, p.offset(1), len - index);
                // Write it in, overwriting the first copy of the `index`th
                // element.
                ptr::write(p, element);
            }
            self.set_len(len + 1);
        }
    }

    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = bumpalo::vec![in &b; 1, 2, 3];
    /// assert_eq!(v.remove(1), 2);
    /// assert_eq!(v, [1, 3]);
    /// ```
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.len();
        assert!(index < len);
        unsafe {
            // infallible
            let ret;
            {
                // the place we are taking from.
                let ptr = self.as_mut_ptr().add(index);
                // copy it out, unsafely having a copy of the value on
                // the stack and in the vector at the same time.
                ret = ptr::read(ptr);

                // Shift everything down to fill in that spot.
                ptr::copy(ptr.offset(1), ptr, len - index - 1);
            }
            self.set_len(len - 1);
            ret
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`.
    /// This method operates in place and preserves the order of the retained
    /// elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3, 4];
    /// vec.retain(|x: &i32| *x % 2 == 0);
    /// assert_eq!(vec, [2, 4]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.drain_filter(|x| !f(x));
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&mut e)` returns `false`.
    /// This method operates in place and preserves the order of the retained
    /// elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3, 4];
    /// vec.retain_mut(|x: &mut i32| *x % 2 == 0);
    /// assert_eq!(vec, [2, 4]);
    /// ```
    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.drain_filter(|x| !f(x));
    }

    /// Creates an iterator that removes the elements in the vector
    /// for which the predicate returns `true` and yields the removed items.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::Bump;
    /// use bumpalo::collections::{CollectIn, Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut numbers = bumpalo::vec![in &b; 1, 2, 3, 4, 5];
    ///
    /// let evens: Vec<_> = numbers.drain_filter(|x| *x % 2 == 0).collect_in(&b);
    ///
    /// assert_eq!(numbers, &[1, 3, 5]);
    /// assert_eq!(evens, &[2, 4]);
    /// ```
    pub fn drain_filter<'a, F>(&'a mut self, filter: F) -> DrainFilter<'a, 'bump, T, F>
    where
        F: FnMut(&mut T) -> bool,
    {
        let old_len = self.len();

        // Guard against us getting leaked (leak amplification)
        unsafe {
            self.set_len(0);
        }

        DrainFilter {
            vec: self,
            idx: 0,
            del: 0,
            old_len,
            pred: filter,
        }
    }

    /// Removes all but the first of consecutive elements in the vector that resolve to the same
    /// key.
    ///
    /// If the vector is sorted, this removes all duplicates.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 10, 20, 21, 30, 20];
    ///
    /// vec.dedup_by_key(|i| *i / 10);
    ///
    /// assert_eq!(vec, [10, 20, 30, 20]);
    /// ```
    #[inline]
    pub fn dedup_by_key<F, K>(&mut self, mut key: F)
    where
        F: FnMut(&mut T) -> K,
        K: PartialEq,
    {
        self.dedup_by(|a, b| key(a) == key(b))
    }

    /// Removes all but the first of consecutive elements in the vector satisfying a given equality
    /// relation.
    ///
    /// The `same_bucket` function is passed references to two elements from the vector and
    /// must determine if the elements compare equal. The elements are passed in opposite order
    /// from their order in the slice, so if `same_bucket(a, b)` returns `true`, `a` is removed.
    ///
    /// If the vector is sorted, this removes all duplicates.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; "foo", "bar", "Bar", "baz", "bar"];
    ///
    /// vec.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    ///
    /// assert_eq!(vec, ["foo", "bar", "baz", "bar"]);
    /// ```
    pub fn dedup_by<F>(&mut self, same_bucket: F)
    where
        F: FnMut(&mut T, &mut T) -> bool,
    {
        let len = {
            let (dedup, _) = partition_dedup_by(self.as_mut_slice(), same_bucket);
            dedup.len()
        };
        self.truncate(len);
    }

    // Proven specification with verus, converted to comments.
    /// # Preconditions
    ///
    /// - old(self).len() < old(self).capacity(),
    ///
    /// # Postconditions
    ///
    /// - self.get_unchecked(old(self).len()) == value,
    /// - self.len()      == old(self).len() + 1,
    /// - self.capacity() == old(self).capacity(),
    /// - forall|i: usize| implies(
    ///       i < old(self).len(),
    ///       self.get_unchecked(i) == old(self).get_unchecked(i)
    ///   )
    #[allow(clippy::inline_always)]
    #[inline(always)]
    unsafe fn push_unchecked(&mut self, value: T) {
        debug_assert!(self.len() < self.capacity());

        // Divergence from verified impl:
        //   Verified implementation has special handling for ZSTs
        //   as ZSTs do not play nicely with separation logic.
        ptr::write(self.buf.ptr().add(self.len), value);

        self.len = self.len + 1;
    }

    /// Appends an element to the back of a vector.
    ///
    /// # Panics
    ///
    /// Panics if the number of elements in the vector overflows a `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2];
    /// vec.push(3);
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    #[inline]
    pub fn push(&mut self, value: T) {
        // This will panic or abort if we would allocate > isize::MAX bytes
        // or if the length increment would overflow for zero-sized types.
        if self.len == self.buf.cap() {
            self.reserve(1);
        }
        unsafe {
            self.push_unchecked(value);
        }
    }

    /// Removes the last element from a vector and returns it, or [`None`] if it
    /// is empty.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
    /// assert_eq!(vec.pop(), Some(3));
    /// assert_eq!(vec, [1, 2]);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                self.len -= 1;
                Some(ptr::read(self.as_ptr().add(self.len())))
            }
        }
    }

    /// Moves all the elements of `other` into `Self`, leaving `other` empty.
    ///
    /// # Panics
    ///
    /// Panics if the number of elements in the vector overflows a `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
    /// let mut vec2 = bumpalo::vec![in &b; 4, 5, 6];
    /// vec.append(&mut vec2);
    /// assert_eq!(vec, [1, 2, 3, 4, 5, 6]);
    /// assert_eq!(vec2, []);
    /// ```
    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        unsafe {
            self.append_elements(other.as_slice() as _);
            other.set_len(0);
        }
    }

    /// Appends elements to `Self` from other buffer.
    #[inline]
    unsafe fn append_elements(&mut self, other: *const [T]) {
        let count = (&(*other)).len();
        self.reserve(count);
        let len = self.len();
        ptr::copy_nonoverlapping(other as *const T, self.as_mut_ptr().add(len), count);
        self.len += count;
    }

    /// Creates a draining iterator that removes the specified range in the vector
    /// and yields the removed items.
    ///
    /// Note 1: The element range is removed even if the iterator is only
    /// partially consumed or not consumed at all.
    ///
    /// Note 2: It is unspecified how many elements are removed from the vector
    /// if the `Drain` value is leaked.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::Bump;
    /// use bumpalo::collections::{CollectIn, Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = bumpalo::vec![in &b; 1, 2, 3];
    ///
    /// let u: Vec<_> = v.drain(1..).collect_in(&b);
    ///
    /// assert_eq!(v, &[1]);
    /// assert_eq!(u, &[2, 3]);
    ///
    /// // A full range clears the vector
    /// v.drain(..);
    /// assert_eq!(v, &[]);
    /// ```
    pub fn drain<R>(&mut self, range: R) -> Drain<T>
    where
        R: RangeBounds<usize>,
    {
        // Memory safety
        //
        // When the Drain is first created, it shortens the length of
        // the source vector to make sure no uninitialized or moved-from elements
        // are accessible at all if the Drain's destructor never gets to run.
        //
        // Drain will ptr::read out the values to remove.
        // When finished, remaining tail of the vec is copied back to cover
        // the hole, and the vector length is restored to the new length.
        //
        let len = self.len();
        let start = match range.start_bound() {
            Included(&n) => n,
            Excluded(&n) => n + 1,
            Unbounded => 0,
        };
        let end = match range.end_bound() {
            Included(&n) => n + 1,
            Excluded(&n) => n,
            Unbounded => len,
        };
        assert!(start <= end);
        assert!(end <= len);

        unsafe {
            // set self.vec length's to start, to be safe in case Drain is leaked
            self.set_len(start);
            // Use the borrow in the IterMut to indicate borrowing behavior of the
            // whole Drain iterator (like &mut T).
            let range_slice = slice::from_raw_parts_mut(self.as_mut_ptr().add(start), end - start);
            Drain {
                tail_start: end,
                tail_len: len - end,
                iter: range_slice.iter(),
                vec: NonNull::from(self),
            }
        }
    }

    /// Clears the vector, removing all values.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = bumpalo::vec![in &b; 1, 2, 3];
    ///
    /// v.clear();
    ///
    /// assert!(v.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0)
    }

    /// Returns the number of elements in the vector, also referred to
    /// as its 'length'.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let a = bumpalo::vec![in &b; 1, 2, 3];
    /// assert_eq!(a.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the vector contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = Vec::new_in(&b);
    /// assert!(v.is_empty());
    ///
    /// v.push(1);
    /// assert!(!v.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated vector. `self` contains elements `[0, at)`,
    /// and the returned vector contains elements `[at, len)`.
    ///
    /// Note that the capacity of `self` does not change.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3];
    /// let vec2 = vec.split_off(1);
    /// assert_eq!(vec, [1]);
    /// assert_eq!(vec2, [2, 3]);
    /// ```
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        assert!(at <= self.len(), "`at` out of bounds");

        let other_len = self.len - at;
        let mut other = Vec::with_capacity_in(other_len, self.buf.bump());

        // Unsafely `set_len` and copy items to `other`.
        unsafe {
            self.set_len(at);
            other.set_len(other_len);

            ptr::copy_nonoverlapping(self.as_ptr().add(at), other.as_mut_ptr(), other.len());
        }
        other
    }
}

#[cfg(feature = "boxed")]
impl<'bump, T> Vec<'bump, T> {
    /// Converts the vector into [`Box<[T]>`][owned slice].
    ///
    /// Note that this will drop any excess capacity.
    ///
    /// [owned slice]: ../../boxed/struct.Box.html
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec, vec};
    ///
    /// let b = Bump::new();
    ///
    /// let v = vec![in &b; 1, 2, 3];
    ///
    /// let slice = v.into_boxed_slice();
    /// ```
    pub fn into_boxed_slice(mut self) -> crate::boxed::Box<'bump, [T]> {
        use crate::boxed::Box;

        // Unlike `alloc::vec::Vec` shrinking here isn't necessary as `bumpalo::boxed::Box` doesn't own memory.
        unsafe {
            let slice = slice::from_raw_parts_mut(self.as_mut_ptr(), self.len);
            let output: Box<'bump, [T]> = Box::from_raw(slice);
            mem::forget(self);
            output
        }
    }
}

impl<'bump, T: 'bump + Clone> Vec<'bump, T> {
    /// Resizes the `Vec` in-place so that `len` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len`, the `Vec` is extended by the
    /// difference, with each additional slot filled with `value`.
    /// If `new_len` is less than `len`, the `Vec` is simply truncated.
    ///
    /// This method requires [`Clone`] to be able clone the passed value. If
    /// you need more flexibility (or want to rely on [`Default`] instead of
    /// [`Clone`]), use [`resize_with`].
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; "hello"];
    /// vec.resize(3, "world");
    /// assert_eq!(vec, ["hello", "world", "world"]);
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 3, 4];
    /// vec.resize(2, 0);
    /// assert_eq!(vec, [1, 2]);
    /// ```
    ///
    /// [`Clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html
    /// [`Default`]: https://doc.rust-lang.org/std/default/trait.Default.html
    /// [`resize_with`]: #method.resize_with
    pub fn resize(&mut self, new_len: usize, value: T) {
        let len = self.len();

        if new_len > len {
            self.extend_with(new_len - len, ExtendElement(value))
        } else {
            self.truncate(new_len);
        }
    }

    // Proven specification with verus, converted to comments.
    /// # Preconditions
    ///
    /// - old(self).len() + slice.len() <= old(self).capacity(),
    ///
    /// # Postconditions
    ///
    /// - forall|i: usize| implies(
    ///       i < old(self).len(),
    ///       self.get_unchecked(i) == old(self).get_unchecked(i)
    ///   ),
    /// - forall|i: usize| implies(
    ///       i < slice.len(),
    ///       self.get_unchecked((old(self).len() + i) as usize)
    ///           == clone(slice.get_unchecked(i))
    ///   ),
    /// - self.len()      == old(self).len() + slice.len(),
    /// - self.capacity() == old(self).capacity(),
    #[inline]
    unsafe fn extend_from_slice_unchecked(&mut self, slice: &[T]) {
        // Guaranteed never to overflow for non ZSTs
        //   size_of::<T>() <= isize::MAX - (isize::MAX % align_of::<T>()))
        //   isize::MAX + isize::MAX < usize::MAX
        debug_assert!(
            core::mem::size_of::<T>() == 0 || self.capacity() >= self.len() + slice.len()
        );
        debug_assert!(
            // is_zst::<T>() ==> capacity >= slen + slice.len()
            core::mem::size_of::<T>() != 0
            // Capacity is usize::MAX for ZSTs
            || self.len() <= usize::MAX - slice.len()
        );

        let mut pos = 0usize;

        loop
        /*
        invariants
            pos <= slice.len(),
            self.len() + (slice.len() - pos) <= old(self).capacity(),
            old(self).capacity() == self.capacity(),

            self.len() == old(self).len() + pos,

            forall|i: usize| implies(
                i < old(self).len(),
                self.get_unchecked(i) == old(self).get_unchecked(i)
            ),
            forall|i: usize| implies(
                i < pos,
                self.get_unchecked((old(self).len() + i) as usize)
                    == clone(slice.get_unchecked(i))
            )
        */
        {
            if pos == slice.len() {
                /*
                pos = slice.len(),
                self.len() = old(self).len() + slice.len(),

                forall|i: usize| i < slice.len() implies {
                    self.get_unchecked((old(self).len() + i) as usize)
                        == clone(slice.get_unchecked(i))
                }
                by {
                    i < pos
                }
                */
                return;
            }

            /*
            pos < slice.len(),
            self.len() < self.capacity()
            */

            let elem = slice.get_unchecked(pos);
            self.push_unchecked(elem.clone());

            /*
            ghost prev_pos = pos
            */

            pos = pos + 1;

            /*
            forall|i: usize| i < pos implies {
                self.get_unchecked((old(self).len() + i) as usize)
                    == clone(slice.get_unchecked(i))
            }
            by {
                if i < pos - 1 {
                    // By invariant
                }
                else {
                    i == prev_pos
                }
            }
            */
        }
    }

    /// Clones and appends all elements in a slice to the `Vec`.
    ///
    /// Iterates over the slice `other`, clones each element, and then appends
    /// it to this `Vec`. The `other` vector is traversed in-order.
    ///
    /// Note that this function is same as [`extend`] except that it is
    /// specialized to work with slices instead. If and when Rust gets
    /// specialization this function will likely be deprecated (but still
    /// available).
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.extend_from_slice(&[2, 3, 4]);
    /// assert_eq!(vec, [1, 2, 3, 4]);
    /// ```
    ///
    /// [`extend`]: #method.extend
    pub fn extend_from_slice(&mut self, other: &[T]) {
        let capacity = self.capacity();

        /*
        Cannot underflow via invariant of the Vec (capacity >= length).

        This also holds for ZSTs, as capacity is usize::MAX
        */
        let remaining_cap = capacity - self.len;

        /*
            self.len() + other.len() <= self.capacity(),
                <==>
            other.len() <= self.capacity() - self.len()
        */

        if other.len() > remaining_cap {
            /*
            Divergence from verified impl:
              Verified implementation's reserve is not the same
              as bumpalo's. Verified implementation reserves with
              respect to capacity, not length. Thus this is equivalent
              to the verified implementation's:

                self.buf.reserve(other.len() - remaining_cap)

            */
            self.reserve(other.len());
        }

        /*
        self.capacity() >= self.len() + other.len()
        */

        unsafe {
            self.extend_from_slice_unchecked(other);
        }
    }
}

impl<'bump, T: 'bump + Copy> Vec<'bump, T> {
    /// Helper method to copy all of the items in `other` and append them to the end of `self`.
    ///
    /// SAFETY:
    ///   * The caller is responsible for:
    ///       * calling [`reserve`](Self::reserve) beforehand to guarantee that there is enough
    ///         capacity to store `other.len()` more items.
    ///       * guaranteeing that `self` and `other` do not overlap.
    unsafe fn extend_from_slice_copy_unchecked(&mut self, other: &[T]) {
        let old_len = self.len();
        debug_assert!(old_len + other.len() <= self.capacity());

        // SAFETY:
        // * `src` is valid for reads of `other.len()` values by virtue of being a `&[T]`.
        // * `dst` is valid for writes of `other.len()` bytes because the caller of this
        //   method is required to `reserve` capacity to store at least `other.len()` items
        //   beforehand.
        // * Because `src` is a `&[T]` and dst is a `&[T]` within the `Vec<T>`,
        //   `copy_nonoverlapping`'s alignment requirements are met.
        // * Caller is required to guarantee that the source and destination ranges cannot overlap
        unsafe {
            let src = other.as_ptr();
            let dst = self.as_mut_ptr().add(old_len);
            ptr::copy_nonoverlapping(src, dst, other.len());
            self.set_len(old_len + other.len());
        }
    }

    /// Copies all elements in the slice `other` and appends them to the `Vec`.
    ///
    /// Note that this function is same as [`extend_from_slice`] except that it is optimized for
    /// slices of types that implement the `Copy` trait. If and when Rust gets specialization
    /// this function will likely be deprecated (but still available).
    ///
    /// To copy and append the data from multiple source slices at once, see
    /// [`extend_from_slices_copy`].
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.extend_from_slice_copy(&[2, 3, 4]);
    /// assert_eq!(vec, [1, 2, 3, 4]);
    /// ```
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 'H' as u8];
    /// vec.extend_from_slice_copy("ello, world!".as_bytes());
    /// assert_eq!(vec, "Hello, world!".as_bytes());
    /// ```
    ///
    /// [`extend_from_slice`]: #method.extend_from_slice
    /// [`extend_from_slices`]: #method.extend_from_slices
    pub fn extend_from_slice_copy(&mut self, other: &[T]) {
        // Reserve space in the Vec for the values to be added
        self.reserve(other.len());

        // Copy values into the space that was just reserved
        // SAFETY:
        // * `self` has enough capacity to store `other.len()` more items as `self.reserve(other.len())`
        //   above guarantees that.
        // * Source and destination data ranges cannot overlap as we just reserved the destination
        //   range from the bump.
        unsafe {
            self.extend_from_slice_copy_unchecked(other);
        }
    }

    /// For each slice in `slices`, copies all elements in the slice and appends them to the `Vec`.
    ///
    /// This method is equivalent to calling [`extend_from_slice_copy`] in a loop, but is able
    /// to precompute the total amount of space to reserve in advance. This reduces the potential
    /// maximum number of reallocations needed from one-per-slice to just one.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1];
    /// vec.extend_from_slices_copy(&[&[2, 3], &[], &[4]]);
    /// assert_eq!(vec, [1, 2, 3, 4]);
    /// ```
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 'H' as u8];
    /// vec.extend_from_slices_copy(&["ello,".as_bytes(), &[], " world!".as_bytes()]);
    /// assert_eq!(vec, "Hello, world!".as_bytes());
    /// ```
    ///
    /// [`extend_from_slice_copy`]: #method.extend_from_slice_copy
    pub fn extend_from_slices_copy(&mut self, slices: &[&[T]]) {
        // Reserve the total amount of capacity we'll need to safely append the aggregated contents
        // of each slice in `slices`.
        let capacity_to_reserve: usize = slices.iter().map(|slice| slice.len()).sum();
        self.reserve(capacity_to_reserve);

        // SAFETY:
        // * `dst` is valid for writes of `capacity_to_reserve` items as
        //   `self.reserve(capacity_to_reserve)` above guarantees that.
        // * Source and destination ranges cannot overlap as we just reserved the destination
        //   range from the bump.
        unsafe {
            // Copy the contents of each slice onto the end of `self`
            slices.iter().for_each(|slice| {
                self.extend_from_slice_copy_unchecked(slice);
            });
        }
    }
}

// This code generalises `extend_with_{element,default}`.
trait ExtendWith<T> {
    fn next(&mut self) -> T;
    fn last(self) -> T;
}

struct ExtendElement<T>(T);
impl<T: Clone> ExtendWith<T> for ExtendElement<T> {
    fn next(&mut self) -> T {
        self.0.clone()
    }
    fn last(self) -> T {
        self.0
    }
}

impl<'bump, T: 'bump> Vec<'bump, T> {
    /// Extend the vector by `n` values, using the given generator.
    fn extend_with<E: ExtendWith<T>>(&mut self, n: usize, mut value: E) {
        self.reserve(n);

        unsafe {
            let mut ptr = self.as_mut_ptr().add(self.len());
            // Use SetLenOnDrop to work around bug where compiler
            // may not realize the store through `ptr` through self.set_len()
            // don't alias.
            let mut local_len = SetLenOnDrop::new(&mut self.len);

            // Write all elements except the last one
            for _ in 1..n {
                ptr::write(ptr, value.next());
                ptr = ptr.offset(1);
                // Increment the length in every step in case next() panics
                local_len.increment_len(1);
            }

            if n > 0 {
                // We can write the last element directly without cloning needlessly
                ptr::write(ptr, value.last());
                local_len.increment_len(1);
            }

            // len set by scope guard
        }
    }
}

// Set the length of the vec when the `SetLenOnDrop` value goes out of scope.
//
// The idea is: The length field in SetLenOnDrop is a local variable
// that the optimizer will see does not alias with any stores through the Vec's data
// pointer. This is a workaround for alias analysis issue #32155
struct SetLenOnDrop<'a> {
    len: &'a mut usize,
    local_len: usize,
}

impl<'a> SetLenOnDrop<'a> {
    #[inline]
    fn new(len: &'a mut usize) -> Self {
        SetLenOnDrop {
            local_len: *len,
            len,
        }
    }

    #[inline]
    fn increment_len(&mut self, increment: usize) {
        self.local_len += increment;
    }

    #[inline]
    fn decrement_len(&mut self, decrement: usize) {
        self.local_len -= decrement;
    }
}

impl<'a> Drop for SetLenOnDrop<'a> {
    #[inline]
    fn drop(&mut self) {
        *self.len = self.local_len;
    }
}

impl<'bump, T: 'bump + PartialEq> Vec<'bump, T> {
    /// Removes consecutive repeated elements in the vector according to the
    /// [`PartialEq`] trait implementation.
    ///
    /// If the vector is sorted, this removes all duplicates.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut vec = bumpalo::vec![in &b; 1, 2, 2, 3, 2];
    ///
    /// vec.dedup();
    ///
    /// assert_eq!(vec, [1, 2, 3, 2]);
    /// ```
    #[inline]
    pub fn dedup(&mut self) {
        self.dedup_by(|a, b| a == b)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Common trait implementations for Vec
////////////////////////////////////////////////////////////////////////////////

impl<'bump, T: 'bump + Clone> Clone for Vec<'bump, T> {
    #[cfg(not(test))]
    fn clone(&self) -> Vec<'bump, T> {
        let mut v = Vec::with_capacity_in(self.len(), self.buf.bump());
        v.extend(self.iter().cloned());
        v
    }

    // HACK(japaric): with cfg(test) the inherent `[T]::to_vec` method, which is
    // required for this method definition, is not available. Instead use the
    // `slice::to_vec`  function which is only available with cfg(test)
    // NB see the slice::hack module in slice.rs for more information
    #[cfg(test)]
    fn clone(&self) -> Vec<'bump, T> {
        let mut v = Vec::new_in(self.buf.bump());
        v.extend(self.iter().cloned());
        v
    }
}

impl<'bump, T: 'bump + Hash> Hash for Vec<'bump, T> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        Hash::hash(&**self, state)
    }
}

impl<'bump, T, I> Index<I> for Vec<'bump, T>
where
    I: ::core::slice::SliceIndex<[T]>,
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&**self, index)
    }
}

impl<'bump, T, I> IndexMut<I> for Vec<'bump, T>
where
    I: ::core::slice::SliceIndex<[T]>,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl<'bump, T: 'bump> ops::Deref for Vec<'bump, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe {
            let p = self.buf.ptr();
            // assume(!p.is_null());
            slice::from_raw_parts(p, self.len)
        }
    }
}

impl<'bump, T: 'bump> ops::DerefMut for Vec<'bump, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            let ptr = self.buf.ptr();
            // assume(!ptr.is_null());
            slice::from_raw_parts_mut(ptr, self.len)
        }
    }
}

impl<'bump, T: 'bump> IntoIterator for Vec<'bump, T> {
    type Item = T;
    type IntoIter = IntoIter<'bump, T>;

    /// Creates a consuming iterator, that is, one that moves each value out of
    /// the vector (from start to end). The vector cannot be used after calling
    /// this.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let v = bumpalo::vec![in &b; "a".to_string(), "b".to_string()];
    /// for s in v.into_iter() {
    ///     // s has type String, not &String
    ///     println!("{}", s);
    /// }
    /// ```
    #[inline]
    fn into_iter(mut self) -> IntoIter<'bump, T> {
        unsafe {
            let begin = self.as_mut_ptr();
            // assume(!begin.is_null());
            let end = if mem::size_of::<T>() == 0 {
                arith_offset(begin as *const i8, self.len() as isize) as *const T
            } else {
                begin.add(self.len()) as *const T
            };
            mem::forget(self);
            IntoIter {
                phantom: PhantomData,
                ptr: begin,
                end,
            }
        }
    }
}

impl<'a, 'bump, T> IntoIterator for &'a Vec<'bump, T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> slice::Iter<'a, T> {
        self.iter()
    }
}

impl<'a, 'bump, T> IntoIterator for &'a mut Vec<'bump, T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> slice::IterMut<'a, T> {
        self.iter_mut()
    }
}

impl<'bump, T: 'bump> Extend<T> for Vec<'bump, T> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.reserve(iter.size_hint().0);

        for t in iter {
            self.push(t);
        }
    }
}

impl<'bump, T: 'bump> Vec<'bump, T> {
    /// Creates a splicing iterator that replaces the specified range in the vector
    /// with the given `replace_with` iterator and yields the removed items.
    /// `replace_with` does not need to be the same length as `range`.
    ///
    /// Note 1: The element range is removed even if the iterator is not
    /// consumed until the end.
    ///
    /// Note 2: It is unspecified how many elements are removed from the vector,
    /// if the `Splice` value is leaked.
    ///
    /// Note 3: The input iterator `replace_with` is only consumed
    /// when the `Splice` value is dropped.
    ///
    /// Note 4: This is optimal if:
    ///
    /// * The tail (elements in the vector after `range`) is empty,
    /// * or `replace_with` yields fewer elements than `range`’s length
    /// * or the lower bound of its `size_hint()` is exact.
    ///
    /// Otherwise, a temporary vector is allocated and the tail is moved twice.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = bumpalo::vec![in &b; 1, 2, 3];
    /// let new = [7, 8];
    /// let u: Vec<_> = Vec::from_iter_in(v.splice(..2, new.iter().cloned()), &b);
    /// assert_eq!(v, &[7, 8, 3]);
    /// assert_eq!(u, &[1, 2]);
    /// ```
    #[inline]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        Splice {
            drain: self.drain(range),
            replace_with: replace_with.into_iter(),
        }
    }
}

/// Extend implementation that copies elements out of references before pushing them onto the Vec.
///
/// This implementation is specialized for slice iterators, where it uses [`copy_from_slice`] to
/// append the entire slice at once.
///
/// [`copy_from_slice`]: https://doc.rust-lang.org/std/primitive.slice.html#method.copy_from_slice
impl<'a, 'bump, T: 'a + Copy> Extend<&'a T> for Vec<'bump, T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned())
    }
}

macro_rules! __impl_slice_eq1 {
    ($Lhs: ty, $Rhs: ty) => {
        __impl_slice_eq1! { $Lhs, $Rhs, Sized }
    };
    ($Lhs: ty, $Rhs: ty, $Bound: ident) => {
        impl<'a, 'b, A: $Bound, B> PartialEq<$Rhs> for $Lhs
        where
            A: PartialEq<B>,
        {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool {
                self[..] == other[..]
            }
        }
    };
}

__impl_slice_eq1! { Vec<'a, A>, Vec<'b, B> }
__impl_slice_eq1! { Vec<'a, A>, &'b [B] }
__impl_slice_eq1! { Vec<'a, A>, &'b mut [B] }
// __impl_slice_eq1! { Cow<'a, [A]>, Vec<'b, B>, Clone }

macro_rules! __impl_slice_eq1_array {
    ($Lhs: ty, $Rhs: ty) => {
        impl<'a, 'b, A, B, const N: usize> PartialEq<$Rhs> for $Lhs
        where
            A: PartialEq<B>,
        {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool {
                self[..] == other[..]
            }
        }
    };
}

__impl_slice_eq1_array! { Vec<'a, A>, [B; N] }
__impl_slice_eq1_array! { Vec<'a, A>, &'b [B; N] }
__impl_slice_eq1_array! { Vec<'a, A>, &'b mut [B; N] }

/// Implements comparison of vectors, lexicographically.
impl<'bump, T: 'bump + PartialOrd> PartialOrd for Vec<'bump, T> {
    #[inline]
    fn partial_cmp(&self, other: &Vec<'bump, T>) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
}

impl<'bump, T: 'bump + Eq> Eq for Vec<'bump, T> {}

/// Implements ordering of vectors, lexicographically.
impl<'bump, T: 'bump + Ord> Ord for Vec<'bump, T> {
    #[inline]
    fn cmp(&self, other: &Vec<'bump, T>) -> Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<'bump, T: 'bump + fmt::Debug> fmt::Debug for Vec<'bump, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'bump, T: 'bump> AsRef<Vec<'bump, T>> for Vec<'bump, T> {
    fn as_ref(&self) -> &Vec<'bump, T> {
        self
    }
}

impl<'bump, T: 'bump> AsMut<Vec<'bump, T>> for Vec<'bump, T> {
    fn as_mut(&mut self) -> &mut Vec<'bump, T> {
        self
    }
}

impl<'bump, T: 'bump> AsRef<[T]> for Vec<'bump, T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<'bump, T: 'bump> AsMut<[T]> for Vec<'bump, T> {
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}

#[cfg(feature = "boxed")]
impl<'bump, T: 'bump> From<Vec<'bump, T>> for crate::boxed::Box<'bump, [T]> {
    fn from(v: Vec<'bump, T>) -> crate::boxed::Box<'bump, [T]> {
        v.into_boxed_slice()
    }
}

impl<'bump, T: 'bump> Borrow<[T]> for Vec<'bump, T> {
    #[inline]
    fn borrow(&self) -> &[T] {
        &self[..]
    }
}

impl<'bump, T: 'bump> BorrowMut<[T]> for Vec<'bump, T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        &mut self[..]
    }
}

impl<'bump, T> Drop for Vec<'bump, T> {
    fn drop(&mut self) {
        unsafe {
            // use drop for [T]
            // use a raw slice to refer to the elements of the vector as weakest necessary type;
            // could avoid questions of validity in certain cases
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.as_mut_ptr(), self.len))
        }
        // RawVec handles deallocation
    }
}

////////////////////////////////////////////////////////////////////////////////
// Clone-on-write
////////////////////////////////////////////////////////////////////////////////

// impl<'a, 'bump, T: Clone> From<Vec<'bump, T>> for Cow<'a, [T]> {
//     fn from(v: Vec<'bump, T>) -> Cow<'a, [T]> {
//         Cow::Owned(v)
//     }
// }

// impl<'a, 'bump, T: Clone> From<&'a Vec<'bump, T>> for Cow<'a, [T]> {
//     fn from(v: &'a Vec<'bump, T>) -> Cow<'a, [T]> {
//         Cow::Borrowed(v.as_slice())
//     }
// }

////////////////////////////////////////////////////////////////////////////////
// Iterators
////////////////////////////////////////////////////////////////////////////////

/// An iterator that moves out of a vector.
///
/// This `struct` is created by the [`Vec::into_iter`] method
/// (provided by the [`IntoIterator`] trait).
///
/// [`IntoIterator`]: https://doc.rust-lang.org/std/iter/trait.IntoIterator.html
pub struct IntoIter<'bump, T> {
    phantom: PhantomData<&'bump [T]>,
    ptr: *const T,
    end: *const T,
}

impl<'bump, T: fmt::Debug> fmt::Debug for IntoIter<'bump, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

impl<'bump, T: 'bump> IntoIter<'bump, T> {
    /// Returns the remaining items of this iterator as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let vec = bumpalo::vec![in &b; 'a', 'b', 'c'];
    /// let mut into_iter = vec.into_iter();
    /// assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
    /// let _ = into_iter.next().unwrap();
    /// assert_eq!(into_iter.as_slice(), &['b', 'c']);
    /// ```
    pub fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.ptr, self.len()) }
    }

    /// Returns the remaining items of this iterator as a mutable slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::Vec};
    ///
    /// let b = Bump::new();
    ///
    /// let vec = bumpalo::vec![in &b; 'a', 'b', 'c'];
    /// let mut into_iter = vec.into_iter();
    /// assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
    /// into_iter.as_mut_slice()[2] = 'z';
    /// assert_eq!(into_iter.next().unwrap(), 'a');
    /// assert_eq!(into_iter.next().unwrap(), 'b');
    /// assert_eq!(into_iter.next().unwrap(), 'z');
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut T, self.len()) }
    }
}

unsafe impl<'bump, T: Send> Send for IntoIter<'bump, T> {}
unsafe impl<'bump, T: Sync> Sync for IntoIter<'bump, T> {}

impl<'bump, T: 'bump> Iterator for IntoIter<'bump, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        unsafe {
            if self.ptr as *const _ == self.end {
                None
            } else if mem::size_of::<T>() == 0 {
                // purposefully don't use 'ptr.offset' because for
                // vectors with 0-size elements this would return the
                // same pointer.
                self.ptr = arith_offset(self.ptr as *const i8, 1) as *mut T;

                // Make up a value of this ZST.
                Some(mem::zeroed())
            } else {
                let old = self.ptr;
                self.ptr = self.ptr.offset(1);

                Some(ptr::read(old))
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = if mem::size_of::<T>() == 0 {
            (self.end as usize).wrapping_sub(self.ptr as usize)
        } else {
            unsafe { offset_from(self.end, self.ptr) as usize }
        };
        (exact, Some(exact))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }
}

impl<'bump, T: 'bump> DoubleEndedIterator for IntoIter<'bump, T> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        unsafe {
            if self.end == self.ptr {
                None
            } else if mem::size_of::<T>() == 0 {
                // See above for why 'ptr.offset' isn't used
                self.end = arith_offset(self.end as *const i8, -1) as *mut T;

                // Make up a value of this ZST.
                Some(mem::zeroed())
            } else {
                self.end = self.end.offset(-1);

                Some(ptr::read(self.end))
            }
        }
    }
}

impl<'bump, T: 'bump> ExactSizeIterator for IntoIter<'bump, T> {}

impl<'bump, T: 'bump> FusedIterator for IntoIter<'bump, T> {}

impl<'bump, T> Drop for IntoIter<'bump, T> {
    fn drop(&mut self) {
        // drop all remaining elements
        self.for_each(drop);
    }
}

/// A draining iterator for `Vec<'bump, T>`.
///
/// This `struct` is created by the [`Vec::drain`] method.
pub struct Drain<'a, 'bump, T: 'a + 'bump> {
    /// Index of tail to preserve
    tail_start: usize,
    /// Length of tail
    tail_len: usize,
    /// Current remaining range to remove
    iter: slice::Iter<'a, T>,
    vec: NonNull<Vec<'bump, T>>,
}

impl<'a, 'bump, T: 'a + 'bump + fmt::Debug> fmt::Debug for Drain<'a, 'bump, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Drain").field(&self.iter.as_slice()).finish()
    }
}

unsafe impl<'a, 'bump, T: Sync> Sync for Drain<'a, 'bump, T> {}
unsafe impl<'a, 'bump, T: Send> Send for Drain<'a, 'bump, T> {}

impl<'a, 'bump, T> Iterator for Drain<'a, 'bump, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.iter
            .next()
            .map(|elt| unsafe { ptr::read(elt as *const _) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, 'bump, T> DoubleEndedIterator for Drain<'a, 'bump, T> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        self.iter
            .next_back()
            .map(|elt| unsafe { ptr::read(elt as *const _) })
    }
}

impl<'a, 'bump, T> Drop for Drain<'a, 'bump, T> {
    fn drop(&mut self) {
        // exhaust self first
        self.for_each(drop);

        if self.tail_len > 0 {
            unsafe {
                let source_vec = self.vec.as_mut();
                // memmove back untouched tail, update to new length
                let start = source_vec.len();
                let tail = self.tail_start;
                if tail != start {
                    let src = source_vec.as_ptr().add(tail);
                    let dst = source_vec.as_mut_ptr().add(start);
                    ptr::copy(src, dst, self.tail_len);
                }
                source_vec.set_len(start + self.tail_len);
            }
        }
    }
}

impl<'a, 'bump, T> ExactSizeIterator for Drain<'a, 'bump, T> {}

impl<'a, 'bump, T> FusedIterator for Drain<'a, 'bump, T> {}

/// A splicing iterator for `Vec`.
///
/// This struct is created by the [`Vec::splice`] method. See its
/// documentation for more information.
#[derive(Debug)]
pub struct Splice<'a, 'bump, I: Iterator + 'a + 'bump> {
    drain: Drain<'a, 'bump, I::Item>,
    replace_with: I,
}

impl<'a, 'bump, I: Iterator> Iterator for Splice<'a, 'bump, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.drain.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.drain.size_hint()
    }
}

impl<'a, 'bump, I: Iterator> DoubleEndedIterator for Splice<'a, 'bump, I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.drain.next_back()
    }
}

impl<'a, 'bump, I: Iterator> ExactSizeIterator for Splice<'a, 'bump, I> {}

impl<'a, 'bump, I: Iterator> Drop for Splice<'a, 'bump, I> {
    fn drop(&mut self) {
        self.drain.by_ref().for_each(drop);

        unsafe {
            if self.drain.tail_len == 0 {
                self.drain.vec.as_mut().extend(self.replace_with.by_ref());
                return;
            }

            // First fill the range left by drain().
            if !self.drain.fill(&mut self.replace_with) {
                return;
            }

            // There may be more elements. Use the lower bound as an estimate.
            // FIXME: Is the upper bound a better guess? Or something else?
            let (lower_bound, _upper_bound) = self.replace_with.size_hint();
            if lower_bound > 0 {
                self.drain.move_tail(lower_bound);
                if !self.drain.fill(&mut self.replace_with) {
                    return;
                }
            }

            // Collect any remaining elements.
            // This is a zero-length vector which does not allocate if `lower_bound` was exact.
            let mut collected = Vec::new_in(self.drain.vec.as_ref().buf.bump());
            collected.extend(self.replace_with.by_ref());
            let mut collected = collected.into_iter();
            // Now we have an exact count.
            if collected.len() > 0 {
                self.drain.move_tail(collected.len());
                let filled = self.drain.fill(&mut collected);
                debug_assert!(filled);
                debug_assert_eq!(collected.len(), 0);
            }
        }
        // Let `Drain::drop` move the tail back if necessary and restore `vec.len`.
    }
}

/// Private helper methods for `Splice::drop`
impl<'a, 'bump, T> Drain<'a, 'bump, T> {
    /// The range from `self.vec.len` to `self.tail_start` contains elements
    /// that have been moved out.
    /// Fill that range as much as possible with new elements from the `replace_with` iterator.
    /// Return whether we filled the entire range. (`replace_with.next()` didn’t return `None`.)
    unsafe fn fill<I: Iterator<Item = T>>(&mut self, replace_with: &mut I) -> bool {
        let vec = self.vec.as_mut();
        let range_start = vec.len;
        let range_end = self.tail_start;
        let range_slice =
            slice::from_raw_parts_mut(vec.as_mut_ptr().add(range_start), range_end - range_start);

        for place in range_slice {
            if let Some(new_item) = replace_with.next() {
                ptr::write(place, new_item);
                vec.len += 1;
            } else {
                return false;
            }
        }
        true
    }

    /// Make room for inserting more elements before the tail.
    unsafe fn move_tail(&mut self, extra_capacity: usize) {
        let vec = self.vec.as_mut();
        let used_capacity = self.tail_start + self.tail_len;
        vec.buf.reserve(used_capacity, extra_capacity);

        let new_tail_start = self.tail_start + extra_capacity;
        let src = vec.as_ptr().add(self.tail_start);
        let dst = vec.as_mut_ptr().add(new_tail_start);
        ptr::copy(src, dst, self.tail_len);
        self.tail_start = new_tail_start;
    }
}

/// An iterator produced by calling [`Vec::drain_filter`].
#[derive(Debug)]
pub struct DrainFilter<'a, 'bump: 'a, T: 'a + 'bump, F>
where
    F: FnMut(&mut T) -> bool,
{
    vec: &'a mut Vec<'bump, T>,
    idx: usize,
    del: usize,
    old_len: usize,
    pred: F,
}

impl<'a, 'bump, T, F> Iterator for DrainFilter<'a, 'bump, T, F>
where
    F: FnMut(&mut T) -> bool,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        unsafe {
            while self.idx != self.old_len {
                let i = self.idx;
                self.idx += 1;
                let v = slice::from_raw_parts_mut(self.vec.as_mut_ptr(), self.old_len);
                if (self.pred)(&mut v[i]) {
                    self.del += 1;
                    return Some(ptr::read(&v[i]));
                } else if self.del > 0 {
                    let del = self.del;
                    let src: *const T = &v[i];
                    let dst: *mut T = &mut v[i - del];
                    // This is safe because self.vec has length 0
                    // thus its elements will not have Drop::drop
                    // called on them in the event of a panic.
                    ptr::copy_nonoverlapping(src, dst, 1);
                }
            }
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.old_len - self.idx))
    }
}

impl<'a, 'bump, T, F> Drop for DrainFilter<'a, 'bump, T, F>
where
    F: FnMut(&mut T) -> bool,
{
    fn drop(&mut self) {
        self.for_each(drop);
        unsafe {
            self.vec.set_len(self.old_len - self.del);
        }
    }
}

#[cfg(feature = "std")]
impl<'bump> io::Write for Vec<'bump, u8> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_from_slice_copy(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.extend_from_slice_copy(buf);
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "serde")]
mod serialize {
    use super::*;

    use serde::{ser::SerializeSeq, Serialize, Serializer};

    impl<'a, T> Serialize for Vec<'a, T>
    where
        T: Serialize,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut seq = serializer.serialize_seq(Some(self.len))?;
            for e in self.iter() {
                seq.serialize_element(e)?;
            }
            seq.end()
        }
    }
}
