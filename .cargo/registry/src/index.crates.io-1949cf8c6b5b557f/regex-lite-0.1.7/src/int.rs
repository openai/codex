use core::num::NonZeroUsize;

/// An extension trait that adds routines to the `u32` primitive type.
pub(crate) trait U32 {
    fn as_usize(self) -> usize;
}

impl U32 for u32 {
    fn as_usize(self) -> usize {
        // OK because we require 32 or 64 bit targets. Therefore, every u32
        // necessarily fits into a usize.
        self as usize
    }
}

/// A `usize` that can never be `usize::MAX`.
///
/// This is similar to `core::num::NonZeroUsize`, but instead of not permitting
/// a zero value, this does not permit a max value.
///
/// This is useful in certain contexts where one wants to optimize the memory
/// usage of things that contain match offsets. Namely, since Rust slices
/// are guaranteed to never have a length exceeding `isize::MAX`, we can use
/// `usize::MAX` as a sentinel to indicate that no match was found. Indeed,
/// types like `Option<NonMaxUsize>` have exactly the same size in memory as a
/// `usize`.
///
/// This type is defined to be `repr(transparent)` for
/// `core::num::NonZeroUsize`, which is in turn defined to be
/// `repr(transparent)` for `usize`.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub(crate) struct NonMaxUsize(NonZeroUsize);

impl NonMaxUsize {
    /// Create a new `NonMaxUsize` from the given value.
    ///
    /// This returns `None` only when the given value is equal to `usize::MAX`.
    pub(crate) fn new(value: usize) -> Option<NonMaxUsize> {
        NonZeroUsize::new(value.wrapping_add(1)).map(NonMaxUsize)
    }

    /// Return the underlying `usize` value. The returned value is guaranteed
    /// to not equal `usize::MAX`.
    pub(crate) fn get(self) -> usize {
        self.0.get().wrapping_sub(1)
    }
}

// We provide our own Debug impl because seeing the internal repr can be quite
// surprising if you aren't expecting it. e.g., 'NonMaxUsize(5)' vs just '5'.
impl core::fmt::Debug for NonMaxUsize {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self.get())
    }
}
