use core::alloc::Layout;
use core::{
    cmp,
    mem,
    ptr,
};

use super::capacity::Capacity;
use super::{
    Repr,
    MAX_SIZE,
};
use crate::{
    ReserveError,
    UnwrapWithMsg,
};

/// The minimum size we'll allocate on the heap is one usize larger than our max inline size
const MIN_HEAP_SIZE: usize = MAX_SIZE + mem::size_of::<usize>();

const UNKNOWN: usize = 0;
pub type StrBuffer = [u8; UNKNOWN];

/// [`HeapBuffer`] grows at an amortized rates of 1.5x
///
/// Note: this is different than [`std::string::String`], which grows at a rate of 2x. It's debated
/// which is better, for now we'll stick with a rate of 1.5x
#[inline(always)]
pub fn amortized_growth(cur_len: usize, additional: usize) -> usize {
    let required = cur_len.saturating_add(additional);
    let amortized = cur_len.saturating_mul(3) / 2;
    amortized.max(required)
}

#[repr(C)]
pub struct HeapBuffer {
    pub ptr: ptr::NonNull<u8>,
    pub len: usize,
    pub cap: Capacity,
}
static_assertions::assert_eq_size!(HeapBuffer, Repr);
static_assertions::assert_eq_align!(HeapBuffer, Repr);

impl HeapBuffer {
    /// Create a [`HeapBuffer`] with the provided text
    #[inline]
    pub fn new(text: &str) -> Result<Self, ReserveError> {
        let len = text.len();
        let (cap, ptr) = allocate_ptr(len)?;

        // copy our string into the buffer we just allocated
        //
        // SAFETY: We know both `src` and `dest` are valid for respectively reads and writes of
        // length `len` because `len` comes from `src`, and `dest` was allocated to be at least that
        // length. We also know they're non-overlapping because `dest` is newly allocated
        unsafe { ptr.as_ptr().copy_from_nonoverlapping(text.as_ptr(), len) };

        Ok(HeapBuffer { ptr, len, cap })
    }

    /// Create an empty [`HeapBuffer`] with a specific capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> Result<Self, ReserveError> {
        let len = 0;
        let (cap, ptr) = allocate_ptr(capacity)?;

        Ok(HeapBuffer { ptr, len, cap })
    }

    /// Create a [`HeapBuffer`] with `text` that has _at least_ `additional` bytes of capacity
    ///
    /// To prevent frequent re-allocations, this method will create a [`HeapBuffer`] with a capacity
    /// of `text.len() + additional` or `text.len() * 1.5`, whichever is greater
    #[inline]
    pub fn with_additional(text: &str, additional: usize) -> Result<Self, ReserveError> {
        let len = text.len();
        let new_capacity = amortized_growth(len, additional);
        let (cap, ptr) = allocate_ptr(new_capacity)?;

        // copy our string into the buffer we just allocated
        //
        // SAFETY: We know both `src` and `dest` are valid for respectively reads and writes of
        // length `len` because `len` comes from `src`, and `dest` was allocated to be at least that
        // length. We also know they're non-overlapping because `dest` is newly allocated
        unsafe { ptr.as_ptr().copy_from_nonoverlapping(text.as_ptr(), len) };

        Ok(HeapBuffer { ptr, len, cap })
    }

    /// Return the capacity of the [`HeapBuffer`]
    #[inline]
    pub fn capacity(&self) -> usize {
        #[cold]
        fn read_capacity_from_heap(this: &HeapBuffer) -> usize {
            // re-adjust the pointer to include the capacity that's on the heap
            let adj_ptr: *const u8 = this.ptr.as_ptr().wrapping_sub(mem::size_of::<usize>());
            let mut buf = [0u8; mem::size_of::<usize>()];
            // SAFETY: `src` and `dst` don't overlap, and are valid for usize number of bytes
            unsafe {
                ptr::copy_nonoverlapping(adj_ptr, buf.as_mut_ptr(), mem::size_of::<usize>());
            }
            usize::from_ne_bytes(buf)
        }

        if self.cap.is_heap() {
            read_capacity_from_heap(self)
        } else {
            // SAFETY: Checked above that the capacity is on the stack
            unsafe { self.cap.as_usize() }
        }
    }

    /// Try to grow the [`HeapBuffer`] by reallocating, returning an error if we fail
    pub fn realloc(&mut self, new_capacity: usize) -> Result<usize, ()> {
        let new_cap = Capacity::new(new_capacity);

        // We can't reallocate to a size less than our length, or else we'd clip the string
        if new_capacity < self.len {
            return Err(());
        }

        // HeapBuffer doesn't support 0 byte heap sizes
        if new_capacity == 0 {
            return Err(());
        }

        // Always allocate at least MIN_HEAP_SIZE
        let new_capacity = cmp::max(new_capacity, MIN_HEAP_SIZE);

        let (new_cap, new_ptr) = match (self.cap.is_heap(), new_cap.is_heap()) {
            // both current and new capacity can be stored inline
            (false, false) => {
                // SAFETY: checked above that our capacity is valid
                let cap = unsafe { self.cap.as_usize() };

                // current capacity is the same as the new, nothing to do!
                if cap == new_capacity {
                    return Ok(new_capacity);
                }

                let cur_layout = inline_capacity::layout(cap);
                let new_layout = inline_capacity::layout(new_capacity);
                let new_size = new_layout.size();

                // It's possible `new_size` could overflow since inline_capacity::layout pads for
                // alignment
                if new_size < new_capacity {
                    return Err(());
                }

                // SAFETY:
                // * We're using the same allocator that we used for `ptr`
                // * The layout is the same because we checked that the capacity is inline
                // * `new_size` will be > 0, we return early if the requested capacity is 0
                // * Checked above if `new_size` overflowed when rounding to alignment
                match ptr::NonNull::new(unsafe {
                    ::alloc::alloc::realloc(self.ptr.as_ptr(), cur_layout, new_size)
                }) {
                    Some(ptr) => (new_cap, ptr),
                    None => return Err(()),
                }
            }
            // both current and new capacity need to be stored on the heap
            (true, true) => {
                let cur_layout = heap_capacity::layout(self.capacity());
                let new_layout = heap_capacity::layout(new_capacity);
                let new_size = new_layout.size();

                // alloc::realloc requires that size > 0
                debug_assert!(new_size > 0);

                // It's possible `new_size` could overflow since heap_capacity::layout requires a
                // few additional bytes
                if new_size < new_capacity {
                    return Err(());
                }

                // move our pointer back one WORD since our capacity is behind it
                let raw_ptr = self.ptr.as_ptr();
                let adj_ptr = raw_ptr.wrapping_sub(mem::size_of::<usize>());

                // SAFETY:
                // * We're using the same allocator that we used for `ptr`
                // * The layout is the same because we checked that the capacity is on the heap
                // * `new_size` will be > 0, we return early if the requested capacity is 0
                // * Checked above if `new_size` overflowed when rounding to alignment
                let cap_ptr = unsafe { alloc::alloc::realloc(adj_ptr, cur_layout, new_size) };
                // Check if reallocation succeeded
                if cap_ptr.is_null() {
                    return Err(());
                }

                // Our allocation succeeded! Write the new capacity
                //
                // SAFETY:
                // * `src` and `dst` are both valid for reads of `usize` number of bytes
                // * `src` and `dst` don't overlap because we created `src`
                unsafe {
                    ptr::copy_nonoverlapping(
                        new_capacity.to_ne_bytes().as_ptr(),
                        cap_ptr,
                        mem::size_of::<usize>(),
                    )
                };

                // Finally, adjust our pointer backup so it points at the string content
                let str_ptr = cap_ptr.wrapping_add(mem::size_of::<usize>());
                // SAFETY: We checked above to make sure the pointer was non-null
                let ptr = unsafe { ptr::NonNull::new_unchecked(str_ptr) };

                (new_cap, ptr)
            }
            // capacity is currently inline or on the heap, but needs to move, can't realloc because
            // we'd need to change the layout!
            (false, true) | (true, false) => return Err(()),
        };

        // set our new pointer and new capacity
        self.ptr = new_ptr;
        self.cap = new_cap;

        Ok(new_capacity)
    }

    /// Set's the length of the content for this [`HeapBuffer`]
    ///
    /// # SAFETY:
    /// * The caller must guarantee that `len` bytes in the buffer are valid UTF-8
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    /// Deallocates the memory owned by the provided [`HeapBuffer`]
    #[inline]
    pub fn dealloc(&mut self) {
        deallocate_ptr(self.ptr, self.cap);
    }
}

impl Clone for HeapBuffer {
    fn clone(&self) -> Self {
        // Create a new HeapBuffer with the same capacity as the original
        let mut new = Self::with_capacity(self.capacity()).unwrap_with_msg();

        // SAFETY:
        // * `src` and `dst` don't overlap because we just created `dst`
        // * `src` and `dst` are both valid for `self.len` bytes because self.len < capacity
        unsafe {
            new.ptr
                .as_ptr()
                .copy_from_nonoverlapping(self.ptr.as_ptr(), self.len)
        };
        // SAFETY:
        // * We copied the text from self, which is valid UTF-8
        unsafe { new.set_len(self.len) };

        new
    }
}

impl Drop for HeapBuffer {
    fn drop(&mut self) {
        self.dealloc()
    }
}

/// Allocates a buffer on the heap that we can use to store a string, optionally stores the capacity
/// of said buffer on the heap.
///
/// Returns a [`Capacity`] that either indicates the capacity is stored on the heap, or is stored
/// in the `Capacity` itself.
#[inline]
pub fn allocate_ptr(capacity: usize) -> Result<(Capacity, ptr::NonNull<u8>), ReserveError> {
    // We allocate at least MIN_HEAP_SIZE bytes because we need to allocate at least one byte
    let capacity = capacity.max(MIN_HEAP_SIZE);
    let cap = Capacity::new(capacity);

    // HeapBuffer doesn't support 0 sized allocations, we should always allocate at least
    // MIN_HEAP_SIZE bytes
    debug_assert!(capacity > 0);

    #[cold]
    fn allocate_with_capacity_on_heap(capacity: usize) -> Result<ptr::NonNull<u8>, ReserveError> {
        // write our capacity onto the heap
        // SAFETY: we know that the capacity is not zero
        let ptr = unsafe { heap_capacity::alloc(capacity)? };
        // SAFETY:
        // * `src` and `dst` don't overlap and are both valid for `usize` bytes
        unsafe {
            ptr::copy_nonoverlapping(
                capacity.to_ne_bytes().as_ptr(),
                ptr.as_ptr(),
                mem::size_of::<usize>(),
            )
        };
        let raw_ptr = ptr.as_ptr().wrapping_add(core::mem::size_of::<usize>());
        // SAFETY: We know `raw_ptr` is non-null because we just created it
        Ok(unsafe { ptr::NonNull::new_unchecked(raw_ptr) })
    }

    let ptr = if cap.is_heap() {
        allocate_with_capacity_on_heap(capacity)
    } else {
        unsafe { inline_capacity::alloc(capacity) }
    };

    Ok((cap, ptr?))
}

/// Deallocates a buffer on the heap, handling when the capacity is also stored on the heap
#[inline]
pub fn deallocate_ptr(ptr: ptr::NonNull<u8>, cap: Capacity) {
    #[cold]
    fn deallocate_with_capacity_on_heap(ptr: ptr::NonNull<u8>) {
        // re-adjust the pointer to include the capacity that's on the heap
        let adj_ptr = ptr.as_ptr().wrapping_sub(mem::size_of::<usize>());
        // read the capacity from the heap so we know how much to deallocate
        let mut buf = [0u8; mem::size_of::<usize>()];
        // SAFETY: `src` and `dst` don't overlap, and are valid for usize number of bytes
        unsafe {
            ptr::copy_nonoverlapping(adj_ptr, buf.as_mut_ptr(), mem::size_of::<usize>());
        }
        let capacity = usize::from_ne_bytes(buf);
        // SAFETY: We know the pointer is not null since we got it as a NonNull
        let ptr = unsafe { ptr::NonNull::new_unchecked(adj_ptr) };
        // SAFETY: We checked above that our capacity is on the heap, and we readjusted the
        // pointer to reference the capacity
        unsafe { heap_capacity::dealloc(ptr, capacity) }
    }

    if cap.is_heap() {
        deallocate_with_capacity_on_heap(ptr);
    } else {
        // SAFETY: Our capacity is always inline on 64-bit archs
        unsafe { inline_capacity::dealloc(ptr, cap.as_usize()) }
    }
}

/// SAFETY: `layout` must not be zero sized
#[inline]
pub unsafe fn do_alloc(layout: Layout) -> Result<ptr::NonNull<u8>, ReserveError> {
    debug_assert!(layout.size() > 0);

    // SAFETY: `alloc(...)` has undefined behavior if the layout is zero-sized. We specify that
    // `capacity` must be > 0 as a constraint to uphold the safety of this method. If capacity
    // is greater than 0, then our layout will be non-zero-sized.
    let raw_ptr = ::alloc::alloc::alloc(layout);

    // Check to make sure our pointer is non-null.
    // Implementations are encouraged to return null on memory exhaustion rather than aborting.
    ptr::NonNull::new(raw_ptr).ok_or(ReserveError(()))
}

mod heap_capacity {
    use core::{
        alloc,
        ptr,
    };

    use super::{
        do_alloc,
        StrBuffer,
    };
    use crate::ReserveError;

    /// SAFETY: `capacity` must not be zero
    pub unsafe fn alloc(capacity: usize) -> Result<ptr::NonNull<u8>, ReserveError> {
        do_alloc(layout(capacity))
    }

    /// Deallocates a pointer which references a `HeapBuffer` whose capacity is on the heap
    ///
    /// # Safety
    /// * `ptr` must point to the start of a `HeapBuffer` whose capacity is on the heap. i.e. we
    ///   must have `ptr -> [cap<usize> ; string<bytes>]`
    pub unsafe fn dealloc(ptr: ptr::NonNull<u8>, capacity: usize) {
        let layout = layout(capacity);
        ::alloc::alloc::dealloc(ptr.as_ptr(), layout);
    }

    #[repr(C)]
    struct HeapBufferInnerHeapCapacity {
        capacity: usize,
        buffer: StrBuffer,
    }

    #[inline(always)]
    pub fn layout(capacity: usize) -> alloc::Layout {
        let buffer_layout = alloc::Layout::array::<u8>(capacity).expect("valid capacity");
        alloc::Layout::new::<HeapBufferInnerHeapCapacity>()
            .extend(buffer_layout)
            .expect("valid layout")
            .0
            .pad_to_align()
    }
}

mod inline_capacity {
    use core::{
        alloc,
        ptr,
    };

    use super::{
        do_alloc,
        StrBuffer,
    };
    use crate::ReserveError;

    /// # SAFETY:
    /// * `capacity` must be > 0
    pub unsafe fn alloc(capacity: usize) -> Result<ptr::NonNull<u8>, ReserveError> {
        do_alloc(layout(capacity))
    }

    /// Deallocates a pointer which references a `HeapBuffer` whose capacity is stored inline
    ///
    /// # Safety
    /// * `ptr` must point to the start of a `HeapBuffer` whose capacity is on the inline
    pub unsafe fn dealloc(ptr: ptr::NonNull<u8>, capacity: usize) {
        let layout = layout(capacity);
        ::alloc::alloc::dealloc(ptr.as_ptr(), layout);
    }

    #[repr(C)]
    struct HeapBufferInnerInlineCapacity {
        buffer: StrBuffer,
    }

    #[inline(always)]
    pub fn layout(capacity: usize) -> alloc::Layout {
        let buffer_layout = alloc::Layout::array::<u8>(capacity).expect("valid capacity");
        alloc::Layout::new::<HeapBufferInnerInlineCapacity>()
            .extend(buffer_layout)
            .expect("valid layout")
            .0
            .pad_to_align()
    }
}

#[cfg(test)]
mod test {
    use test_case::test_case;

    use super::{
        HeapBuffer,
        MIN_HEAP_SIZE,
    };

    const EIGHTEEN_MB: usize = 18 * 1024 * 1024;

    #[test]
    fn test_min_capacity() {
        let h = HeapBuffer::new("short").unwrap();
        assert_eq!(h.capacity(), MIN_HEAP_SIZE);
    }

    #[test_case(&[42; 8]; "short")]
    #[test_case(&[42; 50]; "long")]
    #[test_case(&[42; EIGHTEEN_MB]; "huge")]
    fn test_capacity(buf: &[u8]) {
        // we know the buffer is valid UTF-8
        let s = unsafe { core::str::from_utf8_unchecked(buf) };
        let h = HeapBuffer::new(s).unwrap();

        assert_eq!(h.capacity(), core::cmp::max(s.len(), MIN_HEAP_SIZE));
    }

    #[test_case(&[42; 0], 0, Err(MIN_HEAP_SIZE); "empty_empty")]
    #[test_case(&[42; 64], 0, Err(64); "short_empty")]
    #[test_case(&[42; 64], 32, Err(64); "short_to_shorter")]
    #[test_case(&[42; 64], 128, Ok(128); "short_to_longer")]
    #[test_case(&[42; EIGHTEEN_MB], EIGHTEEN_MB + 128, Ok(EIGHTEEN_MB + 128); "heap_to_heap")]
    fn test_realloc(buf: &[u8], realloc: usize, result: Result<usize, usize>) {
        // we know the buffer is valid UTF-8
        let s = unsafe { core::str::from_utf8_unchecked(buf) };
        let mut h = HeapBuffer::new(s).unwrap();

        // reallocate, asserting our result
        let expected_cap = match result {
            Ok(c) | Err(c) => c,
        };
        let expected_res = result.map_err(|_| ());
        assert_eq!(h.realloc(realloc), expected_res);
        assert_eq!(h.capacity(), expected_cap);
    }

    #[test]
    fn test_realloc_inline_to_heap() {
        // we know the buffer is valid UTF-8
        let s = unsafe { core::str::from_utf8_unchecked(&[42; 128]) };
        let mut h = HeapBuffer::new(s).unwrap();

        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "64")] {
                let expected_result = Ok(EIGHTEEN_MB);
                let expected_capacity = EIGHTEEN_MB;
            } else if #[cfg(target_pointer_width = "32")] {
                // on 32-bit architectures we'd need to change the layout from capacity being inline
                // to the capacity being on the heap, which isn't possible
                let expected_result = Err(());
                let expected_capacity = 128;
            } else {
                compile_error!("Unsupported pointer width!");
            }
        }
        assert_eq!(h.realloc(EIGHTEEN_MB), expected_result);
        assert_eq!(h.capacity(), expected_capacity);
    }

    #[test_case(&[42; 64], 128, 100, Ok(100); "sanity")]
    fn test_realloc_shrink(
        buf: &[u8],
        realloc_one: usize,
        realloc_two: usize,
        exp_result: Result<usize, usize>,
    ) {
        // we know the buffer is valid UTF-8
        let s = unsafe { core::str::from_utf8_unchecked(buf) };
        let mut h = HeapBuffer::new(s).unwrap();

        assert!(
            realloc_one > realloc_two,
            "we have to grow before we can shrink"
        );

        // grow our allocation
        assert_eq!(h.realloc(realloc_one), Ok(realloc_one));

        // shrink our allocation, asserting our result
        let expected_cap = match exp_result {
            Ok(c) | Err(c) => c,
        };
        let expected_res = exp_result.map_err(|_| ());
        assert_eq!(h.realloc(realloc_two), expected_res);
        assert_eq!(h.capacity(), expected_cap);
    }

    #[test]
    fn test_realloc_shrink_heap_to_inline() {
        // TODO: test this case
        assert_eq!(1, 1);
    }

    #[test_case(&[42; 0]; "empty")]
    #[test_case(&[42; 3]; "short")]
    #[test_case(&[42; 64]; "long")]
    #[test_case(&[42; EIGHTEEN_MB]; "huge")]
    fn test_clone(buf: &[u8]) {
        let s = unsafe { core::str::from_utf8_unchecked(buf) };
        let h_a = HeapBuffer::new(s).unwrap();
        let h_b = h_a.clone();

        assert_eq!(h_a.capacity(), h_b.capacity());
    }
}
