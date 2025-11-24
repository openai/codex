use smallvec::SmallVec;

use super::{
    Repr,
    MAX_SIZE,
};

impl Repr {
    /// Consumes the [`Repr`] returning a byte vector in a [`SmallVec`]
    ///
    /// Note: both for the inlined case and the heap case, the buffers are re-used
    #[inline]
    pub fn into_bytes(self) -> SmallVec<[u8; MAX_SIZE]> {
        if let Some(s) = self.as_static_str() {
            SmallVec::from(s.as_bytes())
        } else if self.is_heap_allocated() {
            let string = self.into_string();
            let bytes = string.into_bytes();
            SmallVec::from_vec(bytes)
        } else {
            // SAFETY: We just checked the discriminant to make sure we're an InlineBuffer
            let inline = unsafe { self.into_inline() };
            let (array, length) = inline.into_array();
            SmallVec::from_buf_and_len(array, length)
        }
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use crate::CompactString;

    #[test_case("" ; "empty")]
    #[test_case("abc" ; "short")]
    #[test_case("I am a long string 😊😊😊😊😊" ; "long")]
    fn proptest_roundtrip(s: &'static str) {
        let og_compact = CompactString::from(s);
        assert_eq!(og_compact, s);

        let bytes = og_compact.into_bytes();

        let ex_compact = CompactString::from_utf8(bytes).unwrap();
        assert_eq!(ex_compact, s);

        // test `StaticStr` variant
        let og_compact = CompactString::const_new(s);
        assert_eq!(og_compact, s);

        let bytes = og_compact.into_bytes();

        let ex_compact = CompactString::from_utf8(bytes).unwrap();
        assert_eq!(ex_compact, s);
    }
}
