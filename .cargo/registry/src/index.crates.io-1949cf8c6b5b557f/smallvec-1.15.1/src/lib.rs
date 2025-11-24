// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Small vectors in various sizes. These store a certain number of elements inline, and fall back
//! to the heap for larger allocations.  This can be a useful optimization for improving cache
//! locality and reducing allocator traffic for workloads that fit within the inline buffer.
//!
//! ## `no_std` support
//!
//! By default, `smallvec` does not depend on `std`.  However, the optional
//! `write` feature implements the `std::io::Write` trait for vectors of `u8`.
//! When this feature is enabled, `smallvec` depends on `std`.
//!
//! ## Optional features
//!
//! ### `serde`
//!
//! When this optional dependency is enabled, `SmallVec` implements the `serde::Serialize` and
//! `serde::Deserialize` traits.
//!
//! ### `write`
//!
//! When this feature is enabled, `SmallVec<[u8; _]>` implements the `std::io::Write` trait.
//! This feature is not compatible with `#![no_std]` programs.
//!
//! ### `union`
//!
//! **This feature requires Rust 1.49.**
//!
//! When the `union` feature is enabled `smallvec` will track its state (inline or spilled)
//! without the use of an enum tag, reducing the size of the `smallvec` by one machine word.
//! This means that there is potentially no space overhead compared to `Vec`.
//! Note that `smallvec` can still be larger than `Vec` if the inline buffer is larger than two
//! machine words.
//!
//! To use this feature add `features = ["union"]` in the `smallvec` section of Cargo.toml.
//! Note that this feature requires Rust 1.49.
//!
//! Tracking issue: [rust-lang/rust#55149](https://github.com/rust-lang/rust/issues/55149)
//!
//! ### `const_generics`
//!
//! **This feature requires Rust 1.51.**
//!
//! When this feature is enabled, `SmallVec` works with any arrays of any size, not just a fixed
//! list of sizes.
//!
//! ### `const_new`
//!
//! **This feature requires Rust 1.51.**
//!
//! This feature exposes the functions [`SmallVec::new_const`], [`SmallVec::from_const`], and [`smallvec_inline`] which enables the `SmallVec` to be initialized from a const context.
//! For details, see the
//! [Rust Reference](https://doc.rust-lang.org/reference/const_eval.html#const-functions).
//!
//! ### `drain_filter`
//!
//! **This feature is unstable.** It may change to match the unstable `drain_filter` method in libstd.
//!
//! Enables the `drain_filter` method, which produces an iterator that calls a user-provided
//! closure to determine which elements of the vector to remove and yield from the iterator.
//!
//! ### `drain_keep_rest`
//!
//! **This feature is unstable.** It may change to match the unstable `drain_keep_rest` method in libstd.
//!
//! Enables the `DrainFilter::keep_rest` method.
//!
//! ### `specialization`
//!
//! **This feature is unstable and requires a nightly build of the Rust toolchain.**
//!
//! When this feature is enabled, `SmallVec::from(slice)` has improved performance for slices
//! of `Copy` types.  (Without this feature, you can use `SmallVec::from_slice` to get optimal
//! performance for `Copy` types.)
//!
//! Tracking issue: [rust-lang/rust#31844](https://github.com/rust-lang/rust/issues/31844)
//!
//! ### `may_dangle`
//!
//! **This feature is unstable and requires a nightly build of the Rust toolchain.**
//!
//! This feature makes the Rust compiler less strict about use of vectors that contain borrowed
//! references. For details, see the
//! [Rustonomicon](https://doc.rust-lang.org/1.42.0/nomicon/dropck.html#an-escape-hatch).
//!
//! Tracking issue: [rust-lang/rust#34761](https://github.com/rust-lang/rust/issues/34761)

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "specialization", allow(incomplete_features))]
#![cfg_attr(feature = "specialization", feature(specialization))]
#![cfg_attr(feature = "may_dangle", feature(dropck_eyepatch))]
#![cfg_attr(
    feature = "debugger_visualizer",
    feature(debugger_visualizer),
    debugger_visualizer(natvis_file = "../debug_metadata/smallvec.natvis")
)]
#![deny(missing_docs)]

#[doc(hidden)]
pub extern crate alloc;

#[cfg(any(test, feature = "write"))]
extern crate std;

#[cfg(test)]
mod tests;

#[allow(deprecated)]
use alloc::alloc::{Layout, LayoutErr};
use alloc::boxed::Box;
use alloc::{vec, vec::Vec};
use core::borrow::{Borrow, BorrowMut};
use core::cmp;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::hint::unreachable_unchecked;
use core::iter::{repeat, FromIterator, FusedIterator, IntoIterator};
use core::mem;
use core::mem::MaybeUninit;
use core::ops::{self, Range, RangeBounds};
use core::ptr::{self, NonNull};
use core::slice::{self, SliceIndex};

#[cfg(feature = "malloc_size_of")]
use malloc_size_of::{MallocShallowSizeOf, MallocSizeOf, MallocSizeOfOps};

#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer, SeqAccess, Visitor},
    ser::{Serialize, SerializeSeq, Serializer},
};

#[cfg(feature = "serde")]
use core::marker::PhantomData;

#[cfg(feature = "write")]
use std::io;

#[cfg(feature = "drain_keep_rest")]
use core::mem::ManuallyDrop;

/// Creates a [`SmallVec`] containing the arguments.
///
/// `smallvec!` allows `SmallVec`s to be defined with the same syntax as array expressions.
/// There are two forms of this macro:
///
/// - Create a [`SmallVec`] containing a given list of elements:
///
/// ```
/// # use smallvec::{smallvec, SmallVec};
/// # fn main() {
/// let v: SmallVec<[_; 128]> = smallvec![1, 2, 3];
/// assert_eq!(v[0], 1);
/// assert_eq!(v[1], 2);
/// assert_eq!(v[2], 3);
/// # }
/// ```
///
/// - Create a [`SmallVec`] from a given element and size:
///
/// ```
/// # use smallvec::{smallvec, SmallVec};
/// # fn main() {
/// let v: SmallVec<[_; 0x8000]> = smallvec![1; 3];
/// assert_eq!(v, SmallVec::from_buf([1, 1, 1]));
/// # }
/// ```
///
/// Note that unlike array expressions this syntax supports all elements
/// which implement [`Clone`] and the number of elements doesn't have to be
/// a constant.
///
/// This will use `clone` to duplicate an expression, so one should be careful
/// using this with types having a nonstandard `Clone` implementation. For
/// example, `smallvec![Rc::new(1); 5]` will create a vector of five references
/// to the same boxed integer value, not five references pointing to independently
/// boxed integers.
#[macro_export]
macro_rules! smallvec {
    // count helper: transform any expression into 1
    (@one $x:expr) => (1usize);
    () => (
        $crate::SmallVec::new()
    );
    ($elem:expr; $n:expr) => ({
        $crate::SmallVec::from_elem($elem, $n)
    });
    ($($x:expr),+$(,)?) => ({
        let count = 0usize $(+ $crate::smallvec!(@one $x))+;
        let mut vec = $crate::SmallVec::new();
        if count <= vec.inline_size() {
            $(vec.push($x);)*
            vec
        } else {
            $crate::SmallVec::from_vec($crate::alloc::vec![$($x,)+])
        }
    });
}

/// Creates an inline [`SmallVec`] containing the arguments. This macro is enabled by the feature `const_new`.
///
/// `smallvec_inline!` allows `SmallVec`s to be defined with the same syntax as array expressions in `const` contexts.
/// The inline storage `A` will always be an array of the size specified by the arguments.
/// There are two forms of this macro:
///
/// - Create a [`SmallVec`] containing a given list of elements:
///
/// ```
/// # use smallvec::{smallvec_inline, SmallVec};
/// # fn main() {
/// const V: SmallVec<[i32; 3]> = smallvec_inline![1, 2, 3];
/// assert_eq!(V[0], 1);
/// assert_eq!(V[1], 2);
/// assert_eq!(V[2], 3);
/// # }
/// ```
///
/// - Create a [`SmallVec`] from a given element and size:
///
/// ```
/// # use smallvec::{smallvec_inline, SmallVec};
/// # fn main() {
/// const V: SmallVec<[i32; 3]> = smallvec_inline![1; 3];
/// assert_eq!(V, SmallVec::from_buf([1, 1, 1]));
/// # }
/// ```
///
/// Note that the behavior mimics that of array expressions, in contrast to [`smallvec`].
#[cfg(feature = "const_new")]
#[cfg_attr(docsrs, doc(cfg(feature = "const_new")))]
#[macro_export]
macro_rules! smallvec_inline {
    // count helper: transform any expression into 1
    (@one $x:expr) => (1usize);
    ($elem:expr; $n:expr) => ({
        $crate::SmallVec::<[_; $n]>::from_const([$elem; $n])
    });
    ($($x:expr),+ $(,)?) => ({
        const N: usize = 0usize $(+ $crate::smallvec_inline!(@one $x))*;
        $crate::SmallVec::<[_; N]>::from_const([$($x,)*])
    });
}

/// `panic!()` in debug builds, optimization hint in release.
#[cfg(not(feature = "union"))]
macro_rules! debug_unreachable {
    () => {
        debug_unreachable!("entered unreachable code")
    };
    ($e:expr) => {
        if cfg!(debug_assertions) {
            panic!($e);
        } else {
            unreachable_unchecked();
        }
    };
}

/// Trait to be implemented by a collection that can be extended from a slice
///
/// ## Example
///
/// ```rust
/// use smallvec::{ExtendFromSlice, SmallVec};
///
/// fn initialize<V: ExtendFromSlice<u8>>(v: &mut V) {
///     v.extend_from_slice(b"Test!");
/// }
///
/// let mut vec = Vec::new();
/// initialize(&mut vec);
/// assert_eq!(&vec, b"Test!");
///
/// let mut small_vec = SmallVec::<[u8; 8]>::new();
/// initialize(&mut small_vec);
/// assert_eq!(&small_vec as &[_], b"Test!");
/// ```
#[doc(hidden)]
#[deprecated]
pub trait ExtendFromSlice<T> {
    /// Extends a collection from a slice of its element type
    fn extend_from_slice(&mut self, other: &[T]);
}

#[allow(deprecated)]
impl<T: Clone> ExtendFromSlice<T> for Vec<T> {
    fn extend_from_slice(&mut self, other: &[T]) {
        Vec::extend_from_slice(self, other)
    }
}

/// Error type for APIs with fallible heap allocation
#[derive(Debug)]
pub enum CollectionAllocErr {
    /// Overflow `usize::MAX` or other error during size computation
    CapacityOverflow,
    /// The allocator return an error
    AllocErr {
        /// The layout that was passed to the allocator
        layout: Layout,
    },
}

impl fmt::Display for CollectionAllocErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Allocation error: {:?}", self)
    }
}

#[allow(deprecated)]
impl From<LayoutErr> for CollectionAllocErr {
    fn from(_: LayoutErr) -> Self {
        CollectionAllocErr::CapacityOverflow
    }
}

fn infallible<T>(result: Result<T, CollectionAllocErr>) -> T {
    match result {
        Ok(x) => x,
        Err(CollectionAllocErr::CapacityOverflow) => panic!("capacity overflow"),
        Err(CollectionAllocErr::AllocErr { layout }) => alloc::alloc::handle_alloc_error(layout),
    }
}

/// FIXME: use `Layout::array` when we require a Rust version where it’s stable
/// <https://github.com/rust-lang/rust/issues/55724>
fn layout_array<T>(n: usize) -> Result<Layout, CollectionAllocErr> {
    let size = mem::size_of::<T>()
        .checked_mul(n)
        .ok_or(CollectionAllocErr::CapacityOverflow)?;
    let align = mem::align_of::<T>();
    Layout::from_size_align(size, align).map_err(|_| CollectionAllocErr::CapacityOverflow)
}

unsafe fn deallocate<T>(ptr: NonNull<T>, capacity: usize) {
    // This unwrap should succeed since the same did when allocating.
    let layout = layout_array::<T>(capacity).unwrap();
    alloc::alloc::dealloc(ptr.as_ptr() as *mut u8, layout)
}

/// An iterator that removes the items from a `SmallVec` and yields them by value.
///
/// Returned from [`SmallVec::drain`][1].
///
/// [1]: struct.SmallVec.html#method.drain
pub struct Drain<'a, T: 'a + Array> {
    tail_start: usize,
    tail_len: usize,
    iter: slice::Iter<'a, T::Item>,
    vec: NonNull<SmallVec<T>>,
}

impl<'a, T: 'a + Array> fmt::Debug for Drain<'a, T>
where
    T::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Drain").field(&self.iter.as_slice()).finish()
    }
}

unsafe impl<'a, T: Sync + Array> Sync for Drain<'a, T> {}
unsafe impl<'a, T: Send + Array> Send for Drain<'a, T> {}

impl<'a, T: 'a + Array> Iterator for Drain<'a, T> {
    type Item = T::Item;

    #[inline]
    fn next(&mut self) -> Option<T::Item> {
        self.iter
            .next()
            .map(|reference| unsafe { ptr::read(reference) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T: 'a + Array> DoubleEndedIterator for Drain<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<T::Item> {
        self.iter
            .next_back()
            .map(|reference| unsafe { ptr::read(reference) })
    }
}

impl<'a, T: Array> ExactSizeIterator for Drain<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, T: Array> FusedIterator for Drain<'a, T> {}

impl<'a, T: 'a + Array> Drop for Drain<'a, T> {
    fn drop(&mut self) {
        self.for_each(drop);

        if self.tail_len > 0 {
            unsafe {
                let source_vec = self.vec.as_mut();

                // memmove back untouched tail, update to new length
                let start = source_vec.len();
                let tail = self.tail_start;
                if tail != start {
                    // as_mut_ptr creates a &mut, invalidating other pointers.
                    // This pattern avoids calling it with a pointer already present.
                    let ptr = source_vec.as_mut_ptr();
                    let src = ptr.add(tail);
                    let dst = ptr.add(start);
                    ptr::copy(src, dst, self.tail_len);
                }
                source_vec.set_len(start + self.tail_len);
            }
        }
    }
}

#[cfg(feature = "drain_filter")]
/// An iterator which uses a closure to determine if an element should be removed.
///
/// Returned from [`SmallVec::drain_filter`][1].
///
/// [1]: struct.SmallVec.html#method.drain_filter
pub struct DrainFilter<'a, T, F>
where
    F: FnMut(&mut T::Item) -> bool,
    T: Array,
{
    vec: &'a mut SmallVec<T>,
    /// The index of the item that will be inspected by the next call to `next`.
    idx: usize,
    /// The number of items that have been drained (removed) thus far.
    del: usize,
    /// The original length of `vec` prior to draining.
    old_len: usize,
    /// The filter test predicate.
    pred: F,
    /// A flag that indicates a panic has occurred in the filter test predicate.
    /// This is used as a hint in the drop implementation to prevent consumption
    /// of the remainder of the `DrainFilter`. Any unprocessed items will be
    /// backshifted in the `vec`, but no further items will be dropped or
    /// tested by the filter predicate.
    panic_flag: bool,
}

#[cfg(feature = "drain_filter")]
impl <T, F> fmt::Debug for DrainFilter<'_, T, F>
where
    F: FnMut(&mut T::Item) -> bool,
    T: Array,
    T::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DrainFilter").field(&self.vec.as_slice()).finish()
    }
}

#[cfg(feature = "drain_filter")]
impl <T, F> Iterator for DrainFilter<'_, T, F>
where
    F: FnMut(&mut T::Item) -> bool,
    T: Array,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<T::Item>
    {
        unsafe {
            while self.idx < self.old_len {
                let i = self.idx;
                let v = slice::from_raw_parts_mut(self.vec.as_mut_ptr(), self.old_len);
                self.panic_flag = true;
                let drained = (self.pred)(&mut v[i]);
                self.panic_flag = false;
                // Update the index *after* the predicate is called. If the index
                // is updated prior and the predicate panics, the element at this
                // index would be leaked.
                self.idx += 1;
                if drained {
                    self.del += 1;
                    return Some(ptr::read(&v[i]));
                } else if self.del > 0 {
                    let del = self.del;
                    let src: *const Self::Item = &v[i];
                    let dst: *mut Self::Item = &mut v[i - del];
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

#[cfg(feature = "drain_filter")]
impl <T, F> Drop for DrainFilter<'_, T, F>
where
    F: FnMut(&mut T::Item) -> bool,
    T: Array,
{
    fn drop(&mut self) {
        struct BackshiftOnDrop<'a, 'b, T, F>
        where
            F: FnMut(&mut T::Item) -> bool,
            T: Array
        {
            drain: &'b mut DrainFilter<'a, T, F>,
        }

        impl<'a, 'b, T, F> Drop for BackshiftOnDrop<'a, 'b, T, F>
        where
            F: FnMut(&mut T::Item) -> bool,
            T: Array
        {
            fn drop(&mut self) {
                unsafe {
                    if self.drain.idx < self.drain.old_len && self.drain.del > 0 {
                        // This is a pretty messed up state, and there isn't really an
                        // obviously right thing to do. We don't want to keep trying
                        // to execute `pred`, so we just backshift all the unprocessed
                        // elements and tell the vec that they still exist. The backshift
                        // is required to prevent a double-drop of the last successfully
                        // drained item prior to a panic in the predicate.
                        let ptr = self.drain.vec.as_mut_ptr();
                        let src = ptr.add(self.drain.idx);
                        let dst = src.sub(self.drain.del);
                        let tail_len = self.drain.old_len - self.drain.idx;
                        src.copy_to(dst, tail_len);
                    }
                    self.drain.vec.set_len(self.drain.old_len - self.drain.del);
                }
            }
        }

        let backshift = BackshiftOnDrop { drain: self };

        // Attempt to consume any remaining elements if the filter predicate
        // has not yet panicked. We'll backshift any remaining elements
        // whether we've already panicked or if the consumption here panics.
        if !backshift.drain.panic_flag {
            backshift.drain.for_each(drop);
        }
    }
}

#[cfg(feature = "drain_keep_rest")]
impl <T, F> DrainFilter<'_, T, F>
where
    F: FnMut(&mut T::Item) -> bool,
    T: Array
{
    /// Keep unyielded elements in the source `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use smallvec::{smallvec, SmallVec};
    ///
    /// let mut vec: SmallVec<[char; 2]> = smallvec!['a', 'b', 'c'];
    /// let mut drain = vec.drain_filter(|_| true);
    ///
    /// assert_eq!(drain.next().unwrap(), 'a');
    ///
    /// // This call keeps 'b' and 'c' in the vec.
    /// drain.keep_rest();
    ///
    /// // If we wouldn't call `keep_rest()`,
    /// // `vec` would be empty.
    /// assert_eq!(vec, SmallVec::<[char; 2]>::from_slice(&['b', 'c']));
    /// ```
    pub fn keep_rest(self)
    {
        // At this moment layout looks like this:
        //
        //  _____________________/-- old_len
        // /                     \
        // [kept] [yielded] [tail]
        //        \_______/ ^-- idx
        //                \-- del
        //
        // Normally `Drop` impl would drop [tail] (via .for_each(drop), ie still calling `pred`)
        //
        // 1. Move [tail] after [kept]
        // 2. Update length of the original vec to `old_len - del`
        //    a. In case of ZST, this is the only thing we want to do
        // 3. Do *not* drop self, as everything is put in a consistent state already, there is nothing to do
        let mut this = ManuallyDrop::new(self);

        unsafe {
            // ZSTs have no identity, so we don't need to move them around.
            let needs_move = mem::size_of::<T>() != 0;

            if needs_move && this.idx < this.old_len && this.del > 0 {
                let ptr = this.vec.as_mut_ptr();
                let src = ptr.add(this.idx);
                let dst = src.sub(this.del);
                let tail_len = this.old_len - this.idx;
                src.copy_to(dst, tail_len);
            }

            let new_len = this.old_len - this.del;
            this.vec.set_len(new_len);
        }
    }
}

#[cfg(feature = "union")]
union SmallVecData<A: Array> {
    inline: core::mem::ManuallyDrop<MaybeUninit<A>>,
    heap: (NonNull<A::Item>, usize),
}

#[cfg(all(feature = "union", feature = "const_new"))]
impl<T, const N: usize> SmallVecData<[T; N]> {
    #[cfg_attr(docsrs, doc(cfg(feature = "const_new")))]
    #[inline]
    const fn from_const(inline: MaybeUninit<[T; N]>) -> Self {
        SmallVecData {
            inline: core::mem::ManuallyDrop::new(inline),
        }
    }
}

#[cfg(feature = "union")]
impl<A: Array> SmallVecData<A> {
    #[inline]
    unsafe fn inline(&self) -> ConstNonNull<A::Item> {
        ConstNonNull::new(self.inline.as_ptr() as *const A::Item).unwrap()
    }
    #[inline]
    unsafe fn inline_mut(&mut self) -> NonNull<A::Item> {
        NonNull::new(self.inline.as_mut_ptr() as *mut A::Item).unwrap()
    }
    #[inline]
    fn from_inline(inline: MaybeUninit<A>) -> SmallVecData<A> {
        SmallVecData {
            inline: core::mem::ManuallyDrop::new(inline),
        }
    }
    #[inline]
    unsafe fn into_inline(self) -> MaybeUninit<A> {
        core::mem::ManuallyDrop::into_inner(self.inline)
    }
    #[inline]
    unsafe fn heap(&self) -> (ConstNonNull<A::Item>, usize) {
        (ConstNonNull(self.heap.0), self.heap.1)
    }
    #[inline]
    unsafe fn heap_mut(&mut self) -> (NonNull<A::Item>, &mut usize) {
        let h = &mut self.heap;
        (h.0, &mut h.1)
    }
    #[inline]
    fn from_heap(ptr: NonNull<A::Item>, len: usize) -> SmallVecData<A> {
        SmallVecData { heap: (ptr, len) }
    }
}

#[cfg(not(feature = "union"))]
enum SmallVecData<A: Array> {
    Inline(MaybeUninit<A>),
    // Using NonNull and NonZero here allows to reduce size of `SmallVec`.
    Heap {
        // Since we never allocate on heap
        // unless our capacity is bigger than inline capacity
        // heap capacity cannot be less than 1.
        // Therefore, pointer cannot be null too.
        ptr: NonNull<A::Item>,
        len: usize,
    },
}

#[cfg(all(not(feature = "union"), feature = "const_new"))]
impl<T, const N: usize> SmallVecData<[T; N]> {
    #[cfg_attr(docsrs, doc(cfg(feature = "const_new")))]
    #[inline]
    const fn from_const(inline: MaybeUninit<[T; N]>) -> Self {
        SmallVecData::Inline(inline)
    }
}

#[cfg(not(feature = "union"))]
impl<A: Array> SmallVecData<A> {
    #[inline]
    unsafe fn inline(&self) -> ConstNonNull<A::Item> {
        match self {
            SmallVecData::Inline(a) => ConstNonNull::new(a.as_ptr() as *const A::Item).unwrap(),
            _ => debug_unreachable!(),
        }
    }
    #[inline]
    unsafe fn inline_mut(&mut self) -> NonNull<A::Item> {
        match self {
            SmallVecData::Inline(a) => NonNull::new(a.as_mut_ptr() as *mut A::Item).unwrap(),
            _ => debug_unreachable!(),
        }
    }
    #[inline]
    fn from_inline(inline: MaybeUninit<A>) -> SmallVecData<A> {
        SmallVecData::Inline(inline)
    }
    #[inline]
    unsafe fn into_inline(self) -> MaybeUninit<A> {
        match self {
            SmallVecData::Inline(a) => a,
            _ => debug_unreachable!(),
        }
    }
    #[inline]
    unsafe fn heap(&self) -> (ConstNonNull<A::Item>, usize) {
        match self {
            SmallVecData::Heap { ptr, len } => (ConstNonNull(*ptr), *len),
            _ => debug_unreachable!(),
        }
    }
    #[inline]
    unsafe fn heap_mut(&mut self) -> (NonNull<A::Item>, &mut usize) {
        match self {
            SmallVecData::Heap { ptr, len } => (*ptr, len),
            _ => debug_unreachable!(),
        }
    }
    #[inline]
    fn from_heap(ptr: NonNull<A::Item>, len: usize) -> SmallVecData<A> {
        SmallVecData::Heap { ptr, len }
    }
}

unsafe impl<A: Array + Send> Send for SmallVecData<A> {}
unsafe impl<A: Array + Sync> Sync for SmallVecData<A> {}

/// A `Vec`-like container that can store a small number of elements inline.
///
/// `SmallVec` acts like a vector, but can store a limited amount of data inline within the
/// `SmallVec` struct rather than in a separate allocation.  If the data exceeds this limit, the
/// `SmallVec` will "spill" its data onto the heap, allocating a new buffer to hold it.
///
/// The amount of data that a `SmallVec` can store inline depends on its backing store. The backing
/// store can be any type that implements the `Array` trait; usually it is a small fixed-sized
/// array.  For example a `SmallVec<[u64; 8]>` can hold up to eight 64-bit integers inline.
///
/// ## Example
///
/// ```rust
/// use smallvec::SmallVec;
/// let mut v = SmallVec::<[u8; 4]>::new(); // initialize an empty vector
///
/// // The vector can hold up to 4 items without spilling onto the heap.
/// v.extend(0..4);
/// assert_eq!(v.len(), 4);
/// assert!(!v.spilled());
///
/// // Pushing another element will force the buffer to spill:
/// v.push(4);
/// assert_eq!(v.len(), 5);
/// assert!(v.spilled());
/// ```
pub struct SmallVec<A: Array> {
    // The capacity field is used to determine which of the storage variants is active:
    // If capacity <= Self::inline_capacity() then the inline variant is used and capacity holds the current length of the vector (number of elements actually in use).
    // If capacity > Self::inline_capacity() then the heap variant is used and capacity holds the size of the memory allocation.
    capacity: usize,
    data: SmallVecData<A>,
}

impl<A: Array> SmallVec<A> {
    /// Construct an empty vector
    #[inline]
    pub fn new() -> SmallVec<A> {
        // Try to detect invalid custom implementations of `Array`. Hopefully,
        // this check should be optimized away entirely for valid ones.
        assert!(
            mem::size_of::<A>() == A::size() * mem::size_of::<A::Item>()
                && mem::align_of::<A>() >= mem::align_of::<A::Item>()
        );
        SmallVec {
            capacity: 0,
            data: SmallVecData::from_inline(MaybeUninit::uninit()),
        }
    }

    /// Construct an empty vector with enough capacity pre-allocated to store at least `n`
    /// elements.
    ///
    /// Will create a heap allocation only if `n` is larger than the inline capacity.
    ///
    /// ```
    /// # use smallvec::SmallVec;
    ///
    /// let v: SmallVec<[u8; 3]> = SmallVec::with_capacity(100);
    ///
    /// assert!(v.is_empty());
    /// assert!(v.capacity() >= 100);
    /// ```
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        let mut v = SmallVec::new();
        v.reserve_exact(n);
        v
    }

    /// Construct a new `SmallVec` from a `Vec<A::Item>`.
    ///
    /// Elements will be copied to the inline buffer if `vec.capacity() <= Self::inline_capacity()`.
    ///
    /// ```rust
    /// use smallvec::SmallVec;
    ///
    /// let vec = vec![1, 2, 3, 4, 5];
    /// let small_vec: SmallVec<[_; 3]> = SmallVec::from_vec(vec);
    ///
    /// assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    /// ```
    #[inline]
    pub fn from_vec(mut vec: Vec<A::Item>) -> SmallVec<A> {
        if vec.capacity() <= Self::inline_capacity() {
            // Cannot use Vec with smaller capacity
            // because we use value of `Self::capacity` field as indicator.
            unsafe {
                let mut data = SmallVecData::<A>::from_inline(MaybeUninit::uninit());
                let len = vec.len();
                vec.set_len(0);
                ptr::copy_nonoverlapping(vec.as_ptr(), data.inline_mut().as_ptr(), len);

                SmallVec {
                    capacity: len,
                    data,
                }
            }
        } else {
            let (ptr, cap, len) = (vec.as_mut_ptr(), vec.capacity(), vec.len());
            mem::forget(vec);
            let ptr = NonNull::new(ptr)
                // See docs: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.as_mut_ptr
                .expect("Cannot be null by `Vec` invariant");

            SmallVec {
                capacity: cap,
                data: SmallVecData::from_heap(ptr, len),
            }
        }
    }

    /// Constructs a new `SmallVec` on the stack from an `A` without
    /// copying elements.
    ///
    /// ```rust
    /// use smallvec::SmallVec;
    ///
    /// let buf = [1, 2, 3, 4, 5];
    /// let small_vec: SmallVec<_> = SmallVec::from_buf(buf);
    ///
    /// assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    /// ```
    #[inline]
    pub fn from_buf(buf: A) -> SmallVec<A> {
        SmallVec {
            capacity: A::size(),
            data: SmallVecData::from_inline(MaybeUninit::new(buf)),
        }
    }

    /// Constructs a new `SmallVec` on the stack from an `A` without
    /// copying elements. Also sets the length, which must be less or
    /// equal to the size of `buf`.
    ///
    /// ```rust
    /// use smallvec::SmallVec;
    ///
    /// let buf = [1, 2, 3, 4, 5, 0, 0, 0];
    /// let small_vec: SmallVec<_> = SmallVec::from_buf_and_len(buf, 5);
    ///
    /// assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    /// ```
    #[inline]
    pub fn from_buf_and_len(buf: A, len: usize) -> SmallVec<A> {
        assert!(len <= A::size());
        unsafe { SmallVec::from_buf_and_len_unchecked(MaybeUninit::new(buf), len) }
    }

    /// Constructs a new `SmallVec` on the stack from an `A` without
    /// copying elements. Also sets the length. The user is responsible
    /// for ensuring that `len <= A::size()`.
    ///
    /// ```rust
    /// use smallvec::SmallVec;
    /// use std::mem::MaybeUninit;
    ///
    /// let buf = [1, 2, 3, 4, 5, 0, 0, 0];
    /// let small_vec: SmallVec<_> = unsafe {
    ///     SmallVec::from_buf_and_len_unchecked(MaybeUninit::new(buf), 5)
    /// };
    ///
    /// assert_eq!(&*small_vec, &[1, 2, 3, 4, 5]);
    /// ```
    #[inline]
    pub unsafe fn from_buf_and_len_unchecked(buf: MaybeUninit<A>, len: usize) -> SmallVec<A> {
        SmallVec {
            capacity: len,
            data: SmallVecData::from_inline(buf),
        }
    }

    /// Sets the length of a vector.
    ///
    /// This will explicitly set the size of the vector, without actually
    /// modifying its buffers, so it is up to the caller to ensure that the
    /// vector is actually the specified size.
    pub unsafe fn set_len(&mut self, new_len: usize) {
        let (_, len_ptr, _) = self.triple_mut();
        *len_ptr = new_len;
    }

    /// The maximum number of elements this vector can hold inline
    #[inline]
    fn inline_capacity() -> usize {
        if mem::size_of::<A::Item>() > 0 {
            A::size()
        } else {
            // For zero-size items code like `ptr.add(offset)` always returns the same pointer.
            // Therefore all items are at the same address,
            // and any array size has capacity for infinitely many items.
            // The capacity is limited by the bit width of the length field.
            //
            // `Vec` also does this:
            // https://github.com/rust-lang/rust/blob/1.44.0/src/liballoc/raw_vec.rs#L186
            //
            // In our case, this also ensures that a smallvec of zero-size items never spills,
            // and we never try to allocate zero bytes which `std::alloc::alloc` disallows.
            core::usize::MAX
        }
    }

    /// The maximum number of elements this vector can hold inline
    #[inline]
    pub fn inline_size(&self) -> usize {
        Self::inline_capacity()
    }

    /// The number of elements stored in the vector
    #[inline]
    pub fn len(&self) -> usize {
        self.triple().1
    }

    /// Returns `true` if the vector is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of items the vector can hold without reallocating
    #[inline]
    pub fn capacity(&self) -> usize {
        self.triple().2
    }

    /// Returns a tuple with (data ptr, len, capacity)
    /// Useful to get all `SmallVec` properties with a single check of the current storage variant.
    #[inline]
    fn triple(&self) -> (ConstNonNull<A::Item>, usize, usize) {
        unsafe {
            if self.spilled() {
                let (ptr, len) = self.data.heap();
                (ptr, len, self.capacity)
            } else {
                (self.data.inline(), self.capacity, Self::inline_capacity())
            }
        }
    }

    /// Returns a tuple with (data ptr, len ptr, capacity)
    #[inline]
    fn triple_mut(&mut self) -> (NonNull<A::Item>, &mut usize, usize) {
        unsafe {
            if self.spilled() {
                let (ptr, len_ptr) = self.data.heap_mut();
                (ptr, len_ptr, self.capacity)
            } else {
                (
                    self.data.inline_mut(),
                    &mut self.capacity,
                    Self::inline_capacity(),
                )
            }
        }
    }

    /// Returns `true` if the data has spilled into a separate heap-allocated buffer.
    #[inline]
    pub fn spilled(&self) -> bool {
        self.capacity > Self::inline_capacity()
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
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, A>
    where
        R: RangeBounds<usize>,
    {
        use core::ops::Bound::*;

        let len = self.len();
        let start = match range.start_bound() {
            Included(&n) => n,
            Excluded(&n) => n.checked_add(1).expect("Range start out of bounds"),
            Unbounded => 0,
        };
        let end = match range.end_bound() {
            Included(&n) => n.checked_add(1).expect("Range end out of bounds"),
            Excluded(&n) => n,
            Unbounded => len,
        };

        assert!(start <= end);
        assert!(end <= len);

        unsafe {
            self.set_len(start);

            let range_slice = slice::from_raw_parts(self.as_ptr().add(start), end - start);

            Drain {
                tail_start: end,
                tail_len: len - end,
                iter: range_slice.iter(),
                // Since self is a &mut, passing it to a function would invalidate the slice iterator.
                vec: NonNull::new_unchecked(self as *mut _),
            }
        }
    }

    #[cfg(feature = "drain_filter")]
    /// Creates an iterator which uses a closure to determine if an element should be removed.
    ///
    /// If the closure returns true, the element is removed and yielded. If the closure returns
    /// false, the element will remain in the vector and will not be yielded by the iterator.
    ///
    /// Using this method is equivalent to the following code:
    /// ```
    /// # use smallvec::SmallVec;
    /// # let some_predicate = |x: &mut i32| { *x == 2 || *x == 3 || *x == 6 };
    /// # let mut vec: SmallVec<[i32; 8]> = SmallVec::from_slice(&[1i32, 2, 3, 4, 5, 6]);
    /// let mut i = 0;
    /// while i < vec.len() {
    ///     if some_predicate(&mut vec[i]) {
    ///         let val = vec.remove(i);
    ///         // your code here
    ///     } else {
    ///         i += 1;
    ///     }
    /// }
    ///
    /// # assert_eq!(vec, SmallVec::<[i32; 8]>::from_slice(&[1i32, 4, 5]));
    /// ```
    /// ///
    /// But `drain_filter` is easier to use. `drain_filter` is also more efficient,
    /// because it can backshift the elements of the array in bulk.
    ///
    /// Note that `drain_filter` also lets you mutate every element in the filter closure,
    /// regardless of whether you choose to keep or remove it.
    ///
    /// # Examples
    ///
    /// Splitting an array into evens and odds, reusing the original allocation:
    ///
    /// ```
    /// # use smallvec::SmallVec;
    /// let mut numbers: SmallVec<[i32; 16]> = SmallVec::from_slice(&[1i32, 2, 3, 4, 5, 6, 8, 9, 11, 13, 14, 15]);
    ///
    /// let evens = numbers.drain_filter(|x| *x % 2 == 0).collect::<SmallVec<[i32; 16]>>();
    /// let odds = numbers;
    ///
    /// assert_eq!(evens, SmallVec::<[i32; 16]>::from_slice(&[2i32, 4, 6, 8, 14]));
    /// assert_eq!(odds, SmallVec::<[i32; 16]>::from_slice(&[1i32, 3, 5, 9, 11, 13, 15]));
    /// ```
    pub fn drain_filter<F>(&mut self, filter: F) -> DrainFilter<'_, A, F,>
    where
        F: FnMut(&mut A::Item) -> bool,
    {
        let old_len = self.len();

        // Guard against us getting leaked (leak amplification)
        unsafe {
            self.set_len(0);
        }

        DrainFilter { vec: self, idx: 0, del: 0, old_len, pred: filter, panic_flag: false }
    }

    /// Append an item to the vector.
    #[inline]
    pub fn push(&mut self, value: A::Item) {
        unsafe {
            let (mut ptr, mut len, cap) = self.triple_mut();
            if *len == cap {
                self.reserve_one_unchecked();
                let (heap_ptr, heap_len) = self.data.heap_mut();
                ptr = heap_ptr;
                len = heap_len;
            }
            ptr::write(ptr.as_ptr().add(*len), value);
            *len += 1;
        }
    }

    /// Remove an item from the end of the vector and return it, or None if empty.
    #[inline]
    pub fn pop(&mut self) -> Option<A::Item> {
        unsafe {
            let (ptr, len_ptr, _) = self.triple_mut();
            let ptr: *const _ = ptr.as_ptr();
            if *len_ptr == 0 {
                return None;
            }
            let last_index = *len_ptr - 1;
            *len_ptr = last_index;
            Some(ptr::read(ptr.add(last_index)))
        }
    }

    /// Moves all the elements of `other` into `self`, leaving `other` empty.
    ///
    /// # Example
    ///
    /// ```
    /// # use smallvec::{SmallVec, smallvec};
    /// let mut v0: SmallVec<[u8; 16]> = smallvec![1, 2, 3];
    /// let mut v1: SmallVec<[u8; 32]> = smallvec![4, 5, 6];
    /// v0.append(&mut v1);
    /// assert_eq!(*v0, [1, 2, 3, 4, 5, 6]);
    /// assert_eq!(*v1, []);
    /// ```
    pub fn append<B>(&mut self, other: &mut SmallVec<B>)
    where
        B: Array<Item = A::Item>,
    {
        self.extend(other.drain(..))
    }

    /// Re-allocate to set the capacity to `max(new_cap, inline_size())`.
    ///
    /// Panics if `new_cap` is less than the vector's length
    /// or if the capacity computation overflows `usize`.
    pub fn grow(&mut self, new_cap: usize) {
        infallible(self.try_grow(new_cap))
    }

    /// Re-allocate to set the capacity to `max(new_cap, inline_size())`.
    ///
    /// Panics if `new_cap` is less than the vector's length
    pub fn try_grow(&mut self, new_cap: usize) -> Result<(), CollectionAllocErr> {
        unsafe {
            let unspilled = !self.spilled();
            let (ptr, &mut len, cap) = self.triple_mut();
            assert!(new_cap >= len);
            if new_cap <= Self::inline_capacity() {
                if unspilled {
                    return Ok(());
                }
                self.data = SmallVecData::from_inline(MaybeUninit::uninit());
                ptr::copy_nonoverlapping(ptr.as_ptr(), self.data.inline_mut().as_ptr(), len);
                self.capacity = len;
                deallocate(ptr, cap);
            } else if new_cap != cap {
                let layout = layout_array::<A::Item>(new_cap)?;
                debug_assert!(layout.size() > 0);
                let new_alloc;
                if unspilled {
                    new_alloc = NonNull::new(alloc::alloc::alloc(layout))
                        .ok_or(CollectionAllocErr::AllocErr { layout })?
                        .cast();
                    ptr::copy_nonoverlapping(ptr.as_ptr(), new_alloc.as_ptr(), len);
                } else {
                    // This should never fail since the same succeeded
                    // when previously allocating `ptr`.
                    let old_layout = layout_array::<A::Item>(cap)?;

                    let new_ptr =
                        alloc::alloc::realloc(ptr.as_ptr() as *mut u8, old_layout, layout.size());
                    new_alloc = NonNull::new(new_ptr)
                        .ok_or(CollectionAllocErr::AllocErr { layout })?
                        .cast();
                }
                self.data = SmallVecData::from_heap(new_alloc, len);
                self.capacity = new_cap;
            }
            Ok(())
        }
    }

    /// Reserve capacity for `additional` more elements to be inserted.
    ///
    /// May reserve more space to avoid frequent reallocations.
    ///
    /// Panics if the capacity computation overflows `usize`.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        infallible(self.try_reserve(additional))
    }

    /// Internal method used to grow in push() and insert(), where we know already we have to grow.
    #[cold]
    fn reserve_one_unchecked(&mut self) {
        debug_assert_eq!(self.len(), self.capacity());
        let new_cap = self.len()
            .checked_add(1)
            .and_then(usize::checked_next_power_of_two)
            .expect("capacity overflow");
        infallible(self.try_grow(new_cap))
    }

    /// Reserve capacity for `additional` more elements to be inserted.
    ///
    /// May reserve more space to avoid frequent reallocations.
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), CollectionAllocErr> {
        // prefer triple_mut() even if triple() would work so that the optimizer removes duplicated
        // calls to it from callers.
        let (_, &mut len, cap) = self.triple_mut();
        if cap - len >= additional {
            return Ok(());
        }
        let new_cap = len
            .checked_add(additional)
            .and_then(usize::checked_next_power_of_two)
            .ok_or(CollectionAllocErr::CapacityOverflow)?;
        self.try_grow(new_cap)
    }

    /// Reserve the minimum capacity for `additional` more elements to be inserted.
    ///
    /// Panics if the new capacity overflows `usize`.
    pub fn reserve_exact(&mut self, additional: usize) {
        infallible(self.try_reserve_exact(additional))
    }

    /// Reserve the minimum capacity for `additional` more elements to be inserted.
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), CollectionAllocErr> {
        let (_, &mut len, cap) = self.triple_mut();
        if cap - len >= additional {
            return Ok(());
        }
        let new_cap = len
            .checked_add(additional)
            .ok_or(CollectionAllocErr::CapacityOverflow)?;
        self.try_grow(new_cap)
    }

    /// Shrink the capacity of the vector as much as possible.
    ///
    /// When possible, this will move data from an external heap buffer to the vector's inline
    /// storage.
    pub fn shrink_to_fit(&mut self) {
        if !self.spilled() {
            return;
        }
        let len = self.len();
        if self.inline_size() >= len {
            unsafe {
                let (ptr, len) = self.data.heap();
                self.data = SmallVecData::from_inline(MaybeUninit::uninit());
                ptr::copy_nonoverlapping(ptr.as_ptr(), self.data.inline_mut().as_ptr(), len);
                deallocate(ptr.0, self.capacity);
                self.capacity = len;
            }
        } else if self.capacity() > len {
            self.grow(len);
        }
    }

    /// Shorten the vector, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than or equal to the vector's current length, this has no
    /// effect.
    ///
    /// This does not re-allocate.  If you want the vector's capacity to shrink, call
    /// `shrink_to_fit` after truncating.
    pub fn truncate(&mut self, len: usize) {
        unsafe {
            let (ptr, len_ptr, _) = self.triple_mut();
            let ptr = ptr.as_ptr();
            while len < *len_ptr {
                let last_index = *len_ptr - 1;
                *len_ptr = last_index;
                ptr::drop_in_place(ptr.add(last_index));
            }
        }
    }

    /// Extracts a slice containing the entire vector.
    ///
    /// Equivalent to `&s[..]`.
    pub fn as_slice(&self) -> &[A::Item] {
        self
    }

    /// Extracts a mutable slice of the entire vector.
    ///
    /// Equivalent to `&mut s[..]`.
    pub fn as_mut_slice(&mut self) -> &mut [A::Item] {
        self
    }

    /// Remove the element at position `index`, replacing it with the last element.
    ///
    /// This does not preserve ordering, but is O(1).
    ///
    /// Panics if `index` is out of bounds.
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> A::Item {
        let len = self.len();
        self.swap(len - 1, index);
        self.pop()
            .unwrap_or_else(|| unsafe { unreachable_unchecked() })
    }

    /// Remove all elements from the vector.
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Remove and return the element at position `index`, shifting all elements after it to the
    /// left.
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> A::Item {
        unsafe {
            let (ptr, len_ptr, _) = self.triple_mut();
            let len = *len_ptr;
            assert!(index < len);
            *len_ptr = len - 1;
            let ptr = ptr.as_ptr().add(index);
            let item = ptr::read(ptr);
            ptr::copy(ptr.add(1), ptr, len - index - 1);
            item
        }
    }

    /// Insert an element at position `index`, shifting all elements after it to the right.
    ///
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, element: A::Item) {
        unsafe {
            let (mut ptr, mut len_ptr, cap) = self.triple_mut();
            if *len_ptr == cap {
                self.reserve_one_unchecked();
                let (heap_ptr, heap_len_ptr) = self.data.heap_mut();
                ptr = heap_ptr;
                len_ptr = heap_len_ptr;
            }
            let mut ptr = ptr.as_ptr();
            let len = *len_ptr;
            if index > len {
                panic!("index exceeds length");
            }
            // SAFETY: add is UB if index > len, but we panicked first
            ptr = ptr.add(index);
            if index < len {
                // Shift element to the right of `index`.
                ptr::copy(ptr, ptr.add(1), len - index);
            }
            *len_ptr = len + 1;
            ptr::write(ptr, element);
        }
    }

    /// Insert multiple elements at position `index`, shifting all following elements toward the
    /// back.
    pub fn insert_many<I: IntoIterator<Item = A::Item>>(&mut self, index: usize, iterable: I) {
        let mut iter = iterable.into_iter();
        if index == self.len() {
            return self.extend(iter);
        }

        let (lower_size_bound, _) = iter.size_hint();
        assert!(lower_size_bound <= core::isize::MAX as usize); // Ensure offset is indexable
        assert!(index + lower_size_bound >= index); // Protect against overflow

        let mut num_added = 0;
        let old_len = self.len();
        assert!(index <= old_len);

        unsafe {
            // Reserve space for `lower_size_bound` elements.
            self.reserve(lower_size_bound);
            let start = self.as_mut_ptr();
            let ptr = start.add(index);

            // Move the trailing elements.
            ptr::copy(ptr, ptr.add(lower_size_bound), old_len - index);

            // In case the iterator panics, don't double-drop the items we just copied above.
            self.set_len(0);
            let mut guard = DropOnPanic {
                start,
                skip: index..(index + lower_size_bound),
                len: old_len + lower_size_bound,
            };

            // The set_len above invalidates the previous pointers, so we must re-create them.
            let start = self.as_mut_ptr();
            let ptr = start.add(index);

            while num_added < lower_size_bound {
                let element = match iter.next() {
                    Some(x) => x,
                    None => break,
                };
                let cur = ptr.add(num_added);
                ptr::write(cur, element);
                guard.skip.start += 1;
                num_added += 1;
            }

            if num_added < lower_size_bound {
                // Iterator provided fewer elements than the hint. Move the tail backward.
                ptr::copy(
                    ptr.add(lower_size_bound),
                    ptr.add(num_added),
                    old_len - index,
                );
            }
            // There are no more duplicate or uninitialized slots, so the guard is not needed.
            self.set_len(old_len + num_added);
            mem::forget(guard);
        }

        // Insert any remaining elements one-by-one.
        for element in iter {
            self.insert(index + num_added, element);
            num_added += 1;
        }

        struct DropOnPanic<T> {
            start: *mut T,
            skip: Range<usize>, // Space we copied-out-of, but haven't written-to yet.
            len: usize,
        }

        impl<T> Drop for DropOnPanic<T> {
            fn drop(&mut self) {
                for i in 0..self.len {
                    if !self.skip.contains(&i) {
                        unsafe {
                            ptr::drop_in_place(self.start.add(i));
                        }
                    }
                }
            }
        }
    }

    /// Convert a `SmallVec` to a `Vec`, without reallocating if the `SmallVec` has already spilled onto
    /// the heap.
    pub fn into_vec(mut self) -> Vec<A::Item> {
        if self.spilled() {
            unsafe {
                let (ptr, &mut len) = self.data.heap_mut();
                let v = Vec::from_raw_parts(ptr.as_ptr(), len, self.capacity);
                mem::forget(self);
                v
            }
        } else {
            self.into_iter().collect()
        }
    }

    /// Converts a `SmallVec` into a `Box<[T]>` without reallocating if the `SmallVec` has already spilled
    /// onto the heap.
    ///
    /// Note that this will drop any excess capacity.
    pub fn into_boxed_slice(self) -> Box<[A::Item]> {
        self.into_vec().into_boxed_slice()
    }

    /// Convert the `SmallVec` into an `A` if possible. Otherwise return `Err(Self)`.
    ///
    /// This method returns `Err(Self)` if the `SmallVec` is too short (and the `A` contains uninitialized elements),
    /// or if the `SmallVec` is too long (and all the elements were spilled to the heap).
    pub fn into_inner(self) -> Result<A, Self> {
        if self.spilled() || self.len() != A::size() {
            // Note: A::size, not Self::inline_capacity
            Err(self)
        } else {
            unsafe {
                let data = ptr::read(&self.data);
                mem::forget(self);
                Ok(data.into_inline().assume_init())
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`.
    /// This method operates in place and preserves the order of the retained
    /// elements.
    pub fn retain<F: FnMut(&mut A::Item) -> bool>(&mut self, mut f: F) {
        let mut del = 0;
        let len = self.len();
        for i in 0..len {
            if !f(&mut self[i]) {
                del += 1;
            } else if del > 0 {
                self.swap(i - del, i);
            }
        }
        self.truncate(len - del);
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// This method is identical in behaviour to [`retain`]; it is included only
    /// to maintain api-compatibility with `std::Vec`, where the methods are
    /// separate for historical reasons.
    pub fn retain_mut<F: FnMut(&mut A::Item) -> bool>(&mut self, f: F) {
        self.retain(f)
    }

    /// Removes consecutive duplicate elements.
    pub fn dedup(&mut self)
    where
        A::Item: PartialEq<A::Item>,
    {
        self.dedup_by(|a, b| a == b);
    }

    /// Removes consecutive duplicate elements using the given equality relation.
    pub fn dedup_by<F>(&mut self, mut same_bucket: F)
    where
        F: FnMut(&mut A::Item, &mut A::Item) -> bool,
    {
        // See the implementation of Vec::dedup_by in the
        // standard library for an explanation of this algorithm.
        let len = self.len();
        if len <= 1 {
            return;
        }

        let ptr = self.as_mut_ptr();
        let mut w: usize = 1;

        unsafe {
            for r in 1..len {
                let p_r = ptr.add(r);
                let p_wm1 = ptr.add(w - 1);
                if !same_bucket(&mut *p_r, &mut *p_wm1) {
                    if r != w {
                        let p_w = p_wm1.add(1);
                        mem::swap(&mut *p_r, &mut *p_w);
                    }
                    w += 1;
                }
            }
        }

        self.truncate(w);
    }

    /// Removes consecutive elements that map to the same key.
    pub fn dedup_by_key<F, K>(&mut self, mut key: F)
    where
        F: FnMut(&mut A::Item) -> K,
        K: PartialEq<K>,
    {
        self.dedup_by(|a, b| key(a) == key(b));
    }

    /// Resizes the `SmallVec` in-place so that `len` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len`, the `SmallVec` is extended by the difference, with each
    /// additional slot filled with the result of calling the closure `f`. The return values from `f`
    /// will end up in the `SmallVec` in the order they have been generated.
    ///
    /// If `new_len` is less than `len`, the `SmallVec` is simply truncated.
    ///
    /// This method uses a closure to create new values on every push. If you'd rather `Clone` a given
    /// value, use `resize`. If you want to use the `Default` trait to generate values, you can pass
    /// `Default::default()` as the second argument.
    ///
    /// Added for `std::vec::Vec` compatibility (added in Rust 1.33.0)
    ///
    /// ```
    /// # use smallvec::{smallvec, SmallVec};
    /// let mut vec : SmallVec<[_; 4]> = smallvec![1, 2, 3];
    /// vec.resize_with(5, Default::default);
    /// assert_eq!(&*vec, &[1, 2, 3, 0, 0]);
    ///
    /// let mut vec : SmallVec<[_; 4]> = smallvec![];
    /// let mut p = 1;
    /// vec.resize_with(4, || { p *= 2; p });
    /// assert_eq!(&*vec, &[2, 4, 8, 16]);
    /// ```
    pub fn resize_with<F>(&mut self, new_len: usize, f: F)
    where
        F: FnMut() -> A::Item,
    {
        let old_len = self.len();
        if old_len < new_len {
            let mut f = f;
            let additional = new_len - old_len;
            self.reserve(additional);
            for _ in 0..additional {
                self.push(f());
            }
        } else if old_len > new_len {
            self.truncate(new_len);
        }
    }

    /// Creates a `SmallVec` directly from the raw components of another
    /// `SmallVec`.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * `ptr` needs to have been previously allocated via `SmallVec` for its
    ///   spilled storage (at least, it's highly likely to be incorrect if it
    ///   wasn't).
    /// * `ptr`'s `A::Item` type needs to be the same size and alignment that
    ///   it was allocated with
    /// * `length` needs to be less than or equal to `capacity`.
    /// * `capacity` needs to be the capacity that the pointer was allocated
    ///   with.
    ///
    /// Violating these may cause problems like corrupting the allocator's
    /// internal data structures.
    ///
    /// Additionally, `capacity` must be greater than the amount of inline
    /// storage `A` has; that is, the new `SmallVec` must need to spill over
    /// into heap allocated storage. This condition is asserted against.
    ///
    /// The ownership of `ptr` is effectively transferred to the
    /// `SmallVec` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// # Examples
    ///
    /// ```
    /// # use smallvec::{smallvec, SmallVec};
    /// use std::mem;
    /// use std::ptr;
    ///
    /// fn main() {
    ///     let mut v: SmallVec<[_; 1]> = smallvec![1, 2, 3];
    ///
    ///     // Pull out the important parts of `v`.
    ///     let p = v.as_mut_ptr();
    ///     let len = v.len();
    ///     let cap = v.capacity();
    ///     let spilled = v.spilled();
    ///
    ///     unsafe {
    ///         // Forget all about `v`. The heap allocation that stored the
    ///         // three values won't be deallocated.
    ///         mem::forget(v);
    ///
    ///         // Overwrite memory with [4, 5, 6].
    ///         //
    ///         // This is only safe if `spilled` is true! Otherwise, we are
    ///         // writing into the old `SmallVec`'s inline storage on the
    ///         // stack.
    ///         assert!(spilled);
    ///         for i in 0..len {
    ///             ptr::write(p.add(i), 4 + i);
    ///         }
    ///
    ///         // Put everything back together into a SmallVec with a different
    ///         // amount of inline storage, but which is still less than `cap`.
    ///         let rebuilt = SmallVec::<[_; 2]>::from_raw_parts(p, len, cap);
    ///         assert_eq!(&*rebuilt, &[4, 5, 6]);
    ///     }
    /// }
    #[inline]
    pub unsafe fn from_raw_parts(ptr: *mut A::Item, length: usize, capacity: usize) -> SmallVec<A> {
        // SAFETY: We require caller to provide same ptr as we alloc
        // and we never alloc null pointer.
        let ptr = unsafe {
            debug_assert!(!ptr.is_null(), "Called `from_raw_parts` with null pointer.");
            NonNull::new_unchecked(ptr)
        };
        assert!(capacity > Self::inline_capacity());
        SmallVec {
            capacity,
            data: SmallVecData::from_heap(ptr, length),
        }
    }

    /// Returns a raw pointer to the vector's buffer.
    pub fn as_ptr(&self) -> *const A::Item {
        // We shadow the slice method of the same name to avoid going through
        // `deref`, which creates an intermediate reference that may place
        // additional safety constraints on the contents of the slice.
        self.triple().0.as_ptr()
    }

    /// Returns a raw mutable pointer to the vector's buffer.
    pub fn as_mut_ptr(&mut self) -> *mut A::Item {
        // We shadow the slice method of the same name to avoid going through
        // `deref_mut`, which creates an intermediate reference that may place
        // additional safety constraints on the contents of the slice.
        self.triple_mut().0.as_ptr()
    }
}

impl<A: Array> SmallVec<A>
where
    A::Item: Copy,
{
    /// Copy the elements from a slice into a new `SmallVec`.
    ///
    /// For slices of `Copy` types, this is more efficient than `SmallVec::from(slice)`.
    pub fn from_slice(slice: &[A::Item]) -> Self {
        let len = slice.len();
        if len <= Self::inline_capacity() {
            SmallVec {
                capacity: len,
                data: SmallVecData::from_inline(unsafe {
                    let mut data: MaybeUninit<A> = MaybeUninit::uninit();
                    ptr::copy_nonoverlapping(
                        slice.as_ptr(),
                        data.as_mut_ptr() as *mut A::Item,
                        len,
                    );
                    data
                }),
            }
        } else {
            let mut b = slice.to_vec();
            let cap = b.capacity();
            let ptr = NonNull::new(b.as_mut_ptr()).expect("Vec always contain non null pointers.");
            mem::forget(b);
            SmallVec {
                capacity: cap,
                data: SmallVecData::from_heap(ptr, len),
            }
        }
    }

    /// Copy elements from a slice into the vector at position `index`, shifting any following
    /// elements toward the back.
    ///
    /// For slices of `Copy` types, this is more efficient than `insert`.
    #[inline]
    pub fn insert_from_slice(&mut self, index: usize, slice: &[A::Item]) {
        self.reserve(slice.len());

        let len = self.len();
        assert!(index <= len);

        unsafe {
            let slice_ptr = slice.as_ptr();
            let ptr = self.as_mut_ptr().add(index);
            ptr::copy(ptr, ptr.add(slice.len()), len - index);
            ptr::copy_nonoverlapping(slice_ptr, ptr, slice.len());
            self.set_len(len + slice.len());
        }
    }

    /// Copy elements from a slice and append them to the vector.
    ///
    /// For slices of `Copy` types, this is more efficient than `extend`.
    #[inline]
    pub fn extend_from_slice(&mut self, slice: &[A::Item]) {
        let len = self.len();
        self.insert_from_slice(len, slice);
    }
}

impl<A: Array> SmallVec<A>
where
    A::Item: Clone,
{
    /// Resizes the vector so that its length is equal to `len`.
    ///
    /// If `len` is less than the current length, the vector simply truncated.
    ///
    /// If `len` is greater than the current length, `value` is appended to the
    /// vector until its length equals `len`.
    pub fn resize(&mut self, len: usize, value: A::Item) {
        let old_len = self.len();

        if len > old_len {
            self.extend(repeat(value).take(len - old_len));
        } else {
            self.truncate(len);
        }
    }

    /// Creates a `SmallVec` with `n` copies of `elem`.
    /// ```
    /// use smallvec::SmallVec;
    ///
    /// let v = SmallVec::<[char; 128]>::from_elem('d', 2);
    /// assert_eq!(v, SmallVec::from_buf(['d', 'd']));
    /// ```
    pub fn from_elem(elem: A::Item, n: usize) -> Self {
        if n > Self::inline_capacity() {
            vec![elem; n].into()
        } else {
            let mut v = SmallVec::<A>::new();
            unsafe {
                let (ptr, len_ptr, _) = v.triple_mut();
                let ptr = ptr.as_ptr();
                let mut local_len = SetLenOnDrop::new(len_ptr);

                for i in 0..n {
                    ::core::ptr::write(ptr.add(i), elem.clone());
                    local_len.increment_len(1);
                }
            }
            v
        }
    }
}

impl<A: Array> ops::Deref for SmallVec<A> {
    type Target = [A::Item];
    #[inline]
    fn deref(&self) -> &[A::Item] {
        unsafe {
            let (ptr, len, _) = self.triple();
            slice::from_raw_parts(ptr.as_ptr(), len)
        }
    }
}

impl<A: Array> ops::DerefMut for SmallVec<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [A::Item] {
        unsafe {
            let (ptr, &mut len, _) = self.triple_mut();
            slice::from_raw_parts_mut(ptr.as_ptr(), len)
        }
    }
}

impl<A: Array> AsRef<[A::Item]> for SmallVec<A> {
    #[inline]
    fn as_ref(&self) -> &[A::Item] {
        self
    }
}

impl<A: Array> AsMut<[A::Item]> for SmallVec<A> {
    #[inline]
    fn as_mut(&mut self) -> &mut [A::Item] {
        self
    }
}

impl<A: Array> Borrow<[A::Item]> for SmallVec<A> {
    #[inline]
    fn borrow(&self) -> &[A::Item] {
        self
    }
}

impl<A: Array> BorrowMut<[A::Item]> for SmallVec<A> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [A::Item] {
        self
    }
}

#[cfg(feature = "write")]
#[cfg_attr(docsrs, doc(cfg(feature = "write")))]
impl<A: Array<Item = u8>> io::Write for SmallVec<A> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
impl<A: Array> Serialize for SmallVec<A>
where
    A::Item: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_seq(Some(self.len()))?;
        for item in self {
            state.serialize_element(&item)?;
        }
        state.end()
    }
}

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
impl<'de, A: Array> Deserialize<'de> for SmallVec<A>
where
    A::Item: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(SmallVecVisitor {
            phantom: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
struct SmallVecVisitor<A> {
    phantom: PhantomData<A>,
}

#[cfg(feature = "serde")]
impl<'de, A: Array> Visitor<'de> for SmallVecVisitor<A>
where
    A::Item: Deserialize<'de>,
{
    type Value = SmallVec<A>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence")
    }

    fn visit_seq<B>(self, mut seq: B) -> Result<Self::Value, B::Error>
    where
        B: SeqAccess<'de>,
    {
        use serde::de::Error;
        let len = seq.size_hint().unwrap_or(0);
        let mut values = SmallVec::new();
        values.try_reserve(len).map_err(B::Error::custom)?;

        while let Some(value) = seq.next_element()? {
            values.push(value);
        }

        Ok(values)
    }
}

#[cfg(feature = "malloc_size_of")]
impl<A: Array> MallocShallowSizeOf for SmallVec<A> {
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        if self.spilled() {
            unsafe { ops.malloc_size_of(self.as_ptr()) }
        } else {
            0
        }
    }
}

#[cfg(feature = "malloc_size_of")]
impl<A> MallocSizeOf for SmallVec<A>
where
    A: Array,
    A::Item: MallocSizeOf,
{
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for elem in self.iter() {
            n += elem.size_of(ops);
        }
        n
    }
}

#[cfg(feature = "specialization")]
trait SpecFrom<A: Array, S> {
    fn spec_from(slice: S) -> SmallVec<A>;
}

#[cfg(feature = "specialization")]
mod specialization;

#[cfg(feature = "arbitrary")]
mod arbitrary;

#[cfg(feature = "specialization")]
impl<'a, A: Array> SpecFrom<A, &'a [A::Item]> for SmallVec<A>
where
    A::Item: Copy,
{
    #[inline]
    fn spec_from(slice: &'a [A::Item]) -> SmallVec<A> {
        SmallVec::from_slice(slice)
    }
}

impl<'a, A: Array> From<&'a [A::Item]> for SmallVec<A>
where
    A::Item: Clone,
{
    #[cfg(not(feature = "specialization"))]
    #[inline]
    fn from(slice: &'a [A::Item]) -> SmallVec<A> {
        slice.iter().cloned().collect()
    }

    #[cfg(feature = "specialization")]
    #[inline]
    fn from(slice: &'a [A::Item]) -> SmallVec<A> {
        SmallVec::spec_from(slice)
    }
}

impl<A: Array> From<Vec<A::Item>> for SmallVec<A> {
    #[inline]
    fn from(vec: Vec<A::Item>) -> SmallVec<A> {
        SmallVec::from_vec(vec)
    }
}

impl<A: Array> From<A> for SmallVec<A> {
    #[inline]
    fn from(array: A) -> SmallVec<A> {
        SmallVec::from_buf(array)
    }
}

impl<A: Array, I: SliceIndex<[A::Item]>> ops::Index<I> for SmallVec<A> {
    type Output = I::Output;

    fn index(&self, index: I) -> &I::Output {
        &(**self)[index]
    }
}

impl<A: Array, I: SliceIndex<[A::Item]>> ops::IndexMut<I> for SmallVec<A> {
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        &mut (&mut **self)[index]
    }
}

#[allow(deprecated)]
impl<A: Array> ExtendFromSlice<A::Item> for SmallVec<A>
where
    A::Item: Copy,
{
    fn extend_from_slice(&mut self, other: &[A::Item]) {
        SmallVec::extend_from_slice(self, other)
    }
}

impl<A: Array> FromIterator<A::Item> for SmallVec<A> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = A::Item>>(iterable: I) -> SmallVec<A> {
        let mut v = SmallVec::new();
        v.extend(iterable);
        v
    }
}

impl<A: Array> Extend<A::Item> for SmallVec<A> {
    fn extend<I: IntoIterator<Item = A::Item>>(&mut self, iterable: I) {
        let mut iter = iterable.into_iter();
        let (lower_size_bound, _) = iter.size_hint();
        self.reserve(lower_size_bound);

        unsafe {
            let (ptr, len_ptr, cap) = self.triple_mut();
            let ptr = ptr.as_ptr();
            let mut len = SetLenOnDrop::new(len_ptr);
            while len.get() < cap {
                if let Some(out) = iter.next() {
                    ptr::write(ptr.add(len.get()), out);
                    len.increment_len(1);
                } else {
                    return;
                }
            }
        }

        for elem in iter {
            self.push(elem);
        }
    }
}

impl<A: Array> fmt::Debug for SmallVec<A>
where
    A::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<A: Array> Default for SmallVec<A> {
    #[inline]
    fn default() -> SmallVec<A> {
        SmallVec::new()
    }
}

#[cfg(feature = "may_dangle")]
unsafe impl<#[may_dangle] A: Array> Drop for SmallVec<A> {
    fn drop(&mut self) {
        unsafe {
            if self.spilled() {
                let (ptr, &mut len) = self.data.heap_mut();
                Vec::from_raw_parts(ptr.as_ptr(), len, self.capacity);
            } else {
                ptr::drop_in_place(&mut self[..]);
            }
        }
    }
}

#[cfg(not(feature = "may_dangle"))]
impl<A: Array> Drop for SmallVec<A> {
    fn drop(&mut self) {
        unsafe {
            if self.spilled() {
                let (ptr, &mut len) = self.data.heap_mut();
                drop(Vec::from_raw_parts(ptr.as_ptr(), len, self.capacity));
            } else {
                ptr::drop_in_place(&mut self[..]);
            }
        }
    }
}

impl<A: Array> Clone for SmallVec<A>
where
    A::Item: Clone,
{
    #[inline]
    fn clone(&self) -> SmallVec<A> {
        SmallVec::from(self.as_slice())
    }

    fn clone_from(&mut self, source: &Self) {
        // Inspired from `impl Clone for Vec`.

        // drop anything that will not be overwritten
        self.truncate(source.len());

        // self.len <= other.len due to the truncate above, so the
        // slices here are always in-bounds.
        let (init, tail) = source.split_at(self.len());

        // reuse the contained values' allocations/resources.
        self.clone_from_slice(init);
        self.extend(tail.iter().cloned());
    }
}

impl<A: Array, B: Array> PartialEq<SmallVec<B>> for SmallVec<A>
where
    A::Item: PartialEq<B::Item>,
{
    #[inline]
    fn eq(&self, other: &SmallVec<B>) -> bool {
        self[..] == other[..]
    }
}

impl<A: Array> Eq for SmallVec<A> where A::Item: Eq {}

impl<A: Array> PartialOrd for SmallVec<A>
where
    A::Item: PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &SmallVec<A>) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
}

impl<A: Array> Ord for SmallVec<A>
where
    A::Item: Ord,
{
    #[inline]
    fn cmp(&self, other: &SmallVec<A>) -> cmp::Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<A: Array> Hash for SmallVec<A>
where
    A::Item: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}

unsafe impl<A: Array> Send for SmallVec<A> where A::Item: Send {}

/// An iterator that consumes a `SmallVec` and yields its items by value.
///
/// Returned from [`SmallVec::into_iter`][1].
///
/// [1]: struct.SmallVec.html#method.into_iter
pub struct IntoIter<A: Array> {
    data: SmallVec<A>,
    current: usize,
    end: usize,
}

impl<A: Array> fmt::Debug for IntoIter<A>
where
    A::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

impl<A: Array + Clone> Clone for IntoIter<A>
where
    A::Item: Clone,
{
    fn clone(&self) -> IntoIter<A> {
        SmallVec::from(self.as_slice()).into_iter()
    }
}

impl<A: Array> Drop for IntoIter<A> {
    fn drop(&mut self) {
        for _ in self {}
    }
}

impl<A: Array> Iterator for IntoIter<A> {
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        if self.current == self.end {
            None
        } else {
            unsafe {
                let current = self.current;
                self.current += 1;
                Some(ptr::read(self.data.as_ptr().add(current)))
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.end - self.current;
        (size, Some(size))
    }
}

impl<A: Array> DoubleEndedIterator for IntoIter<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        if self.current == self.end {
            None
        } else {
            unsafe {
                self.end -= 1;
                Some(ptr::read(self.data.as_ptr().add(self.end)))
            }
        }
    }
}

impl<A: Array> ExactSizeIterator for IntoIter<A> {}
impl<A: Array> FusedIterator for IntoIter<A> {}

impl<A: Array> IntoIter<A> {
    /// Returns the remaining items of this iterator as a slice.
    pub fn as_slice(&self) -> &[A::Item] {
        let len = self.end - self.current;
        unsafe { core::slice::from_raw_parts(self.data.as_ptr().add(self.current), len) }
    }

    /// Returns the remaining items of this iterator as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [A::Item] {
        let len = self.end - self.current;
        unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr().add(self.current), len) }
    }
}

impl<A: Array> IntoIterator for SmallVec<A> {
    type IntoIter = IntoIter<A>;
    type Item = A::Item;
    fn into_iter(mut self) -> Self::IntoIter {
        unsafe {
            // Set SmallVec len to zero as `IntoIter` drop handles dropping of the elements
            let len = self.len();
            self.set_len(0);
            IntoIter {
                data: self,
                current: 0,
                end: len,
            }
        }
    }
}

impl<'a, A: Array> IntoIterator for &'a SmallVec<A> {
    type IntoIter = slice::Iter<'a, A::Item>;
    type Item = &'a A::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A: Array> IntoIterator for &'a mut SmallVec<A> {
    type IntoIter = slice::IterMut<'a, A::Item>;
    type Item = &'a mut A::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// Types that can be used as the backing store for a [`SmallVec`].
pub unsafe trait Array {
    /// The type of the array's elements.
    type Item;
    /// Returns the number of items the array can hold.
    fn size() -> usize;
}

/// Set the length of the vec when the `SetLenOnDrop` value goes out of scope.
///
/// Copied from <https://github.com/rust-lang/rust/pull/36355>
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
    fn get(&self) -> usize {
        self.local_len
    }

    #[inline]
    fn increment_len(&mut self, increment: usize) {
        self.local_len += increment;
    }
}

impl<'a> Drop for SetLenOnDrop<'a> {
    #[inline]
    fn drop(&mut self) {
        *self.len = self.local_len;
    }
}

#[cfg(feature = "const_new")]
impl<T, const N: usize> SmallVec<[T; N]> {
    /// Construct an empty vector.
    ///
    /// This is a `const` version of [`SmallVec::new`] that is enabled by the feature `const_new`, with the limitation that it only works for arrays.
    #[cfg_attr(docsrs, doc(cfg(feature = "const_new")))]
    #[inline]
    pub const fn new_const() -> Self {
        SmallVec {
            capacity: 0,
            data: SmallVecData::from_const(MaybeUninit::uninit()),
        }
    }

    /// The array passed as an argument is moved to be an inline version of `SmallVec`.
    ///
    /// This is a `const` version of [`SmallVec::from_buf`] that is enabled by the feature `const_new`, with the limitation that it only works for arrays.
    #[cfg_attr(docsrs, doc(cfg(feature = "const_new")))]
    #[inline]
    pub const fn from_const(items: [T; N]) -> Self {
        SmallVec {
            capacity: N,
            data: SmallVecData::from_const(MaybeUninit::new(items)),
        }
    }

    /// Constructs a new `SmallVec` on the stack from an array without
    /// copying elements. Also sets the length. The user is responsible
    /// for ensuring that `len <= N`.
    /// 
    /// This is a `const` version of [`SmallVec::from_buf_and_len_unchecked`] that is enabled by the feature `const_new`, with the limitation that it only works for arrays.
    #[cfg_attr(docsrs, doc(cfg(feature = "const_new")))]
    #[inline]
    pub const unsafe fn from_const_with_len_unchecked(items: [T; N], len: usize) -> Self {
        SmallVec {
            capacity: len,
            data: SmallVecData::from_const(MaybeUninit::new(items)),
        }
    }
}

#[cfg(feature = "const_generics")]
#[cfg_attr(docsrs, doc(cfg(feature = "const_generics")))]
unsafe impl<T, const N: usize> Array for [T; N] {
    type Item = T;
    #[inline]
    fn size() -> usize {
        N
    }
}

#[cfg(not(feature = "const_generics"))]
macro_rules! impl_array(
    ($($size:expr),+) => {
        $(
            unsafe impl<T> Array for [T; $size] {
                type Item = T;
                #[inline]
                fn size() -> usize { $size }
            }
        )+
    }
);

#[cfg(not(feature = "const_generics"))]
impl_array!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 36, 0x40, 0x60, 0x80, 0x100, 0x200, 0x400, 0x600, 0x800, 0x1000,
    0x2000, 0x4000, 0x6000, 0x8000, 0x10000, 0x20000, 0x40000, 0x60000, 0x80000, 0x10_0000
);

/// Convenience trait for constructing a `SmallVec`
pub trait ToSmallVec<A: Array> {
    /// Construct a new `SmallVec` from a slice.
    fn to_smallvec(&self) -> SmallVec<A>;
}

impl<A: Array> ToSmallVec<A> for [A::Item]
where
    A::Item: Copy,
{
    #[inline]
    fn to_smallvec(&self) -> SmallVec<A> {
        SmallVec::from_slice(self)
    }
}

// Immutable counterpart for `NonNull<T>`.
#[repr(transparent)]
struct ConstNonNull<T>(NonNull<T>);

impl<T> ConstNonNull<T> {
    #[inline]
    fn new(ptr: *const T) -> Option<Self> {
        NonNull::new(ptr as *mut T).map(Self)
    }
    #[inline]
    fn as_ptr(self) -> *const T {
        self.0.as_ptr()
    }
}

impl<T> Clone for ConstNonNull<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ConstNonNull<T> {}

#[cfg(feature = "impl_bincode")]
use bincode::{
    de::{BorrowDecoder, Decode, Decoder, read::Reader},
    enc::{Encode, Encoder, write::Writer},
    error::{DecodeError, EncodeError},
    BorrowDecode,
};

#[cfg(feature = "impl_bincode")]
impl<A, Context> Decode<Context> for SmallVec<A>
where
    A: Array,
    A::Item: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        use core::convert::TryInto;
        let len = u64::decode(decoder)?;
        let len = len.try_into().map_err(|_| DecodeError::OutsideUsizeRange(len))?;
        decoder.claim_container_read::<A::Item>(len)?;

        let mut vec = SmallVec::with_capacity(len);
        if unty::type_equal::<A::Item, u8>() {
            // Initialize the smallvec's buffer.  Note that we need to do this through
            // the raw pointer as we cannot name the type [u8; N] even though A::Item is u8.
            let ptr = vec.as_mut_ptr();
            // SAFETY: A::Item is u8 and the smallvec has been allocated with enough capacity
            unsafe {
                core::ptr::write_bytes(ptr, 0, len);
                vec.set_len(len);
            }
            // Read the data into the smallvec's buffer.
            let slice = vec.as_mut_slice();
            // SAFETY: A::Item is u8
            let slice = unsafe { core::mem::transmute::<&mut [A::Item], &mut [u8]>(slice) };
            decoder.reader().read(slice)?;
        } else {
            for _ in 0..len {
                decoder.unclaim_bytes_read(core::mem::size_of::<A::Item>());
                vec.push(A::Item::decode(decoder)?);
            }
        }
        Ok(vec)
    }
}

#[cfg(feature = "impl_bincode")]
impl<'de, A, Context> BorrowDecode<'de, Context> for SmallVec<A>
where
    A: Array,
    A::Item: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        use core::convert::TryInto;
        let len = u64::decode(decoder)?;
        let len = len.try_into().map_err(|_| DecodeError::OutsideUsizeRange(len))?;
        decoder.claim_container_read::<A::Item>(len)?;

        let mut vec = SmallVec::with_capacity(len);
        if unty::type_equal::<A::Item, u8>() {
            // Initialize the smallvec's buffer.  Note that we need to do this through
            // the raw pointer as we cannot name the type [u8; N] even though A::Item is u8.
            let ptr = vec.as_mut_ptr();
            // SAFETY: A::Item is u8 and the smallvec has been allocated with enough capacity
            unsafe {
                core::ptr::write_bytes(ptr, 0, len);
                vec.set_len(len);
            }
            // Read the data into the smallvec's buffer.
            let slice = vec.as_mut_slice();
            // SAFETY: A::Item is u8
            let slice = unsafe { core::mem::transmute::<&mut [A::Item], &mut [u8]>(slice) };
            decoder.reader().read(slice)?;
        } else {
            for _ in 0..len {
                decoder.unclaim_bytes_read(core::mem::size_of::<A::Item>());
                vec.push(A::Item::borrow_decode(decoder)?);
            }
        }
        Ok(vec)
    }
}

#[cfg(feature = "impl_bincode")]
impl<A> Encode for SmallVec<A>
where
    A: Array,
    A::Item: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        (self.len() as u64).encode(encoder)?;
        if unty::type_equal::<A::Item, u8>() {
            // Safety: A::Item is u8
            let slice: &[u8] = unsafe { core::mem::transmute(self.as_slice()) };
            encoder.writer().write(slice)?;
        } else {
            for item in self.iter() {
                item.encode(encoder)?;
            }
        }
        Ok(())
    }
}
