use smallvec::SmallVec;

use crate::repr::MAX_SIZE;
use crate::CompactString;

impl CompactString {
    /// Converts a [`CompactString`] into a byte vector
    ///
    /// This consumes the [`CompactString`] and returns a [`SmallVec`], so we do not need to copy
    /// contents
    ///
    /// Note: [`SmallVec`] is an inline-able version [`Vec`](alloc::vec::Vec), just like
    /// [`CompactString`] is an inline-able version of [`String`](alloc::string::String).
    ///
    /// # Example
    /// ```
    /// use compact_str::CompactString;
    ///
    /// let c = CompactString::new("hello");
    /// let bytes = c.into_bytes();
    ///
    /// assert_eq!(&[104, 101, 108, 108, 111][..], &bytes[..]);
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "smallvec")))]
    pub fn into_bytes(self) -> SmallVec<[u8; MAX_SIZE]> {
        self.0.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use proptest::prelude::*;
    use test_strategy::proptest;

    use crate::repr::MAX_SIZE;
    use crate::tests::rand_unicode;
    use crate::CompactString;

    /// generates random unicode strings, that are at least MAX_SIZE bytes long
    pub fn rand_long_unicode() -> impl Strategy<Value = String> {
        proptest::collection::vec(proptest::char::any(), (MAX_SIZE + 1)..80)
            .prop_map(|v| v.into_iter().collect())
    }

    #[test]
    fn test_buffer_reuse() {
        let c = CompactString::from("I am a longer string that will be on the heap");
        let c_ptr = c.as_ptr();

        let bytes = c.into_bytes();
        let b_ptr = bytes.as_ptr();

        // Note: inlined CompactStrings also get their buffers re-used, but we can't assert their
        // re-use the same way we do for longer strings, because the underlying array may move on
        // the callstack, whereas for longer strings the buffer is not moving on the heap

        // converting into_bytes should _always_ re-use the underlying buffer
        assert_eq!(c_ptr, b_ptr);
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_buffer_reuse(#[strategy(rand_long_unicode())] s: String) {
        let c = CompactString::from(s);
        let c_ptr = c.as_ptr();

        let bytes = c.into_bytes();
        let b_ptr = bytes.as_ptr();

        // converting into_bytes should _always_ re-use the underlying buffer
        prop_assert_eq!(c_ptr, b_ptr);
    }

    #[proptest]
    #[cfg_attr(miri, ignore)]
    fn proptest_roundtrip(#[strategy(rand_unicode())] s: String) {
        let og_compact = CompactString::from(s.clone());
        prop_assert_eq!(&og_compact, &s);

        let bytes = og_compact.into_bytes();

        let ex_compact = CompactString::from_utf8(bytes).unwrap();
        prop_assert_eq!(&ex_compact, &s);
    }
}
