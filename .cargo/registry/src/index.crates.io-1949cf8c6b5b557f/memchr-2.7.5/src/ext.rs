/// A trait for adding some helper routines to pointers.
pub(crate) trait Pointer {
    /// Returns the distance, in units of `T`, between `self` and `origin`.
    ///
    /// # Safety
    ///
    /// Same as `ptr::offset_from` in addition to `self >= origin`.
    unsafe fn distance(self, origin: Self) -> usize;

    /// Casts this pointer to `usize`.
    ///
    /// Callers should not convert the `usize` back to a pointer if at all
    /// possible. (And if you believe it's necessary, open an issue to discuss
    /// why. Otherwise, it has the potential to violate pointer provenance.)
    /// The purpose of this function is just to be able to do arithmetic, i.e.,
    /// computing offsets or alignments.
    fn as_usize(self) -> usize;
}

impl<T> Pointer for *const T {
    unsafe fn distance(self, origin: *const T) -> usize {
        // TODO: Replace with `ptr::sub_ptr` once stabilized.
        usize::try_from(self.offset_from(origin)).unwrap_unchecked()
    }

    fn as_usize(self) -> usize {
        self as usize
    }
}

impl<T> Pointer for *mut T {
    unsafe fn distance(self, origin: *mut T) -> usize {
        (self as *const T).distance(origin as *const T)
    }

    fn as_usize(self) -> usize {
        (self as *const T).as_usize()
    }
}

/// A trait for adding some helper routines to raw bytes.
#[cfg(test)]
pub(crate) trait Byte {
    /// Converts this byte to a `char` if it's ASCII. Otherwise panics.
    fn to_char(self) -> char;
}

#[cfg(test)]
impl Byte for u8 {
    fn to_char(self) -> char {
        assert!(self.is_ascii());
        char::from(self)
    }
}
