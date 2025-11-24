#![doc = include_str!("../README.md")]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "allocator_api", feature(allocator_api))]

#[doc(hidden)]
pub extern crate alloc as core_alloc;

#[cfg(feature = "boxed")]
pub mod boxed;
#[cfg(feature = "collections")]
pub mod collections;

mod alloc;

use core::cell::Cell;
use core::cmp::Ordering;
use core::fmt::Display;
use core::iter;
use core::marker::PhantomData;
use core::mem;
use core::ptr::{self, NonNull};
use core::slice;
use core::str;
use core_alloc::alloc::{alloc, dealloc, Layout};

#[cfg(feature = "allocator_api")]
use core_alloc::alloc::{AllocError, Allocator};

#[cfg(all(feature = "allocator-api2", not(feature = "allocator_api")))]
use allocator_api2::alloc::{AllocError, Allocator};

pub use alloc::AllocErr;

/// An error returned from [`Bump::try_alloc_try_with`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AllocOrInitError<E> {
    /// Indicates that the initial allocation failed.
    Alloc(AllocErr),
    /// Indicates that the initializer failed with the contained error after
    /// allocation.
    ///
    /// It is possible but not guaranteed that the allocated memory has been
    /// released back to the allocator at this point.
    Init(E),
}
impl<E> From<AllocErr> for AllocOrInitError<E> {
    fn from(e: AllocErr) -> Self {
        Self::Alloc(e)
    }
}
impl<E: Display> Display for AllocOrInitError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AllocOrInitError::Alloc(err) => err.fmt(f),
            AllocOrInitError::Init(err) => write!(f, "initialization failed: {}", err),
        }
    }
}

/// An arena to bump allocate into.
///
/// ## No `Drop`s
///
/// Objects that are bump-allocated will never have their [`Drop`] implementation
/// called &mdash; unless you do it manually yourself. This makes it relatively
/// easy to leak memory or other resources.
///
/// If you have a type which internally manages
///
/// * an allocation from the global heap (e.g. [`Vec<T>`]),
/// * open file descriptors (e.g. [`std::fs::File`]), or
/// * any other resource that must be cleaned up (e.g. an `mmap`)
///
/// and relies on its `Drop` implementation to clean up the internal resource,
/// then if you allocate that type with a `Bump`, you need to find a new way to
/// clean up after it yourself.
///
/// Potential solutions are:
///
/// * Using [`bumpalo::boxed::Box::new_in`] instead of [`Bump::alloc`], that
///   will drop wrapped values similarly to [`std::boxed::Box`]. Note that this
///   requires enabling the `"boxed"` Cargo feature for this crate. **This is
///   often the easiest solution.**
///
/// * Calling [`drop_in_place`][drop_in_place] or using
///   [`std::mem::ManuallyDrop`][manuallydrop] to manually drop these types.
///
/// * Using [`bumpalo::collections::Vec`] instead of [`std::vec::Vec`].
///
/// * Avoiding allocating these problematic types within a `Bump`.
///
/// Note that not calling `Drop` is memory safe! Destructors are never
/// guaranteed to run in Rust, you can't rely on them for enforcing memory
/// safety.
///
/// [`Drop`]: https://doc.rust-lang.org/std/ops/trait.Drop.html
/// [`Vec<T>`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
/// [`std::fs::File`]: https://doc.rust-lang.org/std/fs/struct.File.html
/// [drop_in_place]: https://doc.rust-lang.org/std/ptr/fn.drop_in_place.html
/// [manuallydrop]: https://doc.rust-lang.org/std/mem/struct.ManuallyDrop.html
/// [`bumpalo::collections::Vec`]: collections/vec/struct.Vec.html
/// [`std::vec::Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
/// [`bumpalo::boxed::Box::new_in`]: boxed/struct.Box.html#method.new_in
/// [`std::boxed::Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
///
/// ## Example
///
/// ```
/// use bumpalo::Bump;
///
/// // Create a new bump arena.
/// let bump = Bump::new();
///
/// // Allocate values into the arena.
/// let forty_two = bump.alloc(42);
/// assert_eq!(*forty_two, 42);
///
/// // Mutable references are returned from allocation.
/// let mut s = bump.alloc("bumpalo");
/// *s = "the bump allocator; and also is a buffalo";
/// ```
///
/// ## Allocation Methods Come in Many Flavors
///
/// There are various allocation methods on `Bump`, the simplest being
/// [`alloc`][Bump::alloc]. The others exist to satisfy some combination of
/// fallible allocation and initialization. The allocation methods are
/// summarized in the following table:
///
/// <table>
///   <thead>
///     <tr>
///       <th></th>
///       <th>Infallible Allocation</th>
///       <th>Fallible Allocation</th>
///     </tr>
///   </thead>
///     <tr>
///       <th>By Value</th>
///       <td><a href="#method.alloc"><code>alloc</code></a></td>
///       <td><a href="#method.try_alloc"><code>try_alloc</code></a></td>
///     </tr>
///     <tr>
///       <th>Infallible Initializer Function</th>
///       <td><a href="#method.alloc_with"><code>alloc_with</code></a></td>
///       <td><a href="#method.try_alloc_with"><code>try_alloc_with</code></a></td>
///     </tr>
///     <tr>
///       <th>Fallible Initializer Function</th>
///       <td><a href="#method.alloc_try_with"><code>alloc_try_with</code></a></td>
///       <td><a href="#method.try_alloc_try_with"><code>try_alloc_try_with</code></a></td>
///     </tr>
///   <tbody>
///   </tbody>
/// </table>
///
/// ### Fallible Allocation: The `try_alloc_` Method Prefix
///
/// These allocation methods let you recover from out-of-memory (OOM)
/// scenarios, rather than raising a panic on OOM.
///
/// ```
/// use bumpalo::Bump;
///
/// let bump = Bump::new();
///
/// match bump.try_alloc(MyStruct {
///     // ...
/// }) {
///     Ok(my_struct) => {
///         // Allocation succeeded.
///     }
///     Err(e) => {
///         // Out of memory.
///     }
/// }
///
/// struct MyStruct {
///     // ...
/// }
/// ```
///
/// ### Initializer Functions: The `_with` Method Suffix
///
/// Calling one of the generic `…alloc(x)` methods is essentially equivalent to
/// the matching [`…alloc_with(|| x)`](?search=alloc_with). However if you use
/// `…alloc_with`, then the closure will not be invoked until after allocating
/// space for storing `x` on the heap.
///
/// This can be useful in certain edge-cases related to compiler optimizations.
/// When evaluating for example `bump.alloc(x)`, semantically `x` is first put
/// on the stack and then moved onto the heap. In some cases, the compiler is
/// able to optimize this into constructing `x` directly on the heap, however
/// in many cases it does not.
///
/// The `…alloc_with` functions try to help the compiler be smarter. In most
/// cases doing for example `bump.try_alloc_with(|| x)` on release mode will be
/// enough to help the compiler realize that this optimization is valid and
/// to construct `x` directly onto the heap.
///
/// #### Warning
///
/// These functions critically depend on compiler optimizations to achieve their
/// desired effect. This means that it is not an effective tool when compiling
/// without optimizations on.
///
/// Even when optimizations are on, these functions do not **guarantee** that
/// the value is constructed on the heap. To the best of our knowledge no such
/// guarantee can be made in stable Rust as of 1.54.
///
/// ### Fallible Initialization: The `_try_with` Method Suffix
///
/// The generic [`…alloc_try_with(|| x)`](?search=_try_with) methods behave
/// like the purely `_with` suffixed methods explained above. However, they
/// allow for fallible initialization by accepting a closure that returns a
/// [`Result`] and will attempt to undo the initial allocation if this closure
/// returns [`Err`].
///
/// #### Warning
///
/// If the inner closure returns [`Ok`], space for the entire [`Result`] remains
/// allocated inside `self`. This can be a problem especially if the [`Err`]
/// variant is larger, but even otherwise there may be overhead for the
/// [`Result`]'s discriminant.
///
/// <p><details><summary>Undoing the allocation in the <code>Err</code> case
/// always fails if <code>f</code> successfully made any additional allocations
/// in <code>self</code>.</summary>
///
/// For example, the following will always leak also space for the [`Result`]
/// into this `Bump`, even though the inner reference isn't kept and the [`Err`]
/// payload is returned semantically by value:
///
/// ```rust
/// let bump = bumpalo::Bump::new();
///
/// let r: Result<&mut [u8; 1000], ()> = bump.alloc_try_with(|| {
///     let _ = bump.alloc(0_u8);
///     Err(())
/// });
///
/// assert!(r.is_err());
/// ```
///
///</details></p>
///
/// Since [`Err`] payloads are first placed on the heap and then moved to the
/// stack, `bump.…alloc_try_with(|| x)?` is likely to execute more slowly than
/// the matching `bump.…alloc(x?)` in case of initialization failure. If this
/// happens frequently, using the plain un-suffixed method may perform better.
///
/// [`Result`]: https://doc.rust-lang.org/std/result/enum.Result.html
/// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
/// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
///
/// ### `Bump` Allocation Limits
///
/// `bumpalo` supports setting a limit on the maximum bytes of memory that can
/// be allocated for use in a particular `Bump` arena. This limit can be set and removed with
/// [`set_allocation_limit`][Bump::set_allocation_limit].
/// The allocation limit is only enforced when allocating new backing chunks for
/// a `Bump`. Updating the allocation limit will not affect existing allocations
/// or any future allocations within the `Bump`'s current chunk.
///
/// #### Example
///
/// ```
/// let bump = bumpalo::Bump::new();
///
/// assert_eq!(bump.allocation_limit(), None);
/// bump.set_allocation_limit(Some(0));
///
/// assert!(bump.try_alloc(5).is_err());
///
/// bump.set_allocation_limit(Some(6));
///
/// assert_eq!(bump.allocation_limit(), Some(6));
///
/// bump.set_allocation_limit(None);
///
/// assert_eq!(bump.allocation_limit(), None);
/// ```
///
/// #### Warning
///
/// Because of backwards compatibility, allocations that fail
/// due to allocation limits will not present differently than
/// errors due to resource exhaustion.
#[derive(Debug)]
pub struct Bump<const MIN_ALIGN: usize = 1> {
    // The current chunk we are bump allocating within.
    current_chunk_footer: Cell<NonNull<ChunkFooter>>,
    allocation_limit: Cell<Option<usize>>,
}

#[repr(C)]
#[derive(Debug)]
struct ChunkFooter {
    // Pointer to the start of this chunk allocation. This footer is always at
    // the end of the chunk.
    data: NonNull<u8>,

    // The layout of this chunk's allocation.
    layout: Layout,

    // Link to the previous chunk.
    //
    // Note that the last node in the `prev` linked list is the canonical empty
    // chunk, whose `prev` link points to itself.
    prev: Cell<NonNull<ChunkFooter>>,

    // Bump allocation finger that is always in the range `self.data..=self`.
    ptr: Cell<NonNull<u8>>,

    // The bytes allocated in all chunks so far, the canonical empty chunk has
    // a size of 0 and for all other chunks, `allocated_bytes` will be
    // the allocated_bytes of the current chunk plus the allocated bytes
    // of the `prev` chunk.
    allocated_bytes: usize,
}

/// A wrapper type for the canonical, statically allocated empty chunk.
///
/// For the canonical empty chunk to be `static`, its type must be `Sync`, which
/// is the purpose of this wrapper type. This is safe because the empty chunk is
/// immutable and never actually modified.
#[repr(transparent)]
struct EmptyChunkFooter(ChunkFooter);

unsafe impl Sync for EmptyChunkFooter {}

static EMPTY_CHUNK: EmptyChunkFooter = EmptyChunkFooter(ChunkFooter {
    // This chunk is empty (except the foot itself).
    layout: Layout::new::<ChunkFooter>(),

    // The start of the (empty) allocatable region for this chunk is itself.
    data: unsafe { NonNull::new_unchecked(&EMPTY_CHUNK as *const EmptyChunkFooter as *mut u8) },

    // The end of the (empty) allocatable region for this chunk is also itself.
    ptr: Cell::new(unsafe {
        NonNull::new_unchecked(&EMPTY_CHUNK as *const EmptyChunkFooter as *mut u8)
    }),

    // Invariant: the last chunk footer in all `ChunkFooter::prev` linked lists
    // is the empty chunk footer, whose `prev` points to itself.
    prev: Cell::new(unsafe {
        NonNull::new_unchecked(&EMPTY_CHUNK as *const EmptyChunkFooter as *mut ChunkFooter)
    }),

    // Empty chunks count as 0 allocated bytes in an arena.
    allocated_bytes: 0,
});

impl EmptyChunkFooter {
    fn get(&'static self) -> NonNull<ChunkFooter> {
        NonNull::from(&self.0)
    }
}

impl ChunkFooter {
    // Returns the start and length of the currently allocated region of this
    // chunk.
    fn as_raw_parts(&self) -> (*const u8, usize) {
        let data = self.data.as_ptr() as *const u8;
        let ptr = self.ptr.get().as_ptr() as *const u8;
        debug_assert!(data <= ptr);
        debug_assert!(ptr <= self as *const ChunkFooter as *const u8);
        let len = unsafe { (self as *const ChunkFooter as *const u8).offset_from(ptr) as usize };
        (ptr, len)
    }

    /// Is this chunk the last empty chunk?
    fn is_empty(&self) -> bool {
        ptr::eq(self, EMPTY_CHUNK.get().as_ptr())
    }
}

impl<const MIN_ALIGN: usize> Default for Bump<MIN_ALIGN> {
    fn default() -> Self {
        Self::with_min_align()
    }
}

impl<const MIN_ALIGN: usize> Drop for Bump<MIN_ALIGN> {
    fn drop(&mut self) {
        unsafe {
            dealloc_chunk_list(self.current_chunk_footer.get());
        }
    }
}

#[inline]
unsafe fn dealloc_chunk_list(mut footer: NonNull<ChunkFooter>) {
    while !footer.as_ref().is_empty() {
        let f = footer;
        footer = f.as_ref().prev.get();
        dealloc(f.as_ref().data.as_ptr(), f.as_ref().layout);
    }
}

// `Bump`s are safe to send between threads because nothing aliases its owned
// chunks until you start allocating from it. But by the time you allocate from
// it, the returned references to allocations borrow the `Bump` and therefore
// prevent sending the `Bump` across threads until the borrows end.
unsafe impl<const MIN_ALIGN: usize> Send for Bump<MIN_ALIGN> {}

#[inline]
fn is_pointer_aligned_to<T>(pointer: *mut T, align: usize) -> bool {
    debug_assert!(align.is_power_of_two());

    let pointer = pointer as usize;
    let pointer_aligned = round_down_to(pointer, align);
    pointer == pointer_aligned
}

#[inline]
pub(crate) const fn round_up_to(n: usize, divisor: usize) -> Option<usize> {
    debug_assert!(divisor > 0);
    debug_assert!(divisor.is_power_of_two());
    match n.checked_add(divisor - 1) {
        Some(x) => Some(x & !(divisor - 1)),
        None => None,
    }
}

/// Like `round_up_to` but turns overflow into undefined behavior rather than
/// returning `None`.
#[inline]
pub(crate) unsafe fn round_up_to_unchecked(n: usize, divisor: usize) -> usize {
    match round_up_to(n, divisor) {
        Some(x) => x,
        None => {
            debug_assert!(false, "round_up_to_unchecked failed");
            core::hint::unreachable_unchecked()
        }
    }
}

#[inline]
pub(crate) fn round_down_to(n: usize, divisor: usize) -> usize {
    debug_assert!(divisor > 0);
    debug_assert!(divisor.is_power_of_two());
    n & !(divisor - 1)
}

/// Same as `round_down_to` but preserves pointer provenance.
#[inline]
pub(crate) fn round_mut_ptr_down_to(ptr: *mut u8, divisor: usize) -> *mut u8 {
    debug_assert!(divisor > 0);
    debug_assert!(divisor.is_power_of_two());
    ptr.wrapping_sub(ptr as usize & (divisor - 1))
}

#[inline]
pub(crate) unsafe fn round_mut_ptr_up_to_unchecked(ptr: *mut u8, divisor: usize) -> *mut u8 {
    debug_assert!(divisor > 0);
    debug_assert!(divisor.is_power_of_two());
    let aligned = round_up_to_unchecked(ptr as usize, divisor);
    let delta = aligned - (ptr as usize);
    ptr.add(delta)
}

// The typical page size these days.
//
// Note that we don't need to exactly match page size for correctness, and it is
// okay if this is smaller than the real page size in practice. It isn't worth
// the portability concerns and lack of const propagation that dynamically
// looking up the actual page size implies.
const TYPICAL_PAGE_SIZE: usize = 0x1000;

// We only support alignments of up to 16 bytes for iter_allocated_chunks.
const SUPPORTED_ITER_ALIGNMENT: usize = 16;
const CHUNK_ALIGN: usize = SUPPORTED_ITER_ALIGNMENT;
const FOOTER_SIZE: usize = mem::size_of::<ChunkFooter>();

// Assert that `ChunkFooter` is at most the supported alignment. This will give a
// compile time error if it is not the case
const _FOOTER_ALIGN_ASSERTION: () = {
    assert!(mem::align_of::<ChunkFooter>() <= CHUNK_ALIGN);
};

// Maximum typical overhead per allocation imposed by allocators.
const MALLOC_OVERHEAD: usize = 16;

// This is the overhead from malloc, footer and alignment. For instance, if
// we want to request a chunk of memory that has at least X bytes usable for
// allocations (where X is aligned to CHUNK_ALIGN), then we expect that the
// after adding a footer, malloc overhead and alignment, the chunk of memory
// the allocator actually sets aside for us is X+OVERHEAD rounded up to the
// nearest suitable size boundary.
const OVERHEAD: usize = match round_up_to(MALLOC_OVERHEAD + FOOTER_SIZE, CHUNK_ALIGN) {
    Some(x) => x,
    None => panic!(),
};

// The target size of our first allocation, including our overhead. The
// available bump capacity will be smaller.
const FIRST_ALLOCATION_GOAL: usize = 1 << 9;

// The actual size of the first allocation is going to be a bit smaller than the
// goal. We need to make room for the footer, and we also need take the
// alignment into account. We're trying to avoid this kind of situation:
// https://blog.mozilla.org/nnethercote/2011/08/05/clownshoes-available-in-sizes-2101-and-up/
const DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER: usize = FIRST_ALLOCATION_GOAL - OVERHEAD;

/// The memory size and alignment details for a potential new chunk
/// allocation.
#[derive(Debug, Clone, Copy)]
struct NewChunkMemoryDetails {
    new_size_without_footer: usize,
    align: usize,
    size: usize,
}

/// Wrapper around `Layout::from_size_align` that adds debug assertions.
#[inline]
fn layout_from_size_align(size: usize, align: usize) -> Result<Layout, AllocErr> {
    Layout::from_size_align(size, align).map_err(|_| AllocErr)
}

#[cold]
#[inline(never)]
fn allocation_size_overflow<T>() -> T {
    panic!("requested allocation size overflowed")
}

// NB: We don't have constructors as methods on `impl<N> Bump<N>` that return
// `Self` because then `rustc` can't infer the `N` if it isn't explicitly
// provided, even though it has a default value. There doesn't seem to be a good
// workaround, other than putting constructors on the `Bump<DEFAULT>`; even
// `std` does this same thing with `HashMap`, for example.
impl Bump<1> {
    /// Construct a new arena to bump allocate into.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// # let _ = bump;
    /// ```
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Attempt to construct a new arena to bump allocate into.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::try_new();
    /// # let _ = bump.unwrap();
    /// ```
    pub fn try_new() -> Result<Self, AllocErr> {
        Bump::try_with_capacity(0)
    }

    /// Construct a new arena with the specified byte capacity to bump allocate
    /// into.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::with_capacity(100);
    /// # let _ = bump;
    /// ```
    ///
    /// ## Panics
    ///
    /// Panics if allocating the initial capacity fails.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::try_with_capacity(capacity).unwrap_or_else(|_| oom())
    }

    /// Attempt to construct a new arena with the specified byte capacity to
    /// bump allocate into.
    ///
    /// Propagates errors when allocating the initial capacity.
    ///
    /// ## Example
    ///
    /// ```
    /// # fn _foo() -> Result<(), bumpalo::AllocErr> {
    /// let bump = bumpalo::Bump::try_with_capacity(100)?;
    /// # let _ = bump;
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_with_capacity(capacity: usize) -> Result<Self, AllocErr> {
        Self::try_with_min_align_and_capacity(capacity)
    }
}

impl<const MIN_ALIGN: usize> Bump<MIN_ALIGN> {
    /// Create a new `Bump` that enforces a minimum alignment.
    ///
    /// The minimum alignment must be a power of two and no larger than `16`.
    ///
    /// Enforcing a minimum alignment can speed up allocation of objects with
    /// alignment less than or equal to the minimum alignment. This comes at the
    /// cost of introducing otherwise-unnecessary padding between allocations of
    /// objects with alignment less than the minimum.
    ///
    /// # Example
    ///
    /// ```
    /// type BumpAlign8 = bumpalo::Bump<8>;
    /// let bump = BumpAlign8::with_min_align();
    /// for x in 0..u8::MAX {
    ///     let x = bump.alloc(x);
    ///     assert_eq!((x as *mut _ as usize) % 8, 0, "x is aligned to 8");
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics on invalid minimum alignments.
    //
    // Because of `rustc`'s poor type inference for default type/const
    // parameters (see the comment above the `impl Bump` block with no const
    // `MIN_ALIGN` parameter) and because we don't want to force everyone to
    // specify a minimum alignment with `Bump::new()` et al, we have a separate
    // constructor for specifying the minimum alignment.
    pub fn with_min_align() -> Self {
        assert!(
            MIN_ALIGN.is_power_of_two(),
            "MIN_ALIGN must be a power of two; found {MIN_ALIGN}"
        );
        assert!(
            MIN_ALIGN <= CHUNK_ALIGN,
            "MIN_ALIGN may not be larger than {CHUNK_ALIGN}; found {MIN_ALIGN}"
        );

        Bump {
            current_chunk_footer: Cell::new(EMPTY_CHUNK.get()),
            allocation_limit: Cell::new(None),
        }
    }

    /// Create a new `Bump` that enforces a minimum alignment and starts with
    /// room for at least `capacity` bytes.
    ///
    /// The minimum alignment must be a power of two and no larger than `16`.
    ///
    /// Enforcing a minimum alignment can speed up allocation of objects with
    /// alignment less than or equal to the minimum alignment. This comes at the
    /// cost of introducing otherwise-unnecessary padding between allocations of
    /// objects with alignment less than the minimum.
    ///
    /// # Example
    ///
    /// ```
    /// type BumpAlign8 = bumpalo::Bump<8>;
    /// let mut bump = BumpAlign8::with_min_align_and_capacity(8 * 100);
    /// for x in 0..100_u64 {
    ///     let x = bump.alloc(x);
    ///     assert_eq!((x as *mut _ as usize) % 8, 0, "x is aligned to 8");
    /// }
    /// assert_eq!(
    ///     bump.iter_allocated_chunks().count(), 1,
    ///     "initial chunk had capacity for all allocations",
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// Panics on invalid minimum alignments.
    ///
    /// Panics if allocating the initial capacity fails.
    pub fn with_min_align_and_capacity(capacity: usize) -> Self {
        Self::try_with_min_align_and_capacity(capacity).unwrap_or_else(|_| oom())
    }

    /// Create a new `Bump` that enforces a minimum alignment and starts with
    /// room for at least `capacity` bytes.
    ///
    /// The minimum alignment must be a power of two and no larger than `16`.
    ///
    /// Enforcing a minimum alignment can speed up allocation of objects with
    /// alignment less than or equal to the minimum alignment. This comes at the
    /// cost of introducing otherwise-unnecessary padding between allocations of
    /// objects with alignment less than the minimum.
    ///
    /// # Example
    ///
    /// ```
    /// # fn _foo() -> Result<(), bumpalo::AllocErr> {
    /// type BumpAlign8 = bumpalo::Bump<8>;
    /// let mut bump = BumpAlign8::try_with_min_align_and_capacity(8 * 100)?;
    /// for x in 0..100_u64 {
    ///     let x = bump.alloc(x);
    ///     assert_eq!((x as *mut _ as usize) % 8, 0, "x is aligned to 8");
    /// }
    /// assert_eq!(
    ///     bump.iter_allocated_chunks().count(), 1,
    ///     "initial chunk had capacity for all allocations",
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics on invalid minimum alignments.
    ///
    /// Panics if allocating the initial capacity fails.
    pub fn try_with_min_align_and_capacity(capacity: usize) -> Result<Self, AllocErr> {
        assert!(
            MIN_ALIGN.is_power_of_two(),
            "MIN_ALIGN must be a power of two; found {MIN_ALIGN}"
        );
        assert!(
            MIN_ALIGN <= CHUNK_ALIGN,
            "MIN_ALIGN may not be larger than {CHUNK_ALIGN}; found {MIN_ALIGN}"
        );

        if capacity == 0 {
            return Ok(Bump {
                current_chunk_footer: Cell::new(EMPTY_CHUNK.get()),
                allocation_limit: Cell::new(None),
            });
        }

        let layout = layout_from_size_align(capacity, MIN_ALIGN)?;

        let chunk_footer = unsafe {
            Self::new_chunk(
                Self::new_chunk_memory_details(None, layout).ok_or(AllocErr)?,
                layout,
                EMPTY_CHUNK.get(),
            )
            .ok_or(AllocErr)?
        };

        Ok(Bump {
            current_chunk_footer: Cell::new(chunk_footer),
            allocation_limit: Cell::new(None),
        })
    }

    /// Get this bump arena's minimum alignment.
    ///
    /// All objects allocated in this arena get aligned to this value.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump2 = bumpalo::Bump::<2>::with_min_align();
    /// assert_eq!(bump2.min_align(), 2);
    ///
    /// let bump4 = bumpalo::Bump::<4>::with_min_align();
    /// assert_eq!(bump4.min_align(), 4);
    /// ```
    #[inline]
    pub fn min_align(&self) -> usize {
        MIN_ALIGN
    }

    /// The allocation limit for this arena in bytes.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::with_capacity(0);
    ///
    /// assert_eq!(bump.allocation_limit(), None);
    ///
    /// bump.set_allocation_limit(Some(6));
    ///
    /// assert_eq!(bump.allocation_limit(), Some(6));
    ///
    /// bump.set_allocation_limit(None);
    ///
    /// assert_eq!(bump.allocation_limit(), None);
    /// ```
    pub fn allocation_limit(&self) -> Option<usize> {
        self.allocation_limit.get()
    }

    /// Set the allocation limit in bytes for this arena.
    ///
    /// The allocation limit is only enforced when allocating new backing chunks for
    /// a `Bump`. Updating the allocation limit will not affect existing allocations
    /// or any future allocations within the `Bump`'s current chunk.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::with_capacity(0);
    ///
    /// bump.set_allocation_limit(Some(0));
    ///
    /// assert!(bump.try_alloc(5).is_err());
    /// ```
    pub fn set_allocation_limit(&self, limit: Option<usize>) {
        self.allocation_limit.set(limit);
    }

    /// How much headroom an arena has before it hits its allocation
    /// limit.
    fn allocation_limit_remaining(&self) -> Option<usize> {
        self.allocation_limit.get().and_then(|allocation_limit| {
            let allocated_bytes = self.allocated_bytes();
            if allocated_bytes > allocation_limit {
                None
            } else {
                Some(usize::abs_diff(allocation_limit, allocated_bytes))
            }
        })
    }

    /// Whether a request to allocate a new chunk with a given size for a given
    /// requested layout will fit under the allocation limit set on a `Bump`.
    fn chunk_fits_under_limit(
        allocation_limit_remaining: Option<usize>,
        new_chunk_memory_details: NewChunkMemoryDetails,
    ) -> bool {
        allocation_limit_remaining
            .map(|allocation_limit_left| {
                allocation_limit_left >= new_chunk_memory_details.new_size_without_footer
            })
            .unwrap_or(true)
    }

    /// Determine the memory details including final size, alignment and final
    /// size without footer for a new chunk that would be allocated to fulfill
    /// an allocation request.
    fn new_chunk_memory_details(
        new_size_without_footer: Option<usize>,
        requested_layout: Layout,
    ) -> Option<NewChunkMemoryDetails> {
        // We must have `CHUNK_ALIGN` or better alignment...
        let align = CHUNK_ALIGN
            // and we have to have at least our configured minimum alignment...
            .max(MIN_ALIGN)
            // and make sure we satisfy the requested allocation's alignment.
            .max(requested_layout.align());

        let mut new_size_without_footer =
            new_size_without_footer.unwrap_or(DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER);

        let requested_size =
            round_up_to(requested_layout.size(), align).unwrap_or_else(allocation_size_overflow);
        new_size_without_footer = new_size_without_footer.max(requested_size);

        // We want our allocations to play nice with the memory allocator, and
        // waste as little memory as possible. For small allocations, this means
        // that the entire allocation including the chunk footer and mallocs
        // internal overhead is as close to a power of two as we can go without
        // going over. For larger allocations, we only need to get close to a
        // page boundary without going over.
        if new_size_without_footer < TYPICAL_PAGE_SIZE {
            new_size_without_footer =
                (new_size_without_footer + OVERHEAD).next_power_of_two() - OVERHEAD;
        } else {
            new_size_without_footer =
                round_up_to(new_size_without_footer + OVERHEAD, TYPICAL_PAGE_SIZE)? - OVERHEAD;
        }

        debug_assert_eq!(align % CHUNK_ALIGN, 0);
        debug_assert_eq!(new_size_without_footer % CHUNK_ALIGN, 0);
        let size = new_size_without_footer
            .checked_add(FOOTER_SIZE)
            .unwrap_or_else(allocation_size_overflow);

        Some(NewChunkMemoryDetails {
            new_size_without_footer,
            size,
            align,
        })
    }

    /// Allocate a new chunk and return its initialized footer.
    ///
    /// If given, `layouts` is a tuple of the current chunk size and the
    /// layout of the allocation request that triggered us to fall back to
    /// allocating a new chunk of memory.
    unsafe fn new_chunk(
        new_chunk_memory_details: NewChunkMemoryDetails,
        requested_layout: Layout,
        prev: NonNull<ChunkFooter>,
    ) -> Option<NonNull<ChunkFooter>> {
        let NewChunkMemoryDetails {
            new_size_without_footer,
            align,
            size,
        } = new_chunk_memory_details;

        let layout = layout_from_size_align(size, align).ok()?;

        debug_assert!(size >= requested_layout.size());

        let data = alloc(layout);
        let data = NonNull::new(data)?;

        // The `ChunkFooter` is at the end of the chunk.
        let footer_ptr = data.as_ptr().add(new_size_without_footer);
        debug_assert_eq!((data.as_ptr() as usize) % align, 0);
        debug_assert_eq!(footer_ptr as usize % CHUNK_ALIGN, 0);
        let footer_ptr = footer_ptr as *mut ChunkFooter;

        // The bump pointer is initialized to the end of the range we will bump
        // out of, rounded down to the minimum alignment. It is the
        // `NewChunkMemoryDetails` constructor's responsibility to ensure that
        // even after this rounding we have enough non-zero capacity in the
        // chunk.
        let ptr = round_mut_ptr_down_to(footer_ptr.cast::<u8>(), MIN_ALIGN);
        debug_assert_eq!(ptr as usize % MIN_ALIGN, 0);
        debug_assert!(
            data.as_ptr() < ptr,
            "bump pointer {ptr:#p} should still be greater than or equal to the \
             start of the bump chunk {data:#p}"
        );
        debug_assert_eq!(
            (ptr as usize) - (data.as_ptr() as usize),
            new_size_without_footer
        );

        let ptr = Cell::new(NonNull::new_unchecked(ptr));

        // The `allocated_bytes` of a new chunk counts the total size
        // of the chunks, not how much of the chunks are used.
        let allocated_bytes = prev.as_ref().allocated_bytes + new_size_without_footer;

        ptr::write(
            footer_ptr,
            ChunkFooter {
                data,
                layout,
                prev: Cell::new(prev),
                ptr,
                allocated_bytes,
            },
        );

        Some(NonNull::new_unchecked(footer_ptr))
    }

    /// Reset this bump allocator.
    ///
    /// Performs mass deallocation on everything allocated in this arena by
    /// resetting the pointer into the underlying chunk of memory to the start
    /// of the chunk. Does not run any `Drop` implementations on deallocated
    /// objects; see [the top-level documentation](struct.Bump.html) for details.
    ///
    /// If this arena has allocated multiple chunks to bump allocate into, then
    /// the excess chunks are returned to the global allocator.
    ///
    /// ## Example
    ///
    /// ```
    /// let mut bump = bumpalo::Bump::new();
    ///
    /// // Allocate a bunch of things.
    /// {
    ///     for i in 0..100 {
    ///         bump.alloc(i);
    ///     }
    /// }
    ///
    /// // Reset the arena.
    /// bump.reset();
    ///
    /// // Allocate some new things in the space previously occupied by the
    /// // original things.
    /// for j in 200..400 {
    ///     bump.alloc(j);
    /// }
    ///```
    pub fn reset(&mut self) {
        // Takes `&mut self` so `self` must be unique and there can't be any
        // borrows active that would get invalidated by resetting.
        unsafe {
            if self.current_chunk_footer.get().as_ref().is_empty() {
                return;
            }

            let mut cur_chunk = self.current_chunk_footer.get();

            // Deallocate all chunks except the current one
            let prev_chunk = cur_chunk.as_ref().prev.replace(EMPTY_CHUNK.get());
            dealloc_chunk_list(prev_chunk);

            // Reset the bump finger to the end of the chunk.
            debug_assert!(
                is_pointer_aligned_to(cur_chunk.as_ptr(), MIN_ALIGN),
                "bump pointer {cur_chunk:#p} should be aligned to the minimum alignment of {MIN_ALIGN:#x}"
            );
            cur_chunk.as_ref().ptr.set(cur_chunk.cast());

            // Reset the allocated size of the chunk.
            cur_chunk.as_mut().allocated_bytes = cur_chunk.as_ref().layout.size() - FOOTER_SIZE;

            debug_assert!(
                self.current_chunk_footer
                    .get()
                    .as_ref()
                    .prev
                    .get()
                    .as_ref()
                    .is_empty(),
                "We should only have a single chunk"
            );
            debug_assert_eq!(
                self.current_chunk_footer.get().as_ref().ptr.get(),
                self.current_chunk_footer.get().cast(),
                "Our chunk's bump finger should be reset to the start of its allocation"
            );
        }
    }

    /// Allocate an object in this `Bump` and return an exclusive reference to
    /// it.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for `T` fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc("hello");
    /// assert_eq!(*x, "hello");
    /// ```
    #[inline(always)]
    pub fn alloc<T>(&self, val: T) -> &mut T {
        self.alloc_with(|| val)
    }

    /// Try to allocate an object in this `Bump` and return an exclusive
    /// reference to it.
    ///
    /// ## Errors
    ///
    /// Errors if reserving space for `T` fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.try_alloc("hello");
    /// assert_eq!(x, Ok(&mut "hello"));
    /// ```
    #[inline(always)]
    pub fn try_alloc<T>(&self, val: T) -> Result<&mut T, AllocErr> {
        self.try_alloc_with(|| val)
    }

    /// Pre-allocate space for an object in this `Bump`, initializes it using
    /// the closure, then returns an exclusive reference to it.
    ///
    /// See [The `_with` Method Suffix](#initializer-functions-the-_with-method-suffix) for a
    /// discussion on the differences between the `_with` suffixed methods and
    /// those methods without it, their performance characteristics, and when
    /// you might or might not choose a `_with` suffixed method.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for `T` fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc_with(|| "hello");
    /// assert_eq!(*x, "hello");
    /// ```
    #[inline(always)]
    pub fn alloc_with<F, T>(&self, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        #[inline(always)]
        unsafe fn inner_writer<T, F>(ptr: *mut T, f: F)
        where
            F: FnOnce() -> T,
        {
            // This function is translated as:
            // - allocate space for a T on the stack
            // - call f() with the return value being put onto this stack space
            // - memcpy from the stack to the heap
            //
            // Ideally we want LLVM to always realize that doing a stack
            // allocation is unnecessary and optimize the code so it writes
            // directly into the heap instead. It seems we get it to realize
            // this most consistently if we put this critical line into it's
            // own function instead of inlining it into the surrounding code.
            ptr::write(ptr, f());
        }

        let layout = Layout::new::<T>();

        unsafe {
            let p = self.alloc_layout(layout);
            let p = p.as_ptr() as *mut T;
            inner_writer(p, f);
            &mut *p
        }
    }

    /// Tries to pre-allocate space for an object in this `Bump`, initializes
    /// it using the closure, then returns an exclusive reference to it.
    ///
    /// See [The `_with` Method Suffix](#initializer-functions-the-_with-method-suffix) for a
    /// discussion on the differences between the `_with` suffixed methods and
    /// those methods without it, their performance characteristics, and when
    /// you might or might not choose a `_with` suffixed method.
    ///
    /// ## Errors
    ///
    /// Errors if reserving space for `T` fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.try_alloc_with(|| "hello");
    /// assert_eq!(x, Ok(&mut "hello"));
    /// ```
    #[inline(always)]
    pub fn try_alloc_with<F, T>(&self, f: F) -> Result<&mut T, AllocErr>
    where
        F: FnOnce() -> T,
    {
        #[inline(always)]
        unsafe fn inner_writer<T, F>(ptr: *mut T, f: F)
        where
            F: FnOnce() -> T,
        {
            // This function is translated as:
            // - allocate space for a T on the stack
            // - call f() with the return value being put onto this stack space
            // - memcpy from the stack to the heap
            //
            // Ideally we want LLVM to always realize that doing a stack
            // allocation is unnecessary and optimize the code so it writes
            // directly into the heap instead. It seems we get it to realize
            // this most consistently if we put this critical line into it's
            // own function instead of inlining it into the surrounding code.
            ptr::write(ptr, f());
        }

        //SAFETY: Self-contained:
        // `p` is allocated for `T` and then a `T` is written.
        let layout = Layout::new::<T>();
        let p = self.try_alloc_layout(layout)?;
        let p = p.as_ptr() as *mut T;

        unsafe {
            inner_writer(p, f);
            Ok(&mut *p)
        }
    }

    /// Pre-allocates space for a [`Result`] in this `Bump`, initializes it using
    /// the closure, then returns an exclusive reference to its `T` if [`Ok`].
    ///
    /// Iff the allocation fails, the closure is not run.
    ///
    /// Iff [`Err`], an allocator rewind is *attempted* and the `E` instance is
    /// moved out of the allocator to be consumed or dropped as normal.
    ///
    /// See [The `_with` Method Suffix](#initializer-functions-the-_with-method-suffix) for a
    /// discussion on the differences between the `_with` suffixed methods and
    /// those methods without it, their performance characteristics, and when
    /// you might or might not choose a `_with` suffixed method.
    ///
    /// For caveats specific to fallible initialization, see
    /// [The `_try_with` Method Suffix](#fallible-initialization-the-_try_with-method-suffix).
    ///
    /// [`Result`]: https://doc.rust-lang.org/std/result/enum.Result.html
    /// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    ///
    /// ## Errors
    ///
    /// Iff the allocation succeeds but `f` fails, that error is forwarded by value.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for `Result<T, E>` fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc_try_with(|| Ok("hello"))?;
    /// assert_eq!(*x, "hello");
    /// # Result::<_, ()>::Ok(())
    /// ```
    #[inline(always)]
    pub fn alloc_try_with<F, T, E>(&self, f: F) -> Result<&mut T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let rewind_footer = self.current_chunk_footer.get();
        let rewind_ptr = unsafe { rewind_footer.as_ref() }.ptr.get();
        let mut inner_result_ptr = NonNull::from(self.alloc_with(f));
        match unsafe { inner_result_ptr.as_mut() } {
            Ok(t) => Ok(unsafe {
                //SAFETY:
                // The `&mut Result<T, E>` returned by `alloc_with` may be
                // lifetime-limited by `E`, but the derived `&mut T` still has
                // the same validity as in `alloc_with` since the error variant
                // is already ruled out here.

                // We could conditionally truncate the allocation here, but
                // since it grows backwards, it seems unlikely that we'd get
                // any more than the `Result`'s discriminant this way, if
                // anything at all.
                &mut *(t as *mut _)
            }),
            Err(e) => unsafe {
                // If this result was the last allocation in this arena, we can
                // reclaim its space. In fact, sometimes we can do even better
                // than simply calling `dealloc` on the result pointer: we can
                // reclaim any alignment padding we might have added (which
                // `dealloc` cannot do) if we didn't allocate a new chunk for
                // this result.
                if self.is_last_allocation(inner_result_ptr.cast()) {
                    let current_footer_p = self.current_chunk_footer.get();
                    let current_ptr = &current_footer_p.as_ref().ptr;
                    if current_footer_p == rewind_footer {
                        // It's still the same chunk, so reset the bump pointer
                        // to its original value upon entry to this method
                        // (reclaiming any alignment padding we may have
                        // added).
                        current_ptr.set(rewind_ptr);
                    } else {
                        // We allocated a new chunk for this result.
                        //
                        // We know the result is the only allocation in this
                        // chunk: Any additional allocations since the start of
                        // this method could only have happened when running
                        // the initializer function, which is called *after*
                        // reserving space for this result. Therefore, since we
                        // already determined via the check above that this
                        // result was the last allocation, there must not have
                        // been any other allocations, and this result is the
                        // only allocation in this chunk.
                        //
                        // Because this is the only allocation in this chunk,
                        // we can reset the chunk's bump finger to the start of
                        // the chunk.
                        current_ptr.set(current_footer_p.as_ref().data);
                    }
                }
                //SAFETY:
                // As we received `E` semantically by value from `f`, we can
                // just copy that value here as long as we avoid a double-drop
                // (which can't happen as any specific references to the `E`'s
                // data in `self` are destroyed when this function returns).
                //
                // The order between this and the deallocation doesn't matter
                // because `Self: !Sync`.
                Err(ptr::read(e as *const _))
            },
        }
    }

    /// Tries to pre-allocates space for a [`Result`] in this `Bump`,
    /// initializes it using the closure, then returns an exclusive reference
    /// to its `T` if all [`Ok`].
    ///
    /// Iff the allocation fails, the closure is not run.
    ///
    /// Iff the closure returns [`Err`], an allocator rewind is *attempted* and
    /// the `E` instance is moved out of the allocator to be consumed or dropped
    /// as normal.
    ///
    /// See [The `_with` Method Suffix](#initializer-functions-the-_with-method-suffix) for a
    /// discussion on the differences between the `_with` suffixed methods and
    /// those methods without it, their performance characteristics, and when
    /// you might or might not choose a `_with` suffixed method.
    ///
    /// For caveats specific to fallible initialization, see
    /// [The `_try_with` Method Suffix](#fallible-initialization-the-_try_with-method-suffix).
    ///
    /// [`Result`]: https://doc.rust-lang.org/std/result/enum.Result.html
    /// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    ///
    /// ## Errors
    ///
    /// Errors with the [`Alloc`](`AllocOrInitError::Alloc`) variant iff
    /// reserving space for `Result<T, E>` fails.
    ///
    /// Iff the allocation succeeds but `f` fails, that error is forwarded by
    /// value inside the [`Init`](`AllocOrInitError::Init`) variant.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.try_alloc_try_with(|| Ok("hello"))?;
    /// assert_eq!(*x, "hello");
    /// # Result::<_, bumpalo::AllocOrInitError<()>>::Ok(())
    /// ```
    #[inline(always)]
    pub fn try_alloc_try_with<F, T, E>(&self, f: F) -> Result<&mut T, AllocOrInitError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let rewind_footer = self.current_chunk_footer.get();
        let rewind_ptr = unsafe { rewind_footer.as_ref() }.ptr.get();
        let mut inner_result_ptr = NonNull::from(self.try_alloc_with(f)?);
        match unsafe { inner_result_ptr.as_mut() } {
            Ok(t) => Ok(unsafe {
                //SAFETY:
                // The `&mut Result<T, E>` returned by `alloc_with` may be
                // lifetime-limited by `E`, but the derived `&mut T` still has
                // the same validity as in `alloc_with` since the error variant
                // is already ruled out here.

                // We could conditionally truncate the allocation here, but
                // since it grows backwards, it seems unlikely that we'd get
                // any more than the `Result`'s discriminant this way, if
                // anything at all.
                &mut *(t as *mut _)
            }),
            Err(e) => unsafe {
                // If this result was the last allocation in this arena, we can
                // reclaim its space. In fact, sometimes we can do even better
                // than simply calling `dealloc` on the result pointer: we can
                // reclaim any alignment padding we might have added (which
                // `dealloc` cannot do) if we didn't allocate a new chunk for
                // this result.
                if self.is_last_allocation(inner_result_ptr.cast()) {
                    let current_footer_p = self.current_chunk_footer.get();
                    let current_ptr = &current_footer_p.as_ref().ptr;
                    if current_footer_p == rewind_footer {
                        // It's still the same chunk, so reset the bump pointer
                        // to its original value upon entry to this method
                        // (reclaiming any alignment padding we may have
                        // added).
                        current_ptr.set(rewind_ptr);
                    } else {
                        // We allocated a new chunk for this result.
                        //
                        // We know the result is the only allocation in this
                        // chunk: Any additional allocations since the start of
                        // this method could only have happened when running
                        // the initializer function, which is called *after*
                        // reserving space for this result. Therefore, since we
                        // already determined via the check above that this
                        // result was the last allocation, there must not have
                        // been any other allocations, and this result is the
                        // only allocation in this chunk.
                        //
                        // Because this is the only allocation in this chunk,
                        // we can reset the chunk's bump finger to the start of
                        // the chunk.
                        current_ptr.set(current_footer_p.as_ref().data);
                    }
                }
                //SAFETY:
                // As we received `E` semantically by value from `f`, we can
                // just copy that value here as long as we avoid a double-drop
                // (which can't happen as any specific references to the `E`'s
                // data in `self` are destroyed when this function returns).
                //
                // The order between this and the deallocation doesn't matter
                // because `Self: !Sync`.
                Err(AllocOrInitError::Init(ptr::read(e as *const _)))
            },
        }
    }

    /// `Copy` a slice into this `Bump` and return an exclusive reference to
    /// the copy.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc_slice_copy(&[1, 2, 3]);
    /// assert_eq!(x, &[1, 2, 3]);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_copy<T>(&self, src: &[T]) -> &mut [T]
    where
        T: Copy,
    {
        let layout = Layout::for_value(src);
        let dst = self.alloc_layout(layout).cast::<T>();

        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_ptr(), src.len());
            slice::from_raw_parts_mut(dst.as_ptr(), src.len())
        }
    }

    /// Like `alloc_slice_copy`, but does not panic in case of allocation failure.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.try_alloc_slice_copy(&[1, 2, 3]);
    /// assert_eq!(x, Ok(&mut[1, 2, 3] as &mut [_]));
    ///
    ///
    /// let bump = bumpalo::Bump::new();
    /// bump.set_allocation_limit(Some(4));
    /// let x = bump.try_alloc_slice_copy(&[1, 2, 3, 4, 5, 6]);
    /// assert_eq!(x, Err(bumpalo::AllocErr)); // too big
    /// ```
    #[inline(always)]
    pub fn try_alloc_slice_copy<T>(&self, src: &[T]) -> Result<&mut [T], AllocErr>
    where
        T: Copy,
    {
        let layout = Layout::for_value(src);
        let dst = self.try_alloc_layout(layout)?.cast::<T>();
        let result = unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_ptr(), src.len());
            slice::from_raw_parts_mut(dst.as_ptr(), src.len())
        };
        Ok(result)
    }

    /// `Clone` a slice into this `Bump` and return an exclusive reference to
    /// the clone. Prefer [`alloc_slice_copy`](#method.alloc_slice_copy) if `T` is `Copy`.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// #[derive(Clone, Debug, Eq, PartialEq)]
    /// struct Sheep {
    ///     name: String,
    /// }
    ///
    /// let originals = [
    ///     Sheep { name: "Alice".into() },
    ///     Sheep { name: "Bob".into() },
    ///     Sheep { name: "Cathy".into() },
    /// ];
    ///
    /// let bump = bumpalo::Bump::new();
    /// let clones = bump.alloc_slice_clone(&originals);
    /// assert_eq!(originals, clones);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_clone<T>(&self, src: &[T]) -> &mut [T]
    where
        T: Clone,
    {
        let layout = Layout::for_value(src);
        let dst = self.alloc_layout(layout).cast::<T>();

        unsafe {
            for (i, val) in src.iter().cloned().enumerate() {
                ptr::write(dst.as_ptr().add(i), val);
            }

            slice::from_raw_parts_mut(dst.as_ptr(), src.len())
        }
    }

    /// Like `alloc_slice_clone` but does not panic on failure.
    #[inline(always)]
    pub fn try_alloc_slice_clone<T>(&self, src: &[T]) -> Result<&mut [T], AllocErr>
    where
        T: Clone,
    {
        let layout = Layout::for_value(src);
        let dst = self.try_alloc_layout(layout)?.cast::<T>();

        unsafe {
            for (i, val) in src.iter().cloned().enumerate() {
                ptr::write(dst.as_ptr().add(i), val);
            }

            Ok(slice::from_raw_parts_mut(dst.as_ptr(), src.len()))
        }
    }

    /// `Copy` a string slice into this `Bump` and return an exclusive reference to it.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the string fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let hello = bump.alloc_str("hello world");
    /// assert_eq!("hello world", hello);
    /// ```
    #[inline(always)]
    pub fn alloc_str(&self, src: &str) -> &mut str {
        let buffer = self.alloc_slice_copy(src.as_bytes());
        unsafe {
            // This is OK, because it already came in as str, so it is guaranteed to be utf8
            str::from_utf8_unchecked_mut(buffer)
        }
    }

    /// Same as `alloc_str` but does not panic on failure.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let hello = bump.try_alloc_str("hello world").unwrap();
    /// assert_eq!("hello world", hello);
    ///
    ///
    /// let bump = bumpalo::Bump::new();
    /// bump.set_allocation_limit(Some(5));
    /// let hello = bump.try_alloc_str("hello world");
    /// assert_eq!(Err(bumpalo::AllocErr), hello);
    /// ```
    #[inline(always)]
    pub fn try_alloc_str(&self, src: &str) -> Result<&mut str, AllocErr> {
        let buffer = self.try_alloc_slice_copy(src.as_bytes())?;
        unsafe {
            // This is OK, because it already came in as str, so it is guaranteed to be utf8
            Ok(str::from_utf8_unchecked_mut(buffer))
        }
    }

    /// Allocates a new slice of size `len` into this `Bump` and returns an
    /// exclusive reference to the copy.
    ///
    /// The elements of the slice are initialized using the supplied closure.
    /// The closure argument is the position in the slice.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc_slice_fill_with(5, |i| 5 * (i + 1));
    /// assert_eq!(x, &[5, 10, 15, 20, 25]);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_fill_with<T, F>(&self, len: usize, mut f: F) -> &mut [T]
    where
        F: FnMut(usize) -> T,
    {
        let layout = Layout::array::<T>(len).unwrap_or_else(|_| oom());
        let dst = self.alloc_layout(layout).cast::<T>();

        unsafe {
            for i in 0..len {
                ptr::write(dst.as_ptr().add(i), f(i));
            }

            let result = slice::from_raw_parts_mut(dst.as_ptr(), len);
            debug_assert_eq!(Layout::for_value(result), layout);
            result
        }
    }

    /// Allocates a new slice of size `len` into this `Bump` and returns an
    /// exclusive reference to the copy, failing if the closure return an Err.
    ///
    /// The elements of the slice are initialized using the supplied closure.
    /// The closure argument is the position in the slice.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x: Result<&mut [usize], ()> = bump.alloc_slice_try_fill_with(5, |i| Ok(5 * i));
    /// assert_eq!(x, Ok(bump.alloc_slice_copy(&[0, 5, 10, 15, 20])));
    /// ```
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x: Result<&mut [usize], ()> = bump.alloc_slice_try_fill_with(
    ///    5,
    ///    |n| if n == 2 { Err(()) } else { Ok(n) }
    /// );
    /// assert_eq!(x, Err(()));
    /// ```
    #[inline(always)]
    pub fn alloc_slice_try_fill_with<T, F, E>(&self, len: usize, mut f: F) -> Result<&mut [T], E>
    where
        F: FnMut(usize) -> Result<T, E>,
    {
        let layout = Layout::array::<T>(len).unwrap_or_else(|_| oom());
        let base_ptr = self.alloc_layout(layout);
        let dst = base_ptr.cast::<T>();

        unsafe {
            for i in 0..len {
                match f(i) {
                    Ok(el) => ptr::write(dst.as_ptr().add(i), el),
                    Err(e) => {
                        self.dealloc(base_ptr, layout);
                        return Err(e);
                    }
                }
            }

            let result = slice::from_raw_parts_mut(dst.as_ptr(), len);
            debug_assert_eq!(Layout::for_value(result), layout);
            Ok(result)
        }
    }

    /// Allocates a new slice of size `len` into this `Bump` and returns an
    /// exclusive reference to the copy.
    ///
    /// The elements of the slice are initialized using the supplied closure.
    /// The closure argument is the position in the slice.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.try_alloc_slice_fill_with(5, |i| 5 * (i + 1));
    /// assert_eq!(x, Ok(&mut[5usize, 10, 15, 20, 25] as &mut [_]));
    ///
    ///
    /// let bump = bumpalo::Bump::new();
    /// bump.set_allocation_limit(Some(4));
    /// let x = bump.try_alloc_slice_fill_with(10, |i| 5 * (i + 1));
    /// assert_eq!(x, Err(bumpalo::AllocErr));
    /// ```
    #[inline(always)]
    pub fn try_alloc_slice_fill_with<T, F>(
        &self,
        len: usize,
        mut f: F,
    ) -> Result<&mut [T], AllocErr>
    where
        F: FnMut(usize) -> T,
    {
        let layout = Layout::array::<T>(len).map_err(|_| AllocErr)?;
        let dst = self.try_alloc_layout(layout)?.cast::<T>();

        unsafe {
            for i in 0..len {
                ptr::write(dst.as_ptr().add(i), f(i));
            }

            let result = slice::from_raw_parts_mut(dst.as_ptr(), len);
            debug_assert_eq!(Layout::for_value(result), layout);
            Ok(result)
        }
    }

    /// Allocates a new slice of size `len` into this `Bump` and returns an
    /// exclusive reference to the copy.
    ///
    /// All elements of the slice are initialized to `value`.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc_slice_fill_copy(5, 42);
    /// assert_eq!(x, &[42, 42, 42, 42, 42]);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_fill_copy<T: Copy>(&self, len: usize, value: T) -> &mut [T] {
        self.alloc_slice_fill_with(len, |_| value)
    }

    /// Same as `alloc_slice_fill_copy` but does not panic on failure.
    #[inline(always)]
    pub fn try_alloc_slice_fill_copy<T: Copy>(
        &self,
        len: usize,
        value: T,
    ) -> Result<&mut [T], AllocErr> {
        self.try_alloc_slice_fill_with(len, |_| value)
    }

    /// Allocates a new slice of size `len` slice into this `Bump` and return an
    /// exclusive reference to the copy.
    ///
    /// All elements of the slice are initialized to `value.clone()`.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let s: String = "Hello Bump!".to_string();
    /// let x: &[String] = bump.alloc_slice_fill_clone(2, &s);
    /// assert_eq!(x.len(), 2);
    /// assert_eq!(&x[0], &s);
    /// assert_eq!(&x[1], &s);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_fill_clone<T: Clone>(&self, len: usize, value: &T) -> &mut [T] {
        self.alloc_slice_fill_with(len, |_| value.clone())
    }

    /// Like `alloc_slice_fill_clone` but does not panic on failure.
    #[inline(always)]
    pub fn try_alloc_slice_fill_clone<T: Clone>(
        &self,
        len: usize,
        value: &T,
    ) -> Result<&mut [T], AllocErr> {
        self.try_alloc_slice_fill_with(len, |_| value.clone())
    }

    /// Allocates a new slice of size `len` slice into this `Bump` and return an
    /// exclusive reference to the copy.
    ///
    /// The elements are initialized using the supplied iterator.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails, or if the supplied
    /// iterator returns fewer elements than it promised.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x: &[i32] = bump.alloc_slice_fill_iter([2, 3, 5].iter().cloned().map(|i| i * i));
    /// assert_eq!(x, [4, 9, 25]);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_fill_iter<T, I>(&self, iter: I) -> &mut [T]
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let mut iter = iter.into_iter();
        self.alloc_slice_fill_with(iter.len(), |_| {
            iter.next().expect("Iterator supplied too few elements")
        })
    }

    /// Allocates a new slice of size `len` slice into this `Bump` and return an
    /// exclusive reference to the copy, failing if the iterator returns an Err.
    ///
    /// The elements are initialized using the supplied iterator.
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails, or if the supplied
    /// iterator returns fewer elements than it promised.
    ///
    /// ## Examples
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x: Result<&mut [i32], ()> = bump.alloc_slice_try_fill_iter(
    ///    [2, 3, 5].iter().cloned().map(|i| Ok(i * i))
    /// );
    /// assert_eq!(x, Ok(bump.alloc_slice_copy(&[4, 9, 25])));
    /// ```
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x: Result<&mut [i32], ()> = bump.alloc_slice_try_fill_iter(
    ///    [Ok(2), Err(()), Ok(5)].iter().cloned()
    /// );
    /// assert_eq!(x, Err(()));
    /// ```
    #[inline(always)]
    pub fn alloc_slice_try_fill_iter<T, I, E>(&self, iter: I) -> Result<&mut [T], E>
    where
        I: IntoIterator<Item = Result<T, E>>,
        I::IntoIter: ExactSizeIterator,
    {
        let mut iter = iter.into_iter();
        self.alloc_slice_try_fill_with(iter.len(), |_| {
            iter.next().expect("Iterator supplied too few elements")
        })
    }

    /// Allocates a new slice of size `iter.len()` slice into this `Bump` and return an
    /// exclusive reference to the copy. Does not panic on failure.
    ///
    /// The elements are initialized using the supplied iterator.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x: &[i32] = bump.try_alloc_slice_fill_iter([2, 3, 5]
    ///     .iter().cloned().map(|i| i * i)).unwrap();
    /// assert_eq!(x, [4, 9, 25]);
    /// ```
    #[inline(always)]
    pub fn try_alloc_slice_fill_iter<T, I>(&self, iter: I) -> Result<&mut [T], AllocErr>
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let mut iter = iter.into_iter();
        self.try_alloc_slice_fill_with(iter.len(), |_| {
            iter.next().expect("Iterator supplied too few elements")
        })
    }

    /// Allocates a new slice of size `len` slice into this `Bump` and return an
    /// exclusive reference to the copy.
    ///
    /// All elements of the slice are initialized to [`T::default()`].
    ///
    /// [`T::default()`]: https://doc.rust-lang.org/std/default/trait.Default.html#tymethod.default
    ///
    /// ## Panics
    ///
    /// Panics if reserving space for the slice fails.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let x = bump.alloc_slice_fill_default::<u32>(5);
    /// assert_eq!(x, &[0, 0, 0, 0, 0]);
    /// ```
    #[inline(always)]
    pub fn alloc_slice_fill_default<T: Default>(&self, len: usize) -> &mut [T] {
        self.alloc_slice_fill_with(len, |_| T::default())
    }

    /// Like `alloc_slice_fill_default` but does not panic on failure.
    #[inline(always)]
    pub fn try_alloc_slice_fill_default<T: Default>(
        &self,
        len: usize,
    ) -> Result<&mut [T], AllocErr> {
        self.try_alloc_slice_fill_with(len, |_| T::default())
    }

    /// Allocate space for an object with the given `Layout`.
    ///
    /// The returned pointer points at uninitialized memory, and should be
    /// initialized with
    /// [`std::ptr::write`](https://doc.rust-lang.org/std/ptr/fn.write.html).
    ///
    /// # Panics
    ///
    /// Panics if reserving space matching `layout` fails.
    #[inline(always)]
    pub fn alloc_layout(&self, layout: Layout) -> NonNull<u8> {
        self.try_alloc_layout(layout).unwrap_or_else(|_| oom())
    }

    /// Attempts to allocate space for an object with the given `Layout` or else returns
    /// an `Err`.
    ///
    /// The returned pointer points at uninitialized memory, and should be
    /// initialized with
    /// [`std::ptr::write`](https://doc.rust-lang.org/std/ptr/fn.write.html).
    ///
    /// # Errors
    ///
    /// Errors if reserving space matching `layout` fails.
    #[inline(always)]
    pub fn try_alloc_layout(&self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {
        if let Some(p) = self.try_alloc_layout_fast(layout) {
            Ok(p)
        } else {
            self.alloc_layout_slow(layout).ok_or(AllocErr)
        }
    }

    #[inline(always)]
    fn try_alloc_layout_fast(&self, layout: Layout) -> Option<NonNull<u8>> {
        // We don't need to check for ZSTs here since they will automatically
        // be handled properly: the pointer will be bumped by zero bytes,
        // modulo alignment. This keeps the fast path optimized for non-ZSTs,
        // which are much more common.
        unsafe {
            let footer_ptr = self.current_chunk_footer.get();
            let footer = footer_ptr.as_ref();

            let ptr = footer.ptr.get().as_ptr();
            let start = footer.data.as_ptr();
            debug_assert!(
                start <= ptr,
                "start pointer {start:#p} should be less than or equal to bump pointer {ptr:#p}"
            );
            debug_assert!(
                ptr <= footer_ptr.cast::<u8>().as_ptr(),
                "bump pointer {ptr:#p} should be less than or equal to footer pointer {footer_ptr:#p}"
            );
            debug_assert!(
                is_pointer_aligned_to(ptr, MIN_ALIGN),
                "bump pointer {ptr:#p} should be aligned to the minimum alignment of {MIN_ALIGN:#x}"
            );

            // This `match` should be boiled away by LLVM: `MIN_ALIGN` is a
            // constant and the layout's alignment is also constant in practice
            // after inlining.
            let aligned_ptr = match layout.align().cmp(&MIN_ALIGN) {
                Ordering::Less => {
                    // We need to round the size up to a multiple of `MIN_ALIGN`
                    // to preserve the minimum alignment. This might overflow
                    // since we cannot rely on `Layout`'s guarantees.
                    let aligned_size = round_up_to(layout.size(), MIN_ALIGN)?;

                    let capacity = (ptr as usize) - (start as usize);
                    if aligned_size > capacity {
                        return None;
                    }

                    ptr.wrapping_sub(aligned_size)
                }
                Ordering::Equal => {
                    // `Layout` guarantees that rounding the size up to its
                    // align cannot overflow (but does not guarantee that the
                    // size is initially a multiple of the alignment, which is
                    // why we need to do this rounding).
                    let aligned_size = round_up_to_unchecked(layout.size(), layout.align());

                    let capacity = (ptr as usize) - (start as usize);
                    if aligned_size > capacity {
                        return None;
                    }

                    ptr.wrapping_sub(aligned_size)
                }
                Ordering::Greater => {
                    // `Layout` guarantees that rounding the size up to its
                    // align cannot overflow (but does not guarantee that the
                    // size is initially a multiple of the alignment, which is
                    // why we need to do this rounding).
                    let aligned_size = round_up_to_unchecked(layout.size(), layout.align());

                    let aligned_ptr = round_mut_ptr_down_to(ptr, layout.align());
                    let capacity = (aligned_ptr as usize).wrapping_sub(start as usize);
                    if aligned_ptr < start || aligned_size > capacity {
                        return None;
                    }

                    aligned_ptr.wrapping_sub(aligned_size)
                }
            };

            debug_assert!(
                is_pointer_aligned_to(aligned_ptr, layout.align()),
                "pointer {aligned_ptr:#p} should be aligned to layout alignment of {:#}",
                layout.align()
            );
            debug_assert!(
                is_pointer_aligned_to(aligned_ptr, MIN_ALIGN),
                "pointer {aligned_ptr:#p} should be aligned to minimum alignment of {:#}",
                MIN_ALIGN
            );
            debug_assert!(
                start <= aligned_ptr && aligned_ptr <= ptr,
                "pointer {aligned_ptr:#p} should be in range {start:#p}..{ptr:#p}"
            );

            debug_assert!(!aligned_ptr.is_null());
            let aligned_ptr = NonNull::new_unchecked(aligned_ptr);

            footer.ptr.set(aligned_ptr);
            Some(aligned_ptr)
        }
    }

    /// Gets the remaining capacity in the current chunk (in bytes).
    ///
    /// ## Example
    ///
    /// ```
    /// use bumpalo::Bump;
    ///
    /// let bump = Bump::with_capacity(100);
    ///
    /// let capacity = bump.chunk_capacity();
    /// assert!(capacity >= 100);
    /// ```
    pub fn chunk_capacity(&self) -> usize {
        let current_footer = self.current_chunk_footer.get();
        let current_footer = unsafe { current_footer.as_ref() };

        current_footer.ptr.get().as_ptr() as usize - current_footer.data.as_ptr() as usize
    }

    /// Slow path allocation for when we need to allocate a new chunk from the
    /// parent bump set because there isn't enough room in our current chunk.
    #[inline(never)]
    #[cold]
    fn alloc_layout_slow(&self, layout: Layout) -> Option<NonNull<u8>> {
        unsafe {
            let allocation_limit_remaining = self.allocation_limit_remaining();

            // Get a new chunk from the global allocator.
            let current_footer = self.current_chunk_footer.get();
            let current_layout = current_footer.as_ref().layout;

            // By default, we want our new chunk to be about twice as big
            // as the previous chunk. If the global allocator refuses it,
            // we try to divide it by half until it works or the requested
            // size is smaller than the default footer size.
            let min_new_chunk_size = layout.size().max(DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER);
            let mut base_size = (current_layout.size() - FOOTER_SIZE)
                .checked_mul(2)?
                .max(min_new_chunk_size);
            let chunk_memory_details = iter::from_fn(|| {
                let bypass_min_chunk_size_for_small_limits = matches!(self.allocation_limit(), Some(limit) if layout.size() < limit
                            && base_size >= layout.size()
                            && limit < DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER
                            && self.allocated_bytes() == 0);

                if base_size >= min_new_chunk_size || bypass_min_chunk_size_for_small_limits {
                    let size = base_size;
                    base_size /= 2;
                    Self::new_chunk_memory_details(Some(size), layout)
                } else {
                    None
                }
            });

            let new_footer = chunk_memory_details
                .filter_map(|chunk_memory_details| {
                    if Self::chunk_fits_under_limit(
                        allocation_limit_remaining,
                        chunk_memory_details,
                    ) {
                        Self::new_chunk(chunk_memory_details, layout, current_footer)
                    } else {
                        None
                    }
                })
                .next()?;

            debug_assert_eq!(
                new_footer.as_ref().data.as_ptr() as usize % layout.align(),
                0
            );

            // Set the new chunk as our new current chunk.
            self.current_chunk_footer.set(new_footer);

            // And then we can rely on `tray_alloc_layout_fast` to allocate
            // space within this chunk.
            let ptr = self.try_alloc_layout_fast(layout);
            debug_assert!(ptr.is_some());
            ptr
        }
    }

    /// Returns an iterator over each chunk of allocated memory that
    /// this arena has bump allocated into.
    ///
    /// The chunks are returned ordered by allocation time, with the most
    /// recently allocated chunk being returned first, and the least recently
    /// allocated chunk being returned last.
    ///
    /// The values inside each chunk are also ordered by allocation time, with
    /// the most recent allocation being earlier in the slice, and the least
    /// recent allocation being towards the end of the slice.
    ///
    /// ## Safety
    ///
    /// Because this method takes `&mut self`, we know that the bump arena
    /// reference is unique and therefore there aren't any active references to
    /// any of the objects we've allocated in it either. This potential aliasing
    /// of exclusive references is one common footgun for unsafe code that we
    /// don't need to worry about here.
    ///
    /// However, there could be regions of uninitialized memory used as padding
    /// between allocations, which is why this iterator has items of type
    /// `[MaybeUninit<u8>]`, instead of simply `[u8]`.
    ///
    /// The only way to guarantee that there is no padding between allocations
    /// or within allocated objects is if all of these properties hold:
    ///
    /// 1. Every object allocated in this arena has the same alignment,
    ///    and that alignment is at most 16.
    /// 2. Every object's size is a multiple of its alignment.
    /// 3. None of the objects allocated in this arena contain any internal
    ///    padding.
    ///
    /// If you want to use this `iter_allocated_chunks` method, it is *your*
    /// responsibility to ensure that these properties hold before calling
    /// `MaybeUninit::assume_init` or otherwise reading the returned values.
    ///
    /// Finally, you must also ensure that any values allocated into the bump
    /// arena have not had their `Drop` implementations called on them,
    /// e.g. after dropping a [`bumpalo::boxed::Box<T>`][crate::boxed::Box].
    ///
    /// ## Example
    ///
    /// ```
    /// let mut bump = bumpalo::Bump::new();
    ///
    /// // Allocate a bunch of `i32`s in this bump arena, potentially causing
    /// // additional memory chunks to be reserved.
    /// for i in 0..10000 {
    ///     bump.alloc(i);
    /// }
    ///
    /// // Iterate over each chunk we've bump allocated into. This is safe
    /// // because we have only allocated `i32`s in this arena, which fulfills
    /// // the above requirements.
    /// for ch in bump.iter_allocated_chunks() {
    ///     println!("Used a chunk that is {} bytes long", ch.len());
    ///     println!("The first byte is {:?}", unsafe {
    ///         ch[0].assume_init()
    ///     });
    /// }
    ///
    /// // Within a chunk, allocations are ordered from most recent to least
    /// // recent. If we allocated 'a', then 'b', then 'c', when we iterate
    /// // through the chunk's data, we get them in the order 'c', then 'b',
    /// // then 'a'.
    ///
    /// bump.reset();
    /// bump.alloc(b'a');
    /// bump.alloc(b'b');
    /// bump.alloc(b'c');
    ///
    /// assert_eq!(bump.iter_allocated_chunks().count(), 1);
    /// let chunk = bump.iter_allocated_chunks().nth(0).unwrap();
    /// assert_eq!(chunk.len(), 3);
    ///
    /// // Safe because we've only allocated `u8`s in this arena, which
    /// // fulfills the above requirements.
    /// unsafe {
    ///     assert_eq!(chunk[0].assume_init(), b'c');
    ///     assert_eq!(chunk[1].assume_init(), b'b');
    ///     assert_eq!(chunk[2].assume_init(), b'a');
    /// }
    /// ```
    pub fn iter_allocated_chunks(&mut self) -> ChunkIter<'_, MIN_ALIGN> {
        // Safety: Ensured by mutable borrow of `self`.
        let raw = unsafe { self.iter_allocated_chunks_raw() };
        ChunkIter {
            raw,
            bump: PhantomData,
        }
    }

    /// Returns an iterator over raw pointers to chunks of allocated memory that
    /// this arena has bump allocated into.
    ///
    /// This is an unsafe version of [`iter_allocated_chunks()`](Bump::iter_allocated_chunks),
    /// with the caller responsible for safe usage of the returned pointers as
    /// well as ensuring that the iterator is not invalidated by new
    /// allocations.
    ///
    /// ## Safety
    ///
    /// Allocations from this arena must not be performed while the returned
    /// iterator is alive. If reading the chunk data (or casting to a reference)
    /// the caller must ensure that there exist no mutable references to
    /// previously allocated data.
    ///
    /// In addition, all of the caveats when reading the chunk data from
    /// [`iter_allocated_chunks()`](Bump::iter_allocated_chunks) still apply.
    pub unsafe fn iter_allocated_chunks_raw(&self) -> ChunkRawIter<'_, MIN_ALIGN> {
        ChunkRawIter {
            footer: self.current_chunk_footer.get(),
            bump: PhantomData,
        }
    }

    /// Calculates the number of bytes currently allocated across all chunks in
    /// this bump arena.
    ///
    /// If you allocate types of different alignments or types with
    /// larger-than-typical alignment in the same arena, some padding
    /// bytes might get allocated in the bump arena. Note that those padding
    /// bytes will add to this method's resulting sum, so you cannot rely
    /// on it only counting the sum of the sizes of the things
    /// you've allocated in the arena.
    ///
    /// The allocated bytes do not include the size of bumpalo's metadata,
    /// so the amount of memory requested from the Rust allocator is higher
    /// than the returned value.
    ///
    /// ## Example
    ///
    /// ```
    /// let bump = bumpalo::Bump::new();
    /// let _x = bump.alloc_slice_fill_default::<u32>(5);
    /// let bytes = bump.allocated_bytes();
    /// assert!(bytes >= core::mem::size_of::<u32>() * 5);
    /// ```
    pub fn allocated_bytes(&self) -> usize {
        let footer = self.current_chunk_footer.get();

        unsafe { footer.as_ref().allocated_bytes }
    }

    /// Calculates the number of bytes requested from the Rust allocator for this `Bump`.
    ///
    /// This number is equal to the [`allocated_bytes()`](Self::allocated_bytes) plus
    /// the size of the bump metadata.
    pub fn allocated_bytes_including_metadata(&self) -> usize {
        let metadata_size =
            unsafe { self.iter_allocated_chunks_raw().count() * mem::size_of::<ChunkFooter>() };
        self.allocated_bytes() + metadata_size
    }

    #[inline]
    unsafe fn is_last_allocation(&self, ptr: NonNull<u8>) -> bool {
        let footer = self.current_chunk_footer.get();
        let footer = footer.as_ref();
        footer.ptr.get() == ptr
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        // If the pointer is the last allocation we made, we can reuse the bytes,
        // otherwise they are simply leaked -- at least until somebody calls reset().
        if self.is_last_allocation(ptr) {
            let ptr = self.current_chunk_footer.get().as_ref().ptr.get();
            let ptr = ptr.as_ptr().add(layout.size());

            let ptr = round_mut_ptr_up_to_unchecked(ptr, MIN_ALIGN);
            debug_assert!(
                is_pointer_aligned_to(ptr, MIN_ALIGN),
                "bump pointer {ptr:#p} should be aligned to the minimum alignment of {MIN_ALIGN:#x}"
            );
            let ptr = NonNull::new_unchecked(ptr);
            self.current_chunk_footer.get().as_ref().ptr.set(ptr);
        }
    }

    #[inline]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<u8>, AllocErr> {
        // If the new layout demands greater alignment than the old layout has,
        // then either
        //
        // 1. the pointer happens to satisfy the new layout's alignment, so we
        //    got lucky and can return the pointer as-is, or
        //
        // 2. the pointer is not aligned to the new layout's demanded alignment,
        //    and we are unlucky.
        //
        // In the case of (2), to successfully "shrink" the allocation, we have
        // to allocate a whole new region for the new layout.
        if old_layout.align() < new_layout.align() {
            return if is_pointer_aligned_to(ptr.as_ptr(), new_layout.align()) {
                Ok(ptr)
            } else {
                let new_ptr = self.try_alloc_layout(new_layout)?;

                // We know that these regions are nonoverlapping because
                // `new_ptr` is a fresh allocation.
                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr(), new_layout.size());

                Ok(new_ptr)
            };
        }

        debug_assert!(is_pointer_aligned_to(ptr.as_ptr(), new_layout.align()));

        let old_size = old_layout.size();
        let new_size = new_layout.size();

        // This is how much space we would *actually* reclaim while satisfying
        // the requested alignment.
        let delta = round_down_to(old_size - new_size, new_layout.align().max(MIN_ALIGN));

        if self.is_last_allocation(ptr)
                // Only reclaim the excess space (which requires a copy) if it
                // is worth it: we are actually going to recover "enough" space
                // and we can do a non-overlapping copy.
                //
                // We do `(old_size + 1) / 2` so division rounds up rather than
                // down. Consider when:
                //
                //     old_size = 5
                //     new_size = 3
                //
                // If we do not take care to round up, this will result in:
                //
                //     delta = 2
                //     (old_size / 2) = (5 / 2) = 2
                //
                // And the the check will succeed even though we are have
                // overlapping ranges:
                //
                //     |--------old-allocation-------|
                //     |------from-------|
                //                 |-------to--------|
                //     +-----+-----+-----+-----+-----+
                //     |  a  |  b  |  c  |  .  |  .  |
                //     +-----+-----+-----+-----+-----+
                //
                // But we MUST NOT have overlapping ranges because we use
                // `copy_nonoverlapping` below! Therefore, we round the division
                // up to avoid this issue.
                && delta >= (old_size + 1) / 2
        {
            let footer = self.current_chunk_footer.get();
            let footer = footer.as_ref();

            // NB: new_ptr is aligned, because ptr *has to* be aligned, and we
            // made sure delta is aligned.
            let new_ptr = NonNull::new_unchecked(footer.ptr.get().as_ptr().add(delta));
            debug_assert!(
                is_pointer_aligned_to(new_ptr.as_ptr(), MIN_ALIGN),
                "bump pointer {new_ptr:#p} should be aligned to the minimum alignment of {MIN_ALIGN:#x}"
            );
            footer.ptr.set(new_ptr);

            // NB: we know it is non-overlapping because of the size check
            // in the `if` condition.
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr(), new_size);

            return Ok(new_ptr);
        }

        // If this wasn't the last allocation, or shrinking wasn't worth it,
        // simply return the old pointer as-is.
        Ok(ptr)
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<u8>, AllocErr> {
        let old_size = old_layout.size();

        let new_size = new_layout.size();
        let new_size = round_up_to(new_size, MIN_ALIGN).ok_or(AllocErr)?;

        let align_is_compatible = old_layout.align() >= new_layout.align();

        if align_is_compatible && self.is_last_allocation(ptr) {
            // Try to allocate the delta size within this same block so we can
            // reuse the currently allocated space.
            let delta = new_size - old_size;
            if let Some(p) =
                self.try_alloc_layout_fast(layout_from_size_align(delta, old_layout.align())?)
            {
                ptr::copy(ptr.as_ptr(), p.as_ptr(), old_size);
                return Ok(p);
            }
        }

        // Fallback: do a fresh allocation and copy the existing data into it.
        let new_ptr = self.try_alloc_layout(new_layout)?;
        ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr(), old_size);
        Ok(new_ptr)
    }
}

/// An iterator over each chunk of allocated memory that
/// an arena has bump allocated into.
///
/// The chunks are returned ordered by allocation time, with the most recently
/// allocated chunk being returned first.
///
/// The values inside each chunk are also ordered by allocation time, with the most
/// recent allocation being earlier in the slice.
///
/// This struct is created by the [`iter_allocated_chunks`] method on
/// [`Bump`]. See that function for a safety description regarding reading from the returned items.
///
/// [`Bump`]: struct.Bump.html
/// [`iter_allocated_chunks`]: struct.Bump.html#method.iter_allocated_chunks
#[derive(Debug)]
pub struct ChunkIter<'a, const MIN_ALIGN: usize = 1> {
    raw: ChunkRawIter<'a, MIN_ALIGN>,
    bump: PhantomData<&'a mut Bump>,
}

impl<'a, const MIN_ALIGN: usize> Iterator for ChunkIter<'a, MIN_ALIGN> {
    type Item = &'a [mem::MaybeUninit<u8>];

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let (ptr, len) = self.raw.next()?;
            let slice = slice::from_raw_parts(ptr as *const mem::MaybeUninit<u8>, len);
            Some(slice)
        }
    }
}

impl<'a, const MIN_ALIGN: usize> iter::FusedIterator for ChunkIter<'a, MIN_ALIGN> {}

/// An iterator over raw pointers to chunks of allocated memory that this
/// arena has bump allocated into.
///
/// See [`ChunkIter`] for details regarding the returned chunks.
///
/// This struct is created by the [`iter_allocated_chunks_raw`] method on
/// [`Bump`]. See that function for a safety description regarding reading from
/// the returned items.
///
/// [`Bump`]: struct.Bump.html
/// [`iter_allocated_chunks_raw`]: struct.Bump.html#method.iter_allocated_chunks_raw
#[derive(Debug)]
pub struct ChunkRawIter<'a, const MIN_ALIGN: usize = 1> {
    footer: NonNull<ChunkFooter>,
    bump: PhantomData<&'a Bump<MIN_ALIGN>>,
}

impl<const MIN_ALIGN: usize> Iterator for ChunkRawIter<'_, MIN_ALIGN> {
    type Item = (*mut u8, usize);
    fn next(&mut self) -> Option<(*mut u8, usize)> {
        unsafe {
            let foot = self.footer.as_ref();
            if foot.is_empty() {
                return None;
            }
            let (ptr, len) = foot.as_raw_parts();
            self.footer = foot.prev.get();
            Some((ptr as *mut u8, len))
        }
    }
}

impl<const MIN_ALIGN: usize> iter::FusedIterator for ChunkRawIter<'_, MIN_ALIGN> {}

#[inline(never)]
#[cold]
fn oom() -> ! {
    panic!("out of memory")
}

unsafe impl<'a, const MIN_ALIGN: usize> alloc::Alloc for &'a Bump<MIN_ALIGN> {
    #[inline(always)]
    unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {
        self.try_alloc_layout(layout)
    }

    #[inline]
    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        Bump::<MIN_ALIGN>::dealloc(self, ptr, layout);
    }

    #[inline]
    unsafe fn realloc(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<u8>, AllocErr> {
        let old_size = layout.size();

        if old_size == 0 {
            return self.try_alloc_layout(layout);
        }

        let new_layout = layout_from_size_align(new_size, layout.align())?;
        if new_size <= old_size {
            self.shrink(ptr, layout, new_layout)
        } else {
            self.grow(ptr, layout, new_layout)
        }
    }
}

#[cfg(any(feature = "allocator_api", feature = "allocator-api2"))]
unsafe impl<'a, const MIN_ALIGN: usize> Allocator for &'a Bump<MIN_ALIGN> {
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.try_alloc_layout(layout)
            .map(|p| unsafe {
                NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(p.as_ptr(), layout.size()))
            })
            .map_err(|_| AllocError)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        Bump::<MIN_ALIGN>::dealloc(self, ptr, layout)
    }

    #[inline]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        Bump::<MIN_ALIGN>::shrink(self, ptr, old_layout, new_layout)
            .map(|p| unsafe {
                NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(p.as_ptr(), new_layout.size()))
            })
            .map_err(|_| AllocError)
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        Bump::<MIN_ALIGN>::grow(self, ptr, old_layout, new_layout)
            .map(|p| unsafe {
                NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(p.as_ptr(), new_layout.size()))
            })
            .map_err(|_| AllocError)
    }

    #[inline]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let mut ptr = self.grow(ptr, old_layout, new_layout)?;
        ptr.as_mut()[old_layout.size()..].fill(0);
        Ok(ptr)
    }
}

// NB: Only tests which require private types, fields, or methods should be in
// here. Anything that can just be tested via public API surface should be in
// `bumpalo/tests/all/*`.
#[cfg(test)]
mod tests {
    use super::*;

    // Uses private type `ChunkFooter`.
    #[test]
    fn chunk_footer_is_five_words() {
        assert_eq!(mem::size_of::<ChunkFooter>(), mem::size_of::<usize>() * 6);
    }

    // Uses private `DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER` and `FOOTER_SIZE`.
    #[test]
    fn allocated_bytes() {
        let mut b = Bump::with_capacity(1);

        assert_eq!(b.allocated_bytes(), DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER);
        assert_eq!(
            b.allocated_bytes_including_metadata(),
            DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER + FOOTER_SIZE
        );

        b.reset();

        assert_eq!(b.allocated_bytes(), DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER);
        assert_eq!(
            b.allocated_bytes_including_metadata(),
            DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER + FOOTER_SIZE
        );
    }

    // Uses private `alloc` module.
    #[test]
    fn test_realloc() {
        use crate::alloc::Alloc;

        unsafe {
            const CAPACITY: usize = DEFAULT_CHUNK_SIZE_WITHOUT_FOOTER;
            let mut b = Bump::<1>::with_min_align_and_capacity(CAPACITY);

            // `realloc` doesn't shrink allocations that aren't "worth it".
            let layout = Layout::from_size_align(100, 1).unwrap();
            let p = b.alloc_layout(layout);
            let q = (&b).realloc(p, layout, 51).unwrap();
            assert_eq!(p, q);
            b.reset();

            // `realloc` will shrink allocations that are "worth it".
            let layout = Layout::from_size_align(100, 1).unwrap();
            let p = b.alloc_layout(layout);
            let q = (&b).realloc(p, layout, 50).unwrap();
            assert!(p != q);
            b.reset();

            // `realloc` will reuse the last allocation when growing.
            let layout = Layout::from_size_align(10, 1).unwrap();
            let p = b.alloc_layout(layout);
            let q = (&b).realloc(p, layout, 11).unwrap();
            assert_eq!(q.as_ptr() as usize, p.as_ptr() as usize - 1);
            b.reset();

            // `realloc` will allocate a new chunk when growing the last
            // allocation, if need be.
            let layout = Layout::from_size_align(1, 1).unwrap();
            let p = b.alloc_layout(layout);
            let q = (&b).realloc(p, layout, CAPACITY + 1).unwrap();
            assert_ne!(q.as_ptr() as usize, p.as_ptr() as usize - CAPACITY);
            b.reset();

            // `realloc` will allocate and copy when reallocating anything that
            // wasn't the last allocation.
            let layout = Layout::from_size_align(1, 1).unwrap();
            let p = b.alloc_layout(layout);
            let _ = b.alloc_layout(layout);
            let q = (&b).realloc(p, layout, 2).unwrap();
            assert!(q.as_ptr() as usize != p.as_ptr() as usize - 1);
            b.reset();
        }
    }

    // Uses our private `alloc` module.
    #[test]
    fn invalid_read() {
        use alloc::Alloc;

        let mut b = &Bump::new();

        unsafe {
            let l1 = Layout::from_size_align(12000, 4).unwrap();
            let p1 = Alloc::alloc(&mut b, l1).unwrap();

            let l2 = Layout::from_size_align(1000, 4).unwrap();
            Alloc::alloc(&mut b, l2).unwrap();

            let p1 = b.realloc(p1, l1, 24000).unwrap();
            let l3 = Layout::from_size_align(24000, 4).unwrap();
            b.realloc(p1, l3, 48000).unwrap();
        }
    }
}
