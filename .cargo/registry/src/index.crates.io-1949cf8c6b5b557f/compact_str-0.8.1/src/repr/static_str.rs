use core::{
    mem,
    ptr,
    slice,
    str,
};

use super::{
    Repr,
    MAX_SIZE,
    STATIC_STR_MASK,
};

pub(super) const DISCRIMINANT_SIZE: usize = MAX_SIZE - mem::size_of::<&'static str>();

/// A buffer stored on the stack whose size is equal to the stack size of `String`
/// The last byte is set to 0.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct StaticStr {
    ptr: ptr::NonNull<u8>,
    len: usize,
    #[allow(unused)]
    discriminant: [u8; DISCRIMINANT_SIZE],
}
static_assertions::assert_eq_size!(StaticStr, Repr);
static_assertions::assert_eq_align!(StaticStr, Repr);
static_assertions::assert_eq_size!(&'static str, (*const u8, usize));

impl StaticStr {
    #[inline]
    pub const fn new(text: &'static str) -> Self {
        let mut discriminant = [0; DISCRIMINANT_SIZE];
        discriminant[DISCRIMINANT_SIZE - 1] = STATIC_STR_MASK;

        Self {
            // SAFETY: `&'static str` must have a non-null, properly aligned
            // address
            ptr: unsafe { ptr::NonNull::new_unchecked(text.as_ptr() as *mut _) },
            len: text.len(),
            discriminant,
        }
    }

    #[rustversion::attr(since(1.64), const)]
    pub(super) fn get_text(&self) -> &'static str {
        // SAFETY: `StaticStr` invariants requires it to be a valid str
        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(self.ptr.as_ptr(), self.len)) }
    }

    /// # Safety
    /// * `len` bytes in the buffer must be valid UTF-8 and
    /// * `len` must be <= `self.get_text().len()`
    pub(super) unsafe fn set_len(&mut self, len: usize) {
        *self = Self::new(&self.get_text()[..len]);
    }
}
