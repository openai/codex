use core::str::Utf8Error;

use bytes::Buf;

use super::{
    Repr,
    MAX_SIZE,
};
use crate::UnwrapWithMsg;

impl Repr {
    /// Converts a [`Buf`] of bytes to a [`Repr`], checking that the provided bytes are valid UTF-8
    pub fn from_utf8_buf<B: Buf>(buf: &mut B) -> Result<Self, Utf8Error> {
        // SAFETY: We check below to make sure the provided buffer is valid UTF-8
        let (repr, bytes_written) = unsafe { Self::collect_buf(buf) };

        // Check to make sure the provided bytes are valid UTF-8, return the Repr if they are!
        match core::str::from_utf8(&repr.as_slice()[..bytes_written]) {
            Ok(_) => Ok(repr),
            Err(e) => Err(e),
        }
    }

    /// Converts a [`Buf`] of bytes to a [`Repr`], without checking for valid UTF-8
    ///
    /// # Safety
    /// * The provided buffer must be valid UTF-8
    pub unsafe fn from_utf8_buf_unchecked<B: Buf>(buf: &mut B) -> Self {
        let (repr, _bytes_written) = Self::collect_buf(buf);
        repr
    }

    /// Collects the bytes from a [`Buf`] into a [`Repr`]
    ///
    /// # Safety
    /// * The caller must guarantee that `buf` is valid UTF-8
    unsafe fn collect_buf<B: Buf>(buf: &mut B) -> (Self, usize) {
        // Get an empty Repr we can write into
        let mut repr = super::EMPTY;
        let mut bytes_written = 0;

        debug_assert_eq!(repr.len(), bytes_written);

        while buf.has_remaining() {
            let chunk = buf.chunk();
            let chunk_len = chunk.len();

            // There's an edge case where the final byte of this buffer == `HEAP_MASK`, which is
            // invalid UTF-8, but would result in us creating an inline variant, that identifies as
            // a heap variant. If a user ever tried to reference the data at all, we'd incorrectly
            // try and read data from an invalid memory address, causing undefined behavior.
            if bytes_written < MAX_SIZE && bytes_written + chunk_len == MAX_SIZE {
                let last_byte = chunk[chunk_len - 1];
                // If we hit the edge case, reserve additional space to make the repr becomes heap
                // allocated, which prevents us from writing this last byte inline
                if last_byte >= 0b11000000 {
                    repr.reserve(MAX_SIZE + 1).unwrap_with_msg();
                }
            }

            // reserve at least enough space to fit this chunk
            repr.reserve(chunk_len).unwrap_with_msg();

            // SAFETY: The caller is responsible for making sure the provided buffer is UTF-8. This
            // invariant is documented in the public API
            let slice = repr.as_mut_buf();
            // write the chunk into the Repr
            slice[bytes_written..bytes_written + chunk_len].copy_from_slice(chunk);

            // Set the length of the Repr
            // SAFETY: We just wrote an additional `chunk_len` bytes into the Repr
            bytes_written += chunk_len;
            repr.set_len(bytes_written);

            // advance the pointer of the buffer
            buf.advance(chunk_len);
        }

        (repr, bytes_written)
    }
}

#[cfg(test)]
mod test {
    #[cfg(feature = "std")]
    use std::io::Cursor;

    use test_case::test_case;

    use super::Repr;

    #[test_case(""; "empty")]
    #[test_case("hello world"; "short")]
    #[test_case("hello, this is a long string which should be heap allocated"; "long")]
    fn test_from_utf8_buf(word: &'static str) {
        let mut buf = Cursor::new(word.as_bytes());
        let repr = Repr::from_utf8_buf(&mut buf).unwrap();

        assert_eq!(repr.as_str(), word);
        assert_eq!(repr.len(), word.len());
    }

    #[test]
    fn test_from_utf8_packed() {
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "64")] {
                let packed = "this string is 24 chars!";
            } else if #[cfg(target_pointer_width = "32")] {
                let packed = "i am 12 char";
            } else {
                compile_error!("unsupported architecture!")
            }
        }
        let mut buf = Cursor::new(packed.as_bytes());

        let repr = Repr::from_utf8_buf(&mut buf).unwrap();
        assert_eq!(repr.as_str(), packed);

        // This repr should __not__ be heap allocated
        assert!(!repr.is_heap_allocated());
    }

    #[test]
    fn test_fuzz_panic() {
        let bytes = &[
            255, 255, 255, 255, 255, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 1, 12, 0, 0, 96,
        ];
        let mut buf: Cursor<&[u8]> = Cursor::new(bytes);

        assert!(Repr::from_utf8_buf(&mut buf).is_err());
    }

    #[test]
    fn test_valid_repr_but_invalid_utf8() {
        let bytes = &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192,
        ];
        let mut buf: Cursor<&[u8]> = Cursor::new(bytes);

        assert!(Repr::from_utf8_buf(&mut buf).is_err());
    }

    #[test]
    fn test_fake_heap_variant() {
        let bytes = &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255,
        ];
        let mut buf: Cursor<&[u8]> = Cursor::new(bytes);

        assert!(Repr::from_utf8_buf(&mut buf).is_err());
    }

    #[test]
    fn test_from_non_contiguous() {
        let data = [
            211, 247, 211, 247, 121, 135, 151, 255, 126, 205, 255, 204, 211, 51, 51, 0, 52, 55,
            247, 204, 45, 37, 44, 210, 132, 50, 206, 121, 135, 151, 255, 126, 205, 255, 204, 211,
            51, 51, 0, 52, 55, 247, 204, 45, 44, 210, 132, 50, 206, 51,
        ];
        let (front, back) = data.split_at(data.len() / 2 + 1);
        let mut queue = alloc::collections::VecDeque::with_capacity(data.len());

        // create a non-contiguous slice of memory in queue
        front.into_iter().copied().for_each(|x| queue.push_back(x));
        back.into_iter().copied().for_each(|x| queue.push_front(x));

        // make sure it's non-contiguous
        let (a, b) = queue.as_slices();
        assert!(data.is_empty() || !a.is_empty());
        assert!(data.is_empty() || !b.is_empty());

        assert_eq!(data.len(), queue.len());
        assert!(Repr::from_utf8_buf(&mut queue).is_err());
    }

    #[test]
    #[should_panic(expected = "Utf8Error")]
    fn test_invalid_utf8() {
        let invalid = &[0, 159];
        let mut buf: Cursor<&[u8]> = Cursor::new(invalid);

        Repr::from_utf8_buf(&mut buf).unwrap();
    }
}
