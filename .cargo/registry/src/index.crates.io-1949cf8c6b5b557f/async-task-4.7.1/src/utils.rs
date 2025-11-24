use core::alloc::Layout as StdLayout;
use core::mem;

/// Aborts the process.
///
/// To abort, this function simply panics while panicking.
pub(crate) fn abort() -> ! {
    struct Panic;

    impl Drop for Panic {
        fn drop(&mut self) {
            panic!("aborting the process");
        }
    }

    let _panic = Panic;
    panic!("aborting the process");
}

/// Calls a function and aborts if it panics.
///
/// This is useful in unsafe code where we can't recover from panics.
#[inline]
pub(crate) fn abort_on_panic<T>(f: impl FnOnce() -> T) -> T {
    struct Bomb;

    impl Drop for Bomb {
        fn drop(&mut self) {
            abort();
        }
    }

    let bomb = Bomb;
    let t = f();
    mem::forget(bomb);
    t
}

/// A version of `alloc::alloc::Layout` that can be used in the const
/// position.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    size: usize,
    align: usize,
}

impl Layout {
    /// Creates a new `Layout` with the given size and alignment.
    #[inline]
    pub(crate) const fn from_size_align(size: usize, align: usize) -> Self {
        Self { size, align }
    }

    /// Creates a new `Layout` for the given sized type.
    #[inline]
    pub(crate) const fn new<T>() -> Self {
        Self::from_size_align(mem::size_of::<T>(), mem::align_of::<T>())
    }

    /// Convert this into the standard library's layout type.
    ///
    /// # Safety
    ///
    /// - `align` must be non-zero and a power of two.
    /// - When rounded up to the nearest multiple of `align`, the size
    ///   must not overflow.
    #[inline]
    pub(crate) const unsafe fn into_std(self) -> StdLayout {
        StdLayout::from_size_align_unchecked(self.size, self.align)
    }

    /// Get the alignment of this layout.
    #[inline]
    pub(crate) const fn align(&self) -> usize {
        self.align
    }

    /// Get the size of this layout.
    #[inline]
    pub(crate) const fn size(&self) -> usize {
        self.size
    }

    /// Returns the layout for `a` followed by `b` and the offset of `b`.
    ///
    /// This function was adapted from the `Layout::extend()`:
    /// https://doc.rust-lang.org/nightly/std/alloc/struct.Layout.html#method.extend
    #[inline]
    pub(crate) const fn extend(self, other: Layout) -> Option<(Layout, usize)> {
        let new_align = max(self.align(), other.align());
        let pad = self.padding_needed_for(other.align());

        let offset = leap!(self.size().checked_add(pad));
        let new_size = leap!(offset.checked_add(other.size()));

        // return None if any of the following are true:
        // - align is 0 (implied false by is_power_of_two())
        // - align is not a power of 2
        // - size rounded up to align overflows
        if !new_align.is_power_of_two() || new_size > isize::MAX as usize - (new_align - 1) {
            return None;
        }

        let layout = Layout::from_size_align(new_size, new_align);
        Some((layout, offset))
    }

    /// Returns the padding after `layout` that aligns the following address to `align`.
    ///
    /// This function was adapted from the `Layout::padding_needed_for()`:
    /// https://doc.rust-lang.org/nightly/std/alloc/struct.Layout.html#method.padding_needed_for
    #[inline]
    pub(crate) const fn padding_needed_for(self, align: usize) -> usize {
        let len = self.size();
        let len_rounded_up = len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
        len_rounded_up.wrapping_sub(len)
    }
}

#[inline]
pub(crate) const fn max(left: usize, right: usize) -> usize {
    if left > right {
        left
    } else {
        right
    }
}
