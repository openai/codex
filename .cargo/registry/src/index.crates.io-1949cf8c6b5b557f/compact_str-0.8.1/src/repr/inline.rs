use core::ptr;

use super::{
    Repr,
    LENGTH_MASK,
    MAX_SIZE,
};

/// A buffer stored on the stack whose size is equal to the stack size of `String`
#[cfg(target_pointer_width = "64")]
#[repr(C, align(8))]
pub struct InlineBuffer(pub [u8; MAX_SIZE]);

#[cfg(target_pointer_width = "32")]
#[repr(C, align(4))]
pub struct InlineBuffer(pub [u8; MAX_SIZE]);

static_assertions::assert_eq_size!(InlineBuffer, Repr);
static_assertions::assert_eq_align!(InlineBuffer, Repr);

impl InlineBuffer {
    /// Construct a new [`InlineString`]. A string that lives in a small buffer on the stack
    ///
    /// SAFETY:
    /// * The caller must guarantee that the length of `text` is less than [`MAX_SIZE`]
    #[inline]
    pub unsafe fn new(text: &str) -> Self {
        debug_assert!(text.len() <= MAX_SIZE);

        let len = text.len();
        let mut buffer = InlineBuffer([0u8; MAX_SIZE]);

        // set the length in the last byte
        buffer.0[MAX_SIZE - 1] = len as u8 | LENGTH_MASK;

        // copy the string into our buffer
        //
        // note: in the case where len == MAX_SIZE, we'll overwrite the len, but that's okay because
        // when reading the length we can detect that the last byte is part of UTF-8 and return a
        // length of MAX_SIZE
        //
        // SAFETY:
        // * src (`text`) is valid for `len` bytes because `len` comes from `text`
        // * dst (`buffer`) is valid for `len` bytes because we assert src is less than MAX_SIZE
        // * src and dst don't overlap because we created dst
        //
        ptr::copy_nonoverlapping(text.as_ptr(), buffer.0.as_mut_ptr(), len);

        buffer
    }

    #[inline]
    pub const fn new_const(text: &str) -> Self {
        if text.len() > MAX_SIZE {
            panic!("Provided string has a length greater than our MAX_SIZE");
        }

        let len = text.len();
        let mut buffer = [0u8; MAX_SIZE];

        // set the length
        buffer[MAX_SIZE - 1] = len as u8 | LENGTH_MASK;

        // Note: for loops aren't allowed in `const fn`, hence the while.
        // Note: Iterating forward results in badly optimized code, because the compiler tries to
        //       unroll the loop.
        let text = text.as_bytes();
        let mut i = len;
        while i > 0 {
            buffer[i - 1] = text[i - 1];
            i -= 1;
        }

        InlineBuffer(buffer)
    }

    /// Returns an empty [`InlineBuffer`]
    #[inline(always)]
    pub const fn empty() -> Self {
        Self::new_const("")
    }

    /// Consumes the [`InlineBuffer`] returning the entire underlying array and the length of the
    /// string that it contains
    #[inline]
    #[cfg(feature = "smallvec")]
    pub fn into_array(self) -> ([u8; MAX_SIZE], usize) {
        let mut buffer = self.0;

        let length = core::cmp::min(
            (buffer[MAX_SIZE - 1].wrapping_sub(LENGTH_MASK)) as usize,
            MAX_SIZE,
        );

        let last_byte_ref = &mut buffer[MAX_SIZE - 1];

        // unset the last byte of the buffer if it's just storing the length of the string
        //
        // Note: we should never add an `else` statement here, keeping the conditional simple allows
        // the compiler to optimize this to a conditional-move instead of a branch
        if length < MAX_SIZE {
            *last_byte_ref = 0;
        }

        (buffer, length)
    }

    /// Set's the length of the content for this [`InlineBuffer`]
    ///
    /// # SAFETY:
    /// * The caller must guarantee that `len` bytes in the buffer are valid UTF-8
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= MAX_SIZE);

        // If `length` == MAX_SIZE, then we infer the length to be the capacity of the buffer. We
        // can infer this because the way we encode length doesn't overlap with any valid UTF-8
        // bytes
        if len < MAX_SIZE {
            self.0[MAX_SIZE - 1] = len as u8 | LENGTH_MASK;
        }
    }
}

#[cfg(test)]
mod tests {
    #[rustversion::since(1.63)]
    #[test]
    #[ignore] // we run this in CI, but unless you're compiling in release, this takes a while
    fn test_unused_utf8_bytes() {
        use rayon::prelude::*;

        // test to validate for all char the first and last bytes are never within a specified range
        // note: according to the UTF-8 spec it shouldn't be, but we double check that here
        (0..u32::MAX).into_par_iter().for_each(|i| {
            if let Ok(c) = char::try_from(i) {
                let mut buf = [0_u8; 4];
                c.encode_utf8(&mut buf);

                // check ranges for first byte
                match buf[0] {
                    x @ 128..=191 => panic!("first byte within 128..=191, {}", x),
                    x @ 248..=255 => panic!("first byte within 248..=255, {}", x),
                    _ => (),
                }

                // check ranges for last byte
                if let x @ 192..=255 = buf[c.len_utf8() - 1] {
                    panic!("last byte within 192..=255, {}", x)
                }
            }
        })
    }

    #[cfg(feature = "smallvec")]
    mod smallvec {
        use alloc::string::String;

        use quickcheck_macros::quickcheck;

        use crate::repr::{
            InlineBuffer,
            MAX_SIZE,
        };

        #[test]
        fn test_into_array() {
            let s = "hello world!";

            let inline = unsafe { InlineBuffer::new(s) };
            let (array, length) = inline.into_array();

            assert_eq!(s.len(), length);

            // all bytes after the length should be 0
            assert!(array[length..].iter().all(|b| *b == 0));

            // taking a string slice should give back the same string as the original
            let ex_s = unsafe { core::str::from_utf8_unchecked(&array[..length]) };
            assert_eq!(s, ex_s);
        }

        #[quickcheck]
        #[cfg_attr(miri, ignore)]
        fn quickcheck_into_array(s: String) {
            let mut total_length = 0;
            let s: String = s
                .chars()
                .take_while(|c| {
                    total_length += c.len_utf8();
                    total_length < MAX_SIZE
                })
                .collect();

            let inline = unsafe { InlineBuffer::new(&s) };
            let (array, length) = inline.into_array();
            assert_eq!(s.len(), length);

            // all bytes after the length should be 0
            assert!(array[length..].iter().all(|b| *b == 0));

            // taking a string slice should give back the same string as the original
            let ex_s = unsafe { core::str::from_utf8_unchecked(&array[..length]) };
            assert_eq!(s, ex_s);
        }
    }
}
