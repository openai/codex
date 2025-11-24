//! The `Box<T>` type for heap allocation.
//!
//! [`Box<T>`], casually referred to as a 'box', provides the simplest form of
//! heap allocation in Rust. Boxes provide ownership for this allocation, and
//! drop their contents when they go out of scope. Boxes also ensure that they
//! never allocate more than `isize::MAX` bytes.
//!
//! # Examples
//!
//! Move a value from the stack to the heap by creating a [`Box`]:
//!
//! ```
//! let val: u8 = 5;
//! let boxed: Box<u8> = Box::new(val);
//! ```
//!
//! Move a value from a [`Box`] back to the stack by [dereferencing]:
//!
//! ```
//! let boxed: Box<u8> = Box::new(5);
//! let val: u8 = *boxed;
//! ```
//!
//! Creating a recursive data structure:
//!
//! ```
//! #[derive(Debug)]
//! enum List<T> {
//!     Cons(T, Box<List<T>>),
//!     Nil,
//! }
//!
//! let list: List<i32> = List::Cons(1, Box::new(List::Cons(2, Box::new(List::Nil))));
//! println!("{list:?}");
//! ```
//!
//! This will print `Cons(1, Cons(2, Nil))`.
//!
//! Recursive structures must be boxed, because if the definition of `Cons`
//! looked like this:
//!
//! ```compile_fail,E0072
//! # enum List<T> {
//! Cons(T, List<T>),
//! # }
//! ```
//!
//! It wouldn't work. This is because the size of a `List` depends on how many
//! elements are in the list, and so we don't know how much memory to allocate
//! for a `Cons`. By introducing a [`Box<T>`], which has a defined size, we know how
//! big `Cons` needs to be.
//!
//! # Memory layout
//!
//! For non-zero-sized values, a [`Box`] will use the [`Global`] allocator for
//! its allocation. It is valid to convert both ways between a [`Box`] and a
//! raw pointer allocated with the [`Global`] allocator, given that the
//! [`Layout`] used with the allocator is correct for the type. More precisely,
//! a `value: *mut T` that has been allocated with the [`Global`] allocator
//! with `Layout::for_value(&*value)` may be converted into a box using
//! [`Box::<T>::from_raw(value)`]. Conversely, the memory backing a `value: *mut
//! T` obtained from [`Box::<T>::into_raw`] may be deallocated using the
//! [`Global`] allocator with [`Layout::for_value(&*value)`].
//!
//! For zero-sized values, the `Box` pointer still has to be [valid] for reads
//! and writes and sufficiently aligned. In particular, casting any aligned
//! non-zero integer literal to a raw pointer produces a valid pointer, but a
//! pointer pointing into previously allocated memory that since got freed is
//! not valid. The recommended way to build a Box to a ZST if `Box::new` cannot
//! be used is to use [`ptr::NonNull::dangling`].
//!
//! So long as `T: Sized`, a `Box<T>` is guaranteed to be represented
//! as a single pointer and is also ABI-compatible with C pointers
//! (i.e. the C type `T*`). This means that if you have extern "C"
//! Rust functions that will be called from C, you can define those
//! Rust functions using `Box<T>` types, and use `T*` as corresponding
//! type on the C side. As an example, consider this C header which
//! declares functions that create and destroy some kind of `Foo`
//! value:
//!
//! ```c
//! /* C header */
//!
//! /* Returns ownership to the caller */
//! struct Foo* foo_new(void);
//!
//! /* Takes ownership from the caller; no-op when invoked with null */
//! void foo_delete(struct Foo*);
//! ```
//!
//! These two functions might be implemented in Rust as follows. Here, the
//! `struct Foo*` type from C is translated to `Box<Foo>`, which captures
//! the ownership constraints. Note also that the nullable argument to
//! `foo_delete` is represented in Rust as `Option<Box<Foo>>`, since `Box<Foo>`
//! cannot be null.
//!
//! ```
//! #[repr(C)]
//! pub struct Foo;
//!
//! #[no_mangle]
//! pub extern "C" fn foo_new() -> Box<Foo> {
//!     Box::new(Foo)
//! }
//!
//! #[no_mangle]
//! pub extern "C" fn foo_delete(_: Option<Box<Foo>>) {}
//! ```
//!
//! Even though `Box<T>` has the same representation and C ABI as a C pointer,
//! this does not mean that you can convert an arbitrary `T*` into a `Box<T>`
//! and expect things to work. `Box<T>` values will always be fully aligned,
//! non-null pointers. Moreover, the destructor for `Box<T>` will attempt to
//! free the value with the global allocator. In general, the best practice
//! is to only use `Box<T>` for pointers that originated from the global
//! allocator.
//!
//! **Important.** At least at present, you should avoid using
//! `Box<T>` types for functions that are defined in C but invoked
//! from Rust. In those cases, you should directly mirror the C types
//! as closely as possible. Using types like `Box<T>` where the C
//! definition is just using `T*` can lead to undefined behavior, as
//! described in [rust-lang/unsafe-code-guidelines#198][ucg#198].
//!
//! # Considerations for unsafe code
//!
//! **Warning: This section is not normative and is subject to change, possibly
//! being relaxed in the future! It is a simplified summary of the rules
//! currently implemented in the compiler.**
//!
//! The aliasing rules for `Box<T>` are the same as for `&mut T`. `Box<T>`
//! asserts uniqueness over its content. Using raw pointers derived from a box
//! after that box has been mutated through, moved or borrowed as `&mut T`
//! is not allowed. For more guidance on working with box from unsafe code, see
//! [rust-lang/unsafe-code-guidelines#326][ucg#326].
//!
//!
//! [ucg#198]: https://github.com/rust-lang/unsafe-code-guidelines/issues/198
//! [ucg#326]: https://github.com/rust-lang/unsafe-code-guidelines/issues/326
//! [dereferencing]: core::ops::Deref
//! [`Box::<T>::from_raw(value)`]: Box::from_raw
//! [`Global`]: crate::alloc::Global
//! [`Layout`]: crate::alloc::Layout
//! [`Layout::for_value(&*value)`]: crate::alloc::Layout::for_value
//! [valid]: ptr#safety

use core::any::Any;
use core::borrow;
use core::cmp::Ordering;
use core::convert::{From, TryFrom};

// use core::error::Error;
use core::fmt;
use core::future::Future;
use core::hash::{Hash, Hasher};
#[cfg(not(no_global_oom_handling))]
use core::iter::FromIterator;
use core::iter::{FusedIterator, Iterator};
use core::marker::Unpin;
use core::mem::{self, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::{self, NonNull};
use core::task::{Context, Poll};

use super::alloc::{AllocError, Allocator, Global, Layout};
use super::raw_vec::RawVec;
use super::unique::Unique;
#[cfg(not(no_global_oom_handling))]
use super::vec::Vec;
#[cfg(not(no_global_oom_handling))]
use alloc_crate::alloc::handle_alloc_error;

/// A pointer type for heap allocation.
///
/// See the [module-level documentation](../../std/boxed/index.html) for more.
pub struct Box<T: ?Sized, A: Allocator = Global>(Unique<T>, A);

// Safety: Box owns both T and A, so sending is safe if
// sending is safe for T and A.
unsafe impl<T: ?Sized, A: Allocator> Send for Box<T, A>
where
    T: Send,
    A: Send,
{
}

// Safety: Box owns both T and A, so sharing is safe if
// sharing is safe for T and A.
unsafe impl<T: ?Sized, A: Allocator> Sync for Box<T, A>
where
    T: Sync,
    A: Sync,
{
}

impl<T> Box<T> {
    /// Allocates memory on the heap and then places `x` into it.
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    ///
    /// # Examples
    ///
    /// ```
    /// let five = Box::new(5);
    /// ```
    #[cfg(all(not(no_global_oom_handling)))]
    #[inline(always)]
    #[must_use]
    pub fn new(x: T) -> Self {
        Self::new_in(x, Global)
    }

    /// Constructs a new box with uninitialized contents.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let mut five = Box::<u32>::new_uninit();
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_uninit() -> Box<mem::MaybeUninit<T>> {
        Self::new_uninit_in(Global)
    }

    /// Constructs a new `Box` with uninitialized contents, with the memory
    /// being filled with `0` bytes.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let zero = Box::<u32>::new_zeroed();
    /// let zero = unsafe { zero.assume_init() };
    ///
    /// assert_eq!(*zero, 0)
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_zeroed() -> Box<mem::MaybeUninit<T>> {
        Self::new_zeroed_in(Global)
    }

    /// Constructs a new `Pin<Box<T>>`. If `T` does not implement [`Unpin`], then
    /// `x` will be pinned in memory and unable to be moved.
    ///
    /// Constructing and pinning of the `Box` can also be done in two steps: `Box::pin(x)`
    /// does the same as <code>[Box::into_pin]\([Box::new]\(x))</code>. Consider using
    /// [`into_pin`](Box::into_pin) if you already have a `Box<T>`, or if you want to
    /// construct a (pinned) `Box` in a different way than with [`Box::new`].
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn pin(x: T) -> Pin<Box<T>> {
        Box::new(x).into()
    }

    /// Allocates memory on the heap then places `x` into it,
    /// returning an error if the allocation fails
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// let five = Box::try_new(5)?;
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    #[inline(always)]
    pub fn try_new(x: T) -> Result<Self, AllocError> {
        Self::try_new_in(x, Global)
    }

    /// Constructs a new box with uninitialized contents on the heap,
    /// returning an error if the allocation fails
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// let mut five = Box::<u32>::try_new_uninit()?;
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    #[inline(always)]
    pub fn try_new_uninit() -> Result<Box<mem::MaybeUninit<T>>, AllocError> {
        Box::try_new_uninit_in(Global)
    }

    /// Constructs a new `Box` with uninitialized contents, with the memory
    /// being filled with `0` bytes on the heap
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// let zero = Box::<u32>::try_new_zeroed()?;
    /// let zero = unsafe { zero.assume_init() };
    ///
    /// assert_eq!(*zero, 0);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline(always)]
    pub fn try_new_zeroed() -> Result<Box<mem::MaybeUninit<T>>, AllocError> {
        Box::try_new_zeroed_in(Global)
    }
}

impl<T, A: Allocator> Box<T, A> {
    /// Allocates memory in the given allocator then places `x` into it.
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use std::alloc::System;
    ///
    /// let five = Box::new_in(5, System);
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_in(x: T, alloc: A) -> Self
    where
        A: Allocator,
    {
        let mut boxed = Self::new_uninit_in(alloc);
        unsafe {
            boxed.as_mut_ptr().write(x);
            boxed.assume_init()
        }
    }

    /// Allocates memory in the given allocator then places `x` into it,
    /// returning an error if the allocation fails
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use std::alloc::System;
    ///
    /// let five = Box::try_new_in(5, System)?;
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    #[inline(always)]
    pub fn try_new_in(x: T, alloc: A) -> Result<Self, AllocError>
    where
        A: Allocator,
    {
        let mut boxed = Self::try_new_uninit_in(alloc)?;
        unsafe {
            boxed.as_mut_ptr().write(x);
            Ok(boxed.assume_init())
        }
    }

    /// Constructs a new box with uninitialized contents in the provided allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let mut five = Box::<u32, _>::new_uninit_in(System);
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    // #[unstable(feature = "new_uninit", issue = "63291")]
    #[inline(always)]
    pub fn new_uninit_in(alloc: A) -> Box<mem::MaybeUninit<T>, A>
    where
        A: Allocator,
    {
        let layout = Layout::new::<mem::MaybeUninit<T>>();
        // NOTE: Prefer match over unwrap_or_else since closure sometimes not inlineable.
        // That would make code size bigger.
        match Box::try_new_uninit_in(alloc) {
            Ok(m) => m,
            Err(_) => handle_alloc_error(layout),
        }
    }

    /// Constructs a new box with uninitialized contents in the provided allocator,
    /// returning an error if the allocation fails
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let mut five = Box::<u32, _>::try_new_uninit_in(System)?;
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    #[inline(always)]
    pub fn try_new_uninit_in(alloc: A) -> Result<Box<mem::MaybeUninit<T>, A>, AllocError>
    where
        A: Allocator,
    {
        let ptr = if mem::size_of::<T>() == 0 {
            NonNull::dangling()
        } else {
            let layout = Layout::new::<mem::MaybeUninit<T>>();
            alloc.allocate(layout)?.cast()
        };

        unsafe { Ok(Box::from_raw_in(ptr.as_ptr(), alloc)) }
    }

    /// Constructs a new `Box` with uninitialized contents, with the memory
    /// being filled with `0` bytes in the provided allocator.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let zero = Box::<u32, _>::new_zeroed_in(System);
    /// let zero = unsafe { zero.assume_init() };
    ///
    /// assert_eq!(*zero, 0)
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[cfg(not(no_global_oom_handling))]
    // #[unstable(feature = "new_uninit", issue = "63291")]
    #[must_use]
    #[inline(always)]
    pub fn new_zeroed_in(alloc: A) -> Box<mem::MaybeUninit<T>, A>
    where
        A: Allocator,
    {
        let layout = Layout::new::<mem::MaybeUninit<T>>();
        // NOTE: Prefer match over unwrap_or_else since closure sometimes not inlineable.
        // That would make code size bigger.
        match Box::try_new_zeroed_in(alloc) {
            Ok(m) => m,
            Err(_) => handle_alloc_error(layout),
        }
    }

    /// Constructs a new `Box` with uninitialized contents, with the memory
    /// being filled with `0` bytes in the provided allocator,
    /// returning an error if the allocation fails,
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let zero = Box::<u32, _>::try_new_zeroed_in(System)?;
    /// let zero = unsafe { zero.assume_init() };
    ///
    /// assert_eq!(*zero, 0);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline(always)]
    pub fn try_new_zeroed_in(alloc: A) -> Result<Box<mem::MaybeUninit<T>, A>, AllocError>
    where
        A: Allocator,
    {
        let ptr = if mem::size_of::<T>() == 0 {
            NonNull::dangling()
        } else {
            let layout = Layout::new::<mem::MaybeUninit<T>>();
            alloc.allocate_zeroed(layout)?.cast()
        };
        unsafe { Ok(Box::from_raw_in(ptr.as_ptr(), alloc)) }
    }

    /// Constructs a new `Pin<Box<T, A>>`. If `T` does not implement [`Unpin`], then
    /// `x` will be pinned in memory and unable to be moved.
    ///
    /// Constructing and pinning of the `Box` can also be done in two steps: `Box::pin_in(x, alloc)`
    /// does the same as <code>[Box::into_pin]\([Box::new_in]\(x, alloc))</code>. Consider using
    /// [`into_pin`](Box::into_pin) if you already have a `Box<T, A>`, or if you want to
    /// construct a (pinned) `Box` in a different way than with [`Box::new_in`].
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn pin_in(x: T, alloc: A) -> Pin<Self>
    where
        A: 'static + Allocator,
    {
        Self::into_pin(Self::new_in(x, alloc))
    }

    /// Converts a `Box<T>` into a `Box<[T]>`
    ///
    /// This conversion does not allocate on the heap and happens in place.
    #[inline(always)]
    pub fn into_boxed_slice(boxed: Self) -> Box<[T], A> {
        let (raw, alloc) = Box::into_raw_with_allocator(boxed);
        unsafe { Box::from_raw_in(raw as *mut [T; 1], alloc) }
    }

    /// Consumes the `Box`, returning the wrapped value.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(box_into_inner)]
    ///
    /// let c = Box::new(5);
    ///
    /// assert_eq!(Box::into_inner(c), 5);
    /// ```
    #[inline(always)]
    pub fn into_inner(boxed: Self) -> T {
        // Override our default `Drop` implementation.
        // Though the default `Drop` implementation drops the both the pointer and the allocator,
        // here we only want to drop the allocator.
        let boxed = mem::ManuallyDrop::new(boxed);
        let alloc = unsafe { ptr::read(&boxed.1) };

        let ptr = boxed.0;
        let unboxed = unsafe { ptr.as_ptr().read() };
        unsafe { alloc.deallocate(ptr.as_non_null_ptr().cast(), Layout::new::<T>()) };

        unboxed
    }
}

impl<T> Box<[T]> {
    /// Constructs a new boxed slice with uninitialized contents.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let mut values = Box::<[u32]>::new_uninit_slice(3);
    ///
    /// let values = unsafe {
    ///     // Deferred initialization:
    ///     values[0].as_mut_ptr().write(1);
    ///     values[1].as_mut_ptr().write(2);
    ///     values[2].as_mut_ptr().write(3);
    ///
    ///     values.assume_init()
    /// };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_uninit_slice(len: usize) -> Box<[mem::MaybeUninit<T>]> {
        unsafe { RawVec::with_capacity(len).into_box(len) }
    }

    /// Constructs a new boxed slice with uninitialized contents, with the memory
    /// being filled with `0` bytes.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let values = Box::<[u32]>::new_zeroed_slice(3);
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [0, 0, 0])
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_zeroed_slice(len: usize) -> Box<[mem::MaybeUninit<T>]> {
        unsafe { RawVec::with_capacity_zeroed(len).into_box(len) }
    }

    /// Constructs a new boxed slice with uninitialized contents. Returns an error if
    /// the allocation fails
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// let mut values = Box::<[u32]>::try_new_uninit_slice(3)?;
    /// let values = unsafe {
    ///     // Deferred initialization:
    ///     values[0].as_mut_ptr().write(1);
    ///     values[1].as_mut_ptr().write(2);
    ///     values[2].as_mut_ptr().write(3);
    ///     values.assume_init()
    /// };
    ///
    /// assert_eq!(*values, [1, 2, 3]);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    #[inline(always)]
    pub fn try_new_uninit_slice(len: usize) -> Result<Box<[mem::MaybeUninit<T>]>, AllocError> {
        Self::try_new_uninit_slice_in(len, Global)
    }

    /// Constructs a new boxed slice with uninitialized contents, with the memory
    /// being filled with `0` bytes. Returns an error if the allocation fails
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// let values = Box::<[u32]>::try_new_zeroed_slice(3)?;
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [0, 0, 0]);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline(always)]
    pub fn try_new_zeroed_slice(len: usize) -> Result<Box<[mem::MaybeUninit<T>]>, AllocError> {
        Self::try_new_zeroed_slice_in(len, Global)
    }
}

impl<T, A: Allocator> Box<[T], A> {
    /// Constructs a new boxed slice with uninitialized contents in the provided allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let mut values = Box::<[u32], _>::new_uninit_slice_in(3, System);
    ///
    /// let values = unsafe {
    ///     // Deferred initialization:
    ///     values[0].as_mut_ptr().write(1);
    ///     values[1].as_mut_ptr().write(2);
    ///     values[2].as_mut_ptr().write(3);
    ///
    ///     values.assume_init()
    /// };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_uninit_slice_in(len: usize, alloc: A) -> Box<[mem::MaybeUninit<T>], A> {
        unsafe { RawVec::with_capacity_in(len, alloc).into_box(len) }
    }

    /// Constructs a new boxed slice with uninitialized contents in the provided allocator,
    /// with the memory being filled with `0` bytes.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let values = Box::<[u32], _>::new_zeroed_slice_in(3, System);
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [0, 0, 0])
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[cfg(not(no_global_oom_handling))]
    #[must_use]
    #[inline(always)]
    pub fn new_zeroed_slice_in(len: usize, alloc: A) -> Box<[mem::MaybeUninit<T>], A> {
        unsafe { RawVec::with_capacity_zeroed_in(len, alloc).into_box(len) }
    }

    /// Constructs a new boxed slice with uninitialized contents in the provided allocator. Returns an error if
    /// the allocation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let mut values = Box::<[u32], _>::try_new_uninit_slice_in(3, System)?;
    /// let values = unsafe {
    ///     // Deferred initialization:
    ///     values[0].as_mut_ptr().write(1);
    ///     values[1].as_mut_ptr().write(2);
    ///     values[2].as_mut_ptr().write(3);
    ///     values.assume_init()
    /// };
    ///
    /// assert_eq!(*values, [1, 2, 3]);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    #[inline]
    pub fn try_new_uninit_slice_in(
        len: usize,
        alloc: A,
    ) -> Result<Box<[MaybeUninit<T>], A>, AllocError> {
        let ptr = if mem::size_of::<T>() == 0 || len == 0 {
            NonNull::dangling()
        } else {
            let layout = match Layout::array::<mem::MaybeUninit<T>>(len) {
                Ok(l) => l,
                Err(_) => return Err(AllocError),
            };
            alloc.allocate(layout)?.cast()
        };
        unsafe { Ok(RawVec::from_raw_parts_in(ptr.as_ptr(), len, alloc).into_box(len)) }
    }

    /// Constructs a new boxed slice with uninitialized contents in the provided allocator, with the memory
    /// being filled with `0` bytes. Returns an error if the allocation fails.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and incorrect usage
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api, new_uninit)]
    ///
    /// use std::alloc::System;
    ///
    /// let values = Box::<[u32], _>::try_new_zeroed_slice_in(3, System)?;
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [0, 0, 0]);
    /// # Ok::<(), std::alloc::AllocError>(())
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn try_new_zeroed_slice_in(
        len: usize,
        alloc: A,
    ) -> Result<Box<[mem::MaybeUninit<T>], A>, AllocError> {
        let ptr = if mem::size_of::<T>() == 0 || len == 0 {
            NonNull::dangling()
        } else {
            let layout = match Layout::array::<mem::MaybeUninit<T>>(len) {
                Ok(l) => l,
                Err(_) => return Err(AllocError),
            };
            alloc.allocate_zeroed(layout)?.cast()
        };
        unsafe { Ok(RawVec::from_raw_parts_in(ptr.as_ptr(), len, alloc).into_box(len)) }
    }

    /// Converts `self` into a vector without clones or allocation.
    ///
    /// The resulting vector can be converted back into a box via
    /// `Vec<T>`'s `into_boxed_slice` method.
    ///
    /// # Examples
    ///
    /// ```
    /// let s: Box<[i32]> = Box::new([10, 40, 30]);
    /// let x = s.into_vec();
    /// // `s` cannot be used anymore because it has been converted into `x`.
    ///
    /// assert_eq!(x, vec![10, 40, 30]);
    /// ```
    #[inline]
    pub fn into_vec(self) -> Vec<T, A>
    where
        A: Allocator,
    {
        unsafe {
            let len = self.len();
            let (b, alloc) = Box::into_raw_with_allocator(self);
            Vec::from_raw_parts_in(b as *mut T, len, len, alloc)
        }
    }
}

impl<T, A: Allocator> Box<mem::MaybeUninit<T>, A> {
    /// Converts to `Box<T, A>`.
    ///
    /// # Safety
    ///
    /// As with [`MaybeUninit::assume_init`],
    /// it is up to the caller to guarantee that the value
    /// really is in an initialized state.
    /// Calling this when the content is not yet fully initialized
    /// causes immediate undefined behavior.
    ///
    /// [`MaybeUninit::assume_init`]: mem::MaybeUninit::assume_init
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let mut five = Box::<u32>::new_uninit();
    ///
    /// let five: Box<u32> = unsafe {
    ///     // Deferred initialization:
    ///     five.as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[inline(always)]
    pub unsafe fn assume_init(self) -> Box<T, A> {
        let (raw, alloc) = Self::into_raw_with_allocator(self);
        unsafe { Box::<T, A>::from_raw_in(raw as *mut T, alloc) }
    }

    /// Writes the value and converts to `Box<T, A>`.
    ///
    /// This method converts the box similarly to [`Box::assume_init`] but
    /// writes `value` into it before conversion thus guaranteeing safety.
    /// In some scenarios use of this method may improve performance because
    /// the compiler may be able to optimize copying from stack.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let big_box = Box::<[usize; 1024]>::new_uninit();
    ///
    /// let mut array = [0; 1024];
    /// for (i, place) in array.iter_mut().enumerate() {
    ///     *place = i;
    /// }
    ///
    /// // The optimizer may be able to elide this copy, so previous code writes
    /// // to heap directly.
    /// let big_box = Box::write(big_box, array);
    ///
    /// for (i, x) in big_box.iter().enumerate() {
    ///     assert_eq!(*x, i);
    /// }
    /// ```
    #[inline(always)]
    pub fn write(mut boxed: Self, value: T) -> Box<T, A> {
        unsafe {
            (*boxed).write(value);
            boxed.assume_init()
        }
    }
}

impl<T, A: Allocator> Box<[mem::MaybeUninit<T>], A> {
    /// Converts to `Box<[T], A>`.
    ///
    /// # Safety
    ///
    /// As with [`MaybeUninit::assume_init`],
    /// it is up to the caller to guarantee that the values
    /// really are in an initialized state.
    /// Calling this when the content is not yet fully initialized
    /// causes immediate undefined behavior.
    ///
    /// [`MaybeUninit::assume_init`]: mem::MaybeUninit::assume_init
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(new_uninit)]
    ///
    /// let mut values = Box::<[u32]>::new_uninit_slice(3);
    ///
    /// let values = unsafe {
    ///     // Deferred initialization:
    ///     values[0].as_mut_ptr().write(1);
    ///     values[1].as_mut_ptr().write(2);
    ///     values[2].as_mut_ptr().write(3);
    ///
    ///     values.assume_init()
    /// };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    #[inline(always)]
    pub unsafe fn assume_init(self) -> Box<[T], A> {
        let (raw, alloc) = Self::into_raw_with_allocator(self);
        unsafe { Box::<[T], A>::from_raw_in(raw as *mut [T], alloc) }
    }
}

impl<T: ?Sized> Box<T> {
    /// Constructs a box from a raw pointer.
    ///
    /// After calling this function, the raw pointer is owned by the
    /// resulting `Box`. Specifically, the `Box` destructor will call
    /// the destructor of `T` and free the allocated memory. For this
    /// to be safe, the memory must have been allocated in accordance
    /// with the [memory layout] used by `Box` .
    ///
    /// # Safety
    ///
    /// This function is unsafe because improper use may lead to
    /// memory problems. For example, a double-free may occur if the
    /// function is called twice on the same raw pointer.
    ///
    /// The safety conditions are described in the [memory layout] section.
    ///
    /// # Examples
    ///
    /// Recreate a `Box` which was previously converted to a raw pointer
    /// using [`Box::into_raw`]:
    /// ```
    /// let x = Box::new(5);
    /// let ptr = Box::into_raw(x);
    /// let x = unsafe { Box::from_raw(ptr) };
    /// ```
    /// Manually create a `Box` from scratch by using the global allocator:
    /// ```
    /// use std::alloc::{alloc, Layout};
    ///
    /// unsafe {
    ///     let ptr = alloc(Layout::new::<i32>()) as *mut i32;
    ///     // In general .write is required to avoid attempting to destruct
    ///     // the (uninitialized) previous contents of `ptr`, though for this
    ///     // simple example `*ptr = 5` would have worked as well.
    ///     ptr.write(5);
    ///     let x = Box::from_raw(ptr);
    /// }
    /// ```
    ///
    /// [memory layout]: self#memory-layout
    /// [`Layout`]: crate::Layout
    #[must_use = "call `drop(from_raw(ptr))` if you intend to drop the `Box`"]
    #[inline(always)]
    pub unsafe fn from_raw(raw: *mut T) -> Self {
        unsafe { Self::from_raw_in(raw, Global) }
    }
}

impl<T: ?Sized, A: Allocator> Box<T, A> {
    /// Constructs a box from a raw pointer in the given allocator.
    ///
    /// After calling this function, the raw pointer is owned by the
    /// resulting `Box`. Specifically, the `Box` destructor will call
    /// the destructor of `T` and free the allocated memory. For this
    /// to be safe, the memory must have been allocated in accordance
    /// with the [memory layout] used by `Box` .
    ///
    /// # Safety
    ///
    /// This function is unsafe because improper use may lead to
    /// memory problems. For example, a double-free may occur if the
    /// function is called twice on the same raw pointer.
    ///
    ///
    /// # Examples
    ///
    /// Recreate a `Box` which was previously converted to a raw pointer
    /// using [`Box::into_raw_with_allocator`]:
    /// ```
    /// use std::alloc::System;
    /// # use allocator_api2::boxed::Box;
    ///
    /// let x = Box::new_in(5, System);
    /// let (ptr, alloc) = Box::into_raw_with_allocator(x);
    /// let x = unsafe { Box::from_raw_in(ptr, alloc) };
    /// ```
    /// Manually create a `Box` from scratch by using the system allocator:
    /// ```
    /// use allocator_api2::alloc::{Allocator, Layout, System};
    /// # use allocator_api2::boxed::Box;
    ///
    /// unsafe {
    ///     let ptr = System.allocate(Layout::new::<i32>())?.as_ptr().cast::<i32>();
    ///     // In general .write is required to avoid attempting to destruct
    ///     // the (uninitialized) previous contents of `ptr`, though for this
    ///     // simple example `*ptr = 5` would have worked as well.
    ///     ptr.write(5);
    ///     let x = Box::from_raw_in(ptr, System);
    /// }
    /// # Ok::<(), allocator_api2::alloc::AllocError>(())
    /// ```
    ///
    /// [memory layout]: self#memory-layout
    /// [`Layout`]: crate::Layout
    #[inline(always)]
    pub const unsafe fn from_raw_in(raw: *mut T, alloc: A) -> Self {
        Box(unsafe { Unique::new_unchecked(raw) }, alloc)
    }

    /// Consumes the `Box`, returning a wrapped raw pointer.
    ///
    /// The pointer will be properly aligned and non-null.
    ///
    /// After calling this function, the caller is responsible for the
    /// memory previously managed by the `Box`. In particular, the
    /// caller should properly destroy `T` and release the memory, taking
    /// into account the [memory layout] used by `Box`. The easiest way to
    /// do this is to convert the raw pointer back into a `Box` with the
    /// [`Box::from_raw`] function, allowing the `Box` destructor to perform
    /// the cleanup.
    ///
    /// Note: this is an associated function, which means that you have
    /// to call it as `Box::into_raw(b)` instead of `b.into_raw()`. This
    /// is so that there is no conflict with a method on the inner type.
    ///
    /// # Examples
    /// Converting the raw pointer back into a `Box` with [`Box::from_raw`]
    /// for automatic cleanup:
    /// ```
    /// let x = Box::new(String::from("Hello"));
    /// let ptr = Box::into_raw(x);
    /// let x = unsafe { Box::from_raw(ptr) };
    /// ```
    /// Manual cleanup by explicitly running the destructor and deallocating
    /// the memory:
    /// ```
    /// use std::alloc::{dealloc, Layout};
    /// use std::ptr;
    ///
    /// let x = Box::new(String::from("Hello"));
    /// let p = Box::into_raw(x);
    /// unsafe {
    ///     ptr::drop_in_place(p);
    ///     dealloc(p as *mut u8, Layout::new::<String>());
    /// }
    /// ```
    ///
    /// [memory layout]: self#memory-layout
    #[inline(always)]
    pub fn into_raw(b: Self) -> *mut T {
        Self::into_raw_with_allocator(b).0
    }

    /// Consumes the `Box`, returning a wrapped raw pointer and the allocator.
    ///
    /// The pointer will be properly aligned and non-null.
    ///
    /// After calling this function, the caller is responsible for the
    /// memory previously managed by the `Box`. In particular, the
    /// caller should properly destroy `T` and release the memory, taking
    /// into account the [memory layout] used by `Box`. The easiest way to
    /// do this is to convert the raw pointer back into a `Box` with the
    /// [`Box::from_raw_in`] function, allowing the `Box` destructor to perform
    /// the cleanup.
    ///
    /// Note: this is an associated function, which means that you have
    /// to call it as `Box::into_raw_with_allocator(b)` instead of `b.into_raw_with_allocator()`. This
    /// is so that there is no conflict with a method on the inner type.
    ///
    /// # Examples
    /// Converting the raw pointer back into a `Box` with [`Box::from_raw_in`]
    /// for automatic cleanup:
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use std::alloc::System;
    ///
    /// let x = Box::new_in(String::from("Hello"), System);
    /// let (ptr, alloc) = Box::into_raw_with_allocator(x);
    /// let x = unsafe { Box::from_raw_in(ptr, alloc) };
    /// ```
    /// Manual cleanup by explicitly running the destructor and deallocating
    /// the memory:
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use std::alloc::{Allocator, Layout, System};
    /// use std::ptr::{self, NonNull};
    ///
    /// let x = Box::new_in(String::from("Hello"), System);
    /// let (ptr, alloc) = Box::into_raw_with_allocator(x);
    /// unsafe {
    ///     ptr::drop_in_place(ptr);
    ///     let non_null = NonNull::new_unchecked(ptr);
    ///     alloc.deallocate(non_null.cast(), Layout::new::<String>());
    /// }
    /// ```
    ///
    /// [memory layout]: self#memory-layout
    #[inline(always)]
    pub fn into_raw_with_allocator(b: Self) -> (*mut T, A) {
        let (leaked, alloc) = Box::into_non_null(b);
        (leaked.as_ptr(), alloc)
    }

    #[inline(always)]
    pub fn into_non_null(b: Self) -> (NonNull<T>, A) {
        // Box is recognized as a "unique pointer" by Stacked Borrows, but internally it is a
        // raw pointer for the type system. Turning it directly into a raw pointer would not be
        // recognized as "releasing" the unique pointer to permit aliased raw accesses,
        // so all raw pointer methods have to go through `Box::leak`. Turning *that* to a raw pointer
        // behaves correctly.
        let alloc = unsafe { ptr::read(&b.1) };
        (NonNull::from(Box::leak(b)), alloc)
    }

    /// Returns a reference to the underlying allocator.
    ///
    /// Note: this is an associated function, which means that you have
    /// to call it as `Box::allocator(&b)` instead of `b.allocator()`. This
    /// is so that there is no conflict with a method on the inner type.
    #[inline(always)]
    pub const fn allocator(b: &Self) -> &A {
        &b.1
    }

    /// Consumes and leaks the `Box`, returning a mutable reference,
    /// `&'a mut T`. Note that the type `T` must outlive the chosen lifetime
    /// `'a`. If the type has only static references, or none at all, then this
    /// may be chosen to be `'static`.
    ///
    /// This function is mainly useful for data that lives for the remainder of
    /// the program's life. Dropping the returned reference will cause a memory
    /// leak. If this is not acceptable, the reference should first be wrapped
    /// with the [`Box::from_raw`] function producing a `Box`. This `Box` can
    /// then be dropped which will properly destroy `T` and release the
    /// allocated memory.
    ///
    /// Note: this is an associated function, which means that you have
    /// to call it as `Box::leak(b)` instead of `b.leak()`. This
    /// is so that there is no conflict with a method on the inner type.
    ///
    /// # Examples
    ///
    /// Simple usage:
    ///
    /// ```
    /// let x = Box::new(41);
    /// let static_ref: &'static mut usize = Box::leak(x);
    /// *static_ref += 1;
    /// assert_eq!(*static_ref, 42);
    /// ```
    ///
    /// Unsized data:
    ///
    /// ```
    /// let x = vec![1, 2, 3].into_boxed_slice();
    /// let static_ref = Box::leak(x);
    /// static_ref[0] = 4;
    /// assert_eq!(*static_ref, [4, 2, 3]);
    /// ```
    #[inline(always)]
    pub fn leak<'a>(b: Self) -> &'a mut T
    where
        A: 'a,
    {
        unsafe { &mut *mem::ManuallyDrop::new(b).0.as_ptr() }
    }

    /// Converts a `Box<T>` into a `Pin<Box<T>>`. If `T` does not implement [`Unpin`], then
    /// `*boxed` will be pinned in memory and unable to be moved.
    ///
    /// This conversion does not allocate on the heap and happens in place.
    ///
    /// This is also available via [`From`].
    ///
    /// Constructing and pinning a `Box` with <code>Box::into_pin([Box::new]\(x))</code>
    /// can also be written more concisely using <code>[Box::pin]\(x)</code>.
    /// This `into_pin` method is useful if you already have a `Box<T>`, or you are
    /// constructing a (pinned) `Box` in a different way than with [`Box::new`].
    ///
    /// # Notes
    ///
    /// It's not recommended that crates add an impl like `From<Box<T>> for Pin<T>`,
    /// as it'll introduce an ambiguity when calling `Pin::from`.
    /// A demonstration of such a poor impl is shown below.
    ///
    /// ```compile_fail
    /// # use std::pin::Pin;
    /// struct Foo; // A type defined in this crate.
    /// impl From<Box<()>> for Pin<Foo> {
    ///     fn from(_: Box<()>) -> Pin<Foo> {
    ///         Pin::new(Foo)
    ///     }
    /// }
    ///
    /// let foo = Box::new(());
    /// let bar = Pin::from(foo);
    /// ```
    #[inline(always)]
    pub fn into_pin(boxed: Self) -> Pin<Self>
    where
        A: 'static,
    {
        // It's not possible to move or replace the insides of a `Pin<Box<T>>`
        // when `T: !Unpin`, so it's safe to pin it directly without any
        // additional requirements.
        unsafe { Pin::new_unchecked(boxed) }
    }
}

impl<T: ?Sized, A: Allocator> Drop for Box<T, A> {
    #[inline(always)]
    fn drop(&mut self) {
        let layout = Layout::for_value::<T>(&**self);
        unsafe {
            ptr::drop_in_place(self.0.as_mut());
            self.1.deallocate(self.0.as_non_null_ptr().cast(), layout);
        }
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Default> Default for Box<T> {
    /// Creates a `Box<T>`, with the `Default` value for T.
    #[inline(always)]
    fn default() -> Self {
        Box::new(T::default())
    }
}

impl<T, A: Allocator + Default> Default for Box<[T], A> {
    #[inline(always)]
    fn default() -> Self {
        let ptr: NonNull<[T]> = NonNull::<[T; 0]>::dangling();
        Box(unsafe { Unique::new_unchecked(ptr.as_ptr()) }, A::default())
    }
}

impl<A: Allocator + Default> Default for Box<str, A> {
    #[inline(always)]
    fn default() -> Self {
        // SAFETY: This is the same as `Unique::cast<U>` but with an unsized `U = str`.
        let ptr: Unique<str> = unsafe {
            let bytes: NonNull<[u8]> = NonNull::<[u8; 0]>::dangling();
            Unique::new_unchecked(bytes.as_ptr() as *mut str)
        };
        Box(ptr, A::default())
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Clone, A: Allocator + Clone> Clone for Box<T, A> {
    /// Returns a new box with a `clone()` of this box's contents.
    ///
    /// # Examples
    ///
    /// ```
    /// let x = Box::new(5);
    /// let y = x.clone();
    ///
    /// // The value is the same
    /// assert_eq!(x, y);
    ///
    /// // But they are unique objects
    /// assert_ne!(&*x as *const i32, &*y as *const i32);
    /// ```
    #[inline(always)]
    fn clone(&self) -> Self {
        // Pre-allocate memory to allow writing the cloned value directly.
        let mut boxed = Self::new_uninit_in(self.1.clone());
        unsafe {
            boxed.write((**self).clone());
            boxed.assume_init()
        }
    }

    /// Copies `source`'s contents into `self` without creating a new allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// let x = Box::new(5);
    /// let mut y = Box::new(10);
    /// let yp: *const i32 = &*y;
    ///
    /// y.clone_from(&x);
    ///
    /// // The value is the same
    /// assert_eq!(x, y);
    ///
    /// // And no allocation occurred
    /// assert_eq!(yp, &*y);
    /// ```
    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        (**self).clone_from(&(**source));
    }
}

#[cfg(not(no_global_oom_handling))]
impl Clone for Box<str> {
    #[inline(always)]
    fn clone(&self) -> Self {
        // this makes a copy of the data
        let buf: Box<[u8]> = self.as_bytes().into();
        unsafe { Box::from_raw(Box::into_raw(buf) as *mut str) }
    }
}

impl<T: ?Sized + PartialEq, A: Allocator> PartialEq for Box<T, A> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&**self, &**other)
    }
    #[inline(always)]
    fn ne(&self, other: &Self) -> bool {
        PartialEq::ne(&**self, &**other)
    }
}

impl<T: ?Sized + PartialOrd, A: Allocator> PartialOrd for Box<T, A> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
    #[inline(always)]
    fn lt(&self, other: &Self) -> bool {
        PartialOrd::lt(&**self, &**other)
    }
    #[inline(always)]
    fn le(&self, other: &Self) -> bool {
        PartialOrd::le(&**self, &**other)
    }
    #[inline(always)]
    fn ge(&self, other: &Self) -> bool {
        PartialOrd::ge(&**self, &**other)
    }
    #[inline(always)]
    fn gt(&self, other: &Self) -> bool {
        PartialOrd::gt(&**self, &**other)
    }
}

impl<T: ?Sized + Ord, A: Allocator> Ord for Box<T, A> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<T: ?Sized + Eq, A: Allocator> Eq for Box<T, A> {}

impl<T: ?Sized + Hash, A: Allocator> Hash for Box<T, A> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<T: ?Sized + Hasher, A: Allocator> Hasher for Box<T, A> {
    #[inline(always)]
    fn finish(&self) -> u64 {
        (**self).finish()
    }
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        (**self).write(bytes)
    }
    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        (**self).write_u8(i)
    }
    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        (**self).write_u16(i)
    }
    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        (**self).write_u32(i)
    }
    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        (**self).write_u64(i)
    }
    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        (**self).write_u128(i)
    }
    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        (**self).write_usize(i)
    }
    #[inline(always)]
    fn write_i8(&mut self, i: i8) {
        (**self).write_i8(i)
    }
    #[inline(always)]
    fn write_i16(&mut self, i: i16) {
        (**self).write_i16(i)
    }
    #[inline(always)]
    fn write_i32(&mut self, i: i32) {
        (**self).write_i32(i)
    }
    #[inline(always)]
    fn write_i64(&mut self, i: i64) {
        (**self).write_i64(i)
    }
    #[inline(always)]
    fn write_i128(&mut self, i: i128) {
        (**self).write_i128(i)
    }
    #[inline(always)]
    fn write_isize(&mut self, i: isize) {
        (**self).write_isize(i)
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T> From<T> for Box<T> {
    /// Converts a `T` into a `Box<T>`
    ///
    /// The conversion allocates on the heap and moves `t`
    /// from the stack into it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let x = 5;
    /// let boxed = Box::new(5);
    ///
    /// assert_eq!(Box::from(x), boxed);
    /// ```
    #[inline(always)]
    fn from(t: T) -> Self {
        Box::new(t)
    }
}

impl<T: ?Sized, A: Allocator> From<Box<T, A>> for Pin<Box<T, A>>
where
    A: 'static,
{
    /// Converts a `Box<T>` into a `Pin<Box<T>>`. If `T` does not implement [`Unpin`], then
    /// `*boxed` will be pinned in memory and unable to be moved.
    ///
    /// This conversion does not allocate on the heap and happens in place.
    ///
    /// This is also available via [`Box::into_pin`].
    ///
    /// Constructing and pinning a `Box` with <code><Pin<Box\<T>>>::from([Box::new]\(x))</code>
    /// can also be written more concisely using <code>[Box::pin]\(x)</code>.
    /// This `From` implementation is useful if you already have a `Box<T>`, or you are
    /// constructing a (pinned) `Box` in a different way than with [`Box::new`].
    #[inline(always)]
    fn from(boxed: Box<T, A>) -> Self {
        Box::into_pin(boxed)
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Copy, A: Allocator + Default> From<&[T]> for Box<[T], A> {
    /// Converts a `&[T]` into a `Box<[T]>`
    ///
    /// This conversion allocates on the heap
    /// and performs a copy of `slice` and its contents.
    ///
    /// # Examples
    /// ```rust
    /// // create a &[u8] which will be used to create a Box<[u8]>
    /// let slice: &[u8] = &[104, 101, 108, 108, 111];
    /// let boxed_slice: Box<[u8]> = Box::from(slice);
    ///
    /// println!("{boxed_slice:?}");
    /// ```
    #[inline(always)]
    fn from(slice: &[T]) -> Box<[T], A> {
        let len = slice.len();
        let buf = RawVec::with_capacity_in(len, A::default());
        unsafe {
            ptr::copy_nonoverlapping(slice.as_ptr(), buf.ptr(), len);
            buf.into_box(slice.len()).assume_init()
        }
    }
}

#[cfg(not(no_global_oom_handling))]
impl<A: Allocator + Default> From<&str> for Box<str, A> {
    /// Converts a `&str` into a `Box<str>`
    ///
    /// This conversion allocates on the heap
    /// and performs a copy of `s`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let boxed: Box<str> = Box::from("hello");
    /// println!("{boxed}");
    /// ```
    #[inline(always)]
    fn from(s: &str) -> Box<str, A> {
        let (raw, alloc) = Box::into_raw_with_allocator(Box::<[u8], A>::from(s.as_bytes()));
        unsafe { Box::from_raw_in(raw as *mut str, alloc) }
    }
}

impl<A: Allocator> From<Box<str, A>> for Box<[u8], A> {
    /// Converts a `Box<str>` into a `Box<[u8]>`
    ///
    /// This conversion does not allocate on the heap and happens in place.
    ///
    /// # Examples
    /// ```rust
    /// // create a Box<str> which will be used to create a Box<[u8]>
    /// let boxed: Box<str> = Box::from("hello");
    /// let boxed_str: Box<[u8]> = Box::from(boxed);
    ///
    /// // create a &[u8] which will be used to create a Box<[u8]>
    /// let slice: &[u8] = &[104, 101, 108, 108, 111];
    /// let boxed_slice = Box::from(slice);
    ///
    /// assert_eq!(boxed_slice, boxed_str);
    /// ```
    #[inline(always)]
    fn from(s: Box<str, A>) -> Self {
        let (raw, alloc) = Box::into_raw_with_allocator(s);
        unsafe { Box::from_raw_in(raw as *mut [u8], alloc) }
    }
}

impl<T, A: Allocator, const N: usize> Box<[T; N], A> {
    #[inline(always)]
    pub fn slice(b: Self) -> Box<[T], A> {
        let (ptr, alloc) = Box::into_raw_with_allocator(b);
        unsafe { Box::from_raw_in(ptr, alloc) }
    }

    pub fn into_vec(self) -> Vec<T, A>
    where
        A: Allocator,
    {
        unsafe {
            let (b, alloc) = Box::into_raw_with_allocator(self);
            Vec::from_raw_parts_in(b as *mut T, N, N, alloc)
        }
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T, const N: usize> From<[T; N]> for Box<[T]> {
    /// Converts a `[T; N]` into a `Box<[T]>`
    ///
    /// This conversion moves the array to newly heap-allocated memory.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let boxed: Box<[u8]> = Box::from([4, 2]);
    /// println!("{boxed:?}");
    /// ```
    #[inline(always)]
    fn from(array: [T; N]) -> Box<[T]> {
        Box::slice(Box::new(array))
    }
}

impl<T, A: Allocator, const N: usize> TryFrom<Box<[T], A>> for Box<[T; N], A> {
    type Error = Box<[T], A>;

    /// Attempts to convert a `Box<[T]>` into a `Box<[T; N]>`.
    ///
    /// The conversion occurs in-place and does not require a
    /// new memory allocation.
    ///
    /// # Errors
    ///
    /// Returns the old `Box<[T]>` in the `Err` variant if
    /// `boxed_slice.len()` does not equal `N`.
    #[inline(always)]
    fn try_from(boxed_slice: Box<[T], A>) -> Result<Self, Self::Error> {
        if boxed_slice.len() == N {
            let (ptr, alloc) = Box::into_raw_with_allocator(boxed_slice);
            Ok(unsafe { Box::from_raw_in(ptr as *mut [T; N], alloc) })
        } else {
            Err(boxed_slice)
        }
    }
}

impl<A: Allocator> Box<dyn Any, A> {
    /// Attempt to downcast the box to a concrete type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(value: Box<dyn Any>) {
    ///     if let Ok(string) = value.downcast::<String>() {
    ///         println!("String ({}): {}", string.len(), string);
    ///     }
    /// }
    ///
    /// let my_string = "Hello World".to_string();
    /// print_if_string(Box::new(my_string));
    /// print_if_string(Box::new(0i8));
    /// ```
    #[inline(always)]
    pub fn downcast<T: Any>(self) -> Result<Box<T, A>, Self> {
        if self.is::<T>() {
            unsafe { Ok(self.downcast_unchecked::<T>()) }
        } else {
            Err(self)
        }
    }

    /// Downcasts the box to a concrete type.
    ///
    /// For a safe alternative see [`downcast`].
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    ///
    /// [`downcast`]: Self::downcast
    #[inline(always)]
    pub unsafe fn downcast_unchecked<T: Any>(self) -> Box<T, A> {
        debug_assert!(self.is::<T>());
        unsafe {
            let (raw, alloc): (*mut dyn Any, _) = Box::into_raw_with_allocator(self);
            Box::from_raw_in(raw as *mut T, alloc)
        }
    }
}

impl<A: Allocator> Box<dyn Any + Send, A> {
    /// Attempt to downcast the box to a concrete type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(value: Box<dyn Any + Send>) {
    ///     if let Ok(string) = value.downcast::<String>() {
    ///         println!("String ({}): {}", string.len(), string);
    ///     }
    /// }
    ///
    /// let my_string = "Hello World".to_string();
    /// print_if_string(Box::new(my_string));
    /// print_if_string(Box::new(0i8));
    /// ```
    #[inline(always)]
    pub fn downcast<T: Any>(self) -> Result<Box<T, A>, Self> {
        if self.is::<T>() {
            unsafe { Ok(self.downcast_unchecked::<T>()) }
        } else {
            Err(self)
        }
    }

    /// Downcasts the box to a concrete type.
    ///
    /// For a safe alternative see [`downcast`].
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any + Send> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    ///
    /// [`downcast`]: Self::downcast
    #[inline(always)]
    pub unsafe fn downcast_unchecked<T: Any>(self) -> Box<T, A> {
        debug_assert!(self.is::<T>());
        unsafe {
            let (raw, alloc): (*mut (dyn Any + Send), _) = Box::into_raw_with_allocator(self);
            Box::from_raw_in(raw as *mut T, alloc)
        }
    }
}

impl<A: Allocator> Box<dyn Any + Send + Sync, A> {
    /// Attempt to downcast the box to a concrete type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(value: Box<dyn Any + Send + Sync>) {
    ///     if let Ok(string) = value.downcast::<String>() {
    ///         println!("String ({}): {}", string.len(), string);
    ///     }
    /// }
    ///
    /// let my_string = "Hello World".to_string();
    /// print_if_string(Box::new(my_string));
    /// print_if_string(Box::new(0i8));
    /// ```
    #[inline(always)]
    pub fn downcast<T: Any>(self) -> Result<Box<T, A>, Self> {
        if self.is::<T>() {
            unsafe { Ok(self.downcast_unchecked::<T>()) }
        } else {
            Err(self)
        }
    }

    /// Downcasts the box to a concrete type.
    ///
    /// For a safe alternative see [`downcast`].
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any + Send + Sync> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_unchecked::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    ///
    /// [`downcast`]: Self::downcast
    #[inline(always)]
    pub unsafe fn downcast_unchecked<T: Any>(self) -> Box<T, A> {
        debug_assert!(self.is::<T>());
        unsafe {
            let (raw, alloc): (*mut (dyn Any + Send + Sync), _) =
                Box::into_raw_with_allocator(self);
            Box::from_raw_in(raw as *mut T, alloc)
        }
    }
}

impl<T: fmt::Display + ?Sized, A: Allocator> fmt::Display for Box<T, A> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: fmt::Debug + ?Sized, A: Allocator> fmt::Debug for Box<T, A> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized, A: Allocator> fmt::Pointer for Box<T, A> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // It's not possible to extract the inner Uniq directly from the Box,
        // instead we cast it to a *const which aliases the Unique
        let ptr: *const T = &**self;
        fmt::Pointer::fmt(&ptr, f)
    }
}

impl<T: ?Sized, A: Allocator> Deref for Box<T, A> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

impl<T: ?Sized, A: Allocator> DerefMut for Box<T, A> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}

impl<I: Iterator + ?Sized, A: Allocator> Iterator for Box<I, A> {
    type Item = I::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<I::Item> {
        (**self).next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<I::Item> {
        (**self).nth(n)
    }

    #[inline(always)]
    fn last(self) -> Option<I::Item> {
        BoxIter::last(self)
    }
}

trait BoxIter {
    type Item;
    fn last(self) -> Option<Self::Item>;
}

impl<I: Iterator + ?Sized, A: Allocator> BoxIter for Box<I, A> {
    type Item = I::Item;

    #[inline(always)]
    fn last(self) -> Option<I::Item> {
        #[inline(always)]
        fn some<T>(_: Option<T>, x: T) -> Option<T> {
            Some(x)
        }

        self.fold(None, some)
    }
}

impl<I: DoubleEndedIterator + ?Sized, A: Allocator> DoubleEndedIterator for Box<I, A> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<I::Item> {
        (**self).next_back()
    }
    #[inline(always)]
    fn nth_back(&mut self, n: usize) -> Option<I::Item> {
        (**self).nth_back(n)
    }
}

impl<I: ExactSizeIterator + ?Sized, A: Allocator> ExactSizeIterator for Box<I, A> {
    #[inline(always)]
    fn len(&self) -> usize {
        (**self).len()
    }
}

impl<I: FusedIterator + ?Sized, A: Allocator> FusedIterator for Box<I, A> {}

#[cfg(not(no_global_oom_handling))]
impl<I> FromIterator<I> for Box<[I]> {
    #[inline(always)]
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        iter.into_iter().collect::<Vec<_>>().into_boxed_slice()
    }
}

#[cfg(not(no_global_oom_handling))]
impl<T: Clone, A: Allocator + Clone> Clone for Box<[T], A> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let alloc = Box::allocator(self).clone();
        let mut vec = Vec::with_capacity_in(self.len(), alloc);
        vec.extend_from_slice(self);
        vec.into_boxed_slice()
    }

    #[inline(always)]
    fn clone_from(&mut self, other: &Self) {
        if self.len() == other.len() {
            self.clone_from_slice(other);
        } else {
            *self = other.clone();
        }
    }
}

impl<T: ?Sized, A: Allocator> borrow::Borrow<T> for Box<T, A> {
    #[inline(always)]
    fn borrow(&self) -> &T {
        self
    }
}

impl<T: ?Sized, A: Allocator> borrow::BorrowMut<T> for Box<T, A> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<T: ?Sized, A: Allocator> AsRef<T> for Box<T, A> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: ?Sized, A: Allocator> AsMut<T> for Box<T, A> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

/* Nota bene
 *
 *  We could have chosen not to add this impl, and instead have written a
 *  function of Pin<Box<T>> to Pin<T>. Such a function would not be sound,
 *  because Box<T> implements Unpin even when T does not, as a result of
 *  this impl.
 *
 *  We chose this API instead of the alternative for a few reasons:
 *      - Logically, it is helpful to understand pinning in regard to the
 *        memory region being pointed to. For this reason none of the
 *        standard library pointer types support projecting through a pin
 *        (Box<T> is the only pointer type in std for which this would be
 *        safe.)
 *      - It is in practice very useful to have Box<T> be unconditionally
 *        Unpin because of trait objects, for which the structural auto
 *        trait functionality does not apply (e.g., Box<dyn Foo> would
 *        otherwise not be Unpin).
 *
 *  Another type with the same semantics as Box but only a conditional
 *  implementation of `Unpin` (where `T: Unpin`) would be valid/safe, and
 *  could have a method to project a Pin<T> from it.
 */
impl<T: ?Sized, A: Allocator> Unpin for Box<T, A> where A: 'static {}

impl<F: ?Sized + Future + Unpin, A: Allocator> Future for Box<F, A>
where
    A: 'static,
{
    type Output = F::Output;

    #[inline(always)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        F::poll(Pin::new(&mut *self), cx)
    }
}

#[cfg(feature = "std")]
mod error {
    use std::error::Error;

    use super::Box;

    #[cfg(not(no_global_oom_handling))]
    impl<'a, E: Error + 'a> From<E> for Box<dyn Error + 'a> {
        /// Converts a type of [`Error`] into a box of dyn [`Error`].
        ///
        /// # Examples
        ///
        /// ```
        /// use std::error::Error;
        /// use std::fmt;
        /// use std::mem;
        ///
        /// #[derive(Debug)]
        /// struct AnError;
        ///
        /// impl fmt::Display for AnError {
        ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ///         write!(f, "An error")
        ///     }
        /// }
        ///
        /// impl Error for AnError {}
        ///
        /// let an_error = AnError;
        /// assert!(0 == mem::size_of_val(&an_error));
        /// let a_boxed_error = Box::<dyn Error>::from(an_error);
        /// assert!(mem::size_of::<Box<dyn Error>>() == mem::size_of_val(&a_boxed_error))
        /// ```
        #[inline(always)]
        fn from(err: E) -> Box<dyn Error + 'a> {
            unsafe { Box::from_raw(Box::leak(Box::new(err))) }
        }
    }

    #[cfg(not(no_global_oom_handling))]
    impl<'a, E: Error + Send + Sync + 'a> From<E> for Box<dyn Error + Send + Sync + 'a> {
        /// Converts a type of [`Error`] + [`Send`] + [`Sync`] into a box of
        /// dyn [`Error`] + [`Send`] + [`Sync`].
        ///
        /// # Examples
        ///
        /// ```
        /// use std::error::Error;
        /// use std::fmt;
        /// use std::mem;
        ///
        /// #[derive(Debug)]
        /// struct AnError;
        ///
        /// impl fmt::Display for AnError {
        ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ///         write!(f, "An error")
        ///     }
        /// }
        ///
        /// impl Error for AnError {}
        ///
        /// unsafe impl Send for AnError {}
        ///
        /// unsafe impl Sync for AnError {}
        ///
        /// let an_error = AnError;
        /// assert!(0 == mem::size_of_val(&an_error));
        /// let a_boxed_error = Box::<dyn Error + Send + Sync>::from(an_error);
        /// assert!(
        ///     mem::size_of::<Box<dyn Error + Send + Sync>>() == mem::size_of_val(&a_boxed_error))
        /// ```
        #[inline(always)]
        fn from(err: E) -> Box<dyn Error + Send + Sync + 'a> {
            unsafe { Box::from_raw(Box::leak(Box::new(err))) }
        }
    }

    impl<T: Error> Error for Box<T> {
        #[inline(always)]
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Error::source(&**self)
        }
    }
}

#[cfg(feature = "std")]
impl<R: std::io::Read + ?Sized, A: Allocator> std::io::Read for Box<R, A> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        (**self).read(buf)
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut std::vec::Vec<u8>) -> std::io::Result<usize> {
        (**self).read_to_end(buf)
    }

    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        (**self).read_to_string(buf)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        (**self).read_exact(buf)
    }
}

#[cfg(feature = "std")]
impl<W: std::io::Write + ?Sized, A: Allocator> std::io::Write for Box<W, A> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        (**self).write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        (**self).flush()
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        (**self).write_all(buf)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> std::io::Result<()> {
        (**self).write_fmt(fmt)
    }
}

#[cfg(feature = "std")]
impl<S: std::io::Seek + ?Sized, A: Allocator> std::io::Seek for Box<S, A> {
    #[inline]
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        (**self).seek(pos)
    }

    #[inline]
    fn stream_position(&mut self) -> std::io::Result<u64> {
        (**self).stream_position()
    }
}

#[cfg(feature = "std")]
impl<B: std::io::BufRead + ?Sized, A: Allocator> std::io::BufRead for Box<B, A> {
    #[inline]
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        (**self).fill_buf()
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }

    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut std::vec::Vec<u8>) -> std::io::Result<usize> {
        (**self).read_until(byte, buf)
    }

    #[inline]
    fn read_line(&mut self, buf: &mut std::string::String) -> std::io::Result<usize> {
        (**self).read_line(buf)
    }
}

#[cfg(feature = "alloc")]
impl<A: Allocator> Extend<Box<str, A>> for alloc_crate::string::String {
    fn extend<I: IntoIterator<Item = Box<str, A>>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |s| self.push_str(&s));
    }
}

#[cfg(not(no_global_oom_handling))]
#[cfg(feature = "std")]
impl Clone for Box<std::ffi::CStr> {
    #[inline]
    fn clone(&self) -> Self {
        (**self).into()
    }
}

#[cfg(not(no_global_oom_handling))]
#[cfg(feature = "std")]
impl From<&std::ffi::CStr> for Box<std::ffi::CStr> {
    /// Converts a `&CStr` into a `Box<CStr>`,
    /// by copying the contents into a newly allocated [`Box`].
    fn from(s: &std::ffi::CStr) -> Box<std::ffi::CStr> {
        let boxed: Box<[u8]> = Box::from(s.to_bytes_with_nul());
        unsafe { Box::from_raw(Box::into_raw(boxed) as *mut std::ffi::CStr) }
    }
}

#[cfg(not(no_global_oom_handling))]
#[cfg(feature = "fresh-rust")]
impl Clone for Box<core::ffi::CStr> {
    #[inline]
    fn clone(&self) -> Self {
        (**self).into()
    }
}

#[cfg(not(no_global_oom_handling))]
#[cfg(feature = "fresh-rust")]
impl From<&core::ffi::CStr> for Box<core::ffi::CStr> {
    /// Converts a `&CStr` into a `Box<CStr>`,
    /// by copying the contents into a newly allocated [`Box`].
    fn from(s: &core::ffi::CStr) -> Box<core::ffi::CStr> {
        let boxed: Box<[u8]> = Box::from(s.to_bytes_with_nul());
        unsafe { Box::from_raw(Box::into_raw(boxed) as *mut core::ffi::CStr) }
    }
}

#[cfg(feature = "serde")]
impl<T, A> serde::Serialize for Box<T, A>
where
    T: serde::Serialize,
    A: Allocator,
{
    #[inline(always)]
    fn serialize<S: serde::ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (**self).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T, A> serde::Deserialize<'de> for Box<T, A>
where
    T: serde::Deserialize<'de>,
    A: Allocator + Default,
{
    #[inline(always)]
    fn deserialize<D: serde::de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = T::deserialize(deserializer)?;
        Ok(Box::new_in(value, A::default()))
    }
}
